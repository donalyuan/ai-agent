"""Media Worker owner commands; Timeline and Provider consume read-only projections."""

from __future__ import annotations

from dataclasses import dataclass
from hashlib import sha256
from typing import Any, Literal, SupportsIndex, SupportsInt, cast

from video_agent_api.domain.errors import AssetVersionNotFoundError, ValidationDomainError
from video_agent_api.domain.media import (
    MediaDerivative,
    MediaInspection,
    PreviewArtifact,
    source_fingerprint,
)
from video_agent_api.ports.contracts import StoredObjectRef
from video_agent_api.resilience import (
    OperationsResilienceCoordinator,
    admission_from_refs,
    admission_refs,
)

MEDIA_SCHEMA_VERSION = "1.0.0"
MEDIA_ROUTE = "media"
MEDIA_TASK_QUEUE = "media-tasks"
MEDIA_WORKFLOW_TYPE = "media-operation"
_MEDIA_OPERATIONS = frozenset({"inspect", "derivative", "render", "storage", "pipeline"})


@dataclass(frozen=True, slots=True)
class MediaDispatchAdmission:
    """冻结 Media activity 的输入准入；拒绝发生在任何 outbox/owner 写入之前。"""

    project_id: str
    discriminator: Literal["uploaded_source", "asset_center", "generated_candidate"]
    asset_version_id: str
    asset_version_revision: int
    asset_version_hash: str
    stored_object_ref: StoredObjectRef | dict[str, object]
    operation_key: str
    episode_id: str | None = None
    shot_id: str | None = None
    candidate_id: str | None = None
    candidate_revision: int | None = None
    provenance: str | None = None
    technical_input: dict[str, object] | None = None
    admission_refs: dict[str, object] | None = None


def _stored_object_ref(value: StoredObjectRef | dict[str, object]) -> StoredObjectRef:
    if isinstance(value, StoredObjectRef):
        return value
    try:
        return StoredObjectRef(
            project_id=str(value["projectId"]),
            profile_id=str(value["profileId"]),
            bucket=str(value["bucket"]),
            object_key=str(value["objectKey"]),
            size_bytes=int(cast(str | SupportsInt | SupportsIndex, value["sizeBytes"])),
            checksum=str(value["checksum"]),
            mime_type=str(value["mimeType"]),
            etag=str(value["etag"]) if value.get("etag") is not None else None,
            operation_key=str(value["operationKey"]),
            verified=bool(value.get("verified", True)),
        )
    except (KeyError, TypeError, ValueError) as error:
        raise ValidationDomainError("media stored object reference is invalid") from error


@dataclass(frozen=True, slots=True)
class RecordInspectionCommand:
    project_id: str
    asset_version_id: str
    asset_version_revision: int
    asset_version_hash: str
    operation_key: str
    metadata: dict[str, object]
    tool: str
    tool_version: str
    status: str = "ready"
    raw_diagnostic: str | None = None
    admission_refs: dict[str, object] | None = None


@dataclass(frozen=True, slots=True)
class RecordDerivativeCommand:
    project_id: str
    inspection_id: str
    kind: str
    status: str
    parameters: dict[str, object]
    operation_key: str
    tool: str
    tool_version: str
    derivative_schema_version: str = "1.0.0"
    object_ref: dict[str, object] | None = None
    checksum: str | None = None
    size_bytes: int | None = None
    raw_diagnostic: str | None = None
    admission_refs: dict[str, object] | None = None
    retention_policy: str = "phase-one"
    retention_version: str = "1"
    license_status: str = "approved"
    hold: bool = False


