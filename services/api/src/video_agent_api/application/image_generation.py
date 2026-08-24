"""Catalog-driven image generation with continuity and immutable candidate gates."""

from __future__ import annotations

from base64 import b64decode
from dataclasses import dataclass
from hashlib import sha256
from typing import Any, cast

from video_agent_api.application.assets import AppendAssetVersionCommand, AssetsService
from video_agent_api.application.catalog import CatalogService, RecordProviderCallCommand
from video_agent_api.application.storage_handoffs import upload_verified_bytes
from video_agent_api.domain.assets import StorageObject
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.domain.image_generation import ImageCandidate, ImageReference
from video_agent_api.ports.contracts import (
    ImageGenerationPort,
    ModelSelection,
    PortResult,
    StoragePort,
)
from video_agent_api.providers.gpt_image import GPTImageProvider


@dataclass(frozen=True, slots=True)
class GenerateImageCommand:
    project_id: str
    episode_id: str
    target_id: str
    asset_id: str
    run_id: str
    logical_operation: str
    operation: str
    prompt: str
    provider_id: str
    profile_id: str
    model_id: str
    capability_snapshot_id: str
    capability_revision: int
    continuity_snapshot_id: str
    continuity_snapshot_revision: int
    continuity_snapshot_hash: str
    target_revision: int
    parameters: dict[str, object]
    references: tuple[ImageReference, ...] = ()
    mask_base64: str | None = None


