"""add project creative owner document columns"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0007_projects_creative_owner"
down_revision = "0006_assets_legacy_repair"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.add_column("projects", sa.Column("creation_mode", sa.String(length=32), nullable=True))
    op.add_column("projects", sa.Column("creative_brief_current", sa.JSON(), nullable=True))
    op.add_column(
        "projects",
        sa.Column("creative_brief_history", sa.JSON(), nullable=False, server_default="[]"),
    )
    op.add_column("projects", sa.Column("creative_settings_current", sa.JSON(), nullable=True))
    op.add_column(
        "projects",
        sa.Column("creative_settings_history", sa.JSON(), nullable=False, server_default="[]"),
    )
    op.add_column("projects", sa.Column("source_binding_current", sa.JSON(), nullable=True))
    op.add_column(
        "projects",
        sa.Column("source_binding_history", sa.JSON(), nullable=False, server_default="[]"),
    )
    # SQLite cannot ALTER COLUMN to drop a default; the default is only a migration
    # backfill aid and has no domain meaning, so it is intentionally retained there.


def downgrade() -> None:
    for name in (
        "source_binding_history",
        "source_binding_current",
        "creative_settings_history",
        "creative_settings_current",
        "creative_brief_history",
        "creative_brief_current",
        "creation_mode",
    ):
        op.drop_column("projects", name)
