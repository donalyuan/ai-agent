"""Media Worker owner commands; Timeline and Provider consume read-only projections."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, cast

from video_agent_api.domain.errors import AssetVersionNotFoundError, ValidationDomainError
from video_agent_api.domain.media import (
    MediaDerivative,
    MediaInspection,
    PreviewArtifact,
    source_fingerprint,
)


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


class MediaOwnerService:
    def __init__(self, uow_factory: Any) -> None:
        self._uow_factory = uow_factory

    async def record_inspection(self, command: RecordInspectionCommand) -> MediaInspection:
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
                "ready",
                command.metadata,
                command.tool,
                command.tool_version,
                command.operation_key,
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
