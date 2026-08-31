"""Catalog-driven image generation with continuity and immutable candidate gates."""

from __future__ import annotations

from base64 import b64decode
from dataclasses import dataclass
from hashlib import sha256
from typing import Any, cast

from video_agent_api.application.assets import (
    AssetsService,
    CreateReservationCommand,
)
from video_agent_api.application.catalog import CatalogService, RecordProviderCallCommand
from video_agent_api.application.runtime_composition import CatalogRuntimeComposition
from video_agent_api.application.storage_handoffs import (
    AssetUploadCoordinator,
    asset_upload_intent,
)
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.domain.image_generation import ImageCandidate, ImageReference
from video_agent_api.domain.provider_ops import derive_outbound_correlation
from video_agent_api.ports.contracts import (
    ImageGenerationPort,
    ModelSelection,
    PartReceipt,
    PortResult,
    StoragePort,
)
from video_agent_api.providers.gpt_image import GPTImageProvider
from video_agent_api.resilience import FrozenOperationAdmission, OperationsResilienceCoordinator


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
    # Frozen provider profile revision is part of the persisted operation identity.
    profile_revision: int
    continuity_snapshot_id: str
    continuity_snapshot_revision: int
    continuity_snapshot_hash: str
    target_revision: int
    parameters: dict[str, object]
    references: tuple[ImageReference, ...] = ()
    mask_base64: str | None = None


@dataclass(frozen=True, slots=True)
class ImageGenerationOperation:
    """The API-visible image command projection before a candidate exists."""

    id: str
    run_id: str
    logical_operation: str
    status: str
    candidate: ImageCandidate | None = None


def _request_fingerprint(command: GenerateImageCommand) -> str:
    return sha256(repr(command).encode()).hexdigest()


def _event_id(command: GenerateImageCommand, request_fingerprint: str) -> str:
    return sha256(
        f"image.generation.requested:{command.run_id}:{command.logical_operation}:"
        f"{request_fingerprint}".encode()
    ).hexdigest()


def _command_payload(command: GenerateImageCommand) -> dict[str, object]:
    return {
        "projectId": command.project_id,
        "episodeId": command.episode_id,
        "targetId": command.target_id,
        "assetId": command.asset_id,
        "runId": command.run_id,
        "logicalOperation": command.logical_operation,
        "operation": command.operation,
        "prompt": command.prompt,
        "providerId": command.provider_id,
        "profileId": command.profile_id,
        "modelId": command.model_id,
        "capabilitySnapshotId": command.capability_snapshot_id,
        "capabilityRevision": command.capability_revision,
        "profileRevision": command.profile_revision,
        "continuitySnapshotId": command.continuity_snapshot_id,
        "continuitySnapshotRevision": command.continuity_snapshot_revision,
        "continuitySnapshotHash": command.continuity_snapshot_hash,
        "targetRevision": command.target_revision,
        "parameters": dict(command.parameters),
        "references": [
            {
                "projectId": item.project_id,
                "assetVersionId": item.asset_version_id,
                "assetVersionRevision": item.asset_version_revision,
                "assetVersionHash": item.asset_version_hash,
                "mimeType": item.mime_type,
                "sizeBytes": item.size_bytes,
            }
            for item in command.references
        ],
        "maskBase64": command.mask_base64,
    }


