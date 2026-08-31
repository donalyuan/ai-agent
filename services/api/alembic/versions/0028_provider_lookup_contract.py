"""Persist the frozen remote lookup protocol for ProviderCall reconciliation.

Revision ID: 0028_provider_lookup_contract
Revises: 0027_generation_route_snapshot
"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0028_provider_lookup_contract"
down_revision = "0027_generation_route_snapshot"
branch_labels = None
depends_on = None


def upgrade() -> None:
    with op.batch_alter_table("provider_calls") as batch:
        batch.add_column(sa.Column("remote_lookup_protocol", sa.String(length=128), nullable=True))


def downgrade() -> None:
    with op.batch_alter_table("provider_calls") as batch:
        batch.drop_column("remote_lookup_protocol")
