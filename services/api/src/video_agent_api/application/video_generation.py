"""Agnes submit preflight and deterministic mock video lifecycle."""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass, replace
from hashlib import sha256
from typing import Any, cast

from video_agent_api.application.assets import AppendAssetVersionCommand, AssetsService
from video_agent_api.application.catalog import CatalogService, RecordProviderCallCommand
from video_agent_api.application.storage_handoffs import upload_verified_bytes
from video_agent_api.domain.assets import StorageObject
from video_agent_api.domain.errors import ProjectAccessForbiddenError, ValidationDomainError
from video_agent_api.domain.video_generation import VideoOperation, VideoTakeCandidate
from video_agent_api.ports.contracts import (
    ModelSelection,
    PortResult,
    StoragePort,
    VideoGenerationPort,
)
from video_agent_api.providers.agnes import AgnesVideoProvider


@dataclass(frozen=True, slots=True)
class SubmitVideoCommand:
    project_id: str
    episode_id: str
    scene_id: str
    shot_id: str
    run_id: str
    logical_operation: str
    provider_id: str
    profile_id: str
    model_id: str
    capability_snapshot_id: str
    capability_revision: int
    source_asset_version_id: str
    source_asset_version_revision: int
    source_asset_version_hash: str
    shot_spec_id: str
    shot_spec_revision: int
    shot_spec_hash: str
    duration_seconds: float
    aspect_ratio: str
    parameters: dict[str, object]
    prompt: str
    source_candidate_id: str | None = None
    source_provenance: str | None = None
    source_schema_version: str | None = None
    schema_version: str | None = None


@dataclass(frozen=True, slots=True)
class PollVideoCommand:
    run_id: str
    logical_operation: str


@dataclass(frozen=True, slots=True)
class ReconcileVideoCommand:
    run_id: str
    logical_operation: str
    provider_request_id: str | None


@dataclass(frozen=True, slots=True)
class ReviewVideoCandidateCommand:
    candidate_id: str
    action: str
    expected_revision: int
    successor_logical_operation: str | None = None
    expected_shot_revision: int | None = None


