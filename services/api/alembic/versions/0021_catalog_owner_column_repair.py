"""Repair catalog owner columns missing from databases upgraded by an older 0016.

Revision ID: 0021_catalog_owner_column_repair
Revises: 0020_timeline_export_owner
"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0021_catalog_owner_column_repair"
down_revision = "0020_timeline_export_owner"
branch_labels = None
depends_on = None


def _add_missing(table: str, columns: tuple[sa.Column[object], ...]) -> None:
    existing = {item["name"] for item in sa.inspect(op.get_bind()).get_columns(table)}
    for column in columns:
        if column.name not in existing:
            op.add_column(table, column)


def upgrade() -> None:
    # 0016 was shared before these additive owner columns were included. Existing
    # databases may therefore report a later head while still lacking them.
    _add_missing(
        "providers",
        (
            sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
            sa.Column("approval", sa.String(64), nullable=False, server_default="pending"),
            sa.Column("feature_gate", sa.String(32), nullable=False, server_default="MVP-A"),
            sa.Column("adapter_installed", sa.Boolean(), nullable=False, server_default=sa.false()),
        ),
    )
    _add_missing(
        "provider_profiles",
        (
            sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
            sa.Column(
                "adapter_identity",
                sa.String(128),
                nullable=False,
                server_default="local_workspace",
            ),
            sa.Column(
                "explicit_live_opt_in",
                sa.Boolean(),
                nullable=False,
                server_default=sa.false(),
            ),
            sa.Column(
                "credential_status",
                sa.String(32),
                nullable=False,
                server_default="unconfigured",
            ),
        ),
    )
    _add_missing(
        "models",
        (
            sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
            sa.Column("schema_version", sa.String(32), nullable=False, server_default="1.0.0"),
        ),
    )
    _add_missing(
        "credential_metadata",
        (
            sa.Column("profile_id", sa.String(36), nullable=True),
            sa.Column("credential_id", sa.String(255), nullable=True),
            sa.Column("algorithm", sa.String(64), nullable=True),
            sa.Column("aad_version", sa.String(32), nullable=True),
        ),
    )


def downgrade() -> None:
    # These columns canonically belong to 0016. Removing them at 0020 would
    # recreate the incompatible schema this repair is intended to eliminate.
    pass
