"""Immutable image-generation candidate facts owned by the image workflow."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal
from uuid import uuid4

from .errors import ValidationDomainError


@dataclass(frozen=True, slots=True)
class ImageReference:
    project_id: str
    asset_version_id: str
    asset_version_revision: int
    asset_version_hash: str
    mime_type: str
    size_bytes: int


@dataclass(frozen=True, slots=True)
class ImageCandidate:
    project_id: str
    episode_id: str
    target_id: str
    asset_id: str
    operation: Literal["generate", "edit"]
    run_id: str
    logical_operation: str
    asset_version_id: str
    asset_version_revision: int
    asset_version_hash: str
    continuity_snapshot_id: str
    continuity_snapshot_revision: int
    continuity_snapshot_hash: str
    provenance: dict[str, object]
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    status: Literal["unreferenced", "accepted", "rejected"] = "unreferenced"

    def __post_init__(self) -> None:
        if self.status not in {"unreferenced", "accepted", "rejected"}:
            raise ValidationDomainError("image candidate status is invalid")
        if self.asset_version_revision < 0 or self.continuity_snapshot_revision < 1:
            raise ValidationDomainError("image candidate revision is invalid")
        if not self.project_id or not self.target_id or not self.asset_version_id:
            raise ValidationDomainError("image candidate provenance is incomplete")