def command_from_payload(payload: object) -> GenerateImageCommand:
    if not isinstance(payload, dict):
        raise ValidationDomainError("image generation outbox command is invalid")
    references = payload.get("references", [])
    parameters = payload.get("parameters", {})
    if not isinstance(references, list) or not all(isinstance(item, dict) for item in references):
        raise ValidationDomainError("image generation outbox references are invalid")
    if not isinstance(parameters, dict):
        raise ValidationDomainError("image generation outbox parameters are invalid")
    try:
        return GenerateImageCommand(
            project_id=str(payload["projectId"]),
            episode_id=str(payload["episodeId"]),
            target_id=str(payload["targetId"]),
            asset_id=str(payload["assetId"]),
            run_id=str(payload["runId"]),
            logical_operation=str(payload["logicalOperation"]),
            operation=str(payload["operation"]),
            prompt=str(payload["prompt"]),
            provider_id=str(payload["providerId"]),
            profile_id=str(payload["profileId"]),
            model_id=str(payload["modelId"]),
            capability_snapshot_id=str(payload["capabilitySnapshotId"]),
            capability_revision=int(payload["capabilityRevision"]),
            profile_revision=int(payload["profileRevision"]),
            continuity_snapshot_id=str(payload["continuitySnapshotId"]),
            continuity_snapshot_revision=int(payload["continuitySnapshotRevision"]),
            continuity_snapshot_hash=str(payload["continuitySnapshotHash"]),
            target_revision=int(payload["targetRevision"]),
            parameters=dict(cast(dict[str, object], parameters)),
            references=tuple(
                ImageReference(
                    str(item["projectId"]),
                    str(item["assetVersionId"]),
                    int(item["assetVersionRevision"]),
                    str(item["assetVersionHash"]),
                    str(item["mimeType"]),
                    int(item["sizeBytes"]),
                )
                for item in references
            ),
            mask_base64=(
                str(payload["maskBase64"]) if payload.get("maskBase64") is not None else None
            ),
        )
    except (KeyError, TypeError, ValueError) as error:
        raise ValidationDomainError("image generation outbox command is invalid") from error


def frozen_admission_from_payload(payload: object) -> FrozenOperationAdmission:
    """Decode the command's frozen resource gate without reading mutable defaults."""
    if not isinstance(payload, dict):
        raise ValidationDomainError("image resource admission is invalid")
    try:
        return FrozenOperationAdmission(
            operation_key=str(payload["operationKey"]),
            scope=str(payload["scope"]),
            operation=str(payload["operation"]),
            reference=str(payload["reference"]),
            resource_revision=int(payload["resourceRevision"]),
            capacity_revision=int(payload["capacityRevision"]),
            resource_hash=str(payload["resourceHash"]),
            capacity_hash=str(payload["capacityHash"]),
            allowed=bool(payload["allowed"]),
            diagnostic=(str(payload["diagnostic"]) if payload.get("diagnostic") else None),
            warning=(str(payload["warning"]) if payload.get("warning") else None),
        )
    except (KeyError, TypeError, ValueError) as error:
        raise ValidationDomainError("image resource admission is invalid") from error


def _admission_payload(admission: FrozenOperationAdmission) -> dict[str, object]:
    return {
        "operationKey": admission.operation_key,
        "scope": admission.scope,
        "operation": admission.operation,
        "reference": admission.reference,
        "resourceRevision": admission.resource_revision,
        "capacityRevision": admission.capacity_revision,
        "resourceHash": admission.resource_hash,
        "capacityHash": admission.capacity_hash,
        "allowed": admission.allowed,
        "diagnostic": admission.diagnostic,
        "warning": admission.warning,
    }