class ImageGenerationService:
    def __init__(
        self,
        uow_factory: Any,
        provider: ImageGenerationPort,
        storage: StoragePort,
        catalog: CatalogService,
        assets: AssetsService,
    ) -> None:
        self._uow_factory = uow_factory
        self._provider = provider
        self._storage = storage
        self._catalog = catalog
        self._assets = assets

    async def execute(self, command: GenerateImageCommand) -> ImageCandidate:
        if command.operation not in {"generate", "edit"}:
            raise ValidationDomainError("image operation is invalid")
        if len(command.references) > 8:
            raise ValidationDomainError("image_reference_limit_exceeded")
        if sum(item.size_bytes for item in command.references) > 32 * 1024 * 1024:
            raise ValidationDomainError("image_reference_size_exceeded")
        async with self._uow_factory() as uow:
            existing = uow.image_generation_candidates.get(
                (command.run_id, command.logical_operation)
            )
            if existing is not None:
                return cast(ImageCandidate, existing)
            snapshot = uow.asset_bible_snapshots.get(command.continuity_snapshot_id)
            if snapshot is None or snapshot.project_id != command.project_id:
                raise ValidationDomainError("continuity_snapshot_scope_invalid")
            if (
                snapshot.status != "accepted"
                or snapshot.revision != command.continuity_snapshot_revision
                or snapshot.content_hash != command.continuity_snapshot_hash
                or snapshot.target_id != command.target_id
                or snapshot.target_revision != command.target_revision
            ):
                raise ValidationDomainError("continuity_snapshot_stale")
            episode = await uow.episodes.get(command.episode_id)
            if episode is None or episode.project_id != command.project_id:
                raise ValidationDomainError("image_target_scope_invalid")
            for assignment in snapshot.refs:
                entry = uow.asset_bible_entries.get(assignment.entry_id)
                version = (
                    next(
                        (item for item in entry.versions if item.id == assignment.version_id),
                        None,
                    )
                    if entry is not None
                    else None
                )
                if (
                    entry is None
                    or entry.project_id != command.project_id
                    or version is None
                    or version.revision != assignment.version_revision
                    or version.content_hash != assignment.content_hash
                ):
                    raise ValidationDomainError("continuity_snapshot_incomplete")
            pending = [
                task
                for task in uow.asset_bible_tasks.values()
                if task.project_id == command.project_id
                and task.target_id == command.target_id
                and task.status in {"pending", "acknowledged"}
            ]
            if pending:
                raise ValidationDomainError("continuity_revision_pending")
            profile = uow.profiles.get(command.profile_id)
            model = uow.models.get(command.model_id)
            provider = uow.providers.get(command.provider_id)
            if (
                provider is None
                or profile is None
                or profile.provider_id != provider.id
                or provider.approval != "approved"
                or provider.feature_gate != "MVP-A"
                or not provider.adapter_installed
                or model is None
                or model.profile_id != profile.id
                or not model.enabled
            ):
                raise ValidationDomainError("image_model_unconfigured")
            snapshot_operation = f"image.{command.operation}"
            snapshot_capability = profile.capability_snapshots.get(snapshot_operation)
            if (
                snapshot_capability is None
                or snapshot_capability.id != command.capability_snapshot_id
            ):
                raise ValidationDomainError("image_capability_snapshot_invalid")
            if (
                snapshot_capability.revision != command.capability_revision
                or not snapshot_capability.runnable
            ):
                raise ValidationDomainError("image_capability_snapshot_stale")
            asset = await uow.assets.get(command.asset_id)
            if asset is None or asset.project_id != command.project_id or asset.kind != "image":
                raise ValidationDomainError("image_asset_scope_invalid")
            for reference in command.references:
                if reference.project_id != command.project_id:
                    raise ValidationDomainError("image_reference_scope_invalid")
                version = await uow.asset_versions.get(reference.asset_version_id)
                if (
                    version is None
                    or version.project_id != command.project_id
                    or version.revision != reference.asset_version_revision
                    or version.content_hash != reference.asset_version_hash
                    or version.storage_object.mime_type
                    not in {"image/png", "image/jpeg", "image/webp"}
                ):
                    raise ValidationDomainError("image_reference_stale_or_foreign")

        selection = ModelSelection(
            command.provider_id,
            command.profile_id,
            command.model_id,
            "catalog",
            dict(command.parameters),
        )
        request_fingerprint = sha256(repr(command).encode()).hexdigest()
        await self._catalog.record_provider_call(
            RecordProviderCallCommand(
                command.project_id,
                command.run_id,
                None,
                command.logical_operation,
                f"image.{command.operation}",
                command.provider_id,
                command.profile_id,
                command.model_id,
                request_fingerprint,
                capability_snapshot_id=command.capability_snapshot_id,
            )
        )
        try:
            result: PortResult = (
                self._provider.generate_image(command.prompt, selection, command.run_id)
                if command.operation == "generate"
                else self._provider.edit_image(command.prompt, selection, command.run_id)
            )
        except Exception as error:
            await self._catalog.finalize_provider_call(
                command.run_id,
                command.logical_operation,
                status="failed",
                failure_code=type(error).__name__,
            )
            raise
        payload = result.payload
        encoded = payload.get("base64")
        mime = payload.get("mimeType")
        width = payload.get("width")
        height = payload.get("height")
        if not isinstance(encoded, str) or not isinstance(mime, str):
            raise ValidationDomainError("image_result_invalid")
        if not isinstance(width, int) or not isinstance(height, int):
            raise ValidationDomainError("image_dimensions_missing")
        validator = (
            self._provider if isinstance(self._provider, GPTImageProvider) else GPTImageProvider()
        )
        try:
            data = validator.validate_base64_image(
                encoded, mime, len(b64decode(encoded)), width, height
            )
            if command.mask_base64 is not None:
                validator.validate_base64_image(
                    command.mask_base64,
                    "image/png",
                    len(b64decode(command.mask_base64)),
                    width,
                    height,
                )
        except Exception as error:
            await self._catalog.finalize_provider_call(
                command.run_id,
                command.logical_operation,
                status="failed",
                failure_code=type(error).__name__,
                provider_request_id=result.request_id,
            )
            raise
        checksum = sha256(data).hexdigest()
        try:
            stored = upload_verified_bytes(
                self._storage,
                operation_key=f"image-output:{command.run_id}:{command.logical_operation}",
                project_id=command.project_id,
                profile_id="local",
                object_key=(
                    f"projects/{command.project_id}/generated/{command.run_id}/"
                    f"{command.logical_operation}.bin"
                ),
                content=data,
                mime_type=mime,
                correlation_id=command.run_id,
            )
        except Exception as error:
            await self._catalog.finalize_provider_call(
                command.run_id,
                command.logical_operation,
                status="failed",
                failure_code=type(error).__name__,
                provider_request_id=result.request_id,
            )
            raise
        asset = await self._assets.get_asset(command.asset_id)
        if asset.project_id != command.project_id:
            raise ValidationDomainError("image_asset_scope_invalid")
        version = await self._assets.append_version(
            AppendAssetVersionCommand(
                asset.id,
                StorageObject(
                    "local_workspace",
                    "workspace",
                    stored.object_key,
                    mime,
                    stored.size_bytes,
                    checksum,
                    e_tag=stored.etag,
                    media={"width": width, "height": height},
                ),
                checksum,
            )
        )
        async with self._uow_factory() as uow:
            candidate = ImageCandidate(
                command.project_id,
                command.episode_id,
                command.target_id,
                asset.id,
                command.operation,  # type: ignore[arg-type]
                command.run_id,
                command.logical_operation,
                version.id,
                version.revision,
                version.content_hash or checksum,
                command.continuity_snapshot_id,
                command.continuity_snapshot_revision,
                command.continuity_snapshot_hash,
                {"requestId": result.request_id, "correlationId": result.correlation_id},
            )
            uow.image_generation_candidates[(command.run_id, command.logical_operation)] = candidate
            await uow.commit()
            await self._catalog.finalize_provider_call(
                command.run_id,
                command.logical_operation,
                status="succeeded",
                provider_request_id=result.request_id,
            )
            return candidate
