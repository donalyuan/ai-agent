"""Provider admission, append-only call summary and cost confirmation facts."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal, cast
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError


def derive_outbound_correlation(
    project_id: str,
    run_id: str,
    logical_operation: str,
    operation: str,
    request_fingerprint: str,
) -> str:
    """Create the durable, secret-free external correlation from frozen owner facts."""
    from hashlib import sha256

    value = "\x1f".join((project_id, run_id, logical_operation, operation, request_fingerprint))
    return sha256(value.encode()).hexdigest()


@dataclass(slots=True)
class ProviderOperationPolicy:
    operation: str
    max_concurrency: int = 1
    rate_limit: int = 60
    rate_window_seconds: int = 60
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))

    def update(self, expected_revision: int, **changes: int) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        for key in changes:
            if key not in {"max_concurrency", "rate_limit", "rate_window_seconds"}:
                raise ValidationDomainError("unknown operation policy field")
        if any(value < 1 for value in changes.values()):
            raise ValidationDomainError("operation policy values must be positive")
        for key, value in changes.items():
            setattr(self, key, value)
        self.revision += 1


@dataclass(frozen=True, slots=True)
class ProviderQuotaSnapshot:
    provider_id: str
    profile_id: str
    operation: str
    status: Literal["known", "unknown", "exhausted"]
    remaining: int | None
    reset_at: str | None
    source: str
    captured_at: str
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))


@dataclass(frozen=True, slots=True)
class ProviderCall:
    project_id: str
    run_id: str
    node_run_id: str | None
    logical_operation: str
    operation: str
    provider_id: str
    profile_id: str
    model_id: str
    capability_snapshot_id: str | None
    request_fingerprint: str
    status: Literal["pending", "succeeded", "failed", "unknown", "cancelled"]
    cost_status: Literal["known", "unknown"] = "unknown"
    cost_value: str | None = None
    cost_currency: str | None = None
    cost_source: str | None = None
    provider_request_id: str | None = None
    native_usage: dict[str, object] | None = None
    failure_code: str | None = None
    outbound_correlation: str | None = None
    lookup_outcome: str = "not_attempted"
    remote_lookup_protocol: str | None = None
    remote_lookup_binding: dict[str, object] | None = None
    admission_refs: dict[str, object] | None = None
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    retention_policy: str = "long-term-audit"
    retention_version: str = "1"
    hold: bool = False

    def __post_init__(self) -> None:
        if self.status not in {"pending", "succeeded", "failed", "unknown", "cancelled"}:
            raise ValidationDomainError("provider call state is invalid")
        if not self.project_id or not self.run_id or not self.logical_operation:
            raise ValidationDomainError("provider call identity is incomplete")
        if not isinstance(self.lookup_outcome, str) or not self.lookup_outcome:
            raise ValidationDomainError("provider lookup outcome is invalid")
        if self.remote_lookup_protocol is not None and not self.remote_lookup_protocol:
            raise ValidationDomainError("provider lookup protocol is invalid")
        if self.remote_lookup_binding is not None:
            required = {
                "profileId",
                "modelId",
                "profileRevision",
                "capabilitySnapshotId",
                "capabilityRevision",
                "operation",
                "protocol",
            }
            if (
                set(self.remote_lookup_binding) != required
                or any(
                    not isinstance(self.remote_lookup_binding[key], str)
                    or not self.remote_lookup_binding[key]
                    for key in {
                        "profileId",
                        "modelId",
                        "capabilitySnapshotId",
                        "operation",
                        "protocol",
                    }
                )
                or any(
                    isinstance(self.remote_lookup_binding[key], bool)
                    or not isinstance(self.remote_lookup_binding[key], int)
                    or cast(int, self.remote_lookup_binding[key]) < 1
                    for key in {"profileRevision", "capabilityRevision"}
                )
            ):
                raise ValidationDomainError("provider lookup binding is invalid")


@dataclass(frozen=True, slots=True)
class CostConfirmation:
    project_id: str
    run_id: str
    logical_operation: str
    request_fingerprint: str
    user_uuid: str
    threshold_snapshot_id: str | None
    threshold_revision: int | None
    estimated_cost: str | None
    cost_status: Literal["known", "unknown"]
    operation_kind: str
    batch_size: int
    id: str = field(default_factory=lambda: str(uuid4()))
    retention_policy: str = "diagnostic-30d"
    retention_version: str = "1"
    hold: bool = False
