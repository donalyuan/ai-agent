"""Fixed workflow and Run state machine owned by workflows/runs."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from datetime import UTC, datetime
from typing import Final, Literal
from uuid import uuid4

from .errors import ValidationDomainError

RunStatus = Literal[
    "queued", "running", "waiting_review", "succeeded", "failed", "cancel_requested", "cancelled"
]
NodeRunStatus = Literal[
    "pending",
    "running",
    "waiting_review",
    "succeeded",
    "failed",
    "cancel_requested",
    "cancelled",
    "skipped",
]
RUN_STATUSES: Final[frozenset[str]] = frozenset(
    {"queued", "running", "waiting_review", "succeeded", "failed", "cancel_requested", "cancelled"}
)
NODE_RUN_STATUSES: Final[frozenset[str]] = frozenset(
    {
        "pending",
        "running",
        "waiting_review",
        "succeeded",
        "failed",
        "cancel_requested",
        "cancelled",
        "skipped",
    }
)
NODE_TERMINAL: Final[frozenset[str]] = frozenset({"succeeded", "failed", "cancelled", "skipped"})
RUN_TERMINAL: Final[frozenset[str]] = frozenset({"succeeded", "failed", "cancelled"})
RUN_TRANSITIONS: Final[dict[str, frozenset[str]]] = {
    "queued": frozenset({"running", "failed", "cancel_requested"}),
    "running": frozenset({"waiting_review", "succeeded", "failed", "cancel_requested"}),
    "waiting_review": frozenset({"running", "failed", "cancel_requested"}),
    "cancel_requested": frozenset({"cancelled"}),
    "succeeded": frozenset(),
    "failed": frozenset(),
    "cancelled": frozenset(),
}
NODE_RUN_TRANSITIONS: Final[dict[str, frozenset[str]]] = {
    "pending": frozenset({"running", "skipped", "cancel_requested"}),
    "running": frozenset({"waiting_review", "succeeded", "failed", "cancel_requested"}),
    "waiting_review": frozenset({"running", "succeeded", "failed", "cancel_requested"}),
    "cancel_requested": frozenset({"cancelled"}),
    "succeeded": frozenset(),
    "failed": frozenset(),
    "cancelled": frozenset(),
    "skipped": frozenset(),
}


@dataclass(frozen=True, slots=True)
class WorkflowVersion:
    project_id: str
    template_key: str = "drama-mvp-a-default"
    scope_type: str = "project"
    scope_ids: tuple[str, ...] = ()
    definition: dict[str, object] = field(default_factory=dict)
    revision: int = 1
    content_hash: str = ""
    id: str = field(default_factory=lambda: str(uuid4()))
    status: str = "published"
    version_number: int = 1
    schema_version: str = "1.0.0"
    binding_revision: int = 1

    def __post_init__(self) -> None:
        if (
            self.template_key != "drama-mvp-a-default"
            or self.status != "published"
            or self.scope_type not in {"project", "episode", "scene", "shot"}
            or not self.scope_ids
            or len(set(self.scope_ids)) != len(self.scope_ids)
        ):
            raise ValidationDomainError("MVP-A workflow source must be fixed published and scoped")
        if not self.definition:
            raise ValidationDomainError("MVP-A workflow definition must be explicit")
        canonical = hashlib.sha256(
            json.dumps(self.definition, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()
        if self.content_hash and self.content_hash != canonical:
            raise ValidationDomainError("workflow content hash mismatch")
        object.__setattr__(self, "content_hash", canonical)


@dataclass(frozen=True, slots=True)
class ProjectDefaultWorkflowBinding:
    project_id: str
    workflow_version_id: str
    workflow_content_hash: str
    template_key: str = "drama-mvp-a-default"
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    schema_version: str = "1.0.0"
    created_at: str = field(default_factory=lambda: datetime.now(UTC).isoformat())

    def __post_init__(self) -> None:
        if (
            not self.project_id
            or not self.workflow_version_id
            or self.template_key != "drama-mvp-a-default"
            or len(self.workflow_content_hash) != 64
            or self.revision < 1
        ):
            raise ValidationDomainError("project default workflow binding is invalid")


@dataclass(slots=True)
class NodeRun:
    run_id: str
    node_key: str
    status: NodeRunStatus = "pending"
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    logical_operation: str = ""
    scope_refs: tuple[dict[str, object], ...] = ()
    output_evidence: dict[str, object] | None = None
    failure: dict[str, object] | None = None
    submission_state: Literal["not_submitted", "submitted", "submission_unknown", "reconciled"] = (
        "not_submitted"
    )

    def transition(self, target: NodeRunStatus) -> None:
        validate_node_transition(self.status, target)
        self.status = target
        self.revision += 1


@dataclass(slots=True)
class WorkflowRun:
    project_id: str
    workflow_version_id: str
    status: RunStatus = "queued"
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    rerun_of_run_id: str | None = None
    predecessor_run_id: str | None = None
    nodes: list[NodeRun] = field(default_factory=list)
    input_snapshot: dict[str, object] | None = None
    logical_operations: dict[str, str] = field(default_factory=dict)
    selection_snapshot: dict[str, object] = field(default_factory=dict)
    source_snapshot: dict[str, object] = field(default_factory=dict)
    created_at: str = field(default_factory=lambda: datetime.now(UTC).isoformat())
    updated_at: str = field(default_factory=lambda: datetime.now(UTC).isoformat())

    def transition(self, target: RunStatus) -> None:
        if target not in RUN_TRANSITIONS[self.status]:
            raise ValidationDomainError(f"invalid workflow run transition: {self.status}->{target}")
        self.status = target
        self.revision += 1
        self.updated_at = datetime.now(UTC).isoformat()

    def recompute_from_nodes(self) -> None:
        """Aggregate owner status without allowing late results to escape cancellation."""
        if self.status in RUN_TERMINAL or self.status == "cancel_requested":
            return
        active_nodes = [
            node
            for node in self.nodes
            if not (
                node.status == "failed"
                and (node.failure or {}).get("code") == "review_retake"
                and (node.failure or {}).get("supersededByNodeRunId")
            )
        ]
        if any(node.status == "failed" for node in active_nodes):
            target: RunStatus = "failed"
        elif any(node.status == "waiting_review" for node in active_nodes):
            target = "waiting_review"
        elif active_nodes and all(node.status in {"succeeded", "skipped"} for node in active_nodes):
            target = "succeeded"
        else:
            target = "running"
        if target != self.status:
            if target == "succeeded" and self.status == "waiting_review":
                self.transition("running")
            self.transition(target)

    def bind_operation(self, logical_operation: str, fingerprint: str) -> None:
        existing = self.logical_operations.get(logical_operation)
        if existing is not None and existing != fingerprint:
            raise ValidationDomainError("run logical operation fingerprint conflict")
        self.logical_operations[logical_operation] = fingerprint


@dataclass(frozen=True, slots=True)
class RunEvent:
    run_id: str
    sequence: int
    event_type: str
    correlation_id: str
    payload: dict[str, object]
    node_run_id: str | None = None
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    created_at: str = field(default_factory=lambda: datetime.now(UTC).isoformat())
    retention_policy: str = "long-term-audit"
    retention_version: str = "1"
    hold: bool = False

    def __post_init__(self) -> None:
        if self.sequence < 1 or not self.event_type or not self.correlation_id:
            raise ValidationDomainError("run event is invalid")
        forbidden = {"secret", "credential", "prompt", "mediaBytes", "objectKey"}
        if forbidden.intersection(self.payload):
            raise ValidationDomainError("run event payload contains protected data")


@dataclass(frozen=True, slots=True)
class BudgetGate:
    project_id: str
    run_id: str
    node_run_id: str
    logical_operation: str
    request_fingerprint: str
    operation_kind: str
    batch_size: int
    cost_status: Literal["known", "unknown"]
    estimated_cost: str | None
    currency: str | None
    threshold_snapshot_id: str | None
    threshold_revision: int | None
    status: Literal["pending_confirmation", "confirmed"] = "pending_confirmation"
    confirmation_id: str | None = None
    user_uuid: str | None = None
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    retention_policy: str = "diagnostic-30d"
    retention_version: str = "1"
    hold: bool = False

    def __post_init__(self) -> None:
        if self.batch_size < 1 or not self.logical_operation or not self.request_fingerprint:
            raise ValidationDomainError("budget gate is invalid")


@dataclass(frozen=True, slots=True)
class RunInputSnapshot:
    run_id: str
    project_id: str
    workflow_version_id: str
    workflow_content_hash: str
    scope_refs: tuple[dict[str, object], ...]
    owner_refs: tuple[dict[str, object], ...]
    selection_snapshot: dict[str, object]
    source_snapshot: dict[str, object] = field(default_factory=dict)
    node_inputs: tuple[dict[str, object], ...] = ()
    runnable: bool = True
    diagnostic: str | None = None
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    schema_version: str = "1.0.0"
    created_at: str = field(default_factory=lambda: datetime.now(UTC).isoformat())

    def __post_init__(self) -> None:
        if (
            not self.run_id
            or not self.project_id
            or not self.workflow_version_id
            or len(self.workflow_content_hash) != 64
            or self.revision < 1
        ):
            raise ValidationDomainError("run input snapshot is invalid")


@dataclass(frozen=True, slots=True)
class TemporalStart:
    run_id: str
    node_run_id: str
    logical_operation: str
    workflow_id: str
    request_fingerprint: str
    status: Literal["pending", "started", "submission_unknown", "reconciled"] = "pending"
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    schema_version: str = "1.0.0"
    created_at: str = field(default_factory=lambda: datetime.now(UTC).isoformat())

    def __post_init__(self) -> None:
        expected_suffix = f":{self.run_id}:{self.logical_operation}"
        if (
            not self.node_run_id
            or not self.workflow_id.startswith("phase-one:")
            or not self.workflow_id.endswith(expected_suffix)
            or not self.request_fingerprint
        ):
            raise ValidationDomainError("temporal start fact is invalid")


def temporal_workflow_id(
    project_id: str,
    workflow_version_id: str,
    run_id: str,
    logical_operation: str,
) -> str:
    if not all((project_id, workflow_version_id, run_id, logical_operation)):
        raise ValidationDomainError("temporal workflow identity is incomplete")
    return f"phase-one:{project_id}:{workflow_version_id}:{run_id}:{logical_operation}"


def validate_node_transition(current: str, target: str) -> None:
    if target not in NODE_RUN_TRANSITIONS.get(current, frozenset()):
        raise ValidationDomainError(f"invalid node run transition: {current}->{target}")
