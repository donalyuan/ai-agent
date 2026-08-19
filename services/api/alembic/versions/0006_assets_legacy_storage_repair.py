"""normalize already-migrated local workspace references"""

from __future__ import annotations

import re
from urllib.parse import urlsplit

import sqlalchemy as sa

from alembic import op

# Keep the semantic Alembic version key within the default alembic_version.varchar(32).
revision = "0006_assets_legacy_repair"
down_revision = "0005_assets_integrity_repair"
branch_labels = None
depends_on = None

_REFERENCE_SCHEME = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")
_WORKSPACE_REFERENCE_PREFIX = "workspace://"


def _canonical_object_key(value: object) -> str | None:
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


def _canonical_legacy_storage(value: object) -> tuple[str, str, str] | None:
    """Freeze this repair revision's supported legacy reference forms."""
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
        key = _canonical_object_key(f"{parsed.netloc}{parsed.path}")
        if key is None:
            return None
        return ("local_workspace", "workspace", key)
    key = _canonical_object_key(value)
    if key is None:
        return None
    return ("legacy", "legacy", key)


def _validate_existing_rows() -> list[tuple[str, tuple[str, str, str]]]:
    canonical_rows: list[tuple[str, tuple[str, str, str]]] = []
    rows = op.get_bind().execute(sa.text("SELECT id, storage_ref FROM asset_versions"))
    for version_id, storage_ref in rows:
        canonical = _canonical_legacy_storage(storage_ref)
        if canonical is None:
            raise RuntimeError("0006 cannot normalize an unsafe legacy object reference")
        if canonical[0] == "local_workspace":
            canonical_rows.append((version_id, canonical))
    return canonical_rows


def upgrade() -> None:
    canonical_update = sa.text(
        "UPDATE asset_versions SET storage_provider = :storage_provider, bucket = :bucket, "
        "object_key = :object_key WHERE id = :version_id"
    )
    for version_id, (storage_provider, bucket, object_key) in _validate_existing_rows():
        op.get_bind().execute(
            canonical_update,
            {
                "version_id": version_id,
                "storage_provider": storage_provider,
                "bucket": bucket,
                "object_key": object_key,
            },
        )


def downgrade() -> None:
    op.execute(
        sa.text(
            "UPDATE asset_versions SET storage_provider = 'legacy', bucket = 'legacy', "
            "object_key = storage_ref "
            "WHERE storage_provider = 'local_workspace' AND bucket = 'workspace'"
        )
    )
