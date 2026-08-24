"""异步视频执行与人工审核候选的 framework-free facts。"""

from __future__ import annotations

from dataclasses import dataclass, field, replace
from typing import Literal
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError

VIDEO_STATES = frozenset(
    {"pending", "submitted", "running", "submission_unknown", "succeeded", "failed", "cancelled"}
)


@dataclass(slots=True)
class VideoOperation:
    project_id: str
    run_id: str
    logical_operation: str
    provider_id: str
    profile_id: str
    model_id: str
    capability_snapshot_id: str
    source_asset_version_id: str
    source_asset_version_revision: int
    source_asset_version_hash: str
    shot_spec_id: str
    shot_spec_revision: int
    shot_spec_hash: str
    duration_seconds: float
    aspect_ratio: str
    status: Literal[
        "pending", "submitted", "running", "submission_unknown", "succeeded", "failed", "cancelled"
    ] = "pending"
    provider_request_id: str | None = None
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    cancel_requested: bool = False
    episode_id: str = ""
    target_id: str = ""
    asset_id: str = ""
    observation_fingerprints: tuple[str, ...] = ()
    source_candidate_id: str | None = None
    source_provenance: str | None = None

    def transition(self, target: str) -> None:
        if target not in VIDEO_STATES:
            raise ValidationDomainError("video operation state is invalid")
        order = {
            "pending": 0,
            "submitted": 1,
            "running": 2,
            "submission_unknown": 2,
            "succeeded": 3,
            "failed": 3,
            "cancelled": 3,
        }
        if self.cancel_requested or order[target] < order[self.status]:
            return
        if self.status == "submission_unknown" and target in {"submitted", "running"}:
            return
        self.status = target  # type: ignore[assignment]
        self.revision += 1

    def cancel(self) -> None:
        if self.status in {"succeeded", "failed", "cancelled"}:
            return
        self.cancel_requested = True
        self.status = "cancelled"
        self.revision += 1

    def observe(self, status: str, fingerprint: str, provider_request_id: str | None = None) -> str:
        """Apply poll state monotonically; duplicates remain diagnostic evidence only."""
        if fingerprint in self.observation_fingerprints:
            return self.status
        self.observation_fingerprints = (*self.observation_fingerprints, fingerprint)
        if provider_request_id and self.provider_request_id is None:
            self.provider_request_id = provider_request_id
        order = {
            "pending": 0,
            "submitted": 1,
            "running": 2,
            "submission_unknown": 2,
            "succeeded": 3,
            "failed": 3,
            "cancelled": 3,
        }
        if self.cancel_requested or order.get(self.status, 0) >= 3:
            return self.status
        if status in order and order[status] >= order.get(self.status, 0):
            self.status = status  # type: ignore[assignment]
            self.revision += 1
        return self.status


@dataclass(frozen=True, slots=True)
class VideoTakeCandidate:
    project_id: str
    episode_id: str
    target_id: str
    run_id: str
    logical_operation: str
    source_asset_version_id: str
    source_asset_version_revision: int
    source_asset_version_hash: str
    shot_spec_id: str
    shot_spec_revision: int
    shot_spec_hash: str
    duration_seconds: float
    aspect_ratio: str
    asset_version_id: str
    asset_version_revision: int
    asset_version_hash: str
    provider_request_id: str | None
    status: Literal["pending_review", "accepted", "rejected", "stale"] = "pending_review"
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    source_candidate_id: str | None = None
    source_provenance: str = "agnes_video"

    def decide(self, action: str, expected_revision: int) -> VideoTakeCandidate:
        if self.revision != expected_revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        if action not in {"accept", "reject"}:
            raise ValidationDomainError("video take review action is invalid")
        if self.status != "pending_review":
            raise ValidationDomainError("video take is no longer reviewable")
        return replace(
            self,
            status="accepted" if action == "accept" else "rejected",
            revision=self.revision + 1,
        )
