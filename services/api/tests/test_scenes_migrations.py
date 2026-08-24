from __future__ import annotations

from pathlib import Path

from alembic.config import Config
from sqlalchemy import create_engine, inspect, text

from alembic import command

API_ROOT = Path(__file__).parents[1]
SCENES_MIGRATION = "0013_scenes_owner_repair"
CURRENT_HEAD = "0023_export_dispatch_owner"


def _config(database_url: str) -> Config:
    config = Config(str(API_ROOT / "alembic.ini"))
    config.set_main_option("script_location", str(API_ROOT / "alembic"))
    config.set_main_option("sqlalchemy.url", database_url)
    return config


def test_scenes_owner_migration_cycle_and_constraints(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'scenes-owner.db'}"
    config = _config(database_url)
    command.upgrade(config, "head")
    engine = create_engine(database_url)
    inspector = inspect(engine)
    assert {"scene_order_states", "scene_shot_handoff_acks"} <= set(inspector.get_table_names())
    assert {
        "project_id",
        "episode_id",
        "title",
        "spec_ref",
        "spec_versions",
    } <= {item["name"] for item in inspector.get_columns("scenes")}
    assert {
        "project_id",
        "episode_id",
        "spec_ref",
        "spec_versions",
        "continuity_snapshot",
        "continuity_task_refs",
        "current_image",
        "current_video",
    } <= {item["name"] for item in inspector.get_columns("shots")}
    scene_foreign_keys = {item["name"] for item in inspector.get_foreign_keys("scenes")}
    shot_foreign_keys = {item["name"] for item in inspector.get_foreign_keys("shots")}
    assert "fk_scenes_episode_project" in scene_foreign_keys
    assert "fk_shots_scene_project_episode" in shot_foreign_keys
    assert "uq_scene_episode_number" in {
        item["name"] for item in inspector.get_unique_constraints("scenes")
    }
    assert "uq_shot_scene_number" in {
        item["name"] for item in inspector.get_unique_constraints("shots")
    }
    with engine.connect() as connection:
        assert (
            connection.execute(text("SELECT version_num FROM alembic_version")).scalar_one()
            == CURRENT_HEAD
        )
    command.downgrade(config, "0012_text_review_owner")
    assert {"scene_order_states", "scene_shot_handoff_acks"}.isdisjoint(
        inspect(engine).get_table_names()
    )
    command.upgrade(config, "head")
    engine.dispose()