class ImageGenerationService:
    def __init__(
        self,
        uow_factory: Any,
        provider: ImageGenerationPort,
        storage: StoragePort,
        catalog: CatalogService,
        assets: AssetsService,
        *,
        resilience: OperationsResilienceCoordinator | None = None,
        live_composition: CatalogRuntimeComposition | None = None,
    ) -> None:
        self._uow_factory = uow_factory
        self._provider = provider
        self._storage = storage
        self._catalog = catalog
        self._assets = assets
        self._resilience = resilience
        self._live_composition = live_composition

    async def _resolve_provider(
        self, command: GenerateImageCommand
    ) -> tuple[ImageGenerationPort, ModelSelection]:
        if self._live_composition is None:
            return self._provider, ModelSelection(
                command.provider_id,
                command.profile_id,
                command.model_id,
                "catalog",
                dict(command.parameters),
            )
        composed = await self._live_composition.resolve_provider(
            command.profile_id,
            command.model_id,
            f"image.{command.operation}",
            project_id=command.project_id,
            expected_profile_revision=command.profile_revision,
            expected_capability_snapshot_id=command.capability_snapshot_id,
            expected_capability_revision=command.capability_revision,
        )
        if not isinstance(composed.port, GPTImageProvider):
            raise ValidationDomainError("image live provider adapter is incompatible")
        return composed.port, composed.identity.selection(command.parameters)

    async def enqueue(
        self,
        command: GenerateImageCommand,
        *,
        project_scope: str | None = None,
        frozen_admission: FrozenOperationAdmission | None = None,
    ) -> ImageGenerationOperation:
        if command.operation not in {"generate", "edit"}:
            raise ValidationDomainError("image operation is invalid")
        if isinstance(command.profile_revision, bool) or command.profile_revision < 1:
            raise ValidationDomainError("image profile revision is required")
        self._storage_identity()
        request_fingerprint = _request_fingerprint(command)
        event_id = _event_id(command, request_fingerprint)
        # Existing owner ledger is authoritative on retry; mutable catalog/provider state
        # must not block reconciliation of an already-admitted operation.
        async with self._uow_factory() as uow:
            existing_call_id = uow.provider_call_keys.get(
                (command.run_id, command.logical_operation)
            )
            existing_call = (
                uow.provider_calls.get(existing_call_id) if existing_call_id is not None else None
            )
            existing_candidate = uow.image_generation_candidates.get(
                (command.run_id, command.logical_operation)
            )
            if existing_call is not None:
                expected = (
                    command.project_id,
                    command.logical_operation,
                    f"image.{command.operation}",
                    command.provider_id,
                    command.profile_id,
                    command.model_id,
                    command.capability_snapshot_id,
                    request_fingerprint,
                )
                actual = (
                    existing_call.project_id,
                    existing_call.logical_operation,
                    existing_call.operation,
                    existing_call.provider_id,
                    existing_call.profile_id,
                    existing_call.model_id,
                    existing_call.capability_snapshot_id,
                    existing_call.request_fingerprint,
                )
                if actual != expected:
                    raise ValidationDomainError("image provider operation fingerprint conflict")
                if existing_candidate is not None or existing_call.status != "pending":
                    return ImageGenerationOperation(
                        existing_call.id,
                        command.run_id,
                        command.logical_operation,
                        existing_call.status,
                        (
                            cast(ImageCandidate, existing_candidate)
                            if existing_candidate is not None
                            else None
                        ),
                    )
                if existing_call.admission_refs is not None and self._resilience is not None:
                    frozen = frozen_admission_from_payload(existing_call.admission_refs)
                    revalidated = self._resilience.revalidate(frozen)
                    if not revalidated.allowed:
                        raise ValidationDomainError(
                            revalidated.diagnostic or "image_resource_admission_blocked"
                        )
                if not any(item.get("eventId") == event_id for item in uow.outbox_events):
                    retry_frozen = (
                        frozen_admission_from_payload(existing_call.admission_refs)
                        if existing_call.admission_refs is not None
                        else None
                    )
                    uow.outbox_events.append(
                        self._outbox_event(command, request_fingerprint, retry_frozen)
                    )
                    await uow.commit()
                return ImageGenerationOperation(
                    existing_call.id,
                    command.run_id,
                    command.logical_operation,
                    existing_call.status,
                )
        # The live runnable/capability gate precedes every ProviderCall and outbox write.
        await self._resolve_provider(command)
        if len(command.references) > 8:
            raise ValidationDomainError("image_reference_limit_exceeded")
        if sum(item.size_bytes for item in command.references) > 32 * 1024 * 1024:
            raise ValidationDomainError("image_reference_size_exceeded")
        if project_scope is not None and project_scope != command.project_id:
            raise ValidationDomainError("image_target_scope_invalid")
        admission = self._admit(command, frozen_admission)
        async with self._uow_factory() as uow:
            existing_call_id = uow.provider_call_keys.get(
                (command.run_id, command.logical_operation)
            )
            existing = uow.image_generation_candidates.get(
                (command.run_id, command.logical_operation)
            )
            call = (
                uow.provider_calls.get(existing_call_id) if existing_call_id is not None else None
            )
            if call is not None:
                expected = (
                    command.project_id,
                    command.logical_operation,
                    f"image.{command.operation}",
                    command.provider_id,
                    command.profile_id,
                    command.model_id,
                    command.capability_snapshot_id,
                    request_fingerprint,
                )
                actual = (
                    call.project_id,
                    call.logical_operation,
                    call.operation,
                    call.provider_id,
                    call.profile_id,
                    call.model_id,
                    call.capability_snapshot_id,
                    call.request_fingerprint,
                )
                if actual != expected:
                    raise ValidationDomainError("image provider operation fingerprint conflict")
                if existing is not None:
                    return ImageGenerationOperation(
                        call.id,
                        command.run_id,
                        command.logical_operation,
                        call.status,
                        cast(ImageCandidate, existing),
                    )
                if call.status == "pending" and not any(
                    item.get("eventId") == event_id for item in uow.outbox_events
                ):
                    uow.outbox_events.append(
                        self._outbox_event(command, request_fingerprint, admission)
                    )
                    await uow.commit()
                return ImageGenerationOperation(
                    call.id, command.run_id, command.logical_operation, call.status
                )
            if existing is not None:
                raise ValidationDomainError("image provider operation fingerprint conflict")
            await self._validate_enqueue_preconditions(uow, command)
            # CatalogService owns policy/concurrency/quota admission and the
            # durable ProviderCall ledger.  Image generation must not create a
            # parallel hand-written ledger that can bypass those gates.
            call = await self._catalog.record_provider_call(
                RecordProviderCallCommand(
                    project_id=command.project_id,
                    run_id=command.run_id,
                    node_run_id=None,
                    logical_operation=command.logical_operation,
                    operation=f"image.{command.operation}",
                    provider_id=command.provider_id,
                    profile_id=command.profile_id,
                    model_id=command.model_id,
                    capability_snapshot_id=command.capability_snapshot_id,
                    request_fingerprint=request_fingerprint,
                    admission_refs=(
                        _admission_payload(admission) if admission is not None else None
                    ),
                    outbound_correlation=derive_outbound_correlation(
                        command.project_id,
                        command.run_id,
                        command.logical_operation,
                        f"image.{command.operation}",
                        request_fingerprint,
                    ),
                )
            )
            uow.outbox_events.append(self._outbox_event(command, request_fingerprint, admission))
            await uow.commit()
            return ImageGenerationOperation(
                call.id, command.run_id, command.logical_operation, call.status
            )

    def _outbox_event(
        self,
        command: GenerateImageCommand,
        request_fingerprint: str,
        admission: FrozenOperationAdmission | None,
    ) -> dict[str, object]:
        event: dict[str, object] = {
            "type": "image.generation.requested",
            "eventId": _event_id(command, request_fingerprint),
            "status": "pending",
            "projectId": command.project_id,
            "runId": command.run_id,
            "logicalOperation": command.logical_operation,
            "requestFingerprint": request_fingerprint,
            "executionRoute": "generation",
            "workflowType": "image-generation",
            "taskQueue": "generation-tasks",
            "schemaVersion": "1.0.0",
            "admission": {
                "providerId": command.provider_id,
                "profileId": command.profile_id,
                "profileRevision": command.profile_revision,
                "modelId": command.model_id,
                "capabilitySnapshotId": command.capability_snapshot_id,
                "capabilityRevision": command.capability_revision,
                "continuitySnapshotId": command.continuity_snapshot_id,
                "continuitySnapshotRevision": command.continuity_snapshot_revision,
                "continuitySnapshotHash": command.continuity_snapshot_hash,
                "targetRevision": command.target_revision,
            },
            "command": _command_payload(command),
        }
        if admission is not None:
            event["resourceAdmission"] = _admission_payload(admission)
        return event

    def _admit(
        self,
        command: GenerateImageCommand,
        frozen: FrozenOperationAdmission | None,
    ) -> FrozenOperationAdmission | None:
        if self._resilience is None:
            if frozen is not None:
                raise ValidationDomainError("image_resource_admission_unconfigured")
            return None
        operation_key = f"{command.run_id}:{command.logical_operation}"
        operation = f"image.{command.operation}"
        if frozen is None:
            admission = self._resilience.freeze(
                command.project_id,
                operation,
                operation_key,
                required_bytes=sum(item.size_bytes for item in command.references),
            )
        else:
            if (
                frozen.scope != command.project_id
                or frozen.operation != operation
                or frozen.operation_key != operation_key
            ):
                raise ValidationDomainError("image_resource_admission_mismatch")
            admission = self._resilience.revalidate(frozen)
        if not admission.allowed:
            raise ValidationDomainError(admission.diagnostic or "image_resource_admission_blocked")
        return admission

    def _storage_identity(self) -> tuple[str, int]:
        """Require the exact storage identity before an image operation creates intent."""
        if self._storage is None:
            raise ValidationDomainError("image storage is unconfigured")
        string_fields = (
            "adapter_key",
            "profile_id",
            "bucket_binding_id",
            "bucket",
            "endpoint",
            "region",
        )
        values = {name: getattr(self._storage, name, None) for name in string_fields}
        if not all(isinstance(value, str) and value for value in values.values()):
            raise ValidationDomainError("image storage identity is incomplete")
        profile_revision = getattr(self._storage, "profile_revision", None)
        if (
            isinstance(profile_revision, bool)
            or not isinstance(profile_revision, int)
            or profile_revision < 1
        ):
            raise ValidationDomainError("image storage identity is incomplete")
        return str(values["profile_id"]), profile_revision

    async def _validate_enqueue_preconditions(
        self, uow: Any, command: GenerateImageCommand
    ) -> None:
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
            or not provider.enabled
            or not profile.enabled
            or model is None
            or model.profile_id != profile.id
            or not model.enabled
        ):
            raise ValidationDomainError("image_model_unconfigured")
        snapshot_operation = f"image.{command.operation}"
        snapshot_capability = profile.capability_snapshots.get(snapshot_operation)
        if snapshot_capability is None or snapshot_capability.id != command.capability_snapshot_id:
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
                or version.storage_object.mime_type not in {"image/png", "image/jpeg", "image/webp"}
            ):
                raise ValidationDomainError("image_reference_stale_or_foreign")

    async def execute(
        self,
        command: GenerateImageCommand,
        *,
        frozen_admission: FrozenOperationAdmission | None = None,
    ) -> ImageCandidate:
        operation = await self.enqueue(command, frozen_admission=frozen_admission)
        if operation.candidate is not None:
            return operation.candidate

        _call, acquired = await self._catalog.claim_provider_call(
            command.run_id,
            command.logical_operation,
            expected_operation=f"image.{command.operation}",
        )
        if not acquired:
            raise ValidationDomainError("image provider operation requires reconciliation")
        # Claim the frozen owner ledger before consulting mutable catalog/provider
        # state; retries must reconcile rather than re-resolve or re-submit.
        if not _call.outbound_correlation:
            raise ValidationDomainError("image provider correlation is unavailable")
        provider, selection = await self._resolve_provider(command)
        try:
            result: PortResult = (
                provider.generate_image(command.prompt, selection, _call.outbound_correlation)
                if command.operation == "generate"
                else provider.edit_image(command.prompt, selection, _call.outbound_correlation)
            )
        except Exception as error:
            # A transport failure can occur after remote acceptance. Leave the durable
            # owner ledger unknown so retry cannot issue a second chargeable request.
            await self._catalog.finalize_provider_call(
                command.run_id,
                command.logical_operation,
                status="unknown",
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
        validator = provider if isinstance(provider, GPTImageProvider) else GPTImageProvider()
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
        asset = await self._assets.get_asset(command.asset_id)
        if asset.project_id != command.project_id:
            raise ValidationDomainError("image_asset_scope_invalid")
        request_fingerprint = _request_fingerprint(command)
        storage_profile_id, storage_profile_revision = self._storage_identity()
        reservation = await self._assets.create_reservation(
            CreateReservationCommand(
                project_id=command.project_id,
                asset_id=asset.id,
                fingerprint=request_fingerprint,
                expected_asset_revision=asset.revision,
                declared_kind=asset.kind,
                declared_mime_type=mime,
                declared_size_bytes=len(data),
                declared_checksum=checksum,
                storage_profile_id=str(storage_profile_id),
                storage_profile_revision=storage_profile_revision,
                storage_profile_snapshot_hash=sha256(
                    f"{storage_profile_id}:{storage_profile_revision}".encode()
                ).hexdigest(),
            )
        )
        object_key = reservation.upload_key
        try:
            intent = asset_upload_intent(
                reservation,
                str(storage_profile_id),
                object_key,
                expected_size_bytes=len(data),
                expected_checksum=checksum,
                expected_mime_type=mime,
            )
            session = self._storage.create_multipart(intent, command.run_id)
            receipt = PartReceipt(1, checksum, checksum, len(data))
            self._storage.upload_part(session, receipt, data, command.run_id)
            version = await AssetUploadCoordinator(
                self._storage, self._assets
            ).complete_and_register(reservation.id, session, (receipt,), command.run_id)
        except Exception as error:
            await self._catalog.finalize_provider_call(
                command.run_id,
                command.logical_operation,
                status="failed",
                failure_code=type(error).__name__,
                provider_request_id=result.request_id,
            )
            raise
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
