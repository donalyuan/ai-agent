"""SourceMaterial immutable input and storage handoff facts."""

from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass, field
from typing import Literal
from uuid import uuid4

from .errors import RevisionConflictError, ValidationDomainError

MaterialType = Literal["novel", "synopsis", "existing_script"]
InputMode = Literal["inline_text", "uploaded_file"]


@dataclass(frozen=True, slots=True)
class SourceMaterialVersion:
    project_id: str
    material_type: MaterialType
    input_mode: InputMode
    content_hash: str
    revision: int = 1
    parse_status: str = "pending"
    validation_status: str = "pending"
    id: str = field(default_factory=lambda: str(uuid4()))
    asset_version_id: str | None = None

    def __post_init__(self) -> None:
        if self.material_type not in {"novel", "synopsis", "existing_script"}:
            raise ValidationDomainError("source material type is invalid")
        if self.input_mode not in {"inline_text", "uploaded_file"}:
            raise ValidationDomainError("source material input mode is invalid")
        if (
            not isinstance(self.content_hash, str)
            or re.fullmatch(r"[0-9a-fA-F]{64}", self.content_hash) is None
        ):
            raise ValidationDomainError("source material content hash is invalid")
        if self.input_mode == "inline_text" and self.asset_version_id is not None:
            raise ValidationDomainError("inline source must not reference AssetVersion")
        if self.input_mode == "uploaded_file" and not self.asset_version_id:
            raise ValidationDomainError("uploaded source requires verified AssetVersion")


@dataclass(slots=True)
class SourceMaterial:
    project_id: str
    material_type: MaterialType
    input_mode: InputMode
    current: SourceMaterialVersion | None = None
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    versions: list[SourceMaterialVersion] = field(default_factory=list)

    def __post_init__(self) -> None:
        if self.material_type not in {"novel", "synopsis", "existing_script"}:
            raise ValidationDomainError("source material type is invalid")
        if self.input_mode not in {"inline_text", "uploaded_file"}:
            raise ValidationDomainError("source material input mode is invalid")
        if not self.project_id:
            raise ValidationDomainError("source material project is required")
        if self.revision < 1:
            raise ValidationDomainError("source material revision must be positive")

    def append(
        self,
        *,
        expected_revision: int,
        input_mode: InputMode,
        content: bytes | None = None,
        content_hash: str | None = None,
        asset_version_id: str | None = None,
    ) -> SourceMaterialVersion:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        if input_mode != self.input_mode:
            raise ValidationDomainError("source material input mode is immutable")
        if input_mode == "inline_text" and content is None:
            raise ValidationDomainError("inline source content is required")
        if content is not None and content_hash is not None:
            raise ValidationDomainError("provide content or content_hash, not both")
        digest = content_hash or hashlib.sha256(content).hexdigest()  # type: ignore[arg-type]
        version = SourceMaterialVersion(
            self.project_id,
            self.material_type,
            input_mode,
            digest,
            len(self.versions) + 1,
            "parsed",
            "valid",
            asset_version_id=asset_version_id,
        )
        self.versions.append(version)
        self.current = version
        self.revision += 1
        return version


@dataclass(frozen=True, slots=True)
class SourceMaterialUploadIntent:
    project_id: str
    source_material_id: str
    source_material_revision: int
    material_type: MaterialType
    input_mode: InputMode
    content_hash: str
    operation_key: str
    reservation_id: str
    creation_mode: Literal["adaptation"] = "adaptation"
    project_scope: str | None = None
    run_id: str | None = None
    logical_operation: str | None = None

    def __post_init__(self) -> None:
        if self.input_mode != "uploaded_file":
            raise ValidationDomainError("storage upload requires uploaded_file input")
        if self.material_type not in {"novel", "synopsis", "existing_script"}:
            raise ValidationDomainError("source material type is invalid")
        if self.creation_mode != "adaptation":
            raise ValidationDomainError("source material upload requires adaptation")
        if self.project_scope is not None and self.project_scope != self.project_id:
            raise ValidationDomainError("source material upload scope is invalid")
        if bool(self.run_id) != bool(self.logical_operation):
            raise ValidationDomainError("run upload mapping is incomplete")
        expected = (
            f"source-material-upload:{self.project_id}:{self.source_material_id}:"
            f"{self.source_material_revision}"
        )
        if self.operation_key != expected:
            raise ValidationDomainError("source material upload key is not canonical")


@dataclass(frozen=True, slots=True)
class VerifiedStoredObjectHandoff:
    operation_key: str
    project_id: str
    source_material_id: str
    source_material_revision: int
    object_ref: str
    size_bytes: int
    checksum: str
    mime_type: str
    etag: str | None
    profile_revision: int
    status: Literal["verified", "unknown", "failed"] = "verified"
    id: str = field(default_factory=lambda: str(uuid4()))
    storage_profile_id: str = ""
    bucket_id: str = ""
    upload_session_id: str = ""
    reservation_id: str = ""
    verified_at: str = ""

    def __post_init__(self) -> None:
        if self.status == "verified" and (self.size_bytes < 0 or len(self.checksum) != 64):
            raise ValidationDomainError("verified stored object metadata is invalid")
        expected = (
            f"source-material-upload:{self.project_id}:{self.source_material_id}:"
            f"{self.source_material_revision}"
        )
        if self.operation_key != expected:
            raise ValidationDomainError("source handoff operation key is invalid")