class MediaOwnerService:
    def __init__(
        self, uow_factory: Any, *, resilience: OperationsResilienceCoordinator | None = None
    ) -> None:
        self._uow_factory = uow_factory
        self._resilience = resilience

    def _admit(
        self,
        project_id: str,
        operation: str,
        operation_key: str,
        frozen_refs: dict[str, object] | None,
    ) -> dict[str, object]:
        if self._resilience is None:
            if frozen_refs:
                raise ValidationDomainError("media_resource_admission_unconfigured")
            return {}
        try:
            frozen = (
                admission_from_refs(frozen_refs)
                if frozen_refs is not None
                else self._resilience.freeze(project_id, operation, operation_key)
            )
        except (KeyError, TypeError, ValueError) as error:
            raise ValidationDomainError("media_resource_admission_invalid") from error
        if (
            frozen.scope != project_id
            or frozen.operation != operation
            or frozen.operation_key != operation_key
        ):
            raise ValidationDomainError("media_resource_admission_mismatch")
        admission = self._resilience.revalidate(frozen)
        if not admission.allowed:
            raise ValidationDomainError(admission.diagnostic or "media_resource_admission_blocked")
        return admission_refs(admission)

    def admit_activity(
        self,
        project_id: str,
        operation: str,
        operation_key: str,
        dispatch_refs: dict[str, object] | None = None,
    ) -> dict[str, object]:
        """Revalidate the durable dispatch ledger without minting an activity ledger.

        The dispatch event is the sole cross-process admission record.  Activities
        can only consume its exact identity; creating a new resource admission in
        a restarted worker would make the current host state authoritative.
        """
        if dispatch_refs is None:
            if self._resilience is not None:
                raise ValidationDomainError("media dispatch admission is required")
            return {}
        if self._resilience is None:
            raise ValidationDomainError("media_resource_admission_unconfigured")
        try:
            dispatch = admission_from_refs(dispatch_refs)
        except (KeyError, TypeError, ValueError) as error:
            raise ValidationDomainError("media_resource_admission_invalid") from error
        if (
            dispatch.scope != project_id
            or dispatch.operation != "media.dispatch"
            or not operation_key.startswith(f"{dispatch.operation_key}:")
            and operation_key != dispatch.operation_key
        ):
            raise ValidationDomainError("media_dispatch_admission_mismatch")
        checked = self._resilience.revalidate(dispatch)
        if not checked.allowed:
            raise ValidationDomainError(checked.diagnostic or "media_dispatch_admission_blocked")
        return dict(dispatch_refs)

    def _admit_owner_result(
        self,
        project_id: str,
        operation: str,
        operation_key: str,
        frozen_refs: dict[str, object] | None,
    ) -> dict[str, object]:
        """Keep activity result records bound to their durable dispatch admission."""
        if frozen_refs is not None and frozen_refs.get("operation") == "media.dispatch":
            return self.admit_activity(project_id, operation, operation_key, frozen_refs)
        return self._admit(project_id, operation, operation_key, frozen_refs)

    async def produce_generated_candidate(
        self, uow: Any, *, candidate: Any, asset_version: Any
    ) -> dict[str, object]:
        """Append one Media intent from the accepted-current candidate transaction."""

        # Scenes accepts media through its canonical projection dictionary while
        # worker/reconcile paths may still provide the immutable domain candidate.
        # Read both representations without reconstructing or widening owner state.
        def field(name: str, *aliases: str) -> object | None:
            if isinstance(candidate, dict):
                for key in (name, *aliases):
                    if key in candidate:
                        return cast(object, candidate[key])
                return None
            return cast(object | None, getattr(candidate, name, None))

        candidate_id = field("id", "candidateId")
        project_id = field("project_id", "projectId")
        episode_id = field("episode_id", "episodeId")
        target_id = field("target_id", "targetId")
        candidate_revision = field("revision", "candidateRevision")
        candidate_status = field("status")
        asset_version_id = getattr(asset_version, "id", None)
        asset_version_revision = getattr(asset_version, "revision", None)
        asset_version_hash = getattr(asset_version, "content_hash", None)
        storage = getattr(asset_version, "storage_object", None)
        if (
            candidate_status != "accepted"
            or not all(
                isinstance(value, str) and value
                for value in (
                    candidate_id,
                    project_id,
                    episode_id,
                    target_id,
                    asset_version_id,
                    asset_version_hash,
                )
            )
            or not isinstance(candidate_revision, int)
            or not isinstance(asset_version_revision, int)
            or getattr(asset_version, "project_id", None) != project_id
            or storage is None
        ):
            raise ValidationDomainError("generated media producer input is invalid")
        assert isinstance(project_id, str)
        assert isinstance(candidate_id, str)
        assert isinstance(candidate_revision, int)
        assert isinstance(asset_version_id, str)
        assert isinstance(asset_version_revision, int)
        assert isinstance(asset_version_hash, str)
        operation_key = (
            f"media:generated:{candidate_id}:{candidate_revision}:{asset_version_id}:"
            f"{asset_version_revision}:{asset_version_hash}"
        )
        existing = next(
            (
                event
                for event in uow.outbox_events
                if event.get("type") == "media.dispatch.requested"
                and event.get("operationKey") == operation_key
            ),
            None,
        )
        if existing is not None:
            return cast(dict[str, object], existing)
        frozen = self._admit(project_id, "media.dispatch", operation_key, None)
        event: dict[str, object] = {
            "type": "media.dispatch.requested",
            "operationKey": operation_key,
            "projectId": project_id,
            "discriminator": "generated_candidate",
            "assetVersionId": asset_version_id,
            "assetVersionRevision": asset_version_revision,
            "assetVersionHash": asset_version_hash,
            "storedObjectRef": {
                "projectId": project_id,
                "profileId": storage.storage_provider,
                "bucket": storage.bucket,
                "objectKey": storage.object_key,
                "sizeBytes": storage.size_bytes,
                "checksum": storage.checksum,
                "mimeType": storage.mime_type,
                "etag": storage.e_tag,
                "operationKey": operation_key,
                "verified": True,
            },
            "episodeId": episode_id,
            "shotId": target_id,
            "candidateId": candidate_id,
            "candidateRevision": candidate_revision,
            "provenance": field("source_provenance", "provenance") or "image_generation",
            "technicalInput": {"operation": "pipeline", "steps": ["inspect", "derivative"]},
            "operation": "pipeline",
            "executionRoute": MEDIA_ROUTE,
            "workflowType": MEDIA_WORKFLOW_TYPE,
            "taskQueue": MEDIA_TASK_QUEUE,
            "schemaVersion": MEDIA_SCHEMA_VERSION,
            "resourceAdmission": frozen,
            "status": "pending",
        }
        uow.outbox_events.append(event)
        return event

    async def admit_dispatch(self, command: MediaDispatchAdmission) -> StoredObjectRef:
        """Validate source discriminator and exact owner facts without mutating state."""
        if command.discriminator not in {"uploaded_source", "asset_center", "generated_candidate"}:
            raise ValidationDomainError("media admission discriminator is invalid")
        object_ref = _stored_object_ref(command.stored_object_ref)
        if (
            object_ref.project_id != command.project_id
            or object_ref.operation_key != command.operation_key
            or not object_ref.verified
        ):
            raise ValidationDomainError("media stored object scope or operation mismatch")
        async with self._uow_factory() as uow:
            version = await uow.asset_versions.get(command.asset_version_id)
            if (
                version is None
                or version.project_id != command.project_id
                or version.revision != command.asset_version_revision
                or version.content_hash != command.asset_version_hash
                or version.storage_object.checksum != object_ref.checksum
                or version.storage_object.size_bytes != object_ref.size_bytes
                or version.storage_object.object_key != object_ref.object_key
            ):
                raise ValidationDomainError("media admission AssetVersion is stale or foreign")
            asset = await uow.assets.get(version.asset_id)
            if asset is None or asset.project_id != command.project_id:
                raise ValidationDomainError("media admission asset is stale or foreign")
            if command.discriminator in {"uploaded_source", "asset_center"}:
                if asset.authorization_status != "verified":
                    raise ValidationDomainError("media admission requires verified source")
                if asset.source_type not in {"user_upload", "source_material", "imported"}:
                    raise ValidationDomainError("media admission source discriminator mismatch")
                return object_ref

            # Generated candidates require the accepted candidate and exact Scene/Shot current.
            if not command.episode_id or not command.shot_id or not command.candidate_id:
                raise ValidationDomainError("generated media admission owner scope is incomplete")
            candidate = uow.video_take_candidates.get(command.candidate_id) or next(
                (
                    value
                    for value in uow.image_generation_candidates.values()
                    if getattr(value, "id", None) == command.candidate_id
                ),
                None,
            )
            if (
                candidate is None
                or getattr(candidate, "status", None) != "accepted"
                or getattr(candidate, "project_id", None) != command.project_id
                or getattr(candidate, "episode_id", None) != command.episode_id
                or getattr(candidate, "asset_version_id", None) != command.asset_version_id
                or getattr(candidate, "asset_version_revision", None)
                != command.asset_version_revision
                or getattr(candidate, "asset_version_hash", None) != command.asset_version_hash
                or (
                    command.candidate_revision is not None
                    and getattr(candidate, "revision", None) != command.candidate_revision
                )
            ):
                raise ValidationDomainError("generated media candidate is not accepted/current")
            shot = uow.shots.get(command.shot_id)
            if (
                shot is None
                or shot.project_id != command.project_id
                or shot.episode_id != command.episode_id
            ):
                raise ValidationDomainError("generated media shot scope is stale or foreign")
            current = next(
                (
                    value
                    for value in (shot.current_image, shot.current_video)
                    if value is not None and value.candidate_id == command.candidate_id
                ),
                None,
            )
            if (
                current is None
                or not current.accepted
                or current.asset_version_id != command.asset_version_id
                or current.asset_version_revision != command.asset_version_revision
                or current.asset_version_hash != command.asset_version_hash
                or (command.provenance is not None and current.provenance != command.provenance)
            ):
                raise ValidationDomainError("generated media candidate is not accepted/current")
            return object_ref

    async def enqueue_dispatch(self, command: MediaDispatchAdmission) -> dict[str, object]:
        """Admit first, then append one durable Media outbox event for the stable operation."""
        operation = (command.technical_input or {}).get("operation", "inspect")
        if not isinstance(operation, str) or operation not in _MEDIA_OPERATIONS:
            raise ValidationDomainError("media dispatch operation is invalid")
        object_ref = await self.admit_dispatch(command)
        frozen = self._admit(
            command.project_id,
            "media.dispatch",
            command.operation_key,
            command.admission_refs,
        )
        async with self._uow_factory() as uow:
            existing = next(
                (
                    event
                    for event in uow.outbox_events
                    if event.get("type") == "media.dispatch.requested"
                    and event.get("operationKey") == command.operation_key
                ),
                None,
            )
            if existing is not None:
                if existing.get("assetVersionHash") != command.asset_version_hash:
                    raise ValidationDomainError("media dispatch operation conflict")
                return cast(dict[str, object], existing)
            event: dict[str, object] = {
                "type": "media.dispatch.requested",
                "operationKey": command.operation_key,
                "projectId": command.project_id,
                "discriminator": command.discriminator,
                "assetVersionId": command.asset_version_id,
                "assetVersionRevision": command.asset_version_revision,
                "assetVersionHash": command.asset_version_hash,
                "storedObjectRef": {
                    "projectId": object_ref.project_id,
                    "profileId": object_ref.profile_id,
                    "bucket": object_ref.bucket,
                    "objectKey": object_ref.object_key,
                    "sizeBytes": object_ref.size_bytes,
                    "checksum": object_ref.checksum,
                    "mimeType": object_ref.mime_type,
                    "etag": object_ref.etag,
                    "operationKey": object_ref.operation_key,
                    "verified": object_ref.verified,
                },
                "episodeId": command.episode_id,
                "shotId": command.shot_id,
                "candidateId": command.candidate_id,
                "candidateRevision": command.candidate_revision,
                "provenance": command.provenance,
                "technicalInput": dict(command.technical_input or {}),
                "operation": operation,
                "executionRoute": MEDIA_ROUTE,
                "workflowType": MEDIA_WORKFLOW_TYPE,
                "taskQueue": MEDIA_TASK_QUEUE,
                "schemaVersion": MEDIA_SCHEMA_VERSION,
                "resourceAdmission": frozen,
                "status": "pending",
            }
            uow.outbox_events.append(event)
            await uow.commit()
            return event

    async def record_inspection(self, command: RecordInspectionCommand) -> MediaInspection:
        frozen_admission_refs = self._admit_owner_result(
            command.project_id,
            "media.inspect",
            command.operation_key,
            command.admission_refs,
        )
        async with self._uow_factory() as uow:
            version = await uow.asset_versions.get(command.asset_version_id)
            if version is None:
                raise AssetVersionNotFoundError(command.asset_version_id)
            if (
                version.project_id != command.project_id
                or version.revision != command.asset_version_revision
                or version.content_hash != command.asset_version_hash
            ):
                raise ValidationDomainError("media inspection source is stale or foreign")
            existing = next(
                (
                    item
                    for item in uow.media_inspections.values()
                    if item.operation_key == command.operation_key
                ),
                None,
            )
            if existing is not None:
                if (
                    existing.asset_version_id != command.asset_version_id
                    or existing.source_hash != command.asset_version_hash
                    or existing.metadata != command.metadata
                ):
                    raise ValidationDomainError("media inspection operation conflict")
                return cast(MediaInspection, existing)
            inspection = MediaInspection(
                command.project_id,
                command.asset_version_id,
                command.asset_version_revision,
                command.asset_version_hash,
                cast(Any, command.status),
                command.metadata,
                command.tool,
                command.tool_version,
                command.operation_key,
                raw_diagnostic=command.raw_diagnostic,
                admission_refs=frozen_admission_refs,
            )
            uow.media_inspections[inspection.id] = inspection
            uow.audit_events.append(
                {
                    "type": "media.inspection.ready",
                    "projectId": command.project_id,
                    "assetVersionId": command.asset_version_id,
                    "inspectionId": inspection.id,
                }
            )
            await uow.commit()
            return inspection

    async def record_derivative(self, command: RecordDerivativeCommand) -> MediaDerivative:
        frozen_admission_refs = self._admit_owner_result(
            command.project_id,
            "media.derivative",
            command.operation_key,
            command.admission_refs,
        )
        async with self._uow_factory() as uow:
            inspection = uow.media_inspections.get(command.inspection_id)
            if inspection is None or inspection.project_id != command.project_id:
                raise ValidationDomainError("media derivative inspection is stale or foreign")
            existing = next(
                (
                    item
                    for item in uow.media_derivatives.values()
                    if item.operation_key == command.operation_key
                ),
                None,
            )
            if existing is not None:
                if existing.inspection_id != inspection.id or existing.kind != command.kind:
                    raise ValidationDomainError("media derivative operation conflict")
                return cast(MediaDerivative, existing)
            if any(
                item.inspection_id == inspection.id and item.kind == command.kind
                for item in uow.media_derivatives.values()
            ):
                raise ValidationDomainError("media derivative kind already exists")
            derivative = MediaDerivative(
                project_id=command.project_id,
                inspection_id=inspection.id,
                asset_version_id=inspection.asset_version_id,
                asset_version_revision=inspection.asset_version_revision,
                source_hash=inspection.source_hash,
                source_fingerprint=source_fingerprint(
                    inspection.asset_version_id,
                    inspection.asset_version_revision,
                    inspection.source_hash,
                ),
                kind=cast(Any, command.kind),
                status=cast(Any, command.status),
                parameters=dict(command.parameters),
                operation_key=command.operation_key,
                tool=command.tool,
                tool_version=command.tool_version,
                derivative_schema_version=command.derivative_schema_version,
                object_ref=command.object_ref,
                checksum=command.checksum,
                size_bytes=command.size_bytes,
                raw_diagnostic=command.raw_diagnostic,
                admission_refs=frozen_admission_refs,
                retention_policy=command.retention_policy,
                retention_version=command.retention_version,
                license_status=command.license_status,
                hold=command.hold,
            )
            uow.media_derivatives[derivative.id] = derivative
            uow.audit_events.append(
                {
                    "type": "media.derivative.recorded",
                    "projectId": command.project_id,
                    "inspectionId": inspection.id,
                    "derivativeId": derivative.id,
                    "status": derivative.status,
                }
            )
            await uow.commit()
            return derivative

    async def ready_derivatives(
        self,
        project_id: str,
        asset_version_id: str,
        asset_version_revision: int,
        asset_version_hash: str,
    ) -> tuple[MediaDerivative, ...]:
        expected = source_fingerprint(asset_version_id, asset_version_revision, asset_version_hash)
        async with self._uow_factory() as uow:
            return tuple(
                sorted(
                    (
                        item
                        for item in uow.media_derivatives.values()
                        if item.project_id == project_id
                        and item.asset_version_id == asset_version_id
                        and item.asset_version_revision == asset_version_revision
                        and item.source_fingerprint == expected
                        and item.status == "ready"
                    ),
                    key=lambda item: (item.kind, item.id),
                )
            )

    async def record_preview(self, preview: PreviewArtifact) -> PreviewArtifact:
        frozen_admission_refs = self._admit(
            preview.project_id,
            "media.preview",
            preview.id,
            preview.admission_refs or None,
        )
        preview = PreviewArtifact(
            preview.project_id,
            preview.episode_id,
            preview.cut_id,
            preview.cut_revision,
            preview.timeline_fingerprint,
            preview.render_plan_hash,
            preview.status,
            preview.proxy_derivative_ids,
            id=preview.id,
            schema_version=preview.schema_version,
            raw_diagnostic=preview.raw_diagnostic,
            admission_refs=frozen_admission_refs,
        )
        async with self._uow_factory() as uow:
            cut = uow.timeline_cuts.get(preview.episode_id)
            if (
                cut is None
                or cut.project_id != preview.project_id
                or cut.id != preview.cut_id
                or cut.revision != preview.cut_revision
                or cut.fingerprint() != preview.timeline_fingerprint
            ):
                raise ValidationDomainError("preview source is stale or foreign")
            derivatives = [uow.media_derivatives.get(item) for item in preview.proxy_derivative_ids]
            if any(
                item is None
                or item.project_id != preview.project_id
                or item.kind != "proxy"
                or item.status != "ready"
                for item in derivatives
            ):
                raise ValidationDomainError("preview requires exact ready proxy derivatives")
            uow.preview_artifacts[preview.id] = preview
            await uow.commit()
            return preview


