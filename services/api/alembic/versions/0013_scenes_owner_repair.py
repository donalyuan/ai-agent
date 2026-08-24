"""complete relational Scene/Shot owner persistence

Revision ID: 0013_scenes_owner_repair
Revises: 0012_text_review_owner
"""

from __future__ import annotations

import sqlalchemy as sa
from sqlalchemy.dialects import postgresql

from alembic import op

revision = "0013_scenes_owner_repair"
down_revision = "0012_text_review_owner"
branch_labels = None
depends_on = None

JSON_DOCUMENT = sa.JSON().with_variant(postgresql.JSONB(), "postgresql")


def _add_owner_columns() -> None:
    op.add_column("scenes", sa.Column("project_id", sa.String(36), nullable=True))
    op.add_column("scenes", sa.Column("title", sa.String(255), nullable=True))
    op.add_column("scenes", sa.Column("spec_ref", JSON_DOCUMENT, nullable=True))
    op.add_column(
        "scenes",
        sa.Column("spec_versions", JSON_DOCUMENT, nullable=False, server_default=sa.text("'[]'")),
    )
    op.execute(
        sa.text(
            "UPDATE scenes SET project_id = ("
            "SELECT episodes.project_id FROM episodes WHERE episodes.id = scenes.episode_id"
            "), title = 'Scene ' || CAST(display_number AS VARCHAR(32)) "
            "WHERE project_id IS NULL OR title IS NULL"
        )
    )

    op.add_column("shots", sa.Column("project_id", sa.String(36), nullable=True))
    op.add_column("shots", sa.Column("episode_id", sa.String(36), nullable=True))
    op.add_column("shots", sa.Column("spec_ref", JSON_DOCUMENT, nullable=True))
    op.add_column(
        "shots",
        sa.Column("spec_versions", JSON_DOCUMENT, nullable=False, server_default=sa.text("'[]'")),
    )
    op.add_column("shots", sa.Column("continuity_snapshot", JSON_DOCUMENT, nullable=True))
    op.add_column(
        "shots",
        sa.Column(
            "continuity_task_refs",
            JSON_DOCUMENT,
            nullable=False,
            server_default=sa.text("'[]'"),
        ),
    )
    op.add_column("shots", sa.Column("current_image", JSON_DOCUMENT, nullable=True))
    op.execute(
        sa.text(
            "UPDATE shots SET project_id = ("
            "SELECT scenes.project_id FROM scenes WHERE scenes.id = shots.scene_id"
            "), episode_id = ("
            "SELECT scenes.episode_id FROM scenes WHERE scenes.id = shots.scene_id"
            ") WHERE project_id IS NULL OR episode_id IS NULL"
        )
    )


def _create_constraints() -> None:
    sqlite = op.get_context().dialect.name == "sqlite"
    if sqlite:
        with op.batch_alter_table("episodes", recreate="always") as batch:
            batch.create_unique_constraint("uq_episodes_id_project", ["id", "project_id"])
        with op.batch_alter_table("scenes", recreate="always") as batch:
            batch.alter_column("project_id", existing_type=sa.String(36), nullable=False)
            batch.alter_column("title", existing_type=sa.String(255), nullable=False)
            batch.create_unique_constraint(
                "uq_scenes_id_project_episode", ["id", "project_id", "episode_id"]
            )
            batch.create_check_constraint("ck_scenes_display_number_positive", "display_number > 0")
            batch.create_check_constraint("ck_scenes_title_nonblank", "length(trim(title)) > 0")
            batch.create_foreign_key("fk_scenes_project_id", "projects", ["project_id"], ["id"])
            batch.create_foreign_key(
                "fk_scenes_episode_project",
                "episodes",
                ["episode_id", "project_id"],
                ["id", "project_id"],
            )
        with op.batch_alter_table("shots", recreate="always") as batch:
            batch.alter_column("project_id", existing_type=sa.String(36), nullable=False)
            batch.alter_column("episode_id", existing_type=sa.String(36), nullable=False)
            batch.create_check_constraint("ck_shots_display_number_positive", "display_number > 0")
            batch.create_foreign_key("fk_shots_project_id", "projects", ["project_id"], ["id"])
            batch.create_foreign_key("fk_shots_episode_id", "episodes", ["episode_id"], ["id"])
            batch.create_foreign_key(
                "fk_shots_scene_project_episode",
                "scenes",
                ["scene_id", "project_id", "episode_id"],
                ["id", "project_id", "episode_id"],
            )
    else:
        op.alter_column("scenes", "project_id", existing_type=sa.String(36), nullable=False)
        op.alter_column("scenes", "title", existing_type=sa.String(255), nullable=False)
        op.alter_column("shots", "project_id", existing_type=sa.String(36), nullable=False)
        op.alter_column("shots", "episode_id", existing_type=sa.String(36), nullable=False)
        op.create_unique_constraint("uq_episodes_id_project", "episodes", ["id", "project_id"])
        op.create_unique_constraint(
            "uq_scenes_id_project_episode", "scenes", ["id", "project_id", "episode_id"]
        )
        op.create_check_constraint(
            "ck_scenes_display_number_positive", "scenes", "display_number > 0"
        )
        op.create_check_constraint("ck_scenes_title_nonblank", "scenes", "length(trim(title)) > 0")
        op.create_check_constraint(
            "ck_shots_display_number_positive", "shots", "display_number > 0"
        )
        op.create_foreign_key("fk_scenes_project_id", "scenes", "projects", ["project_id"], ["id"])
        op.create_foreign_key(
            "fk_scenes_episode_project",
            "scenes",
            "episodes",
            ["episode_id", "project_id"],
            ["id", "project_id"],
        )
        op.create_foreign_key("fk_shots_project_id", "shots", "projects", ["project_id"], ["id"])
        op.create_foreign_key("fk_shots_episode_id", "shots", "episodes", ["episode_id"], ["id"])
        op.create_foreign_key(
            "fk_shots_scene_project_episode",
            "shots",
            "scenes",
            ["scene_id", "project_id", "episode_id"],
            ["id", "project_id", "episode_id"],
        )


