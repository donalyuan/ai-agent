"""不依赖 FastAPI/SQLAlchemy 的 projects/episodes 领域实体。"""

from __future__ import annotations

from dataclasses import dataclass, field
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError

SCHEMA_VERSION = "1.0.0"
STATUS_DRAFT = "draft"


def _required_text(value: str, field_name: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValidationDomainError(f"{field_name} must not be blank")
    return value.strip()


def _positive_number(value: int) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        raise ValidationDomainError("number must be a positive integer")
    return value


@dataclass(slots=True)
class Project:
    name: str
    id: str = field(default_factory=lambda: str(uuid4()))
    status: str = STATUS_DRAFT
    schema_version: str = SCHEMA_VERSION
    revision: int = 1
    creation_mode: str | None = None
    creative_brief_current: object | None = None
    creative_brief_history: list[object] = field(default_factory=list)
    creative_settings_current: object | None = None
    creative_settings_history: list[object] = field(default_factory=list)
    source_binding_current: object | None = None
    source_binding_history: list[object] = field(default_factory=list)
    story_spec_ref: dict[str, object] | None = None
    story_spec_history: list[dict[str, object]] = field(default_factory=list)
    source_materials: list[dict[str, object]] = field(default_factory=list)

    def __post_init__(self) -> None:
        self.name = _required_text(self.name, "name")
        if self.revision < 1:
            raise ValidationDomainError("revision must be at least 1")

    def update(self, *, expected_revision: int, name: str | None = None) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        if name is not None:
            self.name = _required_text(name, "name")
        self.revision += 1


@dataclass(slots=True)
class Episode:
    project_id: str
    title: str
    number: int
    id: str = field(default_factory=lambda: str(uuid4()))
    status: str = STATUS_DRAFT
    schema_version: str = SCHEMA_VERSION
    revision: int = 1
    script_spec_ref: dict[str, object] | None = None
    script_spec_history: list[dict[str, object]] = field(default_factory=list)

    def __post_init__(self) -> None:
        self.title = _required_text(self.title, "title")
        self.number = _positive_number(self.number)
        if not self.project_id:
            raise ValidationDomainError("project_id must not be blank")
        if self.revision < 1:
            raise ValidationDomainError("revision must be at least 1")

    def update(
        self,
        *,
        expected_revision: int,
        title: str | None = None,
        number: int | None = None,
    ) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        if title is not None:
            self.title = _required_text(title, "title")
        if number is not None:
            self.number = _positive_number(number)
        self.revision += 1
