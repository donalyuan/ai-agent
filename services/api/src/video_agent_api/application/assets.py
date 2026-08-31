"""Assets/asset versions command and query services."""

from __future__ import annotations

import base64
import json
from collections.abc import Callable
from dataclasses import dataclass
from datetime import UTC, datetime
from uuid import uuid4

from video_agent_api.domain.assets import (
    Asset,
    AssetVersion,
    AssetVersionReservation,
    StorageObject,
)
from video_agent_api.domain.errors import (
    AssetNotFoundError,
    AssetVersionConflictError,
    AssetVersionNotFoundError,
    DatabaseUnavailableError,
    ProjectAccessForbiddenError,
    ProjectNotFoundError,
    ValidationDomainError,
)
from video_agent_api.ports.contracts import StoredObjectRef
from video_agent_api.resilience import (
    OperationsResilienceCoordinator,
    admission_from_refs,
    admission_refs,
)

from .ports import AssetsUnitOfWorkFactory


@dataclass(frozen=True, slots=True)
class CreateAssetCommand:
    project_id: str
    name: str
    kind: str
    source_type: str = "imported"
    catalog_role: str | None = None
    tags: tuple[str, ...] = ()
    authorization_status: str = "unknown"
    copyright_owner: str | None = None
    license_label: str | None = None
    license_reference: str | None = None
    schema_version: str = "1.0.0"


@dataclass(frozen=True, slots=True)
class AppendAssetVersionCommand:
    asset_id: str
    storage_object: StorageObject
    content_hash: str | None = None


@dataclass(frozen=True, slots=True)
class CreateReservationCommand:
    project_id: str
    asset_id: str
    fingerprint: str
    expected_asset_revision: int
    declared_kind: str
    declared_mime_type: str
    declared_size_bytes: int
    declared_checksum: str
    storage_profile_id: str
    storage_profile_revision: int
    storage_profile_snapshot_hash: str
    upload_key: str | None = None
    schema_version: str = "1.0.0"
    admission_refs: dict[str, object] | None = None


@dataclass(frozen=True, slots=True)
class CompleteReservationCommand:
    reservation_id: str
    storage_object: StorageObject
    content_hash: str


@dataclass(frozen=True, slots=True)
class UpdateAssetMetadataCommand:
    asset_id: str
    expected_revision: int
    tags: tuple[str, ...] | None = None
    catalog_role: str | None = None
    authorization_status: str | None = None
    source_type: str | None = None
    copyright_owner: str | None = None
    license_label: str | None = None
    license_reference: str | None = None
    fields: frozenset[str] = frozenset()
    schema_version: str = "1.0.0"


@dataclass(frozen=True, slots=True)
class CancelReservationCommand:
    reservation_id: str
    expected_revision: int
    schema_version: str = "1.0.0"


@dataclass(frozen=True, slots=True)
class AssetCatalogEntry:
    asset: Asset
    version_count: int
    processing_status: str
    latest_version: AssetVersion | None


@dataclass(frozen=True, slots=True)
class AssetCatalogPage:
    items: tuple[AssetCatalogEntry, ...]
    next_cursor: str | None


