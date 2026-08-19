"""Asset domain entities and storage metadata value object.

This module intentionally has no framework or persistence imports.  A version stores a
reference to an object and its inspectable metadata, never the media bytes themselves.
"""

from __future__ import annotations

import re
from collections.abc import Mapping
from dataclasses import dataclass, field
from types import MappingProxyType
from uuid import uuid4

from .entities import SCHEMA_VERSION, STATUS_DRAFT
from .errors import ImmutableAssetVersionError, ValidationDomainError
from .object_key_contract import canonical_object_key

ASSET_KINDS = frozenset({"image", "video", "audio", "text", "document"})
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

    def __post_init__(self) -> None:
        if not self.project_id:
            raise ValidationDomainError("project_id must not be blank")
        if self.kind not in ASSET_KINDS:
            raise ValidationDomainError(f"kind must be one of: {', '.join(sorted(ASSET_KINDS))}")
        self.name = _text(self.name, "name")
        if isinstance(self.revision, bool) or self.revision < 1:
            raise ValidationDomainError("revision must be at least 1")


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
