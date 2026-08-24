"""Persist export execution snapshots and dispatch outbox.

Revision ID: 0023_export_dispatch_owner
Revises: 0022_asset_center_owner
"""

from __future__ import annotations

import sqlalchemy as sa
from sqlalchemy.dialects.postgresql import JSONB

from alembic import op

revision = "0023_export_dispatch_owner"
down_revision = "0022_asset_center_owner"
branch_labels = None
depends_on = None


def _json_type() -> sa.types.TypeEngine[object]:
    return sa.JSON().with_variant(JSONB(), "postgresql")


def upgrade() -> None:
    json_type = _json_type()
    with op.batch_alter_table("episode_export_jobs") as batch:
        batch.add_column(
            sa.Column(
                "execution_snapshot",
                json_type,
                nullable=False,
                server_default=sa.text("'{}'"),
            )
        )
    op.create_table(
        "export_dispatch_outbox",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("revision", sa.Integer(), nullable=False),
        sa.Column("schema_version", sa.String(32), nullable=False),
        sa.Column(
            "created_at", sa.DateTime(timezone=True), nullable=False, server_default=sa.func.now()
        ),
        sa.Column(
            "updated_at", sa.DateTime(timezone=True), nullable=False, server_default=sa.func.now()
        ),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column(
            "batch_id",
            sa.String(36),
            sa.ForeignKey("episode_export_batches.id"),
            nullable=False,
        ),
        sa.Column(
            "job_id",
            sa.String(36),
            sa.ForeignKey("episode_export_jobs.id"),
            nullable=False,
        ),
        sa.Column("logical_operation", sa.String(255), nullable=False),
        sa.Column("workflow_id", sa.String(255), nullable=False),
        sa.Column("status", sa.String(16), nullable=False),
        sa.Column("attempts", sa.Integer(), nullable=False, server_default="0"),
        sa.Column("last_error", sa.Text()),
        sa.Column("dispatched_at", sa.DateTime(timezone=True)),
        sa.Column("payload", json_type, nullable=False),
        sa.UniqueConstraint("job_id", name="uq_export_dispatch_job"),
        sa.UniqueConstraint("workflow_id", name="uq_export_dispatch_workflow"),
        sa.CheckConstraint("status IN ('pending','dispatched')", name="ck_export_dispatch_status"),
        sa.CheckConstraint("attempts >= 0", name="ck_export_dispatch_attempts"),
    )


def downgrade() -> None:
    op.drop_table("export_dispatch_outbox")
    with op.batch_alter_table("episode_export_jobs") as batch:
        batch.drop_column("execution_snapshot")
