from __future__ import annotations

import time
from dataclasses import dataclass, replace
from inspect import isawaitable
from typing import Any, cast
from uuid import uuid4

from video_agent_api.domain.catalog import (
    CapabilitySnapshot,
    Model,
    ModelSyncCandidate,
    Provider,
    ProviderProfile,
    SkillAccessAudit,
    SkillRevisionRecord,
    default_skill_revisions,
)
from video_agent_api.domain.errors import (
    CredentialMasterKeyUnavailableError,
    ProjectAccessForbiddenError,
    RevisionConflictError,
    ValidationDomainError,
    WorkflowRunNotFoundError,
)
from video_agent_api.domain.provider_ops import (
    CostConfirmation,
    ProviderCall,
    ProviderQuotaSnapshot,
    derive_outbound_correlation,
)
from video_agent_api.ports.contracts import FrozenRemoteLookup
from video_agent_api.ports.credentials import (
    CredentialKeyring,
    CredentialMasterKeyUnavailable,
    masked_credential_status,
)

_SAFE_USAGE_KEYS = {
    "inputTokens",
    "outputTokens",
    "totalTokens",
    "input_tokens",
    "output_tokens",
    "total_tokens",
    "characters",
    "durationMs",
    "duration_ms",
    "frames",
    "count",
    "unit",
}


def _capability_revision(uow: Any, call: ProviderCall) -> int | None:
    if not call.capability_snapshot_id:
        return None
    profile = uow.profiles.get(call.profile_id)
    if profile is None:
        return None
    for snapshot in getattr(profile, "capability_snapshots", {}).values():
        if getattr(snapshot, "id", None) == call.capability_snapshot_id:
            return getattr(snapshot, "revision", None)
    return None


def _safe_native_usage(value: dict[str, object] | None) -> dict[str, object] | None:
    if value is None:
        return None
    return {
        key: item
        for key, item in value.items()
        if key in _SAFE_USAGE_KEYS and isinstance(item, (str, int, float, bool))
    }


@dataclass(frozen=True, slots=True)
class CreateProviderCommand:
    name: str
    adapter_key: str


@dataclass(frozen=True, slots=True)
class CreateProfileCommand:
    provider_id: str
    name: str
    adapter_identity: str = "local_workspace"


@dataclass(frozen=True, slots=True)
class CreateModelCommand:
    profile_id: str
    model_key: str


@dataclass(frozen=True, slots=True)
class UpdateCatalogCommand:
    entity_id: str
    expected_revision: int
    changes: dict[str, object]


@dataclass(frozen=True, slots=True)
class RecordProviderCallCommand:
    project_id: str
    run_id: str
    node_run_id: str | None
    logical_operation: str
    operation: str
    provider_id: str
    profile_id: str
    model_id: str
    request_fingerprint: str
    capability_snapshot_id: str | None = None
    status: str = "pending"
    cost_status: str = "unknown"
    cost_value: str | None = None
    cost_currency: str | None = None
    cost_source: str | None = None
    provider_request_id: str | None = None
    native_usage: dict[str, object] | None = None
    outbound_correlation: str | None = None
    lookup_outcome: str = "not_attempted"
    admission_refs: dict[str, object] | None = None


@dataclass(frozen=True, slots=True)
class ConfirmCostCommand:
    project_id: str
    run_id: str
    logical_operation: str
    request_fingerprint: str
    user_uuid: str
    threshold_snapshot_id: str | None
    threshold_revision: int | None
    estimated_cost: str | None
    cost_status: str
    operation_kind: str
    batch_size: int


@dataclass(frozen=True, slots=True)
class SetQuotaCommand:
    profile_id: str
    operation: str
    status: str
    remaining: int | None
    reset_at: str | None
    source: str


@dataclass(frozen=True, slots=True)
class ReplaceCredentialCommand:
    profile_id: str
    credential_id: str
    value: str
    expected_revision: int


@dataclass(frozen=True, slots=True)
class ModelSyncCommand:
    profile_id: str
    remote_models: tuple[str, ...]
    expected_revision: int | None = None
    source: str = "explicit_input"


@dataclass(frozen=True, slots=True)
class AppendSkillRevisionCommand:
    name: str
    version: str
    expected_revision: int
    source_identity: str
    digest: str
    source_type: str
    license_status: str
    capabilities: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class AuditSkillAccessCommand:
    skill_revision_id: str
    run_id: str
    node_run_id: str
    access: str
    selected: bool = False


