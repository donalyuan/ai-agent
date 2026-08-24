"""Projects owner 的创作配置与文本 handoff 事实。"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from decimal import Decimal, InvalidOperation
from typing import Literal
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError

CreationMode = Literal["original", "adaptation"]
SCHEMA_VERSION = "1.0.0"
_CURRENCIES = {"CNY", "USD", "EUR", "JPY", "GBP"}


def _text(value: object, name: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValidationDomainError(f"{name} must not be blank")
    return value.strip()


def _positive_int(value: object, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        raise ValidationDomainError(f"{name} must be a positive integer")
    return value


def _hash(payload: object) -> str:
    return hashlib.sha256(
        json.dumps(
            payload, default=str, ensure_ascii=True, sort_keys=True, separators=(",", ":")
        ).encode()
    ).hexdigest()


@dataclass(frozen=True, slots=True)
class CreativeBriefVersion:
    creative_brief_id: str
    project_id: str
    subject: str
    genre: str
    audience: str
    character_premise: str
    style: str
    episode_duration_seconds: int
    episode_count: int
    scenes_per_episode: int
    shots_per_scene: int
    revision: int = 1
    schema_version: str = SCHEMA_VERSION
    id: str = field(default_factory=lambda: str(uuid4()))
    payload_hash: str = ""

    def __post_init__(self) -> None:
        if not self.project_id:
            raise ValidationDomainError("project_id must not be blank")
        for name in ("subject", "genre", "audience", "character_premise", "style"):
            object.__setattr__(self, name, _text(getattr(self, name), name))
        for name in (
            "episode_duration_seconds",
            "episode_count",
            "scenes_per_episode",
            "shots_per_scene",
        ):
            _positive_int(getattr(self, name), name)
        if self.revision < 1:
            raise ValidationDomainError("revision must be at least 1")
        if not self.payload_hash:
            object.__setattr__(
                self,
                "payload_hash",
                _hash(
                    {
                        "projectId": self.project_id,
                        "creativeBriefId": self.creative_brief_id,
                        "subject": self.subject,
                        "genre": self.genre,
                        "audience": self.audience,
                        "characterPremise": self.character_premise,
                        "style": self.style,
                        "episodeDurationSeconds": self.episode_duration_seconds,
                        "episodeCount": self.episode_count,
                        "scenesPerEpisode": self.scenes_per_episode,
                        "shotsPerScene": self.shots_per_scene,
                        "schema_version": self.schema_version,
                        "revision": self.revision,
                    }
                ),
            )


@dataclass(frozen=True, slots=True)
class CreativeBriefSourceBindingSnapshot:
    project_id: str
    source_material_id: str
    source_material_revision: int
    source_content_hash: str
    creative_brief_id: str
    creative_brief_revision: int
    creative_brief_payload_hash: str
    parse_status: str
    validation_status: str
    binding_status: str
    binding_version: str
    schema_version: str = SCHEMA_VERSION
    id: str = field(default_factory=lambda: str(uuid4()))

    def __post_init__(self) -> None:
        for value in (
            self.project_id,
            self.source_material_id,
            self.source_content_hash,
            self.creative_brief_id,
            self.creative_brief_payload_hash,
            self.parse_status,
            self.validation_status,
            self.binding_status,
            self.binding_version,
        ):
            if not isinstance(value, str) or not value.strip():
                raise ValidationDomainError("source binding snapshot contains blank fields")
        _positive_int(self.source_material_revision, "source_material_revision")
        _positive_int(self.creative_brief_revision, "creative_brief_revision")


@dataclass(frozen=True, slots=True)
class ProjectCreativeSettingsVersion:
    project_id: str
    text_cost_confirmation_threshold: dict[str, str] | None
    revision: int = 1
    schema_version: str = SCHEMA_VERSION
    id: str = field(default_factory=lambda: str(uuid4()))
    payload_hash: str = ""

    def __post_init__(self) -> None:
        if self.revision < 1:
            raise ValidationDomainError("revision must be at least 1")
        threshold = self.text_cost_confirmation_threshold
        if threshold is not None:
            if set(threshold) != {"amount", "currency"}:
                raise ValidationDomainError("threshold must contain amount and currency only")
            try:
                amount = Decimal(str(threshold["amount"]))
            except (InvalidOperation, ValueError) as exc:
                raise ValidationDomainError("threshold amount must be decimal") from exc
            if amount < 0:
                raise ValidationDomainError("threshold amount must be non-negative")
            currency = str(threshold["currency"]).upper()
            if currency not in _CURRENCIES:
                raise ValidationDomainError("threshold currency must be ISO 4217")
            object.__setattr__(
                self,
                "text_cost_confirmation_threshold",
                {"amount": str(amount), "currency": currency},
            )
        if not self.payload_hash:
            object.__setattr__(
                self,
                "payload_hash",
                _hash(
                    {
                        "projectId": self.project_id,
                        "threshold": threshold,
                        "revision": self.revision,
                    }
                ),
            )


@dataclass(frozen=True, slots=True)
class ProjectEpisodeTextHandoff:
    handoff_id: str
    project_id: str
    project_revision: int
    batch_revision: int
    story_spec_id: str
    story_spec_revision: int
    story_spec_hash: str
    episode_script_refs: tuple[dict[str, object], ...]
    payload_hash: str
    correlation_id: str
    accepted: bool = True
    schema_version: str = SCHEMA_VERSION

    def __post_init__(self) -> None:
        if not self.accepted:
            raise ValidationDomainError("text handoff must be accepted")
        if not self.episode_script_refs:
            raise ValidationDomainError("episode_script_refs must not be empty")
        ids = [str(item.get("episodeId", "")) for item in self.episode_script_refs]
        if any(not value for value in ids) or len(set(ids)) != len(ids):
            raise ValidationDomainError("episode_script_refs must contain unique episode IDs")
        if self.project_revision < 1 or self.batch_revision < 1:
            raise ValidationDomainError("handoff revisions must be positive")


@dataclass(frozen=True, slots=True)
class ProjectEpisodeTextHandoffAck:
    handoff_id: str
    fingerprint: str
    project_revision: int
    episode_revisions: tuple[tuple[str, int], ...]
    correlation_id: str
    id: str = field(default_factory=lambda: str(uuid4()))


def ensure_revision(expected: int, current: int, entity_id: str) -> None:
    if expected != current:
        raise RevisionConflictError(entity_id, expected, current)
