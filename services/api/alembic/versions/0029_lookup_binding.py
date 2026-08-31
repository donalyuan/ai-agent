"""Persist the complete frozen remote lookup binding.

Revision ID: 0029_lookup_binding
Revises: 0028_provider_lookup_contract
"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0029_lookup_binding"
down_revision = "0028_provider_lookup_contract"
branch_labels = None
depends_on = None


def upgrade() -> None:
    with op.batch_alter_table("provider_calls") as batch:
        batch.add_column(sa.Column("remote_lookup_binding", sa.JSON(), nullable=True))


def downgrade() -> None:
    with op.batch_alter_table("provider_calls") as batch:
        batch.drop_column("remote_lookup_binding")
