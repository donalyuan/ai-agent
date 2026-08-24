"""add durable phase-one owner document ledger"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0011_phase_one_documents"
down_revision = "0010_creative_owner_refs"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.create_table(
        "phase_one_documents",
        sa.Column("id", sa.String(length=36), primary_key=True),
        sa.Column("owner", sa.String(length=64), nullable=False),
        sa.Column("collection", sa.String(length=128), nullable=False),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="0"),
        sa.Column("document", sa.JSON(), nullable=False),
        sa.Column(
            "retention_policy", sa.String(length=64), nullable=False, server_default="phase-one"
        ),
        sa.Column("retention_version", sa.String(length=32), nullable=False, server_default="1"),
        sa.Column("hold", sa.Boolean(), nullable=False, server_default=sa.false()),
        sa.Column("created_at", sa.DateTime(timezone=True)),
        sa.Column("updated_at", sa.DateTime(timezone=True)),
        sa.UniqueConstraint("owner", "collection", name="uq_phase_one_document_owner_collection"),
    )


def downgrade() -> None:
    op.drop_table("phase_one_documents")
