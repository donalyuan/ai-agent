"""Persist the frozen Generation route on workflow node runs.

Revision ID: 0027_generation_route_snapshot
Revises: 0026_asset_reservation_admission
"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0027_generation_route_snapshot"
down_revision = "0026_asset_reservation_admission"
branch_labels = None
depends_on = None


def upgrade() -> None:
    with op.batch_alter_table("workflow_node_runs") as batch:
        batch.add_column(
            sa.Column(
                "execution_route", sa.String(length=32), nullable=False, server_default="legacy"
            )
        )
        batch.add_column(
            sa.Column(
                "workflow_type",
                sa.String(length=128),
                nullable=False,
                server_default="phase_one_run",
            )
        )
        batch.add_column(
            sa.Column(
                "task_queue", sa.String(length=128), nullable=False, server_default="agent-tasks"
            )
        )
        batch.add_column(sa.Column("operation_snapshot", sa.JSON(), nullable=True))
    with op.batch_alter_table("workflow_node_runs") as batch:
        batch.alter_column("execution_route", server_default=None)
        batch.alter_column("workflow_type", server_default=None)
        batch.alter_column("task_queue", server_default=None)


def downgrade() -> None:
    with op.batch_alter_table("workflow_node_runs") as batch:
        batch.drop_column("operation_snapshot")
        batch.drop_column("task_queue")
        batch.drop_column("workflow_type")
        batch.drop_column("execution_route")
