"""Image/video only AssetEdit session, plan, candidate and review decisions."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError


@dataclass(frozen=True, slots=True)
class AssetVersionRef:
    id: str
    revision: int
    content_hash: str
    kind: Literal["image", "video"]
    project_id: str = ""
    mime_type: str = ""

    def __post_init__(self) -> None:
        if self.kind not in {"image", "video"} or self.revision < 0 or len(self.content_hash) != 64:
            raise ValidationDomainError("asset edit reference is invalid")
        if bool(self.project_id) != bool(self.mime_type) or (
            self.mime_type and "/" not in self.mime_type
        ):
            raise ValidationDomainError("asset edit reference owner metadata is invalid")


@dataclass(frozen=True, slots=True)
class ContinuitySnapshotRef:
    id: str
    revision: int
    content_hash: str
    target_id: str

    def __post_init__(self) -> None:
        if not self.id or self.revision < 1 or len(self.content_hash) != 64 or not self.target_id:
            raise ValidationDomainError("continuity snapshot reference is invalid")


@dataclass(frozen=True, slots=True)
class AssetEditSelection:
    project_id: str
    episode_id: str
    target_id: str
    primary: AssetVersionRef
    references: tuple[AssetVersionRef, ...] = ()


@dataclass(slots=True)
class AssetEditSession:
    project_id: str
    episode_id: str
    selection: AssetEditSelection
    continuity: ContinuitySnapshotRef
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    status: Literal["active", "closed"] = "active"

    def switch_selection(self, selection: AssetEditSelection, expected_revision: int) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        if selection.project_id != self.project_id or selection.episode_id != self.episode_id:
            raise ValidationDomainError("asset edit selection scope is invalid")
        self.selection = selection
        self.revision += 1


@dataclass(slots=True)
class AssetEditPlan:
    project_id: str
    episode_id: str
    base: AssetVersionRef
    references: tuple[AssetVersionRef, ...]
    instruction: str
    turn_id: str
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    continuity: ContinuitySnapshotRef | None = None
    target_id: str = ""
    session_id: str = ""
    run_id: str = ""
    node_run_id: str = ""
    logical_operation: str = ""
    correlation_id: str = ""
    schema_version: str = "1.0.0"
    status: Literal["pending_review", "stale", "executing"] = "pending_review"

    def __post_init__(self) -> None:
        if not self.instruction.strip() or any(
            reference.kind not in {"image", "video"} for reference in self.references
        ):
            raise ValidationDomainError("asset edit plan is invalid")
        if self.base.kind not in {"image", "video"}:
            raise ValidationDomainError("asset edit base must be image or video")
        if any(reference.id == self.base.id for reference in self.references):
            raise ValidationDomainError("references must not duplicate the base asset")
        if len({reference.id for reference in self.references}) != len(self.references):
            raise ValidationDomainError("asset edit references must be unique")
        if self.schema_version != "1.0.0":
            raise ValidationDomainError("unsupported schemaVersion")
        if self.continuity is not None and self.continuity.target_id != self.target_id:
            raise ValidationDomainError("asset edit continuity target mismatch")
        operation_identity = (
            self.session_id,
            self.run_id,
            self.node_run_id,
            self.logical_operation,
            self.correlation_id,
        )
        if any(operation_identity) and not all(operation_identity):
            raise ValidationDomainError("turn-bound plan operation identity is incomplete")


@dataclass(slots=True)
class AssetEditCandidate:
    plan_id: str
    asset_version: AssetVersionRef
    status: Literal[
        "generated", "pending_review", "accepted", "rejected", "stale", "superseded"
    ] = "pending_review"
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    provenance: dict[str, object] = field(default_factory=dict)
    project_id: str = ""
    episode_id: str = ""
    target_id: str = ""

    def decide(self, action: str, expected_revision: int) -> None:
        if action not in {"accept", "reject", "retake"}:
            raise ValidationDomainError("review decision must be accept, reject or retake")
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        if self.status != "pending_review":
            raise ValidationDomainError("candidate is terminal")
        self.status = {
            "accept": "accepted",
            "reject": "rejected",
            "retake": "superseded",
        }[action]  # type: ignore[assignment]
        self.revision += 1


@dataclass(slots=True)
class AssetEditExecution:
    plan_id: str
    plan_revision: int
    run_id: str
    node_run_id: str
    logical_operation: str
    correlation_id: str
    request_fingerprint: str
    status: Literal[
        "queued",
        "running",
        "waiting_reconciliation",
        "succeeded",
        "failed",
        "submission_unknown",
        "cancel_requested",
        "cancelled",
    ] = "queued"
    provider_request_id: str | None = None
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))

    def transition(self, target: str) -> None:
        if target not in {
            "queued",
            "running",
            "waiting_reconciliation",
            "succeeded",
            "failed",
            "submission_unknown",
            "cancel_requested",
            "cancelled",
        }:
            raise ValidationDomainError("asset edit execution state is invalid")
        transitions: dict[str, set[str]] = {
            "queued": {"running", "cancel_requested", "failed", "submission_unknown"},
            "running": {
                "waiting_reconciliation",
                "succeeded",
                "failed",
                "submission_unknown",
                "cancel_requested",
            },
            "waiting_reconciliation": {"running", "succeeded", "failed", "submission_unknown"},
            "submission_unknown": {"running", "succeeded", "failed", "cancel_requested"},
            "cancel_requested": {"cancelled", "submission_unknown", "succeeded"},
            "succeeded": set(),
            "failed": set(),
            "cancelled": set(),
        }
        if target == self.status:
            return
        if target not in transitions[self.status]:
            raise ValidationDomainError("asset edit execution transition is invalid")
        self.status = target  # type: ignore[assignment]
        self.revision += 1


@dataclass(frozen=True, slots=True)
class AcceptDecision:
    candidate_id: str
    action: Literal["accept", "reject", "retake"]
    expected_revision: int
    scope: tuple[str, ...]
    id: str = field(default_factory=lambda: str(uuid4()))
    retention_policy: str = "long-term-audit"
    retention_version: str = "1"
    hold: bool = False


@dataclass(frozen=True, slots=True)
class EditImpact:
    plan_id: str
    status: Literal["clear", "stale", "continuity_stale"]
    reasons: tuple[str, ...] = ()
    id: str = field(default_factory=lambda: str(uuid4()))


def reject_unsupported_asset_edit(kind: str, payload: dict[str, object]) -> None:
    if kind not in {"image", "video"} or any(
        key in payload for key in {"mask", "selection", "timeRange", "keyframes"}
    ):
        raise ValidationDomainError("unsupported_feature")
