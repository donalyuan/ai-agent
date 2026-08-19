"""complete asset identity and append-only storage metadata"""

from __future__ import annotations

import re
from urllib.parse import urlsplit

import sqlalchemy as sa
from sqlalchemy.dialects import postgresql

from alembic import op

revision = "0004_assets_asset_versions_slice"
down_revision = "0003_projects_episodes_slice"
branch_labels = None
depends_on = None

JSON_DOCUMENT = sa.JSON().with_variant(postgresql.JSONB(), "postgresql")
ASSET_KINDS_CHECK = "kind IN ('image', 'video', 'audio', 'text', 'document')"
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
    """Freeze this revision's legacy reference semantics for replayability."""
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


def _hex64_check(column: str) -> str:
    stripped = f"lower({column})"
    for character in "0123456789abcdef":
        stripped = f"replace({stripped}, '{character}', '')"
    return f"length({column}) = 64 AND length({stripped}) = 0"


def _validate_legacy_rows() -> list[tuple[str, tuple[str, str, str]]]:
    """Reject rows whose integrity facts cannot be recovered from legacy columns."""
    connection = op.get_bind()
    invalid = connection.execute(
        sa.text(
            "SELECT COUNT(*) FROM asset_versions av "
            "LEFT JOIN assets a ON a.id = av.asset_id "
            f"WHERE av.checksum IS NULL OR NOT ({_hex64_check('av.checksum')}) "
            "OR av.storage_ref IS NULL "
            "OR a.project_id IS NULL OR av.version_number <= 0"
        )
    ).scalar_one()
    invalid_kinds = connection.execute(
        sa.text(
            "SELECT COUNT(*) FROM assets "
            "WHERE kind NOT IN ('image', 'video', 'audio', 'text', 'document')"
        )
    ).scalar_one()
    canonical_rows: list[tuple[str, tuple[str, str, str]]] = []
    for version_id, storage_ref in connection.execute(
        sa.text("SELECT id, storage_ref FROM asset_versions")
    ):
        canonical = _canonical_legacy_storage(storage_ref)
        if canonical is None:
            raise RuntimeError(
                "0004 cannot migrate legacy asset versions with an unsafe object reference"
            )
        canonical_rows.append((version_id, canonical))
    if invalid or invalid_kinds:
        raise RuntimeError(
            "0004 cannot migrate legacy asset versions without real checksum, "
            "safe object reference, project ownership, positive version number, or supported kind"
        )
    return canonical_rows


def _tighten_asset_name() -> None:
    if op.get_context().dialect.name == "sqlite":
        with op.batch_alter_table("assets", recreate="always") as batch:
            batch.alter_column("name", existing_type=sa.String(length=255), nullable=False)
            batch.create_check_constraint("ck_assets_kind", ASSET_KINDS_CHECK)
            batch.create_check_constraint("ck_assets_name_nonblank", "length(trim(name)) > 0")
    else:
        op.alter_column("assets", "name", existing_type=sa.String(length=255), nullable=False)
        op.create_check_constraint("ck_assets_kind", "assets", ASSET_KINDS_CHECK)
        op.create_check_constraint("ck_assets_name_nonblank", "assets", "length(trim(name)) > 0")


def _existing_columns(table_name: str) -> set[str]:
    return {column["name"] for column in sa.inspect(op.get_bind()).get_columns(table_name)}


def _existing_checks(table_name: str) -> set[str]:
    return {
        constraint["name"]
        for constraint in sa.inspect(op.get_bind()).get_check_constraints(table_name)
        if constraint.get("name")
    }


