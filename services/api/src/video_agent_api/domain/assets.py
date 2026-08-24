"""Asset domain entities and storage metadata value object.

This module intentionally has no framework or persistence imports.  A version stores a
reference to an object and its inspectable metadata, never the media bytes themselves.
"""

from __future__ import annotations

import re
from collections.abc import Mapping
from dataclasses import dataclass, field
from datetime import UTC, datetime
from types import MappingProxyType
from typing import Literal
from uuid import uuid4

from .entities import SCHEMA_VERSION, STATUS_DRAFT
from .errors import ImmutableAssetVersionError, ValidationDomainError
from .object_key_contract import canonical_object_key

ASSET_KINDS = frozenset({"image", "video", "audio", "text", "document"})
ASSET_SOURCE_TYPES = frozenset({"user_upload", "provider_generated", "source_material", "imported"})
ASSET_CATALOG_ROLES = frozenset(
    {
        "character",
        "location",
        "prop",
        "storyboard",
        "video_take",
        "dialogue",
        "music",
        "ambience",
        "effects",
        "other",
    }
)
ASSET_AUTHORIZATION_STATUSES = frozenset(
    {"unknown", "declared", "verified", "restricted", "expired"}
)
_HASH = re.compile(r"^[a-fA-F0-9]{64}$")
_MIME = re.compile(r"^[^/\s]+/[^/\s]+$")


