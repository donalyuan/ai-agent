"""persist Project StorySpec and Episode ScriptSpec owner references"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0010_creative_owner_refs"
down_revision = "0009_phase_one_owner_tables"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.add_column("projects", sa.Column("story_spec_ref", sa.JSON(), nullable=True))
    op.add_column(
        "projects", sa.Column("story_spec_history", sa.JSON(), nullable=False, server_default="[]")
    )
    op.add_column("episodes", sa.Column("script_spec_ref", sa.JSON(), nullable=True))
    op.add_column(
        "episodes", sa.Column("script_spec_history", sa.JSON(), nullable=False, server_default="[]")
    )
    op.add_column(
        "projects", sa.Column("source_materials", sa.JSON(), nullable=False, server_default="[]")
    )


def downgrade() -> None:
    op.drop_column("episodes", "script_spec_history")
    op.drop_column("episodes", "script_spec_ref")
    op.drop_column("projects", "story_spec_history")
    op.drop_column("projects", "story_spec_ref")
    op.drop_column("projects", "source_materials")