class AssetsService:
    """Each command owns one UoW; versions are append-only and never updated in place."""

    def __init__(
        self,
        uow_factory: AssetsUnitOfWorkFactory,
        *,
        resilience_factory: Callable[[str], OperationsResilienceCoordinator] | None = None,
    ) -> None:
        self._uow_factory = uow_factory
        self._resilience_factory = resilience_factory

    def _freeze_upload_admission(
        self, project_id: str, operation_key: str, declared_size_bytes: int
    ) -> dict[str, object]:
        if self._resilience_factory is None:
            return {}
        admission = self._resilience_factory(project_id).freeze(
            project_id,
            "asset.upload",
            operation_key,
            required_bytes=declared_size_bytes,
        )
        if not admission.allowed:
            raise ValidationDomainError(
                admission.diagnostic or "asset_upload_resource_admission_blocked"
            )
        return admission_refs(admission)

    def revalidate_reservation_admission(self, reservation: AssetVersionReservation) -> None:
        """Block multipart side effects when the reservation's frozen admission changed."""
        if self._resilience_factory is None or not reservation.admission_refs:
            return
        try:
            frozen = admission_from_refs(reservation.admission_refs)
        except (KeyError, TypeError, ValueError) as error:
            raise ValidationDomainError("asset_upload_resource_admission_invalid") from error
        if (
            frozen.scope != reservation.project_id
            or frozen.operation != "asset.upload"
            or frozen.operation_key != reservation.operation_key
        ):
            raise ValidationDomainError("asset_upload_resource_admission_mismatch")
        current = self._resilience_factory(reservation.project_id).revalidate(frozen)
        if not current.allowed:
            raise ValidationDomainError(
                current.diagnostic or "asset_upload_resource_admission_blocked"
            )

    @staticmethod
    def _require_schema_version(value: str) -> None:
        if value != "1.0.0":
            raise ValidationDomainError("unsupported schemaVersion")

    async def create_asset(self, command: CreateAssetCommand) -> Asset:
        self._require_schema_version(command.schema_version)
        async with self._uow_factory() as uow:
            if await uow.projects.get(command.project_id) is None:
                raise ProjectNotFoundError(command.project_id)
            asset = Asset(
                command.project_id,
                command.kind,
                command.name,
                source_type=command.source_type,
                catalog_role=command.catalog_role,
                tags=command.tags,
                authorization_status=command.authorization_status,
                copyright_owner=command.copyright_owner,
                license_label=command.license_label,
                license_reference=command.license_reference,
            )
            await uow.assets.add(asset)
            uow.audit_events.append({"type": "asset.created", "assetId": asset.id})
            uow.outbox_events.append({"type": "asset.created", "assetId": asset.id})
            await uow.commit()
            return asset

    async def get_asset(self, asset_id: str, *, project_scope: str | None = None) -> Asset:
        async with self._uow_factory() as uow:
            asset = await uow.assets.get(asset_id)
        if asset is None:
            raise AssetNotFoundError(asset_id)
        if project_scope is not None and asset.project_id != project_scope:
            raise ProjectAccessForbiddenError(project_scope)
        return asset

    async def list_assets(self, project_id: str) -> list[Asset]:
        async with self._uow_factory() as uow:
            if await uow.projects.get(project_id) is None:
                raise ProjectNotFoundError(project_id)
            assets = await uow.assets.list_by_project(project_id)
        return sorted(assets, key=lambda value: value.id)

    async def catalog(
        self,
        project_id: str,
        *,
        cursor: str | None = None,
        limit: int = 50,
        filters: dict[str, str] | None = None,
    ) -> AssetCatalogPage:
        if limit < 1 or limit > 100:
            raise ValidationDomainError("asset catalog limit is invalid")
        filters = filters or {}
        allowed = {
            "kind",
            "catalogRole",
            "tag",
            "sourceType",
            "authorizationStatus",
            "processingStatus",
        }
        if set(filters).difference(allowed):
            raise ValidationDomainError("asset catalog filter is invalid")
        allowed_processing = {"unknown", "pending", "ready", "failed", "stale"}
        if filters.get("processingStatus") not in allowed_processing | {None}:
            raise ValidationDomainError("asset catalog processingStatus is invalid")
        cursor_key: tuple[str, str] | None = None
        if cursor:
            try:
                padding = "=" * (-len(cursor) % 4)
                decoded = json.loads(base64.urlsafe_b64decode(cursor + padding))
                if not isinstance(decoded, list) or len(decoded) != 2:
                    raise ValueError
                cursor_key = (str(decoded[0]), str(decoded[1]))
            except (ValueError, TypeError, json.JSONDecodeError) as error:
                raise ValidationDomainError("asset catalog cursor is invalid") from error
        async with self._uow_factory() as uow:
            if await uow.projects.get(project_id) is None:
                raise ProjectNotFoundError(project_id)
            assets = list(await uow.assets.list_by_project(project_id))
            values: list[AssetCatalogEntry] = []
            for item in assets:
                versions = list(await uow.asset_versions.list_by_asset(item.id))
                versions.sort(key=lambda value: (value.version_number, value.id))
                latest = versions[-1] if versions else None
                processing_status = "unknown"
                if latest is not None:
                    matching = [
                        inspection
                        for inspection in uow.media_inspections.values()
                        if getattr(inspection, "asset_version_id", None) == latest.id
                        and getattr(inspection, "asset_version_revision", None) == latest.revision
                        and getattr(inspection, "source_hash", None) == latest.content_hash
                    ]
                    processing_status = str(matching[-1].status) if matching else "pending"
                if cursor_key and (item.updated_at, item.id) <= cursor_key:
                    continue
                if (
                    (filters.get("kind") and item.kind != filters["kind"])
                    or (filters.get("catalogRole") and item.catalog_role != filters["catalogRole"])
                    or (filters.get("tag") and filters["tag"] not in item.tags)
                    or (filters.get("sourceType") and item.source_type != filters["sourceType"])
                    or (
                        filters.get("authorizationStatus")
                        and item.authorization_status != filters["authorizationStatus"]
                    )
                    or (
                        filters.get("processingStatus")
                        and processing_status != filters["processingStatus"]
                    )
                ):
                    continue
                values.append(AssetCatalogEntry(item, len(versions), processing_status, latest))
        values.sort(key=lambda item: (item.asset.updated_at, item.asset.id))
        selected = tuple(values[:limit])
        next_cursor = (
            base64.urlsafe_b64encode(
                json.dumps(
                    [selected[-1].asset.updated_at, selected[-1].asset.id],
                    separators=(",", ":"),
                ).encode()
            )
            .decode()
            .rstrip("=")
            if len(values) > limit
            else None
        )
        return AssetCatalogPage(selected, next_cursor)

    async def update_metadata(
        self, command: UpdateAssetMetadataCommand, *, project_scope: str | None = None
    ) -> Asset:
        self._require_schema_version(command.schema_version)
        async with self._uow_factory() as uow:
            asset = await uow.assets.get(command.asset_id)
            if asset is None:
                raise AssetNotFoundError(command.asset_id)
            if project_scope is not None and asset.project_id != project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            if command.expected_revision != asset.revision:
                raise AssetVersionConflictError(asset.id, command.expected_revision)
            fields = command.fields or frozenset(
                name
                for name, value in (
                    ("tags", command.tags),
                    ("source_type", command.source_type),
                    ("catalog_role", command.catalog_role),
                    ("authorization_status", command.authorization_status),
                    ("copyright_owner", command.copyright_owner),
                    ("license_label", command.license_label),
                    ("license_reference", command.license_reference),
                )
                if value is not None
            )
            if "tags" in fields and command.tags is not None:
                if len(command.tags) > 32 or len(set(command.tags)) != len(command.tags):
                    raise ValidationDomainError("tags must be bounded and unique")
                asset.tags = command.tags
            if "source_type" in fields and command.source_type is not None:
                asset.source_type = command.source_type
            if "catalog_role" in fields:
                asset.catalog_role = command.catalog_role
            if "authorization_status" in fields and command.authorization_status is not None:
                asset.authorization_status = command.authorization_status
            if "copyright_owner" in fields:
                asset.copyright_owner = command.copyright_owner
            if "license_label" in fields:
                asset.license_label = command.license_label
            if "license_reference" in fields:
                asset.license_reference = command.license_reference
            asset.__post_init__()
            asset.revision += 1
            asset.updated_at = datetime.now(UTC).isoformat()
            await uow.assets.save(asset)
            uow.audit_events.append(
                {"type": "asset.metadata.updated", "assetId": asset.id, "revision": asset.revision}
            )
            uow.outbox_events.append(
                {"type": "asset.metadata.updated", "assetId": asset.id, "revision": asset.revision}
            )
            await uow.commit()
            return asset

    async def append_version(self, command: AppendAssetVersionCommand) -> AssetVersion:
        async with self._uow_factory() as uow:
            asset = await uow.assets.get(command.asset_id)
            if asset is None:
                raise AssetNotFoundError(command.asset_id)
            version_number = await uow.asset_versions.next_version_number(asset.id)
            version = AssetVersion(
                asset_id=asset.id,
                project_id=asset.project_id,
                version_number=version_number,
                storage_object=command.storage_object,
                content_hash=command.content_hash,
            )
            await uow.asset_versions.add(version)
            await uow.commit()
            return version

    async def create_reservation(
        self, command: CreateReservationCommand
    ) -> AssetVersionReservation:
        self._require_schema_version(command.schema_version)
        reservation_id = str(uuid4())
        operation_key = f"asset-upload:{command.project_id}:{command.asset_id}:{reservation_id}"
        frozen_admission = (
            dict(command.admission_refs)
            if command.admission_refs is not None
            else self._freeze_upload_admission(
                command.project_id, operation_key, command.declared_size_bytes
            )
        )
        async with self._uow_factory() as uow:
            asset = await uow.assets.get(command.asset_id)
            if asset is None or asset.project_id != command.project_id:
                raise AssetNotFoundError(command.asset_id)
            if (
                asset.revision != command.expected_asset_revision
                or asset.kind != command.declared_kind
            ):
                raise AssetVersionConflictError(asset.id, command.expected_asset_revision)
            existing = next(
                (
                    item
                    for item in uow.asset_reservations.values()
                    if item.asset_id == command.asset_id and item.fingerprint == command.fingerprint
                ),
                None,
            )
            if existing:
                snapshot = (
                    existing.expected_asset_revision,
                    existing.declared_kind,
                    existing.declared_mime_type,
                    existing.declared_size_bytes,
                    existing.declared_checksum,
                    existing.storage_profile_id,
                    existing.storage_profile_revision,
                    existing.storage_profile_snapshot_hash,
                )
                requested = (
                    command.expected_asset_revision,
                    command.declared_kind,
                    command.declared_mime_type,
                    command.declared_size_bytes,
                    command.declared_checksum,
                    command.storage_profile_id,
                    command.storage_profile_revision,
                    command.storage_profile_snapshot_hash,
                )
                if snapshot != requested:
                    raise AssetVersionConflictError(existing.id, existing.revision)
                self.revalidate_reservation_admission(existing)
                return existing
            extension_by_mime = {
                "image/png": "png",
                "image/jpeg": "jpg",
                "video/mp4": "mp4",
                "audio/wav": "wav",
                "audio/mpeg": "mp3",
                "text/plain": "txt",
                "application/pdf": "pdf",
            }
            upload_key = command.upload_key or (
                f"projects/{command.project_id}/assets/{command.asset_id}/{reservation_id}/"
                f"original.{extension_by_mime.get(command.declared_mime_type, 'bin')}"
            )
            reservation = AssetVersionReservation(
                command.project_id,
                command.asset_id,
                operation_key,
                command.fingerprint,
                id=reservation_id,
                expected_asset_revision=command.expected_asset_revision,
                declared_kind=command.declared_kind,
                declared_mime_type=command.declared_mime_type,
                declared_size_bytes=command.declared_size_bytes,
                declared_checksum=command.declared_checksum,
                storage_profile_id=command.storage_profile_id,
                storage_profile_revision=command.storage_profile_revision,
                storage_profile_snapshot_hash=command.storage_profile_snapshot_hash,
                admission_refs=frozen_admission,
                upload_key=upload_key,
            )
            uow.asset_reservations[reservation.id] = reservation
            await uow.commit()
        return reservation

    async def get_reservation(
        self, project_id: str, reservation_id: str
    ) -> AssetVersionReservation:
        async with self._uow_factory() as uow:
            reservation = uow.asset_reservations.get(reservation_id)
            if reservation is None or reservation.project_id != project_id:
                raise AssetVersionConflictError(reservation_id, 0)
            return reservation

    async def cancel_reservation(
        self, project_id: str, command: CancelReservationCommand
    ) -> AssetVersionReservation:
        self._require_schema_version(command.schema_version)
        async with self._uow_factory() as uow:
            reservation = uow.asset_reservations.get(command.reservation_id)
            if reservation is None or reservation.project_id != project_id:
                raise AssetVersionConflictError(command.reservation_id, 0)
            if reservation.revision != command.expected_revision:
                raise AssetVersionConflictError(reservation.id, command.expected_revision)
            reservation.transition("cancelled")
            reservation.diagnostic = "storage_abort_required"
            uow.audit_events.append(
                {"type": "asset.reservation.cancelled", "reservationId": reservation.id}
            )
            await uow.commit()
            return reservation

    async def complete_reservation(self, command: CompleteReservationCommand) -> AssetVersion:
        async with self._uow_factory() as uow:
            reservation = uow.asset_reservations.get(command.reservation_id)
            if reservation is None:
                raise AssetVersionConflictError(command.reservation_id, 0)
            if reservation.status == "registered" and reservation.registered_version_id:
                existing = await uow.asset_versions.get(reservation.registered_version_id)
                if existing is None or existing.content_hash != command.content_hash:
                    raise AssetVersionConflictError(command.reservation_id, 0)
                if existing.storage_object != command.storage_object:
                    raise AssetVersionConflictError(command.reservation_id, 0)
                return existing
            if reservation.status != "reserved":
                raise AssetVersionConflictError(command.reservation_id, 0)
            self.revalidate_reservation_admission(reservation)
            asset = await uow.assets.get(reservation.asset_id)
            if asset is None:
                raise AssetNotFoundError(reservation.asset_id)
            if (
                reservation.declared_mime_type != command.storage_object.mime_type
                or reservation.declared_size_bytes != command.storage_object.size_bytes
                or reservation.declared_checksum != command.storage_object.checksum
                or reservation.declared_checksum != command.content_hash
                or asset.revision != reservation.expected_asset_revision
                or asset.kind != reservation.declared_kind
                or asset.authorization_status in {"restricted", "expired"}
            ):
                raise AssetVersionConflictError(command.reservation_id, reservation.revision)
            version_number = await uow.asset_versions.next_version_number(asset.id)
            version = AssetVersion(
                asset.id,
                asset.project_id,
                version_number,
                command.storage_object,
                command.content_hash,
            )
            await uow.asset_versions.add(version)
            reservation.transition("registered", version.id)
            uow.audit_events.append(
                {
                    "type": "asset.version.registered",
                    "reservationId": reservation.id,
                    "assetVersionId": version.id,
                }
            )
            uow.outbox_events.append(
                {
                    "type": "asset.version.registered",
                    "reservationId": reservation.id,
                    "assetVersionId": version.id,
                }
            )
            await uow.commit()
            return version

    async def get_version(
        self, version_id: str, *, project_scope: str | None = None
    ) -> AssetVersion:
        async with self._uow_factory() as uow:
            version = await uow.asset_versions.get(version_id)
        if version is None:
            raise AssetVersionNotFoundError(version_id)
        if project_scope is not None and version.project_id != project_scope:
            raise ProjectAccessForbiddenError(project_scope)
        return version

    async def list_versions(
        self, asset_id: str, *, project_scope: str | None = None
    ) -> list[AssetVersion]:
        async with self._uow_factory() as uow:
            asset = await uow.assets.get(asset_id)
            if asset is None:
                raise AssetNotFoundError(asset_id)
            if project_scope is not None and asset.project_id != project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            versions = await uow.asset_versions.list_by_asset(asset_id)
        return sorted(versions, key=lambda value: (value.version_number, value.id))

    async def media_projection(self, project_id: str, version_id: str) -> dict[str, object]:
        async with self._uow_factory() as uow:
            version = await uow.asset_versions.get(version_id)
            if version is None or version.project_id != project_id:
                raise AssetVersionNotFoundError(version_id)
            asset = await uow.assets.get(version.asset_id)
            if asset is None or asset.project_id != project_id:
                raise AssetVersionNotFoundError(version_id)
            inspections = [
                item
                for item in uow.media_inspections.values()
                if getattr(item, "asset_version_id", None) == version.id
                and getattr(item, "asset_version_revision", None) == version.revision
                and getattr(item, "source_hash", None) == version.content_hash
            ]
            inspection = inspections[-1] if inspections else None
            derivatives = [
                item
                for item in uow.media_derivatives.values()
                if getattr(item, "asset_version_id", None) == version.id
                and getattr(item, "asset_version_revision", None) == version.revision
                and getattr(item, "source_hash", None) == version.content_hash
            ]
        if inspection is None:
            return {
                "schemaVersion": "1.0.0",
                "projectId": project_id,
                "assetVersionId": version.id,
                "assetVersionRevision": version.revision,
                "sourceHash": version.content_hash,
                "status": "unavailable",
                "diagnostic": "media_projection_unavailable",
                "inspection": None,
                "derivatives": [],
            }
        safe_derivatives = [
            {
                "id": item.id,
                "kind": item.kind,
                "status": item.status,
                "tool": item.tool,
                "toolVersion": item.tool_version,
                "schemaVersion": item.derivative_schema_version,
                "checksum": item.checksum,
                "sizeBytes": item.size_bytes,
                "diagnostic": item.raw_diagnostic,
                "grantAvailable": bool(
                    item.status == "ready"
                    and item.source_fingerprint == inspection.source_fingerprint
                    and asset.authorization_status == "verified"
                    and item.license_status == "approved"
                    and not item.hold
                ),
            }
            for item in derivatives
        ]
        return {
            "schemaVersion": "1.0.0",
            "projectId": project_id,
            "assetVersionId": version.id,
            "assetVersionRevision": version.revision,
            "sourceHash": version.content_hash,
            "status": inspection.status,
            "diagnostic": inspection.raw_diagnostic,
            "inspection": {
                "id": inspection.id,
                "revision": inspection.revision,
                "status": inspection.status,
                "metadata": dict(inspection.metadata),
                "tool": inspection.tool,
                "toolVersion": inspection.tool_version,
                "schemaVersion": inspection.schema_version,
            },
            "derivatives": safe_derivatives,
        }

    async def media_grant_source(
        self, project_id: str, version_id: str, derivative_id: str
    ) -> StoredObjectRef:
        async with self._uow_factory() as uow:
            version = await uow.asset_versions.get(version_id)
            if version is None or version.project_id != project_id:
                raise AssetVersionNotFoundError(version_id)
            asset = await uow.assets.get(version.asset_id)
            derivative = uow.media_derivatives.get(derivative_id)
            inspection = (
                uow.media_inspections.get(getattr(derivative, "inspection_id", ""))
                if derivative is not None
                else None
            )
            if (
                asset is None
                or asset.authorization_status != "verified"
                or derivative is None
                or inspection is None
                or derivative.project_id != project_id
                or derivative.asset_version_id != version.id
                or derivative.asset_version_revision != version.revision
                or derivative.source_hash != version.content_hash
                or derivative.source_fingerprint != inspection.source_fingerprint
                or derivative.status != "ready"
                or derivative.object_ref is None
                or derivative.checksum is None
                or derivative.size_bytes is None
                or derivative.license_status != "approved"
                or derivative.hold
            ):
                raise ValidationDomainError("media derivative is stale or unauthorized")
            profile_id = str(derivative.object_ref["profileId"])
            if profile_id not in {"local", "local-test-offline", "local_workspace"}:
                raise DatabaseUnavailableError("media grant storage profile is unconfigured")
            mime_type = {
                "thumbnail": "image/jpeg",
                "keyframe_index": "application/json",
                "waveform": "application/json",
            }.get(derivative.kind, str(inspection.metadata["mimeType"]))
            return StoredObjectRef(
                project_id,
                profile_id,
                "workspace",
                str(derivative.object_ref["objectKey"]),
                derivative.size_bytes,
                derivative.checksum,
                mime_type,
                derivative.checksum,
                str(derivative.object_ref["operationKey"]),
            )

    async def usage_projection(self, project_id: str, version_id: str) -> dict[str, object]:
        references: list[dict[str, object]] = []
        unavailable: list[str] = []
        async with self._uow_factory() as uow:
            version = await uow.asset_versions.get(version_id)
            if version is None or version.project_id != project_id:
                raise AssetVersionNotFoundError(version_id)
            try:
                for source in uow.source_materials.values():
                    if getattr(source, "project_id", None) != project_id:
                        continue
                    for item in getattr(source, "versions", ()):
                        if getattr(item, "asset_version_id", None) == version_id:
                            references.append(
                                {
                                    "ownerType": "source_material",
                                    "ownerId": source.id,
                                    "ownerRevision": source.revision,
                                    "scope": {"projectId": project_id},
                                    "state": "current" if source.current is item else "historical",
                                    "sourceHash": version.content_hash,
                                    "deepLink": (
                                        f"/projects/{project_id}/workbench?source={source.id}"
                                    ),
                                }
                            )
            except Exception:
                unavailable.append("source_material")
            try:
                for shot in uow.shots.values():
                    if getattr(shot, "project_id", None) != project_id:
                        continue
                    for kind in ("current_image", "current_video"):
                        item = getattr(shot, kind, None)
                        if getattr(item, "asset_version_id", None) == version_id:
                            assert item is not None
                            references.append(
                                {
                                    "ownerType": "shot",
                                    "ownerId": shot.id,
                                    "ownerRevision": shot.revision,
                                    "scope": {
                                        "projectId": project_id,
                                        "episodeId": shot.episode_id,
                                        "shotId": shot.id,
                                    },
                                    "state": kind,
                                    "sourceHash": item.asset_version_hash,
                                    "deepLink": (f"/projects/{project_id}/review?shot={shot.id}"),
                                }
                            )
            except Exception:
                unavailable.append("scene_shot")
            try:
                for candidate in uow.asset_edit_candidates.values():
                    item = getattr(candidate, "asset_version", None)
                    if (
                        getattr(candidate, "project_id", None) == project_id
                        and getattr(item, "id", None) == version_id
                    ):
                        assert item is not None
                        references.append(
                            {
                                "ownerType": "asset_edit_candidate",
                                "ownerId": candidate.id,
                                "ownerRevision": candidate.revision,
                                "scope": {
                                    "projectId": project_id,
                                    "episodeId": candidate.episode_id,
                                    "targetId": candidate.target_id,
                                },
                                "state": candidate.status,
                                "sourceHash": item.content_hash,
                                "deepLink": (
                                    f"/projects/{project_id}/review?candidate={candidate.id}"
                                ),
                            }
                        )
            except Exception:
                unavailable.append("asset_edit")
            try:
                for cut in uow.timeline_cuts.values():
                    if getattr(cut, "project_id", None) != project_id:
                        continue
                    for clip in getattr(cut, "clips", ()):
                        if clip.get("assetVersionId") == version_id:
                            references.append(
                                {
                                    "ownerType": "timeline_clip",
                                    "ownerId": str(clip.get("id")),
                                    "ownerRevision": cut.revision,
                                    "scope": {
                                        "projectId": project_id,
                                        "episodeId": cut.episode_id,
                                    },
                                    "state": "current",
                                    "sourceHash": str(clip.get("assetVersionHash")),
                                    "deepLink": (
                                        f"/projects/{project_id}/episodes/{cut.episode_id}/timeline"
                                    ),
                                }
                            )
                    for cue in getattr(cut, "cues", ()):
                        if getattr(cue, "asset_version_id", None) == version_id:
                            references.append(
                                {
                                    "ownerType": "timeline_sound_cue",
                                    "ownerId": cue.id,
                                    "ownerRevision": cut.revision,
                                    "scope": {
                                        "projectId": project_id,
                                        "episodeId": cut.episode_id,
                                    },
                                    "state": "current",
                                    "sourceHash": cue.asset_version_hash or version.content_hash,
                                    "deepLink": (
                                        f"/projects/{project_id}/episodes/{cut.episode_id}/timeline"
                                    ),
                                }
                            )
                for timeline in uow.timeline_versions.values():
                    if getattr(timeline, "project_id", None) != project_id:
                        continue
                    snapshot = getattr(timeline, "cut_snapshot", {})
                    for clip in snapshot.get("clips", ()):
                        if clip.get("assetVersionId") == version_id:
                            references.append(
                                {
                                    "ownerType": "timeline_version",
                                    "ownerId": timeline.id,
                                    "ownerRevision": timeline.revision,
                                    "scope": {
                                        "projectId": project_id,
                                        "episodeId": timeline.episode_id,
                                    },
                                    "state": "historical",
                                    "sourceHash": str(clip.get("assetVersionHash")),
                                    "deepLink": (
                                        f"/projects/{project_id}/episodes/{timeline.episode_id}/timeline"
                                    ),
                                }
                            )
            except Exception:
                unavailable.append("timeline")
            try:
                timeline_ids = {
                    str(item["ownerId"])
                    for item in references
                    if item["ownerType"] == "timeline_version"
                }
                for job in uow.export_jobs.values():
                    if (
                        getattr(job, "project_id", None) == project_id
                        and getattr(job, "timeline_version_id", None) in timeline_ids
                    ):
                        references.append(
                            {
                                "ownerType": "export_manifest",
                                "ownerId": job.id,
                                "ownerRevision": job.revision,
                                "scope": {
                                    "projectId": project_id,
                                    "episodeId": job.episode_id,
                                },
                                "state": job.status,
                                "sourceHash": version.content_hash,
                                "deepLink": f"/projects/{project_id}/exports?job={job.id}",
                            }
                        )
            except Exception:
                unavailable.append("export")
        status = "complete" if not unavailable else ("partial" if references else "unavailable")
        return {
            "schemaVersion": "1.0.0",
            "projectId": project_id,
            "assetVersionId": version_id,
            "status": status,
            "diagnostic": "usage_projection_unavailable" if unavailable else None,
            "unavailableOwners": unavailable,
            "references": references,
        }

    async def timeline_selection(
        self, project_id: str, version_id: str, episode_id: str
    ) -> dict[str, object]:
        async with self._uow_factory() as uow:
            episode = await uow.episodes.get(episode_id)
            version = await uow.asset_versions.get(version_id)
            if (
                episode is None
                or episode.project_id != project_id
                or version is None
                or version.project_id != project_id
            ):
                raise AssetVersionNotFoundError(version_id)
            asset = await uow.assets.get(version.asset_id)
            if asset is None or asset.authorization_status != "verified":
                raise ValidationDomainError("asset selection is unauthorized")
            derivatives = sorted(
                (
                    item
                    for item in uow.media_derivatives.values()
                    if getattr(item, "project_id", None) == project_id
                    and getattr(item, "asset_version_id", None) == version.id
                    and getattr(item, "asset_version_revision", None) == version.revision
                    and getattr(item, "source_hash", None) == version.content_hash
                    and getattr(item, "kind", None) == "proxy"
                    and getattr(item, "status", None) == "ready"
                ),
                key=lambda item: (str(getattr(item, "kind", "")), str(item.id)),
            )
            if not derivatives:
                raise ValidationDomainError("asset selection derivative is not ready")
            derivative = derivatives[0]
            inspection = uow.media_inspections.get(derivative.inspection_id)
            if (
                inspection is None
                or inspection.project_id != project_id
                or inspection.asset_version_id != version.id
                or inspection.asset_version_revision != version.revision
                or inspection.source_hash != version.content_hash
                or inspection.status != "ready"
                or inspection.source_fingerprint != derivative.source_fingerprint
            ):
                raise ValidationDomainError("asset selection inspection is unavailable")
            metadata = inspection.metadata
            available_frames = (
                metadata.get("durationFrames") if isinstance(metadata, dict) else None
            )
            if (
                isinstance(available_frames, bool)
                or not isinstance(available_frames, int)
                or available_frames < 1
            ):
                raise ValidationDomainError("asset selection derivative frame count is unavailable")
            if asset.kind in {"image", "video"}:
                accepted = any(
                    getattr(current, "asset_version_id", None) == version.id
                    and getattr(current, "asset_version_revision", None) == version.revision
                    and getattr(current, "asset_version_hash", None) == version.content_hash
                    and getattr(current, "accepted", False)
                    for shot in uow.shots.values()
                    if getattr(shot, "project_id", None) == project_id
                    for current in (
                        getattr(shot, "current_image", None),
                        getattr(shot, "current_video", None),
                    )
                )
                if not accepted:
                    raise ValidationDomainError("asset selection is not accepted current")
            return {
                "schemaVersion": "1.0.0",
                "projectId": project_id,
                "episodeId": episode_id,
                "assetVersionId": version.id,
                "assetVersionRevision": version.revision,
                "assetVersionHash": version.content_hash,
                "kind": asset.kind,
                "authorizationStatus": asset.authorization_status,
                "licenseLabel": asset.license_label,
                "licenseStatus": "approved" if asset.license else "unknown",
                "storageVerified": version.storage_object is not None,
                "derivativeFingerprint": derivative.source_fingerprint,
                "acceptedCurrent": True,
                "derivativeStatus": "ready",
                "availableFrames": available_frames,
            }
