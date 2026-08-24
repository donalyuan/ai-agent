"""add phase-one owner ledgers"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0009_phase_one_owner_tables"
down_revision = "0008_scenes_shots"
branch_labels = None
depends_on = None


def _ledger(name: str, scope_column: str, scope_table: str) -> None:
    op.create_table(
        name,
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column(
            scope_column,
            sa.String(36),
            sa.ForeignKey(f"{scope_table}.id"),
            nullable=False,
        ),
        sa.Column("revision", sa.Integer(), nullable=False),
        sa.Column("schema_version", sa.String(32), nullable=False),
        sa.Column("status", sa.String(32), nullable=False),
        sa.Column("document", sa.JSON(), nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True)),
        sa.Column("updated_at", sa.DateTime(timezone=True)),
    )


def upgrade() -> None:
    _ledger("asset_bible_records", "project_id", "projects")
    _ledger("workflow_runs", "project_id", "projects")
    _ledger("provider_catalog_records", "project_id", "projects")
    _ledger("timeline_cuts", "episode_id", "episodes")
    _ledger("timeline_versions", "episode_id", "episodes")
    _ledger("asset_edit_records", "project_id", "projects")
    _ledger("export_jobs", "episode_id", "episodes")
    _ledger("operation_evidence", "project_id", "projects")


def downgrade() -> None:
    for name in (
        "operation_evidence",
        "export_jobs",
        "asset_edit_records",
        "timeline_versions",
        "timeline_cuts",
        "provider_catalog_records",
        "workflow_runs",
        "asset_bible_records",
    ):
        op.drop_table(name)
