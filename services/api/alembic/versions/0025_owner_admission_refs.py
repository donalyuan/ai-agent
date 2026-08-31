"""Persist frozen resource admission on existing owner operations.

Revision ID: 0025_owner_admission_refs
Revises: 0024_provider_outbound_corr
"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0025_owner_admission_refs"
down_revision = "0024_provider_outbound_corr"
branch_labels = None
depends_on = None


def upgrade() -> None:
    with op.batch_alter_table("provider_calls") as batch:
        batch.add_column(sa.Column("admission_refs", sa.JSON()))
    with op.batch_alter_table("workflow_node_runs") as batch:
        batch.add_column(sa.Column("admission_refs", sa.JSON()))
    with op.batch_alter_table("video_operations") as batch:
        batch.add_column(sa.Column("admission_refs", sa.JSON()))


def downgrade() -> None:
    with op.batch_alter_table("video_operations") as batch:
        batch.drop_column("admission_refs")
    with op.batch_alter_table("workflow_node_runs") as batch:
        batch.drop_column("admission_refs")
    with op.batch_alter_table("provider_calls") as batch:
        batch.drop_column("admission_refs")
