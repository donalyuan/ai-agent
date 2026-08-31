"""Agnes submit preflight and deterministic mock video lifecycle."""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass, replace
from hashlib import sha256
from typing import Any, cast

from video_agent_api.application.assets import AppendAssetVersionCommand, AssetsService
from video_agent_api.application.catalog import CatalogService, RecordProviderCallCommand
from video_agent_api.application.runtime_composition import CatalogRuntimeComposition
from video_agent_api.application.storage_handoffs import upload_verified_bytes
from video_agent_api.domain.assets import StorageObject
from video_agent_api.domain.errors import ProjectAccessForbiddenError, ValidationDomainError
from video_agent_api.domain.provider_ops import derive_outbound_correlation
from video_agent_api.domain.video_generation import VideoOperation, VideoTakeCandidate
from video_agent_api.ports.contracts import (
    ModelSelection,
    PortResult,
    StoragePort,
    VideoGenerationPort,
)
from video_agent_api.providers.agnes import AgnesVideoProvider
from video_agent_api.resilience import (
    FrozenOperationAdmission,
    OperationsResilienceCoordinator,
    admission_from_refs,
    admission_refs,
)


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


def _command_payload(command: SubmitVideoCommand) -> dict[str, object]:
    return asdict(command)


def command_from_payload(payload: object) -> SubmitVideoCommand:
    if not isinstance(payload, dict):
        raise ValidationDomainError("video generation outbox command is invalid")
    try:
        return SubmitVideoCommand(**payload)
    except (TypeError, ValueError) as error:
        raise ValidationDomainError("video generation outbox command is invalid") from error


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
        *,
        resilience: OperationsResilienceCoordinator | None = None,
        live_composition: CatalogRuntimeComposition | None = None,
        media_owner: Any | None = None,
    ) -> None:
        self._uow_factory = uow_factory
        self._provider = provider
        self._catalog = catalog
        self._storage = storage
        self._assets = assets
        self._resilience = resilience
        self._live_composition = live_composition
        self._media_owner = media_owner

    async def _resolve_provider(
        self,
        *,
        project_id: str,
        profile_id: str,
        model_id: str,
        capability_snapshot_id: str | None,
        capability_revision: int | None = None,
        profile_revision: int | None = None,
        parameters: dict[str, object] | None = None,
    ) -> tuple[VideoGenerationPort, ModelSelection]:
        if self._live_composition is None:
            return self._provider, ModelSelection(
                "", profile_id, model_id, "catalog", parameters or {}
            )
        composed = await self._live_composition.resolve_provider(
            profile_id,
            model_id,
            "video.submit",
            project_id=project_id,
            expected_profile_revision=profile_revision,
            expected_capability_snapshot_id=capability_snapshot_id,
            expected_capability_revision=capability_revision,
        )
        if not isinstance(composed.port, AgnesVideoProvider):
            raise ValidationDomainError("video live provider adapter is incompatible")
        return composed.port, composed.identity.selection(parameters)

    async def _resolve_operation_provider(self, operation: VideoOperation) -> VideoGenerationPort:
        """Rebuild a live adapter from the operation's frozen Run selection.

        Poll/reconcile happen after the submit activity may have restarted.  The
        process-global provider is intentionally unusable for live traffic, so
        the persisted selection snapshot is the only authority for composing the
        adapter.  Expected revisions make catalog drift fail closed instead of
        silently polling a different account or capability.
        """
        if self._live_composition is None:
            return self._provider
        async with self._uow_factory() as uow:
            run = uow.workflow_runs.get(operation.run_id)
            selection = run.selection_snapshot if run is not None else None
        if not isinstance(selection, dict):
            raise ValidationDomainError("video frozen selection unavailable")
        if (
            selection.get("providerId") != operation.provider_id
            or selection.get("profileId") != operation.profile_id
            or selection.get("modelId") != operation.model_id
        ):
            raise ValidationDomainError("video frozen selection mismatch")
        raw_profile_revision = selection.get("profileRevision")
        snapshots = selection.get("capabilitySnapshots")
        capability = snapshots.get("video.submit") if isinstance(snapshots, dict) else None
        raw_capability_revision = (
            capability.get("revision") if isinstance(capability, dict) else None
        )
        if (
            isinstance(raw_profile_revision, bool)
            or not isinstance(raw_profile_revision, int)
            or isinstance(raw_capability_revision, bool)
            or not isinstance(raw_capability_revision, int)
            or not isinstance(capability, dict)
            or capability.get("id") != operation.capability_snapshot_id
        ):
            raise ValidationDomainError("video frozen capability unavailable")
        provider, _selection = await self._resolve_provider(
            project_id=operation.project_id,
            profile_id=operation.profile_id,
            model_id=operation.model_id,
            capability_snapshot_id=operation.capability_snapshot_id,
            capability_revision=raw_capability_revision,
            profile_revision=raw_profile_revision,
        )
        return provider

    async def submit(
        self,
        command: SubmitVideoCommand,
        *,
        execute_provider: bool = True,
        frozen_admission: FrozenOperationAdmission | None = None,
    ) -> VideoOperation:
        if command.duration_seconds <= 0 or not command.aspect_ratio:
            raise ValidationDomainError("video_duration_or_aspect_missing")
        if (
            command.source_schema_version is not None
            and command.schema_version is not None
            and command.source_schema_version != command.schema_version
        ):
            raise ValidationDomainError("schema_version_conflict")
        request_fingerprint = video_request_fingerprint(command)
        # Retries must consult the durable owner ledger before mutable catalog/provider gates.
        async with self._uow_factory() as uow:
            existing = uow.video_operations.get((command.run_id, command.logical_operation))
            if existing is not None:
                call_id = uow.provider_call_keys.get((command.run_id, command.logical_operation))
                call = uow.provider_calls.get(call_id) if call_id is not None else None
                if call is None or call.request_fingerprint != request_fingerprint:
                    raise ValidationDomainError("agnes_video_operation_fingerprint_conflict")
                if frozen_admission is not None and self._resilience is not None:
                    admission = self._resilience.revalidate(frozen_admission)
                    if not admission.allowed:
                        raise ValidationDomainError(
                            admission.diagnostic or "video_resource_admission_blocked"
                        )
                return cast(VideoOperation, existing)
        frozen_profile_revision: int | None = None
        node_run_id: str
        adapter_key: str
        async with self._uow_factory() as uow:
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
            raw_profile_revision = frozen.get("profileRevision")
            if isinstance(raw_profile_revision, bool) or not isinstance(raw_profile_revision, int):
                raise ValidationDomainError("agnes_video_frozen_profile_revision_invalid")
            frozen_profile_revision = raw_profile_revision
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
            # The durable ProviderCall performs the authoritative reservation below,
            # but a rejected policy must not leave a VideoOperation intent behind.
            self._preflight_policy(uow, profile, "video.submit")
            admission_payload: dict[str, object] | None = None
            if self._resilience is not None:
                if frozen_admission is not None:
                    try:
                        admission = frozen_admission
                        if (
                            admission.scope != command.project_id
                            or admission.operation != "video.submit"
                            or admission.operation_key
                            != f"{command.run_id}:{command.logical_operation}"
                        ):
                            raise ValidationDomainError("video_resource_admission_mismatch")
                        admission = self._resilience.revalidate(admission)
                    except (TypeError, AttributeError) as error:
                        raise ValidationDomainError("video_resource_admission_invalid") from error
                else:
                    admission = self._resilience.freeze(
                        command.project_id,
                        "video.submit",
                        f"{command.run_id}:{command.logical_operation}",
                    )
                if not admission.allowed:
                    raise ValidationDomainError(
                        admission.diagnostic or "video_resource_admission_blocked"
                    )
                admission_payload = admission_refs(admission)
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
                outbound_correlation=derive_outbound_correlation(
                    command.project_id,
                    command.run_id,
                    command.logical_operation,
                    "video.submit",
                    request_fingerprint,
                ),
                admission_refs=admission_payload,
            )
        port, selection = await self._resolve_provider(
            project_id=command.project_id,
            profile_id=command.profile_id,
            model_id=command.model_id,
            capability_snapshot_id=command.capability_snapshot_id,
            capability_revision=command.capability_revision,
            profile_revision=frozen_profile_revision,
            parameters=command.parameters,
        )
        if isinstance(port, AgnesVideoProvider):
            port.validate_capability("submit", command.parameters)
        selection = ModelSelection(
            command.provider_id,
            selection.profile_id,
            selection.model_id,
            adapter_key if self._live_composition is None else selection.adapter_key,
            selection.default_parameters,
        )
        # ProviderOperationPolicy admission is authoritative and must happen
        # before the VideoOperation intent is persisted.  A rejected policy must
        # therefore leave neither owner fact behind.
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
                admission_refs=admission_payload,
            )
        )
        async with self._uow_factory() as uow:
            existing = uow.video_operations.get((command.run_id, command.logical_operation))
            if existing is not None:
                return cast(VideoOperation, existing)
            uow.video_operations[(command.run_id, command.logical_operation)] = operation
            await uow.commit()
        if not execute_provider:
            return operation
        provider_call, acquired = await self._catalog.claim_provider_call(
            command.run_id,
            command.logical_operation,
            expected_operation="video.submit",
        )
        if not acquired:
            # A prior ambiguous claim can only be reconciled through the existing
            # VideoOperation owner; submitting again could duplicate a paid request.
            raise ValidationDomainError("video provider operation requires reconciliation")
        if not provider_call.outbound_correlation:
            raise ValidationDomainError("video provider correlation is unavailable")
        try:
            result: PortResult = port.submit_video(
                command.prompt, selection, provider_call.outbound_correlation
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

    async def enqueue(
        self, command: SubmitVideoCommand, *, project_scope: str | None = None
    ) -> VideoOperation:
        if project_scope is not None and project_scope != command.project_id:
            raise ProjectAccessForbiddenError(project_scope)
        operation = await self.submit(command, execute_provider=False)
        fingerprint = video_request_fingerprint(command)
        async with self._uow_factory() as uow:
            if not any(
                event.get("type") == "video.generation.requested"
                and event.get("runId") == command.run_id
                and event.get("logicalOperation") == command.logical_operation
                for event in uow.outbox_events
            ):
                uow.outbox_events.append(
                    {
                        "type": "video.generation.requested",
                        "eventId": sha256(
                            f"video.generation.requested:{command.run_id}:{command.logical_operation}:{fingerprint}".encode()
                        ).hexdigest(),
                        "status": "pending",
                        "projectId": command.project_id,
                        "runId": command.run_id,
                        "logicalOperation": command.logical_operation,
                        "requestFingerprint": fingerprint,
                        "executionRoute": "generation",
                        "workflowType": "video-generation",
                        "taskQueue": "generation-tasks",
                        "schemaVersion": "1.0.0",
                        # Carry the exact frozen resource/capacity admission from
                        # the owner operation into Temporal; dispatcher must not
                        # recompute mutable runtime state.
                        "resourceAdmission": (
                            dict(operation.admission_refs)
                            if operation.admission_refs is not None
                            else None
                        ),
                        "command": _command_payload(command),
                        "action": "submit",
                    }
                )
                await uow.commit()
        return operation

    async def execute(
        self,
        command: SubmitVideoCommand,
        *,
        frozen_admission: FrozenOperationAdmission | None = None,
    ) -> VideoOperation:
        operation = await self.submit(
            command, execute_provider=False, frozen_admission=frozen_admission
        )
        if operation.status in {"submitted", "running", "succeeded", "failed", "cancelled"}:
            return operation
        provider_call, acquired = await self._catalog.claim_provider_call(
            command.run_id, command.logical_operation, expected_operation="video.submit"
        )
        if not acquired:
            raise ValidationDomainError("video provider operation requires reconciliation")
        if not provider_call.outbound_correlation:
            raise ValidationDomainError("video provider correlation is unavailable")
        # The durable claim is authoritative on retries; only after claiming may
        # mutable catalog/provider state be resolved for the external side effect.
        async with self._uow_factory() as uow:
            run = uow.workflow_runs.get(command.run_id)
            frozen_profile_revision = (
                run.selection_snapshot.get("profileRevision") if run is not None else None
            )
        if isinstance(frozen_profile_revision, bool) or not isinstance(
            frozen_profile_revision, int
        ):
            raise ValidationDomainError("agnes_video_frozen_profile_revision_invalid")
        port, selection = await self._resolve_provider(
            project_id=command.project_id,
            profile_id=command.profile_id,
            model_id=command.model_id,
            capability_snapshot_id=command.capability_snapshot_id,
            capability_revision=command.capability_revision,
            profile_revision=frozen_profile_revision,
            parameters=command.parameters,
        )
        if isinstance(port, AgnesVideoProvider):
            port.validate_capability("submit", command.parameters)
        try:
            result: PortResult = port.submit_video(
                command.prompt, selection, provider_call.outbound_correlation
            )
        except Exception as error:
            await self._catalog.finalize_provider_call(
                command.run_id,
                command.logical_operation,
                status="unknown",
                failure_code=type(error).__name__,
            )
            async with self._uow_factory() as uow:
                current = cast(
                    VideoOperation,
                    uow.video_operations[(command.run_id, command.logical_operation)],
                )
                current.transition("submission_unknown")
                await uow.commit()
            raise
        provider_request_id = result.payload.get("providerRequestId") or result.request_id
        async with self._uow_factory() as uow:
            current = cast(
                VideoOperation,
                uow.video_operations[(command.run_id, command.logical_operation)],
            )
            current.provider_request_id = str(provider_request_id)
            current.transition("submitted")
            await uow.commit()
            return current

    @staticmethod
    def _preflight_policy(uow: Any, profile: Any, operation: str) -> None:
        if profile.adapter_identity == "local_workspace":
            return
        policy = profile.operation_policies.get(operation, {})
        max_concurrency = int(str(policy.get("maxConcurrency", 1)))
        active = sum(
            1
            for value in uow.provider_calls.values()
            if value.profile_id == profile.id
            and value.operation == operation
            and value.status
            in (
                {"pending", "unknown"}
                if profile.adapter_identity != "local_workspace"
                else {"pending"}
            )
            and value.outbound_correlation is not None
        )
        if active >= max_concurrency:
            raise ValidationDomainError("provider_operation_concurrency_exhausted")
        rate_limit = int(str(policy.get("rateLimit", 60)))
        previous_count, _started = profile.request_windows.get(operation, (0, 0.0))
        if previous_count >= rate_limit:
            raise ValidationDomainError("provider_operation_rate_limited")
        quota = profile.quota_snapshots.get(operation)
        if quota is not None and quota.status == "exhausted":
            raise ValidationDomainError("provider_quota_exhausted")

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
        if provider_request_id and operation.outbound_correlation:
            try:
                provider = await self._resolve_operation_provider(operation)
                provider.cancel_video(provider_request_id, operation.outbound_correlation)
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
            correlation = operation.outbound_correlation
            if not correlation:
                operation.transition("submission_unknown")
                await uow.commit()
                return operation
        provider = await self._resolve_operation_provider(operation)
        try:
            result = provider.get_video_status(request_id, correlation)
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
            # Reconciliation starts from the frozen owner ledger/admission.  A
            # stale runtime snapshot blocks the lookup without mutating state.
            if self._resilience is not None and operation.admission_refs is None:
                raise ValidationDomainError("video_resource_admission_missing")
            if self._resilience is not None and operation.admission_refs is not None:
                try:
                    frozen = self._resilience.revalidate(
                        admission_from_refs(operation.admission_refs)
                    )
                except (KeyError, TypeError, ValueError) as error:
                    raise ValidationDomainError("video_resource_admission_invalid") from error
                if not frozen.allowed:
                    raise ValidationDomainError(
                        frozen.diagnostic or "video_resource_admission_blocked"
                    )
            correlation = operation.outbound_correlation
            if not correlation:
                # An ambiguous operation without the frozen external identity
                # cannot be safely looked up or resubmitted after restart.
                return operation
        provider = await self._resolve_operation_provider(operation)
        try:
            result = provider.get_video_status(provider_request_id, correlation)
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
            provider = await self._resolve_operation_provider(operation)
            if not isinstance(provider, AgnesVideoProvider):
                raise ValidationDomainError("video_media_validator_unconfigured")
            _, checksum = provider.validate_video_media(
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

                # The accepted owner fact must be visible to the producer before
                # it appends the same-transaction Media dispatch outbox event.
                uow.video_take_candidates[candidate.id] = updated
                await _accept_current_media(
                    uow,
                    project_id=candidate.project_id,
                    episode_id=candidate.episode_id,
                    shot_id=candidate.target_id,
                    candidate={
                        "candidateId": candidate.id,
                        "candidateRevision": updated.revision,
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
                    media_owner=self._media_owner,
                )
            uow.video_take_candidates[candidate.id] = updated
            await uow.commit()
            return updated
