"""Episode 内 Scene/Shot owner 的纯领域模型。"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from typing import Literal
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError

ReviewDecision = Literal["accept", "reject", "retake"]
SCHEMA_VERSION = "1.0.0"


def _hash(payload: object) -> str:
    return hashlib.sha256(
        json.dumps(payload, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def _hex_hash(value: str, field_name: str) -> str:
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value.lower()):
        raise ValidationDomainError(f"{field_name} must be a 64-character hexadecimal hash")
    return value.lower()


@dataclass(frozen=True, slots=True)
class SpecVersion:
    owner_id: str
    project_id: str
    episode_id: str
    kind: Literal["scene", "shot"]
    payload: dict[str, object]
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    content_hash: str = ""

    def __post_init__(self) -> None:
        if self.kind not in {"scene", "shot"} or not self.payload:
            raise ValidationDomainError("spec version is invalid")
        if not self.content_hash:
            object.__setattr__(
                self,
                "content_hash",
                _hash(self.payload),
            )


@dataclass(frozen=True, slots=True)
class ImmutableOwnerRef:
    id: str
    revision: int
    content_hash: str

    def __post_init__(self) -> None:
        if not self.id or self.revision < 1:
            raise ValidationDomainError("immutable owner reference is invalid")
        _hex_hash(self.content_hash, "immutable owner reference content_hash")


@dataclass(slots=True)
class Shot:
    scene_id: str
    project_id: str
    episode_id: str
    display_number: int
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    schema_version: str = SCHEMA_VERSION
    status: str = "draft"
    spec_ref: ImmutableOwnerRef | None = None
    continuity_snapshot: ImmutableOwnerRef | None = None
    continuity_task_refs: list[ImmutableOwnerRef] = field(default_factory=list)
    current_image: AcceptedMediaEligibility | None = None
    current_video: AcceptedMediaEligibility | None = None
    spec_versions: list[SpecVersion] = field(default_factory=list)

    def __post_init__(self) -> None:
        if (
            not self.scene_id
            or not self.project_id
            or not self.episode_id
            or self.display_number < 1
        ):
            raise ValidationDomainError("shot scope and display_number are required")


@dataclass(slots=True)
class Scene:
    project_id: str
    episode_id: str
    display_number: int
    title: str = "Untitled Scene"
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    schema_version: str = SCHEMA_VERSION
    status: str = "draft"
    spec_ref: ImmutableOwnerRef | None = None
    shots: list[Shot] = field(default_factory=list)
    spec_versions: list[SpecVersion] = field(default_factory=list)

    def __post_init__(self) -> None:
        if (
            not self.project_id
            or not self.episode_id
            or self.display_number < 1
            or not self.title.strip()
        ):
            raise ValidationDomainError("scene scope and display_number are required")

    def reorder_shots(self, ids: list[str], expected_revision: int) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        current = [shot.id for shot in self.shots]
        if len(ids) != len(current) or set(ids) != set(current):
            raise ValidationDomainError("reorder must contain complete same-parent shot set")
        by_id = {shot.id: shot for shot in self.shots}
        self.shots = [by_id[item] for item in ids]
        for index, shot in enumerate(self.shots, 1):
            shot.display_number = index
        self.revision += 1

    def append_spec(self, payload: dict[str, object]) -> SpecVersion:
        spec = SpecVersion(
            self.id,
            self.project_id,
            self.episode_id,
            "scene",
            dict(payload),
            len(self.spec_versions) + 1,
        )
        self.spec_versions.append(spec)
        self.spec_ref = ImmutableOwnerRef(spec.id, spec.revision, spec.content_hash)
        self.revision += 1
        return spec

    def append_shot_spec(self, shot: Shot, payload: dict[str, object]) -> SpecVersion:
        if shot.scene_id != self.id:
            raise ValidationDomainError("shot does not belong to scene")
        spec = SpecVersion(
            shot.id,
            self.project_id,
            self.episode_id,
            "shot",
            dict(payload),
            len(shot.spec_versions) + 1,
        )
        shot.spec_versions.append(spec)
        shot.spec_ref = ImmutableOwnerRef(spec.id, spec.revision, spec.content_hash)
        shot.revision += 1
        return spec


def reorder_scenes(
    scenes: list[Scene], ids: list[str], expected_revision: int, current_revision: int = 1
) -> int:
    if not scenes:
        raise ValidationDomainError("scene parent scope is empty")
    if expected_revision != current_revision:
        raise RevisionConflictError(scenes[0].episode_id, expected_revision, current_revision)
    current = [item.id for item in scenes]
    if len(ids) != len(current) or set(ids) != set(current):
        raise ValidationDomainError("reorder must contain complete same-parent scene set")
    by_id = {item.id: item for item in scenes}
    for index, item_id in enumerate(ids, 1):
        by_id[item_id].display_number = index
        by_id[item_id].revision += 1
    return current_revision + 1


def validate_review_decision(decision: str) -> ReviewDecision:
    if decision not in {"accept", "reject", "retake"}:
        raise ValidationDomainError("review decision must be accept, reject or retake")
    return decision  # type: ignore[return-value]


@dataclass(frozen=True, slots=True)
class NarrativeCandidate:
    candidate_id: str
    source_hash: str
    payload: dict[str, object]

    def __post_init__(self) -> None:
        if not self.candidate_id or not self.payload:
            raise ValidationDomainError("narrative candidate is incomplete")
        _hex_hash(self.source_hash, "source_hash")

    @property
    def payload_hash(self) -> str:
        return _hash(self.payload)


@dataclass(frozen=True, slots=True)
class SceneShotBatchHandoff:
    handoff_id: str
    project_id: str
    episode_id: str
    batch_revision: int
    correlation_id: str
    payload_hash: str
    accepted: bool
    scenes: tuple[dict[str, object], ...]
    schema_version: str = SCHEMA_VERSION

    def __post_init__(self) -> None:
        if not self.accepted or not self.handoff_id or not self.correlation_id or not self.scenes:
            raise ValidationDomainError("accepted complete scene/shot handoff is required")
        if self.batch_revision < 1 or self.schema_version != SCHEMA_VERSION:
            raise ValidationDomainError("scene/shot handoff version is invalid")
        _hex_hash(self.payload_hash, "payload_hash")
        canonical = _hash(list(self.scenes))
        if canonical != self.payload_hash:
            raise ValidationDomainError("scene/shot handoff payload hash mismatch")


@dataclass(frozen=True, slots=True)
class SceneShotOwnerAck:
    handoff_id: str
    project_id: str
    episode_id: str
    scene_ids: tuple[str, ...]
    shot_ids: tuple[str, ...]
    payload_hash: str
    correlation_id: str
    id: str = field(default_factory=lambda: str(uuid4()))


@dataclass(frozen=True, slots=True)
class AcceptedMediaEligibility:
    candidate_id: str
    candidate_revision: int
    project_id: str
    episode_id: str
    target_id: str
    asset_version_id: str
    asset_version_revision: int
    asset_version_hash: str
    provenance: str
    media_kind: Literal["image", "video"]
    shot_spec_revision: int | None = None
    shot_spec_hash: str | None = None
    duration_ms: int | None = None
    aspect_ratio: str | None = None
    derivative_status: str = "pending"
    accepted: bool = True

    def __post_init__(self) -> None:
        for value in (
            self.candidate_id,
            self.project_id,
            self.episode_id,
            self.target_id,
            self.asset_version_id,
            self.provenance,
        ):
            if not value:
                raise ValidationDomainError("media eligibility contains blank owner facts")
        if not self.accepted or self.candidate_revision < 1 or self.asset_version_revision < 0:
            raise ValidationDomainError("media eligibility is not accepted/current capable")
        if self.media_kind not in {"image", "video"}:
            raise ValidationDomainError("media eligibility kind is invalid")
        if self.provenance not in {"text_review", "media_review", "asset_edit"}:
            raise ValidationDomainError("media eligibility provenance is not accepted")
        if self.derivative_status not in {"pending", "ready", "failed", "stale"}:
            raise ValidationDomainError("media eligibility derivative status is invalid")
        _hex_hash(self.asset_version_hash, "asset_version_hash")
        if self.media_kind == "video":
            if (
                self.shot_spec_revision is None
                or self.shot_spec_revision < 1
                or self.shot_spec_hash is None
                or self.duration_ms is None
                or self.duration_ms < 1
                or self.aspect_ratio not in {"9:16", "16:9", "1:1"}
            ):
                raise ValidationDomainError("video eligibility snapshot is incomplete")
            _hex_hash(self.shot_spec_hash, "shot_spec_hash")

    @property
    def timeline_ready(self) -> bool:
        return self.derivative_status == "ready"