def upgrade() -> None:
    canonical_rows = _validate_legacy_rows()
    op.add_column("assets", sa.Column("name", sa.String(length=255), nullable=True))
    op.execute(sa.text("UPDATE assets SET name = 'Untitled Asset' WHERE name IS NULL"))
    _tighten_asset_name()

    columns = (
        sa.Column("project_id", sa.String(length=36), nullable=True),
        sa.Column("content_hash", sa.String(length=64), nullable=True),
        sa.Column("storage_provider", sa.String(length=128), nullable=True),
        sa.Column("bucket", sa.String(length=255), nullable=True),
        sa.Column("region", sa.String(length=255), nullable=True),
        sa.Column("object_key", sa.String(length=1024), nullable=True),
        sa.Column("e_tag", sa.String(length=255), nullable=True),
        sa.Column("mime_type", sa.String(length=255), nullable=True),
        sa.Column("size_bytes", sa.Integer(), nullable=True),
        sa.Column("media_metadata", JSON_DOCUMENT, nullable=True),
    )
    for column in columns:
        op.add_column("asset_versions", column)
    op.execute(
        sa.text(
            "UPDATE asset_versions SET "
            "project_id = (SELECT project_id FROM assets "
            "WHERE assets.id = asset_versions.asset_id), "
            "content_hash = checksum, mime_type = 'application/octet-stream', size_bytes = 0"
        )
    )
    canonical_update = sa.text(
        "UPDATE asset_versions SET storage_provider = :storage_provider, bucket = :bucket, "
        "object_key = :object_key WHERE id = :version_id"
    )
    for version_id, (storage_provider, bucket, object_key) in canonical_rows:
        op.get_bind().execute(
            canonical_update,
            {
                "version_id": version_id,
                "storage_provider": storage_provider,
                "bucket": bucket,
                "object_key": object_key,
            },
        )
    if op.get_context().dialect.name == "sqlite":
        with op.batch_alter_table("asset_versions", recreate="always") as batch:
            for name, type_ in (
                ("project_id", sa.String(length=36)),
                ("content_hash", sa.String(length=64)),
                ("storage_provider", sa.String(length=128)),
                ("bucket", sa.String(length=255)),
                ("object_key", sa.String(length=1024)),
                ("mime_type", sa.String(length=255)),
                ("size_bytes", sa.Integer()),
            ):
                batch.alter_column(name, existing_type=type_, nullable=False)
            batch.alter_column("checksum", existing_type=sa.String(length=128), nullable=False)
            batch.create_check_constraint("ck_asset_versions_size_nonnegative", "size_bytes >= 0")
            batch.create_check_constraint(
                "ck_asset_versions_version_positive", "version_number > 0"
            )
            batch.create_check_constraint(
                "ck_asset_versions_checksum_length", "length(checksum) = 64"
            )
            batch.create_check_constraint(
                "ck_asset_versions_content_hash_length", "length(content_hash) = 64"
            )
            batch.create_check_constraint(
                "ck_asset_versions_object_key_nonblank", "length(trim(object_key)) > 0"
            )
            batch.create_check_constraint(
                "ck_asset_versions_mime_type_nonblank", "length(trim(mime_type)) > 0"
            )
    else:
        for name, type_ in (
            ("project_id", sa.String(length=36)),
            ("content_hash", sa.String(length=64)),
            ("storage_provider", sa.String(length=128)),
            ("bucket", sa.String(length=255)),
            ("object_key", sa.String(length=1024)),
            ("mime_type", sa.String(length=255)),
            ("size_bytes", sa.Integer()),
        ):
            op.alter_column("asset_versions", name, existing_type=type_, nullable=False)
        op.alter_column(
            "asset_versions", "checksum", existing_type=sa.String(length=128), nullable=False
        )
        op.create_check_constraint(
            "ck_asset_versions_size_nonnegative", "asset_versions", "size_bytes >= 0"
        )
        op.create_check_constraint(
            "ck_asset_versions_version_positive", "asset_versions", "version_number > 0"
        )
        op.create_check_constraint(
            "ck_asset_versions_checksum_length", "asset_versions", "length(checksum) = 64"
        )
        op.create_check_constraint(
            "ck_asset_versions_content_hash_length",
            "asset_versions",
            "length(content_hash) = 64",
        )
        op.create_check_constraint(
            "ck_asset_versions_object_key_nonblank",
            "asset_versions",
            "length(trim(object_key)) > 0",
        )
        op.create_check_constraint(
            "ck_asset_versions_mime_type_nonblank",
            "asset_versions",
            "length(trim(mime_type)) > 0",
        )
    op.create_index("ix_asset_versions_project_id", "asset_versions", ["project_id"])


def downgrade() -> None:
    op.drop_index("ix_asset_versions_project_id", table_name="asset_versions")
    asset_version_checks = (
        "ck_asset_versions_size_nonnegative",
        "ck_asset_versions_version_positive",
        "ck_asset_versions_checksum_length",
        "ck_asset_versions_content_hash_length",
        "ck_asset_versions_object_key_nonblank",
        "ck_asset_versions_mime_type_nonblank",
    )
    asset_version_columns = (
        "media_metadata",
        "size_bytes",
        "mime_type",
        "e_tag",
        "object_key",
        "region",
        "bucket",
        "storage_provider",
        "content_hash",
        "project_id",
    )
    if op.get_context().dialect.name == "sqlite":
        existing_columns = _existing_columns("asset_versions")
        existing_checks = _existing_checks("asset_versions")
        with op.batch_alter_table("asset_versions", recreate="always") as batch:
            for constraint_name in asset_version_checks:
                if constraint_name in existing_checks:
                    batch.drop_constraint(constraint_name, type_="check")
            for column_name in asset_version_columns:
                if column_name in existing_columns:
                    batch.drop_column(column_name)
    else:
        existing_columns = _existing_columns("asset_versions")
        existing_checks = _existing_checks("asset_versions")
        for constraint_name in asset_version_checks:
            if constraint_name in existing_checks:
                op.drop_constraint(constraint_name, "asset_versions", type_="check")
        for column_name in asset_version_columns:
            if column_name in existing_columns:
                op.drop_column("asset_versions", column_name)
    if op.get_context().dialect.name == "sqlite":
        existing_columns = _existing_columns("assets")
        existing_checks = _existing_checks("assets")
        with op.batch_alter_table("assets", recreate="always") as batch:
            for constraint_name in ("ck_assets_kind", "ck_assets_name_nonblank"):
                if constraint_name in existing_checks:
                    batch.drop_constraint(constraint_name, type_="check")
            if "name" in existing_columns:
                batch.drop_column("name")
    else:
        existing_columns = _existing_columns("assets")
        existing_checks = _existing_checks("assets")
        for constraint_name in ("ck_assets_kind", "ck_assets_name_nonblank"):
            if constraint_name in existing_checks:
                op.drop_constraint(constraint_name, "assets", type_="check")
        if "name" in existing_columns:
            op.drop_column("assets", "name")