class MediaDispatchService(MediaOwnerService):
    """Drain Media outbox rows without changing owner facts or operation identity."""

    async def dispatch_pending(self, starter: Any, *, limit: int = 100) -> dict[str, int]:
        async with self._uow_factory() as uow:
            pending = [
                dict(event)
                for event in uow.outbox_events
                if event.get("type") == "media.dispatch.requested"
                and event.get("status") == "pending"
            ][:limit]
        dispatched = 0
        failed = 0

        async def record_failure(operation_key: str, diagnostic: str) -> None:
            """Persist a bounded diagnostic while keeping the row pending for reconcile."""
            async with self._uow_factory() as uow:
                for current in uow.outbox_events:
                    if (
                        current.get("type") == "media.dispatch.requested"
                        and current.get("operationKey") == operation_key
                        and current.get("status") == "pending"
                    ):
                        current["lastDiagnostic"] = diagnostic[:240]
                        break
                await uow.commit()

        for event in pending:
            operation_key = str(event["operationKey"])
            workflow_id = "media-" + sha256(operation_key.encode()).hexdigest()
            operation = event.get("operation")
            # Route identity is immutable.  A malformed or legacy row is kept
            # pending for owner reconciliation, never reinterpreted as inspect.
            if (
                not isinstance(operation, str)
                or operation not in _MEDIA_OPERATIONS
                or event.get("executionRoute") != MEDIA_ROUTE
                or event.get("workflowType") != MEDIA_WORKFLOW_TYPE
                or event.get("taskQueue") != MEDIA_TASK_QUEUE
                or event.get("schemaVersion") != MEDIA_SCHEMA_VERSION
                or not isinstance(event.get("resourceAdmission"), dict)
                or not isinstance(event.get("storedObjectRef"), dict)
            ):
                failed += 1
                await record_failure(operation_key, "media dispatch route or admission is invalid")
                continue
            # Re-check owner facts after a process restart. The outbox row is
            # durable, but accepted/current candidate state may have changed
            # while the dispatcher was offline.
            try:
                episode_id = event.get("episodeId")
                shot_id = event.get("shotId")
                candidate_id = event.get("candidateId")
                candidate_revision = event.get("candidateRevision")
                provenance = event.get("provenance")
                technical_input = event.get("technicalInput")
                resource_admission = event.get("resourceAdmission")
                await self.admit_dispatch(
                    MediaDispatchAdmission(
                        project_id=str(event["projectId"]),
                        discriminator=cast(Any, event["discriminator"]),
                        asset_version_id=str(event["assetVersionId"]),
                        asset_version_revision=int(event["assetVersionRevision"]),
                        asset_version_hash=str(event["assetVersionHash"]),
                        stored_object_ref=cast(dict[str, object], event["storedObjectRef"]),
                        operation_key=operation_key,
                        episode_id=episode_id if isinstance(episode_id, str) else None,
                        shot_id=shot_id if isinstance(shot_id, str) else None,
                        candidate_id=candidate_id if isinstance(candidate_id, str) else None,
                        candidate_revision=(
                            candidate_revision if isinstance(candidate_revision, int) else None
                        ),
                        provenance=provenance if isinstance(provenance, str) else None,
                        technical_input=(
                            technical_input if isinstance(technical_input, dict) else None
                        ),
                        admission_refs=(
                            resource_admission if isinstance(resource_admission, dict) else None
                        ),
                    )
                )
            except Exception as error:
                failed += 1
                await record_failure(
                    operation_key,
                    f"media dispatch admission failed: {type(error).__name__}",
                )
                continue
            # Keep the source discriminator in the Temporal payload; it selects
            # the owner admission branch and is never inferred from mutable catalog.
            payload = {
                **event,
                "operation": operation,
                "operationDiscriminator": str(event.get("discriminator", "")),
                "workflowId": workflow_id,
            }
            try:
                await starter.start(payload)
            except Exception as error:
                failed += 1
                await record_failure(
                    operation_key,
                    f"media workflow start failed: {type(error).__name__}",
                )
                continue
            async with self._uow_factory() as uow:
                for current in uow.outbox_events:
                    if (
                        current.get("type") == "media.dispatch.requested"
                        and current.get("operationKey") == operation_key
                        and current.get("status") == "pending"
                    ):
                        current["status"] = "dispatched"
                        current["workflowId"] = workflow_id
                        dispatched += 1
                        break
                await uow.commit()
        return {"dispatched": dispatched, "failed": failed}