def video_request_fingerprint(command: SubmitVideoCommand) -> str:
    return sha256(
        json.dumps(asdict(command), sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def _normalized_status(payload: dict[str, object]) -> str:
    raw_status = str(payload.get("status", "submission_unknown")).lower()
    return {
        "queued": "submitted",
        "processing": "running",
        "complete": "succeeded",
        "success": "succeeded",
        "error": "failed",
    }.get(raw_status, raw_status)


class AgnesVideoService:
    def __init__(
        self,
        uow_factory: Any,
        provider: VideoGenerationPort,
        catalog: CatalogService,
        storage: StoragePort | None = None,
        assets: AssetsService | None = None,
    ) -> None:
        self._uow_factory = uow_factory
        self._provider = provider
        self._catalog = catalog
        self._storage = storage
        self._assets = assets

    async def submit(self, command: SubmitVideoCommand) -> VideoOperation:
        if command.duration_seconds <= 0 or not command.aspect_ratio:
            raise ValidationDomainError("video_duration_or_aspect_missing")
        if (
            command.source_schema_version is not None
            and command.schema_version is not None
            and command.source_schema_version != command.schema_version
        ):
            raise ValidationDomainError("schema_version_conflict")
        if isinstance(self._provider, AgnesVideoProvider):
            # Capability validation is deliberately before intent/ProviderCall writes.
            self._provider.validate_capability("submit", command.parameters)
        request_fingerprint = video_request_fingerprint(command)
        node_run_id: str
        adapter_key: str
        async with self._uow_factory() as uow:
            existing = uow.video_operations.get((command.run_id, command.logical_operation))
            if existing is not None:
                call_id = uow.provider_call_keys.get((command.run_id, command.logical_operation))
                call = uow.provider_calls.get(call_id) if call_id is not None else None
                if call is None or call.request_fingerprint != request_fingerprint:
                    raise ValidationDomainError("agnes_video_operation_fingerprint_conflict")
                return cast(VideoOperation, existing)
            run = uow.workflow_runs.get(command.run_id)
            node = (
                next(
                    (
                        item
                        for item in run.nodes
                        if item.node_key == "media.generate.video"
                        and item.logical_operation == command.logical_operation
                    ),
                    None,
                )
                if run is not None
                else None
            )
            if (
                run is None
                or run.project_id != command.project_id
                or run.status != "running"
                or node is None
                or node.status != "running"
            ):
                raise ValidationDomainError("agnes_video_run_scope_or_node_invalid")
            frozen = run.selection_snapshot
            frozen_capabilities = frozen.get("capabilitySnapshots")
            frozen_video_capability = (
                frozen_capabilities.get("video.submit")
                if isinstance(frozen_capabilities, dict)
                else None
            )
            if (
                frozen.get("providerId") != command.provider_id
                or frozen.get("profileId") != command.profile_id
                or frozen.get("modelId") != command.model_id
                or not isinstance(frozen_video_capability, dict)
                or frozen_video_capability.get("id") != command.capability_snapshot_id
                or frozen_video_capability.get("revision") != command.capability_revision
                or not isinstance(frozen.get("adapterKey"), str)
            ):
                raise ValidationDomainError("agnes_video_run_selection_mismatch")
            node_run_id = node.id
            adapter_key = str(frozen["adapterKey"])
            gate = uow.budget_gates.get(f"{command.run_id}:{command.logical_operation}")
            if (
                gate is None
                or gate.project_id != command.project_id
                or gate.run_id != command.run_id
                or gate.node_run_id != node.id
                or gate.logical_operation != command.logical_operation
                or gate.request_fingerprint != request_fingerprint
                or gate.status != "confirmed"
                or not gate.operation_kind.startswith("video")
            ):
                raise ValidationDomainError("agnes_video_budget_gate_unconfirmed_or_stale")
            profile = uow.profiles.get(command.profile_id)
            provider = uow.providers.get(command.provider_id)
            model = uow.models.get(command.model_id)
            if (
                profile is None
                or provider is None
                or model is None
                or profile.provider_id != provider.id
                or model.profile_id != profile.id
                or not model.enabled
                or not provider.enabled
                or not provider.adapter_installed
                or provider.approval != "approved"
                or not profile.enabled
                or provider.adapter_key != adapter_key
                or profile.adapter_identity != frozen.get("adapterIdentity")
                or profile.revision != frozen.get("profileRevision")
            ):
                raise ValidationDomainError("agnes_video_selection_unconfigured")
            if provider.adapter_key == "agnes" and (
                provider.feature_gate != "MVP-A"
                or not profile.explicit_live_opt_in
                or profile.credential_status != "configured"
            ):
                raise ValidationDomainError("agnes_video_unconfigured")
            capability = profile.capability_snapshots.get("video.submit")
            if (
                capability is None
                or capability.id != command.capability_snapshot_id
                or capability.revision != command.capability_revision
                or capability.provider_id != command.provider_id
                or capability.profile_id != command.profile_id
                or capability.operation != "video.submit"
                or not capability.runnable
                or (capability.model_id is not None and capability.model_id != command.model_id)
            ):
                raise ValidationDomainError("agnes_video_capability_snapshot_invalid")
            shot = uow.shots.get(command.shot_id)
            if (
                shot is None
                or shot.project_id != command.project_id
                or shot.episode_id != command.episode_id
                or shot.scene_id != command.scene_id
            ):
                raise ValidationDomainError("agnes_video_shot_scope_invalid")
            if shot.current_image is None:
                raise ValidationDomainError("agnes_video_image_candidate_unaccepted")
            current = shot.current_image
            if (
                current.asset_version_id != command.source_asset_version_id
                or current.asset_version_revision != command.source_asset_version_revision
                or current.asset_version_hash != command.source_asset_version_hash
            ):
                raise ValidationDomainError("agnes_video_source_stale")
            if (
                command.source_candidate_id is not None
                and current.candidate_id != command.source_candidate_id
            ):
                raise ValidationDomainError("agnes_video_source_candidate_stale")
            if (
                command.source_provenance is not None
                and current.provenance != command.source_provenance
            ):
                raise ValidationDomainError("agnes_video_source_provenance_stale")
            canonical_schema = getattr(
                version := await uow.asset_versions.get(command.source_asset_version_id),
                "schema_version",
                None,
            )
            if (
                canonical_schema is not None
                and command.source_schema_version is not None
                and canonical_schema != command.source_schema_version
            ):
                raise ValidationDomainError("schema_version_conflict")
            if shot.spec_ref is None or (
                shot.spec_ref.id != command.shot_spec_id
                or shot.spec_ref.revision != command.shot_spec_revision
                or shot.spec_ref.content_hash != command.shot_spec_hash
            ):
                raise ValidationDomainError("agnes_video_shot_spec_stale")
            version = await uow.asset_versions.get(command.source_asset_version_id)
            if version is None or version.project_id != command.project_id:
                raise ValidationDomainError("agnes_video_source_foreign")
            operation = VideoOperation(
                command.project_id,
                command.run_id,
                command.logical_operation,
                command.provider_id,
                command.profile_id,
                command.model_id,
                command.capability_snapshot_id,
                command.source_asset_version_id,
                command.source_asset_version_revision,
                command.source_asset_version_hash,
                command.shot_spec_id,
                command.shot_spec_revision,
                command.shot_spec_hash,
                command.duration_seconds,
                command.aspect_ratio,
                episode_id=command.episode_id,
                target_id=command.shot_id,
                asset_id=command.source_asset_version_id,
                source_candidate_id=command.source_candidate_id,
                source_provenance=command.source_provenance,
            )
            uow.video_operations[(command.run_id, command.logical_operation)] = operation
            await uow.commit()

        selection = ModelSelection(
            command.provider_id,
            command.profile_id,
            command.model_id,
            adapter_key,
            dict(command.parameters),
        )
        await self._catalog.record_provider_call(
            RecordProviderCallCommand(
                command.project_id,
                command.run_id,
                node_run_id,
                command.logical_operation,
                "video.submit",
                command.provider_id,
                command.profile_id,
                command.model_id,
                request_fingerprint,
                capability_snapshot_id=command.capability_snapshot_id,
            )
        )
        try:
            result: PortResult = self._provider.submit_video(
                command.prompt, selection, command.run_id
            )
        except Exception as error:
            await self._catalog.finalize_provider_call(
                command.run_id,
                command.logical_operation,
                status="unknown",
                failure_code=type(error).__name__,
            )
            async with self._uow_factory() as uow:
                operation = cast(
                    VideoOperation,
                    uow.video_operations[(command.run_id, command.logical_operation)],
                )
                operation.transition("submission_unknown")
                await uow.commit()
            raise
        provider_request_id = result.payload.get("providerRequestId") or result.request_id
        async with self._uow_factory() as uow:
            operation = cast(
                VideoOperation,
                uow.video_operations[(command.run_id, command.logical_operation)],
            )
            operation.provider_request_id = str(provider_request_id)
            operation.transition("submitted")
            await uow.commit()
            return operation

    async def cancel(
        self,
        run_id: str,
        logical_operation: str,
        *,
        project_scope: str | None = None,
    ) -> str:
        async with self._uow_factory() as uow:
            operation = uow.video_operations.get((run_id, logical_operation))
            if operation is None:
                raise ValidationDomainError("video operation not found")
            operation = cast(VideoOperation, operation)
            if project_scope is not None and operation.project_id != project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            if operation.status in {"succeeded", "failed", "cancelled"}:
                return operation.status
            provider_request_id = operation.provider_request_id
            operation.cancel()
            await uow.commit()
        if provider_request_id:
            try:
                self._provider.cancel_video(provider_request_id, run_id)
                await self._catalog.finalize_provider_call(
                    run_id,
                    logical_operation,
                    status="cancelled",
                    provider_request_id=provider_request_id,
                )
            except Exception:
                # Cancellation remains persisted; reconciliation/poll retains diagnostic evidence.
                pass
        return "cancelled"

    async def poll(
        self, command: PollVideoCommand, *, project_scope: str | None = None
    ) -> VideoOperation:
        async with self._uow_factory() as uow:
            operation = uow.video_operations.get((command.run_id, command.logical_operation))
            if operation is None:
                raise ValidationDomainError("video operation not found")
            operation = cast(VideoOperation, operation)
            if project_scope is not None and operation.project_id != project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            if operation.status in {"succeeded", "failed", "cancelled"}:
                return operation
            if not operation.provider_request_id:
                operation.transition("submission_unknown")
                await uow.commit()
                return operation
            request_id = operation.provider_request_id
        try:
            result = self._provider.get_video_status(request_id, command.run_id)
        except Exception:
            async with self._uow_factory() as uow:
                operation = cast(
                    VideoOperation,
                    uow.video_operations[(command.run_id, command.logical_operation)],
                )
                operation.transition("submission_unknown")
                await uow.commit()
                return operation
        payload = result.payload
        normalized = _normalized_status(payload)
        fingerprint = sha256(json.dumps(payload, sort_keys=True, default=str).encode()).hexdigest()
        async with self._uow_factory() as uow:
            operation = cast(
                VideoOperation, uow.video_operations[(command.run_id, command.logical_operation)]
            )
            operation.observe(
                normalized, fingerprint, str(payload.get("providerRequestId") or request_id)
            )
            await uow.commit()
            if operation.status in {"succeeded", "failed"}:
                usage = payload.get("usage")
                await self._catalog.finalize_provider_call(
                    command.run_id,
                    command.logical_operation,
                    status=operation.status,
                    provider_request_id=operation.provider_request_id,
                    native_usage=usage if isinstance(usage, dict) else None,
                )
            return operation

    async def reconcile(
        self, command: ReconcileVideoCommand, *, project_scope: str | None = None
    ) -> VideoOperation:
        """Resolve submission_unknown using persisted provider identity without resubmitting."""
        async with self._uow_factory() as uow:
            operation = uow.video_operations.get((command.run_id, command.logical_operation))
            if operation is None:
                raise ValidationDomainError("video operation not found")
            operation = cast(VideoOperation, operation)
            if project_scope is not None and operation.project_id != project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            if operation.status != "submission_unknown":
                raise ValidationDomainError("video_reconciliation_requires_submission_unknown")
            if command.provider_request_id is not None and (
                command.provider_request_id != operation.provider_request_id
            ):
                raise ValidationDomainError("video_reconciliation_provider_request_mismatch")
            provider_request_id = operation.provider_request_id
            if provider_request_id is None:
                return operation
        try:
            result = self._provider.get_video_status(provider_request_id, command.run_id)
        except Exception:
            async with self._uow_factory() as uow:
                return cast(
                    VideoOperation,
                    uow.video_operations[(command.run_id, command.logical_operation)],
                )
        payload = result.payload
        normalized = _normalized_status(payload)
        fingerprint = sha256(json.dumps(payload, sort_keys=True, default=str).encode()).hexdigest()
        async with self._uow_factory() as uow:
            operation = cast(
                VideoOperation,
                uow.video_operations[(command.run_id, command.logical_operation)],
            )
            if operation.status != "submission_unknown":
                raise ValidationDomainError("video_reconciliation_requires_submission_unknown")
            operation.observe(
                normalized,
                fingerprint,
                str(payload.get("providerRequestId") or provider_request_id),
            )
            await uow.commit()
        if operation.status in {"succeeded", "failed"}:
            usage = payload.get("usage")
            await self._catalog.finalize_provider_call(
                command.run_id,
                command.logical_operation,
                status=operation.status,
                provider_request_id=operation.provider_request_id,
                native_usage=usage if isinstance(usage, dict) else None,
            )
        return operation

    async def register_result(
        self,
        run_id: str,
        logical_operation: str,
        *,
        asset_version_id: str,
        asset_version_revision: int,
        asset_version_hash: str,
        provider_request_id: str | None,
        asset_id: str | None = None,
        media_bytes: bytes | None = None,
        mime_type: str = "video/mp4",
        width: int = 1,
        height: int = 1,
    ) -> VideoTakeCandidate:
        async with self._uow_factory() as uow:
            operation = uow.video_operations.get((run_id, logical_operation))
            if operation is None:
                raise ValidationDomainError("video operation not found")
            operation = cast(VideoOperation, operation)
            existing = next(
                (
                    item
                    for item in uow.video_take_candidates.values()
                    if item.run_id == run_id and item.logical_operation == logical_operation
                ),
                None,
            )
            if existing is not None:
                return cast(VideoTakeCandidate, existing)
            if operation.status not in {"succeeded", "cancelled"}:
                raise ValidationDomainError("video operation is not terminal")
            version = await uow.asset_versions.get(asset_version_id)
            source_version = await uow.asset_versions.get(operation.source_asset_version_id)
        if media_bytes is not None:
            if self._storage is None or self._assets is None:
                raise ValidationDomainError("video_storage_unconfigured")
            if not isinstance(self._provider, AgnesVideoProvider):
                raise ValidationDomainError("video_media_validator_unconfigured")
            _, checksum = self._provider.validate_video_media(
                media_bytes,
                mime_type,
                duration_seconds=operation.duration_seconds,
                width=width,
                height=height,
            )
            target_asset_id = asset_id or getattr(source_version, "asset_id", None)
            if not target_asset_id:
                raise ValidationDomainError("video_result_asset_scope_invalid")
            stored = upload_verified_bytes(
                self._storage,
                operation_key=f"video-output:{run_id}:{logical_operation}",
                project_id=operation.project_id,
                profile_id="local",
                object_key=(
                    f"projects/{operation.project_id}/generated/{run_id}/{logical_operation}.mp4"
                ),
                content=media_bytes,
                mime_type=mime_type,
                correlation_id=run_id,
            )
            created = await self._assets.append_version(
                AppendAssetVersionCommand(
                    target_asset_id,
                    StorageObject(
                        "local_workspace",
                        stored.bucket,
                        stored.object_key,
                        mime_type,
                        stored.size_bytes,
                        checksum,
                        e_tag=stored.etag,
                        media={
                            "duration_ms": round(operation.duration_seconds * 1000),
                            "width": width,
                            "height": height,
                        },
                    ),
                    checksum,
                )
            )
            asset_version_id = created.id
            asset_version_revision = created.revision
            asset_version_hash = created.content_hash or checksum
            version = created
        async with self._uow_factory() as uow:
            existing = next(
                (
                    item
                    for item in uow.video_take_candidates.values()
                    if item.run_id == run_id and item.logical_operation == logical_operation
                ),
                None,
            )
            if existing is not None:
                return cast(VideoTakeCandidate, existing)
            if version is not None:
                if (
                    version.project_id != operation.project_id
                    or getattr(version, "revision", None) != asset_version_revision
                    or getattr(version, "content_hash", None) != asset_version_hash
                ):
                    raise ValidationDomainError("video_result_asset_version_stale")
                if (
                    getattr(
                        getattr(version, "storage_object", None), "mime_type", "video/mp4"
                    ).split("/", 1)[0]
                    != "video"
                ):
                    raise ValidationDomainError("video_result_mime_invalid")
            candidate = VideoTakeCandidate(
                project_id=operation.project_id,
                episode_id=operation.episode_id,
                target_id=operation.target_id,
                run_id=operation.run_id,
                logical_operation=operation.logical_operation,
                source_asset_version_id=operation.source_asset_version_id,
                source_asset_version_revision=operation.source_asset_version_revision,
                source_asset_version_hash=operation.source_asset_version_hash,
                shot_spec_id=operation.shot_spec_id,
                shot_spec_revision=operation.shot_spec_revision,
                shot_spec_hash=operation.shot_spec_hash,
                duration_seconds=operation.duration_seconds,
                aspect_ratio=operation.aspect_ratio,
                asset_version_id=asset_version_id,
                asset_version_revision=asset_version_revision,
                asset_version_hash=asset_version_hash,
                provider_request_id=provider_request_id,
                source_candidate_id=operation.source_candidate_id,
                source_provenance=operation.source_provenance or "agnes_video",
            )
            uow.video_take_candidates[candidate.id] = candidate
            await uow.commit()
            return candidate

    async def review_candidate(
        self,
        candidate_id: str,
        action: str,
        expected_revision: int,
        successor_logical_operation: str | None = None,
        expected_shot_revision: int | None = None,
    ) -> VideoTakeCandidate:
        if action == "approve" or action not in {"accept", "reject", "retake"}:
            raise ValidationDomainError("video take review action is invalid")
        async with self._uow_factory() as uow:
            existing = uow.video_take_candidates.get(candidate_id)
            if existing is None:
                raise ValidationDomainError("video take candidate not found")
            candidate = cast(VideoTakeCandidate, existing)
            if action == "retake":
                if candidate.revision != expected_revision:
                    raise ValidationDomainError("video take candidate is stale")
                if not successor_logical_operation or not successor_logical_operation.strip():
                    raise ValidationDomainError("retake requires new logical operation")
                key = (candidate.run_id, successor_logical_operation)
                if key in uow.video_operations:
                    raise ValidationDomainError("retake logical operation already exists")
                operation = next(
                    (
                        item
                        for item in uow.video_operations.values()
                        if item.run_id == candidate.run_id
                        and item.logical_operation == candidate.logical_operation
                    ),
                    None,
                )
                if operation is None:
                    raise ValidationDomainError("video operation not found")
                operation = cast(VideoOperation, operation)
                candidate = replace(candidate, status="stale", revision=candidate.revision + 1)
                uow.video_take_candidates[candidate.id] = candidate
                uow.video_operations[key] = VideoOperation(
                    project_id=operation.project_id,
                    run_id=operation.run_id,
                    logical_operation=successor_logical_operation,
                    provider_id=operation.provider_id,
                    profile_id=operation.profile_id,
                    model_id=operation.model_id,
                    capability_snapshot_id=operation.capability_snapshot_id,
                    source_asset_version_id=operation.source_asset_version_id,
                    source_asset_version_revision=operation.source_asset_version_revision,
                    source_asset_version_hash=operation.source_asset_version_hash,
                    shot_spec_id=operation.shot_spec_id,
                    shot_spec_revision=operation.shot_spec_revision,
                    shot_spec_hash=operation.shot_spec_hash,
                    duration_seconds=operation.duration_seconds,
                    aspect_ratio=operation.aspect_ratio,
                    episode_id=operation.episode_id,
                    target_id=operation.target_id,
                    asset_id=operation.asset_id,
                )
                await uow.commit()
                return candidate
            updated = candidate.decide(action, expected_revision)
            if action == "accept":
                from video_agent_api.application.scenes import _accept_current_media

                shot = uow.shots.get(candidate.target_id)
                if shot is None:
                    raise ValidationDomainError("video candidate shot not found")
                shot_revision = shot.revision
                if expected_shot_revision is not None and expected_shot_revision != shot_revision:
                    raise ValidationDomainError("video candidate shot is stale")

                await _accept_current_media(
                    uow,
                    project_id=candidate.project_id,
                    episode_id=candidate.episode_id,
                    shot_id=candidate.target_id,
                    candidate={
                        "candidateId": candidate.id,
                        "candidateRevision": candidate.revision,
                        "projectId": candidate.project_id,
                        "episodeId": candidate.episode_id,
                        "targetId": candidate.target_id,
                        "assetVersionId": candidate.asset_version_id,
                        "assetVersionRevision": candidate.asset_version_revision,
                        "assetVersionHash": candidate.asset_version_hash,
                        "provenance": "agnes_video",
                        "mediaKind": "video",
                        "shotSpecRevision": candidate.shot_spec_revision,
                        "shotSpecHash": candidate.shot_spec_hash,
                        "durationMs": round(candidate.duration_seconds * 1000),
                        "aspectRatio": candidate.aspect_ratio,
                    },
                    expected_shot_revision=shot_revision,
                )
            uow.video_take_candidates[candidate.id] = updated
            await uow.commit()
            return updated