class CatalogService:
    def __init__(self, uow_factory: Any, keyring: CredentialKeyring | None = None) -> None:
        self._uow_factory = uow_factory
        self._keyring = keyring or CredentialKeyring()

    async def resolve_credential(self, credential_ref: str, profile_id: str) -> str:
        """Open a catalog-owned envelope only at the live adapter boundary."""
        async with self._uow_factory() as uow:
            envelope = uow.credential_envelopes.get(profile_id)
            if envelope is None or envelope.credential_id != credential_ref:
                raise ValidationDomainError("live_provider_unconfigured")
            try:
                return self._keyring.open(
                    envelope, profile_id=profile_id, credential_id=credential_ref
                )
            except CredentialMasterKeyUnavailable as error:
                raise CredentialMasterKeyUnavailableError() from error
            except Exception as error:
                raise ValidationDomainError("live_provider_unconfigured") from error

    async def remote_lookup_bindings(self) -> tuple[dict[str, object], ...]:
        """Return only persisted runnable lookup contracts; this performs no I/O."""
        async with self._uow_factory() as uow:
            bindings: list[dict[str, object]] = []
            for profile in uow.profiles.values():
                provider = uow.providers.get(profile.provider_id)
                if (
                    provider is None
                    or not provider.enabled
                    or not profile.enabled
                    or not profile.explicit_live_opt_in
                    or profile.credential_status != "configured"
                ):
                    continue
                for operation, snapshot in profile.capability_snapshots.items():
                    protocol = snapshot.remote_lookup_protocol
                    if (
                        not snapshot.runnable
                        or not protocol
                        or protocol == "unsupported"
                        or not snapshot.id
                        or not snapshot.model_id
                    ):
                        continue
                    bindings.append(
                        {
                            "profileId": profile.id,
                            "modelId": snapshot.model_id,
                            "profileRevision": profile.revision,
                            "capabilitySnapshotId": snapshot.id,
                            "capabilityRevision": snapshot.revision,
                            "operation": operation,
                            "protocol": protocol,
                        }
                    )
            return tuple(bindings)

    async def bootstrap(self) -> None:
        # Persist dependency owners in stages. SQLAlchemy cannot infer flush ordering
        # between these disconnected collection-backed ORM objects on PostgreSQL.
        async with self._uow_factory() as uow:
            if not uow.skills:
                uow.skills.extend(default_skill_revisions())
            provider = next(
                (item for item in uow.providers.values() if item.adapter_key == "mock"),
                None,
            )
            if provider is None:
                provider = Provider("Mock Provider", "mock", "approved", "MVP-A", True, True)
                uow.providers[provider.id] = provider
            await uow.commit()

        async with self._uow_factory() as uow:
            provider = next(item for item in uow.providers.values() if item.adapter_key == "mock")
            profile = next(
                (
                    item
                    for item in uow.profiles.values()
                    if item.provider_id == provider.id
                    and item.adapter_identity == "local_workspace"
                ),
                None,
            )
            if profile is None:
                profile = ProviderProfile(
                    provider.id, "Local test/offline", "local_workspace", True
                )
                uow.profiles[profile.id] = profile
            await uow.commit()

        async with self._uow_factory() as uow:
            provider = next(item for item in uow.providers.values() if item.adapter_key == "mock")
            profile = next(
                item
                for item in uow.profiles.values()
                if item.provider_id == provider.id and item.adapter_identity == "local_workspace"
            )
            model = next(
                (
                    item
                    for item in uow.models.values()
                    if item.profile_id == profile.id and item.enabled
                ),
                None,
            )
            if model is None:
                model = Model(profile.id, "mock-model", enabled=True)
                uow.models[model.id] = model
            await uow.commit()

        async with self._uow_factory() as uow:
            provider = next(item for item in uow.providers.values() if item.adapter_key == "mock")
            profile = next(
                item
                for item in uow.profiles.values()
                if item.provider_id == provider.id and item.adapter_identity == "local_workspace"
            )
            model = next(
                item
                for item in uow.models.values()
                if item.profile_id == profile.id and item.enabled
            )
            for operation in ("text.generate", "image.generate", "video.submit"):
                if operation not in profile.capability_snapshots:
                    profile.capability_snapshots[operation] = CapabilitySnapshot(
                        provider.id,
                        profile.id,
                        operation,
                        profile.revision,
                        True,
                        (operation,),
                        "local",
                        model.id,
                    )
            await uow.commit()

    async def create_provider(self, command: CreateProviderCommand) -> Provider:
        provider = Provider(command.name, command.adapter_key)
        async with self._uow_factory() as uow:
            uow.providers[provider.id] = provider
            await uow.commit()
        return provider

    async def create_profile(self, command: CreateProfileCommand) -> ProviderProfile:
        async with self._uow_factory() as uow:
            if command.provider_id not in uow.providers:
                raise ValidationDomainError("provider not found")
            profile = ProviderProfile(command.provider_id, command.name, command.adapter_identity)
            uow.profiles[profile.id] = profile
            await uow.commit()
            return profile

    async def create_model(self, command: CreateModelCommand) -> Model:
        async with self._uow_factory() as uow:
            if command.profile_id not in uow.profiles:
                raise ValidationDomainError("profile not found")
            model = Model(command.profile_id, command.model_key)
            uow.models[model.id] = model
            await uow.commit()
            return model

    async def update_operation_policy(
        self, profile_id: str, operation: str, expected_revision: int, policy: dict[str, object]
    ) -> ProviderProfile:
        async with self._uow_factory() as uow:
            profile = uow.profiles.get(profile_id)
            if profile is None:
                raise ValidationDomainError("profile not found")
            if expected_revision != profile.revision:
                raise RevisionConflictError(profile.id, expected_revision, profile.revision)
            required = {"maxConcurrency", "rateLimit", "rateWindowSeconds"}
            if set(policy) != required or any(int(str(policy[key])) < 1 for key in required):
                raise ValidationDomainError("operation policy is invalid")
            profile.operation_policies[operation] = dict(policy)
            profile.revision += 1
            await uow.commit()
            return cast(ProviderProfile, profile)

    async def set_quota(self, command: SetQuotaCommand) -> ProviderQuotaSnapshot:
        if command.status not in {"known", "unknown", "exhausted"}:
            raise ValidationDomainError("provider quota status is invalid")
        async with self._uow_factory() as uow:
            profile = uow.profiles.get(command.profile_id)
            if profile is None:
                raise ValidationDomainError("profile not found")
            snapshot = ProviderQuotaSnapshot(
                profile.provider_id,
                profile.id,
                command.operation,
                command.status,  # type: ignore[arg-type]
                command.remaining,
                command.reset_at,
                command.source,
                str(time.time()),
                profile.revision,
            )
            profile.quota_snapshots[command.operation] = snapshot
            await uow.commit()
            return snapshot

    async def admit_operation(
        self, profile_id: str, operation: str, *, live: bool = False, now: float | None = None
    ) -> dict[str, object]:
        async with self._uow_factory() as uow:
            profile = uow.profiles.get(profile_id)
            if profile is None:
                raise ValidationDomainError("profile not found")
            profile.admit(operation, now if now is not None else time.time(), live=live)
            await uow.commit()
            return {
                "status": "admitted",
                "profileId": profile.id,
                "operation": operation,
                "policyRevision": profile.revision,
            }

    async def release_operation(self, profile_id: str, operation: str) -> None:
        async with self._uow_factory() as uow:
            profile = uow.profiles.get(profile_id)
            if profile is None:
                raise ValidationDomainError("profile not found")
            profile.release(operation)
            await uow.commit()

    async def record_provider_call(self, command: RecordProviderCallCommand) -> ProviderCall:
        key = (command.run_id, command.logical_operation)
        outbound_correlation = command.outbound_correlation or derive_outbound_correlation(
            command.project_id,
            command.run_id,
            command.logical_operation,
            command.operation,
            command.request_fingerprint,
        )
        async with self._uow_factory() as uow:
            existing_id = uow.provider_call_keys.get(key)
            if existing_id:
                existing = uow.provider_calls[existing_id]
                expected_binding = (
                    command.project_id,
                    command.run_id,
                    command.node_run_id,
                    command.logical_operation,
                    command.operation,
                    command.provider_id,
                    command.profile_id,
                    command.model_id,
                    command.capability_snapshot_id,
                    command.request_fingerprint,
                    outbound_correlation,
                    command.admission_refs,
                )
                actual_binding = (
                    existing.project_id,
                    existing.run_id,
                    existing.node_run_id,
                    existing.logical_operation,
                    existing.operation,
                    existing.provider_id,
                    existing.profile_id,
                    existing.model_id,
                    existing.capability_snapshot_id,
                    existing.request_fingerprint,
                    existing.outbound_correlation,
                    existing.admission_refs,
                )
                if actual_binding != expected_binding:
                    raise ValidationDomainError("provider operation fingerprint conflict")
                return cast(ProviderCall, existing)
            profile = uow.profiles.get(command.profile_id)
            provider = uow.providers.get(command.provider_id)
            remote_lookup_protocol: str | None = None
            remote_lookup_binding: dict[str, object] | None = None
            if profile is not None and provider is not None:
                if profile.provider_id != provider.id:
                    raise ValidationDomainError("provider call catalog binding unavailable")
                # Reserve policy capacity in the same transaction as the durable
                # attempt. A rejected policy must not leave a ProviderCall behind.
                active = sum(
                    1
                    for value in uow.provider_calls.values()
                    if value.profile_id == profile.id
                    and value.operation == command.operation
                    and value.status
                    in (
                        {"pending", "unknown"}
                        if profile.adapter_identity != "local_workspace"
                        else {"pending"}
                    )
                    and value.outbound_correlation is not None
                )
                if active:
                    profile.active_operations[command.operation] = active
                else:
                    profile.active_operations.pop(command.operation, None)
                if profile.adapter_identity != "local_workspace":
                    profile.admit(command.operation, time.time(), live=True)
                snapshot = profile.capability_snapshots.get(command.operation)
                if snapshot is not None and snapshot.id == command.capability_snapshot_id:
                    remote_lookup_protocol = snapshot.remote_lookup_protocol
                    if snapshot.remote_lookup_protocol:
                        remote_lookup_binding = {
                            "profileId": profile.id,
                            "modelId": snapshot.model_id or command.model_id,
                            "profileRevision": profile.revision,
                            "capabilitySnapshotId": snapshot.id,
                            "capabilityRevision": snapshot.revision,
                            "operation": command.operation,
                            "protocol": snapshot.remote_lookup_protocol,
                        }
            call = ProviderCall(
                project_id=command.project_id,
                run_id=command.run_id,
                node_run_id=command.node_run_id,
                logical_operation=command.logical_operation,
                operation=command.operation,
                provider_id=command.provider_id,
                profile_id=command.profile_id,
                model_id=command.model_id,
                capability_snapshot_id=command.capability_snapshot_id,
                request_fingerprint=command.request_fingerprint,
                status=command.status,  # type: ignore[arg-type]
                cost_status=command.cost_status,  # type: ignore[arg-type]
                cost_value=command.cost_value,
                cost_currency=command.cost_currency,
                cost_source=command.cost_source,
                provider_request_id=command.provider_request_id,
                native_usage=command.native_usage,
                outbound_correlation=outbound_correlation,
                lookup_outcome=command.lookup_outcome,
                remote_lookup_protocol=remote_lookup_protocol,
                remote_lookup_binding=remote_lookup_binding,
                admission_refs=(dict(command.admission_refs) if command.admission_refs else None),
            )
            uow.provider_calls[call.id] = call
            uow.provider_call_keys[key] = call.id
            uow.usage_audits.append(
                {
                    "providerCallId": call.id,
                    "nativeUsage": dict(call.native_usage or {}),
                    "costStatus": call.cost_status,
                    "costValue": call.cost_value,
                    "costCurrency": call.cost_currency,
                    "costSource": call.cost_source,
                }
            )
            await uow.commit()
            return call

    async def begin_text_provider_call(
        self,
        *,
        project_id: str,
        run_id: str,
        node_run_id: str,
        logical_operation: str,
        provider_id: str,
        profile_id: str,
        model_id: str,
        capability_snapshot_id: str | None,
        request_fingerprint: str,
        admission_refs: dict[str, object] | None = None,
    ) -> ProviderCall:
        """Expose a narrow recorder contract without coupling Text to catalog commands."""
        return await self.record_provider_call(
            RecordProviderCallCommand(
                project_id=project_id,
                run_id=run_id,
                node_run_id=node_run_id,
                logical_operation=logical_operation,
                operation="text.generate",
                provider_id=provider_id,
                profile_id=profile_id,
                model_id=model_id,
                capability_snapshot_id=capability_snapshot_id,
                request_fingerprint=request_fingerprint,
                admission_refs=admission_refs,
            )
        )

    async def claim_text_provider_call(
        self, run_id: str, logical_operation: str
    ) -> tuple[ProviderCall, bool]:
        """Keep the text-facing port narrow while sharing the durable claim rule."""
        return await self.claim_provider_call(
            run_id, logical_operation, expected_operation="text.generate"
        )

    async def claim_provider_call(
        self, run_id: str, logical_operation: str, *, expected_operation: str
    ) -> tuple[ProviderCall, bool]:
        """Mark a prepared owner call unknown before an external side effect can occur."""
        async with self._uow_factory() as uow:
            call_id = uow.provider_call_keys.get((run_id, logical_operation))
            if call_id is None:
                raise ValidationDomainError("provider call not found")
            call = uow.provider_calls[call_id]
            if call.operation != expected_operation:
                raise ValidationDomainError("provider call operation mismatch")
            if call.status != "pending":
                return cast(ProviderCall, call), False

            claimed = replace(call, revision=call.revision + 1, status="unknown")
            uow.provider_calls[call_id] = claimed
            await uow.commit()
            return cast(ProviderCall, claimed), True

    async def confirm_cost(self, command: ConfirmCostCommand) -> CostConfirmation:
        key = (command.run_id, command.logical_operation)
        if command.batch_size < 1 or command.cost_status not in {"known", "unknown"}:
            raise ValidationDomainError("cost confirmation is invalid")
        async with self._uow_factory() as uow:
            existing = uow.cost_confirmations.get(key)
            if existing is not None:
                if (
                    existing.request_fingerprint != command.request_fingerprint
                    or existing.user_uuid != command.user_uuid
                ):
                    raise ValidationDomainError("cost confirmation fingerprint conflict")
                return cast(CostConfirmation, existing)
            confirmation = CostConfirmation(
                project_id=command.project_id,
                run_id=command.run_id,
                logical_operation=command.logical_operation,
                request_fingerprint=command.request_fingerprint,
                user_uuid=command.user_uuid,
                threshold_snapshot_id=command.threshold_snapshot_id,
                threshold_revision=command.threshold_revision,
                estimated_cost=command.estimated_cost,
                cost_status=command.cost_status,  # type: ignore[arg-type]
                operation_kind=command.operation_kind,
                batch_size=command.batch_size,
            )
            uow.cost_confirmations[key] = confirmation
            await uow.commit()
            return confirmation

    async def finalize_provider_call(
        self,
        run_id: str,
        logical_operation: str,
        *,
        status: str,
        failure_code: str | None = None,
        provider_request_id: str | None = None,
        native_usage: dict[str, object] | None = None,
        lookup_outcome: str | None = None,
    ) -> ProviderCall:
        if status not in {"pending", "succeeded", "failed", "unknown", "cancelled"}:
            raise ValidationDomainError("provider call status is invalid")
        async with self._uow_factory() as uow:
            call_id = uow.provider_call_keys.get((run_id, logical_operation))
            if call_id is None:
                raise ValidationDomainError("provider call not found")
            call = uow.provider_calls[call_id]
            safe_usage = _safe_native_usage(native_usage)
            next_provider_request_id = provider_request_id or call.provider_request_id
            next_usage = safe_usage if safe_usage is not None else call.native_usage
            next_lookup_outcome = lookup_outcome or call.lookup_outcome
            if not isinstance(next_lookup_outcome, str) or not next_lookup_outcome:
                raise ValidationDomainError("provider lookup outcome is invalid")
            if call.status in {"succeeded", "failed", "cancelled"} and status != call.status:
                raise ValidationDomainError("provider call terminal status conflict")
            if (
                call.status == status
                and call.failure_code == failure_code
                and call.provider_request_id == next_provider_request_id
                and call.native_usage == next_usage
                and call.lookup_outcome == next_lookup_outcome
            ):
                return cast(ProviderCall, call)
            updated = replace(
                call,
                revision=call.revision + 1,
                status=status,
                failure_code=failure_code,
                provider_request_id=next_provider_request_id,
                native_usage=next_usage,
                lookup_outcome=next_lookup_outcome,
            )
            uow.provider_calls[call_id] = updated
            if call.status in {"pending", "unknown"} and status in {
                "succeeded",
                "failed",
                "cancelled",
            }:
                profile = uow.profiles.get(call.profile_id)
                if profile is not None:
                    profile.release(call.operation)
            await uow.commit()
            return cast(ProviderCall, updated)

    async def reconcile_provider_call(
        self,
        run_id: str,
        logical_operation: str,
        *,
        lookups: tuple[FrozenRemoteLookup, ...] = (),
    ) -> ProviderCall:
        """Reconcile an ambiguous call without ever submitting it again."""
        unsupported = False
        async with self._uow_factory() as uow:
            call_id = uow.provider_call_keys.get((run_id, logical_operation))
            if call_id is None:
                raise ValidationDomainError("provider call not found")
            call = cast(ProviderCall, uow.provider_calls[call_id])
            if call.status != "unknown":
                return call
            # Recovery is executable only with the complete persisted seven-field
            # binding.  The former field-only path allowed a mutable catalog
            # lookup to authorize a retry after restart, so missing/legacy
            # bindings now remain owner-specific ``unknown``.
            persisted_binding = call.remote_lookup_binding
            required_binding = {
                "profileId",
                "modelId",
                "profileRevision",
                "capabilitySnapshotId",
                "capabilityRevision",
                "operation",
                "protocol",
            }
            binding_complete = (
                isinstance(persisted_binding, dict) and set(persisted_binding) == required_binding
            )
            persisted = cast(dict[str, object], persisted_binding) if binding_complete else None
            persisted_values = persisted or {}
            matches = tuple(
                candidate
                for candidate in lookups
                if binding_complete
                and candidate.profile_id == persisted_values.get("profileId")
                and candidate.model_id == persisted_values.get("modelId")
                and candidate.profile_revision == persisted_values.get("profileRevision")
                and candidate.capability_snapshot_id == persisted_values.get("capabilitySnapshotId")
                and candidate.capability_revision == persisted_values.get("capabilityRevision")
                and candidate.operation == persisted_values.get("operation")
                and candidate.protocol == persisted_values.get("protocol")
            )
            lookup = matches[0] if len(matches) == 1 else None
            if not binding_complete or (
                persisted is not None and persisted.get("protocol") == "unsupported"
            ):
                unsupported = True
            correlation = call.outbound_correlation or call.provider_request_id
            if lookup is None or not correlation:
                unsupported = True
        if unsupported or lookup is None or correlation is None:
            return await self.finalize_provider_call(
                run_id,
                logical_operation,
                status="unknown",
                lookup_outcome="unsupported",
            )
        result = lookup.port.lookup_provider_request(correlation, lookup.protocol)
        if isawaitable(result):
            result = await result
        if result is None:
            return await self.finalize_provider_call(
                run_id,
                logical_operation,
                status="unknown",
                lookup_outcome="not_found",
            )
        request_id = getattr(result, "request_id", None)
        payload = getattr(result, "payload", None)
        usage = payload.get("usage") if isinstance(payload, dict) else None
        return await self.finalize_provider_call(
            run_id,
            logical_operation,
            status="succeeded",
            provider_request_id=request_id if isinstance(request_id, str) else None,
            native_usage=usage if isinstance(usage, dict) else None,
            lookup_outcome="found",
        )

    async def update_provider(self, command: UpdateCatalogCommand) -> Provider:
        async with self._uow_factory() as uow:
            provider = uow.providers.get(command.entity_id)
            if provider is None:
                raise ValidationDomainError("provider not found")
            changes = self._normalize_changes(
                command.changes,
                {
                    "name",
                    "adapter_key",
                    "approval",
                    "feature_gate",
                    "adapter_installed",
                    "enabled",
                },
            )
            if changes.get("enabled") is True:
                self._require_provider_runnable(provider)
            provider.update(command.expected_revision, **changes)
            await uow.commit()
            return cast(Provider, provider)

    async def update_profile(self, command: UpdateCatalogCommand) -> ProviderProfile:
        async with self._uow_factory() as uow:
            profile = uow.profiles.get(command.entity_id)
            if profile is None:
                raise ValidationDomainError("profile not found")
            changes = self._normalize_changes(
                command.changes,
                {
                    "name",
                    "adapter_identity",
                    "enabled",
                    "explicit_live_opt_in",
                    "operation_policies",
                },
            )
            if changes.get("enabled") is True:
                provider = uow.providers.get(profile.provider_id)
                if provider is None or not provider.enabled:
                    raise ValidationDomainError("provider_disabled")
                if profile.adapter_identity != "local_workspace" and (
                    not profile.explicit_live_opt_in or profile.credential_status != "configured"
                ):
                    raise ValidationDomainError("live_provider_unconfigured")
            policy_changes = command.changes.get("operationPolicies")
            if policy_changes is None:
                policy_changes = command.changes.get("operation_policies")
            if policy_changes is not None:
                self._validate_operation_policies(policy_changes)
            profile.update(command.expected_revision, **changes)
            if policy_changes is not None:
                profile.operation_policies = {
                    str(operation): dict(policy)
                    for operation, policy in cast(
                        dict[str, dict[str, object]], policy_changes
                    ).items()
                }
            await uow.commit()
            return cast(ProviderProfile, profile)

    async def update_model(self, command: UpdateCatalogCommand) -> Model:
        async with self._uow_factory() as uow:
            model = uow.models.get(command.entity_id)
            if model is None:
                raise ValidationDomainError("model not found")
            model.update(
                command.expected_revision,
                **self._normalize_changes(command.changes, {"model_key", "enabled"}),
            )
            await uow.commit()
            return cast(Model, model)

    @staticmethod
    def _normalize_changes(changes: dict[str, object], allowed: set[str]) -> dict[str, object]:
        aliases = {
            "adapterKey": "adapter_key",
            "featureGate": "feature_gate",
            "adapterInstalled": "adapter_installed",
            "explicitLiveOptIn": "explicit_live_opt_in",
            "modelKey": "model_key",
            "operationPolicies": "operation_policies",
        }
        normalized = {aliases.get(key, key): value for key, value in changes.items()}
        unknown = set(normalized) - allowed
        if unknown:
            raise ValidationDomainError(f"unknown catalog field: {sorted(unknown)[0]}")
        return normalized

    @staticmethod
    def _validate_operation_policies(value: object) -> None:
        if not isinstance(value, dict):
            raise ValidationDomainError("operation policy is invalid")
        required = {"maxConcurrency", "rateLimit", "rateWindowSeconds"}
        for operation, policy in value.items():
            if not isinstance(operation, str) or not isinstance(policy, dict):
                raise ValidationDomainError("operation policy is invalid")
            if set(policy) != required:
                raise ValidationDomainError("operation policy is invalid")
            try:
                if any(int(str(policy[key])) < 1 for key in required):
                    raise ValueError
            except (TypeError, ValueError) as error:
                raise ValidationDomainError("operation policy is invalid") from error

    @staticmethod
    def _require_provider_runnable(provider: Provider) -> None:
        if (
            provider.approval != "approved"
            or provider.feature_gate != "MVP-A"
            or not provider.adapter_installed
        ):
            raise ValidationDomainError("provider_unconfigured")

    async def set_provider_enabled(
        self, entity_id: str, expected_revision: int, enabled: bool
    ) -> Provider:
        return await self.update_provider(
            UpdateCatalogCommand(entity_id, expected_revision, {"enabled": enabled})
        )

    async def set_profile_enabled(
        self, entity_id: str, expected_revision: int, enabled: bool
    ) -> ProviderProfile:
        return await self.update_profile(
            UpdateCatalogCommand(entity_id, expected_revision, {"enabled": enabled})
        )

    async def set_model_enabled(
        self, entity_id: str, expected_revision: int, enabled: bool
    ) -> Model:
        async with self._uow_factory() as uow:
            model = uow.models.get(entity_id)
            if model is None:
                raise ValidationDomainError("model not found")
            if enabled:
                profile = uow.profiles.get(model.profile_id)
                provider = uow.providers.get(profile.provider_id) if profile else None
                if (
                    profile is None
                    or provider is None
                    or not profile.enabled
                    or not provider.enabled
                ):
                    raise ValidationDomainError("catalog_resource_disabled")
                if not any(snapshot.runnable for snapshot in profile.capability_snapshots.values()):
                    raise ValidationDomainError("capability_snapshot_unavailable")
            model.update(expected_revision, enabled=enabled)
            await uow.commit()
            return cast(Model, model)

    async def set_skill_enabled(
        self, entity_id: str, expected_revision: int, enabled: bool
    ) -> SkillRevisionRecord:
        async with self._uow_factory() as uow:
            current = next((item for item in uow.skills if item.id == entity_id), None)
            if current is None:
                raise ValidationDomainError("skill revision not found")
            if current.revision != expected_revision:
                raise RevisionConflictError(current.id, expected_revision, current.revision)
            if enabled and (
                current.approval != "approved" or current.provenance != "verified_snapshot"
            ):
                raise ValidationDomainError("skill_not_approved")
            updated = replace(
                current,
                id=str(uuid4()),
                revision=current.revision + 1,
                enabled=enabled,
            )
            # SkillRevision rows are immutable; a lifecycle toggle is a new owner revision.
            uow.skills.append(updated)
            await uow.commit()
            return cast(SkillRevisionRecord, updated)

    async def disable_model(self, entity_id: str, expected_revision: int) -> Model:
        async with self._uow_factory() as uow:
            model = uow.models.get(entity_id)
            if model is None:
                raise ValidationDomainError("model not found")
            model.disable_or_delete(expected_revision)
            await uow.commit()
            return cast(Model, model)

    async def delete_model(self, entity_id: str, expected_revision: int) -> None:
        """仅在可证明没有任何历史引用时物理删除，否则只能停用。"""

        def contains_model_reference(value: object, model_id: str) -> bool:
            if value == model_id:
                return True
            if isinstance(value, dict):
                return any(contains_model_reference(item, model_id) for item in value.values())
            if isinstance(value, (list, tuple, set, frozenset)):
                return any(contains_model_reference(item, model_id) for item in value)
            return False

        async with self._uow_factory() as uow:
            model = uow.models.get(entity_id)
            if model is None:
                raise ValidationDomainError("model not found")
            if model.revision != expected_revision:
                raise RevisionConflictError(model.id, expected_revision, model.revision)
            if not all(
                hasattr(uow, field)
                for field in (
                    "provider_calls",
                    "profiles",
                    "workflow_runs",
                    "workflow_by_project",
                    "workflow_bindings",
                    "catalog_overrides",
                )
            ):
                raise ValidationDomainError("reference_proof_unavailable")
            profile = uow.profiles.get(model.profile_id)
            referenced = (
                any(
                    getattr(snapshot, "model_id", None) == model.id
                    for item in uow.profiles.values()
                    for snapshot in getattr(item, "capability_snapshots", {}).values()
                )
                or any(
                    getattr(call, "model_id", None) == model.id
                    for call in uow.provider_calls.values()
                )
                or any(
                    contains_model_reference(value, model.id)
                    for value in uow.catalog_overrides.values()
                )
                or any(
                    contains_model_reference(getattr(run, "selection_snapshot", {}), model.id)
                    for run in uow.workflow_runs.values()
                )
                or any(
                    contains_model_reference(getattr(workflow, "definition", {}), model.id)
                    for workflow in uow.workflow_by_project.values()
                )
                or any(
                    contains_model_reference(getattr(binding, "__dict__", binding), model.id)
                    for binding in uow.workflow_bindings.values()
                )
                or profile is None
            )
            if referenced:
                raise ValidationDomainError("model_in_use; disable_model is allowed")
            del uow.models[model.id]
            await uow.commit()

    async def snapshot(
        self,
        profile_id: str,
        operation: str,
        runnable: bool = True,
        expected_revision: int | None = None,
    ) -> CapabilitySnapshot:
        async with self._uow_factory() as uow:
            profile = uow.profiles.get(profile_id)
            if profile is None:
                raise ValidationDomainError("profile not found")
            if expected_revision is not None and profile.revision != expected_revision:
                raise RevisionConflictError(profile.id, expected_revision, profile.revision)
            provider = uow.providers.get(profile.provider_id)
            if (
                provider is None
                or provider.approval != "approved"
                or provider.feature_gate != "MVP-A"
                or not provider.adapter_installed
            ):
                raise ValidationDomainError("provider_unconfigured")
            if profile.adapter_identity != "local_workspace":
                if not profile.explicit_live_opt_in or profile.credential_status != "configured":
                    raise ValidationDomainError("live_provider_unconfigured")
                raise ValidationDomainError("live_provider_transport_unconfigured")
            previous = profile.capability_snapshots.get(operation)
            model_ids = [
                model.id
                for model in uow.models.values()
                if model.profile_id == profile.id and model.enabled
            ]
            snapshot = CapabilitySnapshot(
                profile.provider_id,
                profile.id,
                operation,
                1 if previous is None else previous.revision + 1,
                runnable,
                (operation,),
                "local",
                model_ids[0] if len(model_ids) == 1 else None,
            )
            profile.capability_snapshots[operation] = snapshot
            await uow.commit()
            return snapshot

    async def replace_credential(self, command: ReplaceCredentialCommand) -> dict[str, str]:
        async with self._uow_factory() as uow:
            profile = uow.profiles.get(command.profile_id)
            if profile is None:
                raise ValidationDomainError("profile not found")
            if profile.revision != command.expected_revision:
                raise ValidationDomainError("revision conflict")
            try:
                envelope = self._keyring.seal(
                    command.value,
                    profile_id=profile.id,
                    credential_id=command.credential_id,
                )
            except CredentialMasterKeyUnavailable as error:
                raise CredentialMasterKeyUnavailableError() from error
            uow.credential_envelopes[profile.id] = envelope
            profile.credential_status = "configured"
            profile.revision += 1
            await uow.commit()
            return dict(masked_credential_status(envelope))

    async def credential_status(self, profile_id: str) -> dict[str, str]:
        async with self._uow_factory() as uow:
            if profile_id not in uow.profiles:
                raise ValidationDomainError("profile not found")
            return dict(masked_credential_status(uow.credential_envelopes.get(profile_id)))

    async def rotate_credentials(self, target: CredentialKeyring) -> dict[str, object]:
        async with self._uow_factory() as uow:
            replacements: dict[str, object] = {}
            try:
                for profile_id, envelope in uow.credential_envelopes.items():
                    replacements[profile_id] = self._keyring.reencrypt(
                        envelope,
                        profile_id=profile_id,
                        credential_id=envelope.credential_id,
                        target=target,
                    )
            except (CredentialMasterKeyUnavailable, ValueError) as error:
                raise CredentialMasterKeyUnavailableError("credential rotation failed") from error
            uow.credential_envelopes.update(replacements)
            await uow.commit()
            self._keyring = target
            return {"status": "rotated", "count": len(replacements), "keyVersion": target.version}

    async def preview_model_sync(self, command: ModelSyncCommand) -> ModelSyncCandidate:
        if command.source != "explicit_input":
            raise ValidationDomainError("model sync source must be explicit_input")
        async with self._uow_factory() as uow:
            profile = uow.profiles.get(command.profile_id)
            if profile is None:
                raise ValidationDomainError("profile not found")
            if (
                command.expected_revision is not None
                and profile.revision != command.expected_revision
            ):
                raise RevisionConflictError(profile.id, command.expected_revision, profile.revision)
            if len(command.remote_models) != len(set(command.remote_models)):
                raise ValidationDomainError("model sync candidate contains duplicates")
            local = {
                item.model_key
                for item in uow.models.values()
                if item.profile_id == command.profile_id
            }
            remote = set(command.remote_models)
            candidate = ModelSyncCandidate(
                command.profile_id,
                tuple(sorted(remote)),
                tuple(sorted(remote - local)),
                tuple(sorted(local - remote)),
                (),
            )
            uow.model_sync_candidates[candidate.id] = candidate
            await uow.commit()
            return candidate

    async def decide_model_sync(
        self, candidate_id: str, expected_revision: int, decision: str
    ) -> ModelSyncCandidate:
        if decision not in {"accept", "reject"}:
            raise ValidationDomainError("model sync decision is invalid")
        async with self._uow_factory() as uow:
            candidate = uow.model_sync_candidates.get(candidate_id)
            if (
                candidate is None
                or candidate.revision != expected_revision
                or candidate.status != "pending"
            ):
                raise RevisionConflictError(candidate.id, expected_revision, candidate.revision)
            if decision == "accept":
                for key in candidate.added:
                    model = Model(candidate.profile_id, key)
                    uow.models[model.id] = model
                for model in uow.models.values():
                    if (
                        model.profile_id == candidate.profile_id
                        and model.model_key in candidate.removed
                    ):
                        model.enabled = False
                        model.revision += 1
            updated = replace(
                candidate,
                status="accepted" if decision == "accept" else "rejected",
                revision=candidate.revision + 1,
            )
            uow.model_sync_candidates[candidate.id] = updated
            await uow.commit()
            return cast(ModelSyncCandidate, updated)

    async def append_skill_revision(
        self, command: AppendSkillRevisionCommand
    ) -> SkillRevisionRecord:
        async with self._uow_factory() as uow:
            revisions = [item for item in uow.skills if item.name == command.name]
            current_revision = max((item.revision for item in revisions), default=0)
            if current_revision != command.expected_revision:
                raise RevisionConflictError(
                    command.name, command.expected_revision, current_revision
                )
            if command.source_type == "git" and "commit:" not in command.source_identity:
                raise ValidationDomainError("git skill source requires commit identity")
            if (
                command.source_type == "public_markdown"
                and "archive:" not in command.source_identity
            ):
                raise ValidationDomainError("public markdown source requires archive identity")
            if len(command.digest) != 64:
                raise ValidationDomainError("skill digest is invalid")
            item = SkillRevisionRecord(
                command.name,
                command.version,
                "pending_provenance",
                "not_approved",
                False,
                command.source_identity,
                command.digest,
                revision=current_revision + 1,
                source_type=command.source_type,  # type: ignore[arg-type]
                license_status=command.license_status,
                capabilities=command.capabilities,
            )
            uow.skills.append(item)
            await uow.commit()
            return item

    async def audit_skill_access(self, command: AuditSkillAccessCommand) -> SkillAccessAudit:
        async with self._uow_factory() as uow:
            skill = next(
                (item for item in uow.skills if item.id == command.skill_revision_id), None
            )
            if skill is None:
                raise ValidationDomainError("skill revision not found")
            current = max(
                (item for item in uow.skills if item.name == skill.name),
                key=lambda item: item.revision,
            )
            allowed = command.access == "metadata" or (
                command.access in {"content", "reference"}
                and command.selected
                and skill.id == current.id
                and skill.enabled
                and skill.approval == "approved"
            )
            audit = SkillAccessAudit(
                skill.id,
                command.run_id,
                command.node_run_id,
                command.access,  # type: ignore[arg-type]
                allowed,
                "authorized" if allowed else "skill_access_not_authorized",
            )
            uow.skill_access_audits.append(audit)
            await uow.commit()
        if not allowed:
            raise ValidationDomainError("skill_access_not_authorized")
        return audit

    async def provider_call_summaries(
        self,
        project_id: str,
        run_id: str,
        *,
        node_run_id: str | None = None,
        logical_operation: str | None = None,
        project_scope: str | None = None,
    ) -> list[dict[str, object]]:
        async with self._uow_factory() as uow:
            if project_scope is not None and project_scope != project_id:
                raise ProjectAccessForbiddenError(project_id)
            run = uow.workflow_runs.get(run_id)
            if project_scope is not None:
                if run is None:
                    raise WorkflowRunNotFoundError(run_id)
                if getattr(run, "project_id", None) != project_id:
                    raise ProjectAccessForbiddenError(project_id)
            calls = [
                item
                for item in uow.provider_calls.values()
                if item.project_id == project_id
                and item.run_id == run_id
                and (node_run_id is None or item.node_run_id == node_run_id)
                and (logical_operation is None or item.logical_operation == logical_operation)
            ]
            return [
                {
                    "schemaVersion": "1.0.0",
                    "id": item.id,
                    "revision": item.revision,
                    "runId": item.run_id,
                    "nodeRunId": item.node_run_id,
                    "logicalOperation": item.logical_operation,
                    "operation": item.operation,
                    "providerId": item.provider_id,
                    "providerRevision": getattr(
                        uow.providers.get(item.provider_id), "revision", None
                    ),
                    "profileId": item.profile_id,
                    "profileRevision": getattr(uow.profiles.get(item.profile_id), "revision", None),
                    "modelId": item.model_id,
                    "modelRevision": getattr(uow.models.get(item.model_id), "revision", None),
                    "capabilitySnapshotId": item.capability_snapshot_id,
                    "capabilitySnapshotRevision": _capability_revision(uow, item),
                    "status": item.status,
                    "nativeUsage": _safe_native_usage(item.native_usage),
                    "cost": {
                        "status": item.cost_status,
                        "value": item.cost_value,
                        "currency": item.cost_currency,
                        "source": item.cost_source,
                    },
                    "failure": (
                        {"code": item.failure_code, "message": "provider call failed"}
                        if item.failure_code
                        else None
                    ),
                }
                for item in calls
            ]

    async def cleanup_audit_facts(self) -> dict[str, object]:
        async with self._uow_factory() as uow:
            return {
                "status": "skipped",
                "diagnostic": "catalog_audit_no_gc",
                "capabilitySnapshots": sum(
                    len(profile.capability_snapshots) for profile in uow.profiles.values()
                ),
                "providerCalls": len(uow.provider_calls),
            }

    async def projection(self) -> dict[str, object]:
        async with self._uow_factory() as uow:
            operation_schemas: dict[str, dict[str, object]] = {
                profile.id: {} for profile in uow.profiles.values()
            }
            for model in uow.models.values():
                raw_schema = getattr(model, "parameter_schema", None)
                if not isinstance(raw_schema, dict) or not raw_schema:
                    continue
                profile_id = getattr(model, "profile_id", None)
                if profile_id not in operation_schemas:
                    continue
                if isinstance(raw_schema.get("properties"), dict):
                    profile = uow.profiles[profile_id]
                    operations = set(profile.operation_policies) | set(profile.capability_snapshots)
                    for operation in operations:
                        operation_schemas[profile_id][operation] = dict(raw_schema)
                else:
                    for operation, schema in raw_schema.items():
                        if isinstance(operation, str) and isinstance(schema, dict):
                            operation_schemas[profile_id][operation] = dict(schema)
            return {
                "schema_version": "1.0.0",
                "providers": list(uow.providers.values()),
                "profiles": list(uow.profiles.values()),
                "models": list(uow.models.values()),
                # History stays append-only; settings consumers resolve one current
                # lifecycle row per skill name.
                "skills": list(
                    {
                        item.name: item
                        for item in sorted(uow.skills, key=lambda value: value.revision)
                    }.values()
                ),
                "profile_parameter_schemas": operation_schemas,
                "credentialStatuses": {
                    profile_id: dict(masked_credential_status(envelope))
                    for profile_id, envelope in uow.credential_envelopes.items()
                },
            }
