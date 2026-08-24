"""结构化文本生成与一次人工审核的 owner facts。"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from typing import Literal, cast
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError

TextKind = Literal[
    "story_spec",
    "script_spec",
    "episode",
    "scene",
    "shot",
    "shot_spec",
    "asset_bible_spec",
]
TEXT_KIND_ALLOWLIST = {
    "story_spec",
    "script_spec",
    "episode",
    "scene",
    "shot",
    "shot_spec",
    "asset_bible_spec",
}
CANONICAL_TEXT_KINDS = (
    "story_spec",
    "script_spec",
    "episode",
    "scene",
    "shot",
    "shot_spec",
    "asset_bible_spec",
)


def _hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, default=str, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


@dataclass(frozen=True, slots=True)
class StructuredTextCandidate:
    project_id: str
    kind: TextKind
    scope_id: str
    payload: dict[str, object]
    source_candidate_ids: tuple[str, ...] = ()
    source_hashes: tuple[str, ...] = ()
    revision: int = 1
    schema_version: str = "1.0.0"
    id: str = field(default_factory=lambda: str(uuid4()))
    payload_hash: str = ""
    run_id: str = ""
    status: Literal["provisional", "accepted", "rejected", "stale"] = "provisional"
    supersedes_id: str | None = None

    def __post_init__(self) -> None:
        if self.kind not in TEXT_KIND_ALLOWLIST:
            raise ValidationDomainError("structured text kind is invalid")
        if not self.project_id or not self.scope_id or not self.payload:
            raise ValidationDomainError("structured text scope and payload are required")
        if not self.payload_hash:
            object.__setattr__(self, "payload_hash", _hash(self.payload))
        if len(self.source_candidate_ids) != len(self.source_hashes):
            raise ValidationDomainError("candidate source IDs/hashes are not aligned")
        if self.payload.get("kind") != self.kind:
            raise ValidationDomainError("structured text payload kind is invalid")
        if self.payload.get("schema_version") != self.schema_version:
            raise ValidationDomainError("structured text schema version is invalid")
        if self.payload.get("scopeId") != self.scope_id:
            raise ValidationDomainError("structured text payload scope is invalid")


@dataclass(frozen=True, slots=True)
class TextReviewBatch:
    project_id: str
    run_id: str
    brief_revision: int
    candidates: tuple[StructuredTextCandidate, ...]
    input_snapshot: dict[str, object]
    status: Literal["pending_review", "accepted", "rejected", "stale"] = "pending_review"
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    fingerprint: str = ""
    schema_version: str = "1.0.0"
    supersedes_batch_id: str | None = None

    def __post_init__(self) -> None:
        if not self.candidates:
            raise ValidationDomainError("text review batch must contain candidates")
        brief = self.input_snapshot.get("creativeBrief")
        if not isinstance(brief, dict):
            raise ValidationDomainError("text review batch input snapshot is required")
        episode_count = brief.get("episodeCount")
        scenes_per_episode = brief.get("scenesPerEpisode")
        shots_per_scene = brief.get("shotsPerScene")
        if any(
            isinstance(value, bool) or not isinstance(value, int) or value < 1
            for value in (episode_count, scenes_per_episode, shots_per_scene)
        ):
            raise ValidationDomainError("text review batch exact counts are invalid")
        episode_count = cast(int, episode_count)
        scenes_per_episode = cast(int, scenes_per_episode)
        shots_per_scene = cast(int, shots_per_scene)
        expected_counts = {
            "story_spec": 1,
            "script_spec": episode_count,
            "episode": episode_count,
            "scene": episode_count * scenes_per_episode,
            "shot": episode_count * scenes_per_episode * shots_per_scene,
            "shot_spec": episode_count * scenes_per_episode * shots_per_scene,
            "asset_bible_spec": 6,
        }
        actual_counts = {
            kind: sum(candidate.kind == kind for candidate in self.candidates)
            for kind in CANONICAL_TEXT_KINDS
        }
        if actual_counts != expected_counts:
            raise ValidationDomainError("text review batch candidate graph is incomplete")
        ids = {candidate.id for candidate in self.candidates}
        if len(ids) != len(self.candidates):
            raise ValidationDomainError("text review batch candidates must be unique")
        by_id = {candidate.id: candidate for candidate in self.candidates}
        for candidate in self.candidates:
            if candidate.project_id != self.project_id or (
                candidate.run_id and candidate.run_id != self.run_id
            ):
                raise ValidationDomainError("text candidate is foreign to batch")
            for source_id, source_hash in zip(
                candidate.source_candidate_ids, candidate.source_hashes, strict=True
            ):
                source = by_id.get(source_id)
                if source is None or source.payload_hash != source_hash:
                    raise ValidationDomainError("text candidate source closure is stale or partial")
        if not self.fingerprint:
            object.__setattr__(
                self,
                "fingerprint",
                _hash(
                    {
                        "projectId": self.project_id,
                        "runId": self.run_id,
                        "briefRevision": self.brief_revision,
                        "inputSnapshot": self.input_snapshot,
                        "candidates": [candidate.id for candidate in self.candidates],
                    }
                ),
            )

    def decide(self, expected_revision: int, action: str) -> TextReviewBatch:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        if self.status != "pending_review" or action not in {"accept", "reject"}:
            raise ValidationDomainError("text review batch is terminal or action is invalid")
        return TextReviewBatch(
            project_id=self.project_id,
            run_id=self.run_id,
            brief_revision=self.brief_revision,
            candidates=self.candidates,
            input_snapshot=self.input_snapshot,
            status="accepted" if action == "accept" else "rejected",
            id=self.id,
            revision=self.revision + 1,
            fingerprint=self.fingerprint,
            schema_version=self.schema_version,
            supersedes_batch_id=self.supersedes_batch_id,
        )


@dataclass(frozen=True, slots=True)
class TextOwnerHandoff:
    batch_id: str
    batch_revision: int
    project_id: str
    run_id: str
    candidate_refs: tuple[dict[str, object], ...]
    payload_hash: str
    correlation_id: str
    required_owners: tuple[str, ...]
    id: str = field(default_factory=lambda: str(uuid4()))


@dataclass(frozen=True, slots=True)
class TextOwnerHandoffAck:
    handoff_id: str
    owner: str
    owner_revision: int
    fingerprint: str
    correlation_id: str
    id: str = field(default_factory=lambda: str(uuid4()))