def _text(value: object, name: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValidationDomainError(f"{name} must not be blank")
    return value.strip()


def _hash(value: object, name: str) -> str:
    value = _text(value, name)
    if not _HASH.fullmatch(value):
        raise ValidationDomainError(f"{name} must be a 64-character hexadecimal hash")
    return value


def _object_key(value: object) -> str:
    key = canonical_object_key(value)
    if key is None:
        raise ValidationDomainError(
            "object_key must be a canonical relative path without references"
        )
    return key


def _non_negative(value: object, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValidationDomainError(f"{name} must be a non-negative integer")
    return value


@dataclass(frozen=True, slots=True)
class StorageObject:
    storage_provider: str
    bucket: str
    object_key: str
    mime_type: str
    size_bytes: int
    checksum: str
    region: str | None = None
    e_tag: str | None = None
    media: Mapping[str, int] | None = None

    def __deepcopy__(self, memo: dict[int, object]) -> StorageObject:
        del memo
        return self

    def __post_init__(self) -> None:
        object.__setattr__(
            self, "storage_provider", _text(self.storage_provider, "storage_provider")
        )
        object.__setattr__(self, "bucket", _text(self.bucket, "bucket"))
        object.__setattr__(self, "object_key", _object_key(self.object_key))
        object.__setattr__(self, "mime_type", _text(self.mime_type, "mime_type"))
        if not _MIME.fullmatch(self.mime_type):
            raise ValidationDomainError("mime_type must use type/subtype form")
        object.__setattr__(self, "size_bytes", _non_negative(self.size_bytes, "size_bytes"))
        object.__setattr__(self, "checksum", _hash(self.checksum, "checksum"))
        if self.region is not None:
            object.__setattr__(self, "region", _text(self.region, "region"))
        if self.e_tag is not None:
            object.__setattr__(self, "e_tag", _text(self.e_tag, "e_tag"))
        if self.media is not None:
            if not isinstance(self.media, dict):
                raise ValidationDomainError("media must be an object")
            allowed = {"duration_ms", "width", "height"}
            if set(self.media) - allowed:
                raise ValidationDomainError("media contains unknown fields")
            normalized: dict[str, int] = {}
            for key, raw in self.media.items():
                if isinstance(raw, bool) or not isinstance(raw, int):
                    raise ValidationDomainError(f"media.{key} must be an integer")
                if key == "duration_ms" and raw < 0:
                    raise ValidationDomainError("media.duration_ms must be non-negative")
                if key in {"width", "height"} and raw < 1:
                    raise ValidationDomainError(f"media.{key} must be positive")
                normalized[key] = raw
            object.__setattr__(self, "media", MappingProxyType(normalized))


@dataclass(slots=True)
class Asset:
    project_id: str
    kind: str
    name: str
    id: str = field(default_factory=lambda: str(uuid4()))
    status: str = STATUS_DRAFT
    schema_version: str = SCHEMA_VERSION
    revision: int = 1
    source_type: str = "imported"
    catalog_role: str | None = None
    tags: tuple[str, ...] = ()
    authorization_status: str = "unknown"
    copyright_owner: str | None = None
    license_label: str | None = None
    license_reference: str | None = None
    updated_at: str = field(default_factory=lambda: datetime.now(UTC).isoformat())

    def __post_init__(self) -> None:
        if not self.project_id:
            raise ValidationDomainError("project_id must not be blank")
        if self.kind not in ASSET_KINDS:
            raise ValidationDomainError(f"kind must be one of: {', '.join(sorted(ASSET_KINDS))}")
        self.name = _text(self.name, "name")
        if isinstance(self.revision, bool) or self.revision < 1:
            raise ValidationDomainError("revision must be at least 1")
        if len(self.tags) > 32 or len(set(self.tags)) != len(self.tags):
            raise ValidationDomainError("tags must be bounded and unique")
        if self.source_type not in ASSET_SOURCE_TYPES:
            raise ValidationDomainError("asset source_type is invalid")
        if self.catalog_role is not None and self.catalog_role not in ASSET_CATALOG_ROLES:
            raise ValidationDomainError("asset catalog_role is invalid")
        if self.authorization_status not in ASSET_AUTHORIZATION_STATUSES:
            raise ValidationDomainError("asset authorization_status is invalid")
        normalized_tags = tuple(_text(item, "tag") for item in self.tags)
        if any(len(item) > 64 for item in normalized_tags):
            raise ValidationDomainError("tag must not exceed 64 characters")
        self.tags = normalized_tags
        for field_name in ("copyright_owner", "license_label", "license_reference"):
            value = getattr(self, field_name)
            if value is not None:
                setattr(self, field_name, _text(value, field_name))

    @property
    def license(self) -> str | None:
        """历史代码读取同一 canonical license label，不形成第二事实源。"""
        return self.license_label

    @license.setter
    def license(self, value: str | None) -> None:
        self.license_label = value


@dataclass(slots=True)
class AssetVersionReservation:
    project_id: str
    asset_id: str
    operation_key: str
    fingerprint: str
    status: Literal["reserved", "registered", "cancelled", "failed"] = "reserved"
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    registered_version_id: str | None = None
    expected_asset_revision: int = 1
    declared_kind: str = "image"
    declared_mime_type: str = "image/png"
    declared_size_bytes: int = 0
    declared_checksum: str = "0" * 64
    storage_profile_id: str = "local-test-offline"
    storage_profile_revision: int = 1
    storage_profile_snapshot_hash: str = "0" * 64
    upload_key: str = ""
    diagnostic: str | None = None
    schema_version: str = SCHEMA_VERSION

    def __post_init__(self) -> None:
        expected_key = f"asset-upload:{self.project_id}:{self.asset_id}:{self.id}"
        if self.operation_key != expected_key:
            raise ValidationDomainError("asset reservation identity is invalid")
        if self.schema_version != SCHEMA_VERSION:
            raise ValidationDomainError("unsupported schemaVersion")
        expected_prefix = f"projects/{self.project_id}/assets/{self.asset_id}/{self.id}/"
        if not self.upload_key:
            legacy_extension = self.declared_mime_type.partition("/")[2].replace("jpeg", "jpg")
            self.upload_key = f"{expected_prefix}original.{legacy_extension or 'bin'}"
        self.upload_key = _object_key(self.upload_key)
        if not self.upload_key.startswith(expected_prefix):
            raise ValidationDomainError("asset reservation upload key is invalid")
        _hash(self.fingerprint, "fingerprint")
        if self.status not in {"reserved", "registered", "cancelled", "failed"}:
            raise ValidationDomainError("asset reservation status is invalid")
        if self.revision < 1:
            raise ValidationDomainError("asset reservation revision is invalid")
        if self.status == "registered" and not self.registered_version_id:
            raise ValidationDomainError("registered reservation requires AssetVersion")
        if self.status != "registered" and self.registered_version_id is not None:
            raise ValidationDomainError("unregistered reservation cannot reference AssetVersion")
        if self.expected_asset_revision < 1 or self.declared_kind not in ASSET_KINDS:
            raise ValidationDomainError("asset reservation owner snapshot is invalid")
        if not _MIME.fullmatch(self.declared_mime_type) or self.declared_size_bytes < 0:
            raise ValidationDomainError("asset reservation declared media is invalid")
        _hash(self.declared_checksum, "declared_checksum")
        _hash(self.storage_profile_snapshot_hash, "storage_profile_snapshot_hash")
        if self.storage_profile_revision < 1:
            raise ValidationDomainError("storage profile revision is invalid")

    def transition(self, target: str, registered_version_id: str | None = None) -> None:
        if target not in {"registered", "cancelled", "failed"} or self.status != "reserved":
            raise ValidationDomainError("invalid reservation transition")
        if target == "registered" and not registered_version_id:
            raise ValidationDomainError("registered reservation requires AssetVersion")
        self.status = target  # type: ignore[assignment]
        self.registered_version_id = registered_version_id
        self.revision += 1


@dataclass(frozen=True, slots=True)
class AssetVersion:
    asset_id: str
    project_id: str
    version_number: int
    storage_object: StorageObject
    content_hash: str | None = None
    id: str = field(default_factory=lambda: str(uuid4()))
    status: str = STATUS_DRAFT
    schema_version: str = SCHEMA_VERSION
    revision: int = 0

    def __deepcopy__(self, memo: dict[int, object]) -> AssetVersion:
        del memo
        return self

    def __post_init__(self) -> None:
        if not self.asset_id or not self.project_id:
            raise ValidationDomainError("asset_id and project_id must not be blank")
        if isinstance(self.version_number, bool) or self.version_number < 1:
            raise ValidationDomainError("version_number must be a positive integer")
        if self.content_hash is None:
            object.__setattr__(self, "content_hash", self.storage_object.checksum)
        else:
            object.__setattr__(self, "content_hash", _hash(self.content_hash, "content_hash"))
        if isinstance(self.revision, bool) or self.revision < 0:
            raise ValidationDomainError("revision must be non-negative")

    def update_storage(self, storage_object: StorageObject) -> None:
        del storage_object
        raise ImmutableAssetVersionError(f"asset version is immutable: {self.id}")
