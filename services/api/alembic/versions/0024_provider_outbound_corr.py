"""Persist provider outbound correlation without changing owner states.

Revision ID: 0024_provider_outbound_corr
Revises: 0023_export_dispatch_owner
"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0024_provider_outbound_corr"
down_revision = "0023_export_dispatch_owner"
branch_labels = None
depends_on = None


def upgrade() -> None:
    with op.batch_alter_table("provider_calls") as batch:
        batch.add_column(sa.Column("outbound_correlation", sa.String(128)))
        batch.add_column(
            sa.Column(
                "lookup_outcome",
                sa.String(32),
                nullable=False,
                server_default="not_attempted",
            )
        )
    with op.batch_alter_table("video_operations") as batch:
        batch.add_column(sa.Column("outbound_correlation", sa.String(128)))
        batch.add_column(
            sa.Column(
                "lookup_outcome",
                sa.String(32),
                nullable=False,
                server_default="not_attempted",
            )
        )


def downgrade() -> None:
    with op.batch_alter_table("video_operations") as batch:
        batch.drop_column("lookup_outcome")
        batch.drop_column("outbound_correlation")
    with op.batch_alter_table("provider_calls") as batch:
        batch.drop_column("lookup_outcome")
        batch.drop_column("outbound_correlation")
