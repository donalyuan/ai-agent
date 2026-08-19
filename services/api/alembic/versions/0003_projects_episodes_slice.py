"""add episode title and project-scoped positive numbering"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0003_projects_episodes_slice"
down_revision = "0002_version_contract_alignment"
branch_labels = None
depends_on = None


def upgrade() -> None:
    # Keep the first add nullable so existing rows can be deterministically backfilled.
    op.add_column("episodes", sa.Column("title", sa.String(length=255), nullable=True))
    op.execute(
        sa.text(
            "UPDATE episodes SET title = 'Episode ' || CAST(display_number AS VARCHAR(32)) "
            "WHERE title IS NULL"
        )
    )
    if op.get_context().dialect.name == "sqlite":
        with op.batch_alter_table("episodes", recreate="always") as batch:
            batch.alter_column("title", existing_type=sa.String(length=255), nullable=False)
            batch.create_check_constraint(
                "ck_episodes_display_number_positive", "display_number > 0"
            )
            batch.create_unique_constraint(
                "uq_episode_project_display_number", ["project_id", "display_number"]
            )
    else:
        op.alter_column("episodes", "title", existing_type=sa.String(length=255), nullable=False)
        op.create_check_constraint(
            "ck_episodes_display_number_positive", "episodes", "display_number > 0"
        )
        op.create_unique_constraint(
            "uq_episode_project_display_number", "episodes", ["project_id", "display_number"]
        )


def downgrade() -> None:
    if op.get_context().dialect.name == "sqlite":
        with op.batch_alter_table("episodes", recreate="always") as batch:
            batch.drop_constraint("uq_episode_project_display_number", type_="unique")
            batch.drop_constraint("ck_episodes_display_number_positive", type_="check")
            batch.drop_column("title")
    else:
        op.drop_constraint("uq_episode_project_display_number", "episodes", type_="unique")
        op.drop_constraint("ck_episodes_display_number_positive", "episodes", type_="check")
        op.drop_column("episodes", "title")
