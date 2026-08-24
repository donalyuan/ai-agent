"""add scene and shot owner tables"""

from __future__ import annotations

import sqlalchemy as sa

from alembic import op

revision = "0008_scenes_shots"
down_revision = "0007_projects_creative_owner"
branch_labels = None
depends_on = None


def upgrade() -> None:
    op.add_column("shots", sa.Column("current_video", sa.JSON(), nullable=True))
    if op.get_context().dialect.name == "sqlite":
        with op.batch_alter_table("scenes", recreate="always") as batch:
            batch.create_unique_constraint(
                "uq_scene_episode_number", ["episode_id", "display_number"]
            )
        with op.batch_alter_table("shots", recreate="always") as batch:
            batch.create_unique_constraint("uq_shot_scene_number", ["scene_id", "display_number"])
    else:
        op.create_unique_constraint(
            "uq_scene_episode_number", "scenes", ["episode_id", "display_number"]
        )
        op.create_unique_constraint("uq_shot_scene_number", "shots", ["scene_id", "display_number"])


def downgrade() -> None:
    if op.get_context().dialect.name == "sqlite":
        with op.batch_alter_table("shots", recreate="always") as batch:
            batch.drop_constraint("uq_shot_scene_number", type_="unique")
            batch.drop_column("current_video")
        with op.batch_alter_table("scenes", recreate="always") as batch:
            batch.drop_constraint("uq_scene_episode_number", type_="unique")
    else:
        op.drop_constraint("uq_shot_scene_number", "shots", type_="unique")
        op.drop_constraint("uq_scene_episode_number", "scenes", type_="unique")
        op.drop_column("shots", "current_video")
