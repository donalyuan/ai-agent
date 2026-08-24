"""Provider admission, append-only call summary and cost confirmation facts."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError


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
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    retention_policy: str = "long-term-audit"
    retention_version: str = "1"
    hold: bool = False


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
