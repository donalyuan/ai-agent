from __future__ import annotations

from pathlib import Path

from alembic.config import Config
from alembic.script import ScriptDirectory
from sqlalchemy import create_engine, inspect, text

from alembic import command

API_ROOT = Path(__file__).parents[1]
TEXT_MIGRATION = "0012_text_review_owner"
CURRENT_HEAD = "0029_lookup_binding"


def _config(database_url: str) -> Config:
    config = Config(str(API_ROOT / "alembic.ini"))
    config.set_main_option("script_location", str(API_ROOT / "alembic"))
    config.set_main_option("sqlalchemy.url", database_url)
    return config


def test_text_owner_migration_cycle_and_table_boundaries(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'text-owner.db'}"
    config = _config(database_url)
    assert ScriptDirectory.from_config(config).get_current_head() == CURRENT_HEAD
    command.upgrade(config, "head")
    engine = create_engine(database_url)
    expected = {
        "skill_route_decisions",
        "skill_route_selections",
        "text_generation_candidates",
        "text_review_batches",
        "text_review_batch_members",
        "text_review_confirmations",
        "text_owner_handoffs",
        "text_owner_handoff_acks",
        "text_generation_audits",
    }
    assert expected <= set(inspect(engine).get_table_names())
    with engine.connect() as connection:
        assert (
            connection.execute(text("SELECT version_num FROM alembic_version")).scalar_one()
            == CURRENT_HEAD
        )
    command.downgrade(config, "0011_phase_one_documents")
    assert expected.isdisjoint(inspect(engine).get_table_names())
    assert {"projects", "episodes", "scenes", "shots"} <= set(inspect(engine).get_table_names())
    command.upgrade(config, "head")
    with engine.connect() as connection:
        assert (
            connection.execute(text("SELECT version_num FROM alembic_version")).scalar_one()
            == CURRENT_HEAD
        )
    engine.dispose()