def upgrade() -> None:
    _add_owner_columns()
    _create_constraints()
    op.create_table(
        "scene_order_states",
        sa.Column("episode_id", sa.String(36), sa.ForeignKey("episodes.id"), primary_key=True),
        sa.Column("revision", sa.Integer(), nullable=False, server_default="1"),
        sa.CheckConstraint("revision >= 1", name="ck_scene_order_states_revision_positive"),
    )
    op.execute(
        sa.text(
            "INSERT INTO scene_order_states (episode_id, revision) "
            "SELECT DISTINCT episode_id, 1 FROM scenes"
        )
    )
    op.create_table(
        "scene_shot_handoff_acks",
        sa.Column("id", sa.String(36), primary_key=True),
        sa.Column("handoff_id", sa.String(255), nullable=False),
        sa.Column("project_id", sa.String(36), sa.ForeignKey("projects.id"), nullable=False),
        sa.Column("episode_id", sa.String(36), sa.ForeignKey("episodes.id"), nullable=False),
        sa.Column("payload_hash", sa.String(64), nullable=False),
        sa.Column("correlation_id", sa.String(255), nullable=False),
        sa.Column("scene_ids", JSON_DOCUMENT, nullable=False),
        sa.Column("shot_ids", JSON_DOCUMENT, nullable=False),
        sa.Column("created_at", sa.DateTime(timezone=True)),
        sa.UniqueConstraint("handoff_id", name="uq_scene_shot_handoff_ack_handoff"),
    )


def downgrade() -> None:
    op.drop_table("scene_shot_handoff_acks")
    op.drop_table("scene_order_states")
    sqlite = op.get_context().dialect.name == "sqlite"
    if sqlite:
        with op.batch_alter_table("shots", recreate="always") as batch:
            batch.drop_constraint("fk_shots_scene_project_episode", type_="foreignkey")
            batch.drop_constraint("fk_shots_episode_id", type_="foreignkey")
            batch.drop_constraint("fk_shots_project_id", type_="foreignkey")
            batch.drop_constraint("ck_shots_display_number_positive", type_="check")
            for column in (
                "current_image",
                "continuity_task_refs",
                "continuity_snapshot",
                "spec_versions",
                "spec_ref",
                "episode_id",
                "project_id",
            ):
                batch.drop_column(column)
        with op.batch_alter_table("scenes", recreate="always") as batch:
            batch.drop_constraint("fk_scenes_episode_project", type_="foreignkey")
            batch.drop_constraint("fk_scenes_project_id", type_="foreignkey")
            batch.drop_constraint("ck_scenes_title_nonblank", type_="check")
            batch.drop_constraint("ck_scenes_display_number_positive", type_="check")
            batch.drop_constraint("uq_scenes_id_project_episode", type_="unique")
            for column in ("spec_versions", "spec_ref", "title", "project_id"):
                batch.drop_column(column)
        with op.batch_alter_table("episodes", recreate="always") as batch:
            batch.drop_constraint("uq_episodes_id_project", type_="unique")
    else:
        for constraint in (
            "fk_shots_scene_project_episode",
            "fk_shots_episode_id",
            "fk_shots_project_id",
        ):
            op.drop_constraint(constraint, "shots", type_="foreignkey")
        op.drop_constraint("ck_shots_display_number_positive", "shots", type_="check")
        for constraint, kind in (
            ("fk_scenes_episode_project", "foreignkey"),
            ("fk_scenes_project_id", "foreignkey"),
            ("ck_scenes_title_nonblank", "check"),
            ("ck_scenes_display_number_positive", "check"),
            ("uq_scenes_id_project_episode", "unique"),
        ):
            op.drop_constraint(constraint, "scenes", type_=kind)
        op.drop_constraint("uq_episodes_id_project", "episodes", type_="unique")
        for column in (
            "current_image",
            "continuity_task_refs",
            "continuity_snapshot",
            "spec_versions",
            "spec_ref",
            "episode_id",
            "project_id",
        ):
            op.drop_column("shots", column)
        for column in ("spec_versions", "spec_ref", "title", "project_id"):
            op.drop_column("scenes", column)
