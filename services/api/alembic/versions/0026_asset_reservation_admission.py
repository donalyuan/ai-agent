"""Persist frozen resource admission for Assets upload reservations.

Revision ID: 0026_asset_reservation_admission
Revises: 0025_owner_admission_refs
"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0026_asset_reservation_admission"
down_revision = "0025_owner_admission_refs"
branch_labels = None
depends_on = None


def upgrade() -> None:
    with op.batch_alter_table("asset_version_reservations") as batch:
        batch.add_column(sa.Column("admission_refs", sa.JSON()))


def downgrade() -> None:
    with op.batch_alter_table("asset_version_reservations") as batch:
        batch.drop_column("admission_refs")
