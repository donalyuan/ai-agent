"""Canonical object-key validation shared by domain objects and Alembic repairs."""

from __future__ import annotations

import re
from dataclasses import dataclass
from urllib.parse import urlsplit

_REFERENCE_SCHEME = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
_WORKSPACE_REFERENCE_PREFIX = "workspace://"


@dataclass(frozen=True, slots=True)
class CanonicalLegacyStorage:
    """The only storage metadata a legacy reference can contribute to a migration."""

    storage_provider: str
    bucket: str
    object_key: str


def canonical_object_key(value: object) -> str | None:
    """Return a canonical POSIX key, never normalizing an unsafe input into a safe one."""
    if not isinstance(value, str) or not value or value != value.strip():
        return None
    if _REFERENCE_SCHEME.match(value) or "?" in value or "#" in value:
        return None
    if (
        value.startswith(("/", "\\"))
        or "\\" in value
        or any(part in {"", ".", ".."} or not part.strip() for part in value.split("/"))
    ):
        return None
    return value


def canonical_legacy_storage(value: object) -> CanonicalLegacyStorage | None:
    """Parse the sole supported legacy URI form and return values safe to persist."""
    if not isinstance(value, str) or not value or value != value.strip():
        return None
    if value.startswith(_WORKSPACE_REFERENCE_PREFIX):
        parsed = urlsplit(value)
        if (
            parsed.scheme != "workspace"
            or not parsed.netloc
            or not parsed.path
            or "?" in value
            or "#" in value
        ):
            return None
        key = canonical_object_key(f"{parsed.netloc}{parsed.path}")
        if key is None:
            return None
        return CanonicalLegacyStorage("local_workspace", "workspace", key)
    key = canonical_object_key(value)
    if key is None:
        return None
    return CanonicalLegacyStorage("legacy", "legacy", key)
