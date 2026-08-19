from __future__ import annotations

from pathlib import Path

import pytest
from alembic.config import Config
from sqlalchemy import create_engine, inspect, text
from sqlalchemy.exc import IntegrityError

from alembic import command

API_ROOT = Path(__file__).parents[1]


def _alembic_config(database_url: str) -> Config:
    config = Config(str(API_ROOT / "alembic.ini"))
    config.set_main_option("script_location", str(API_ROOT / "alembic"))
    config.set_main_option("sqlalchemy.url", database_url)
    return config


def test_projects_episodes_migration_cycle_and_constraints(tmp_path: Path, monkeypatch) -> None:
    monkeypatch.delenv("MIGRATION_DATABASE_URL", raising=False)
    database_url = f"sqlite:///{tmp_path / 'migration.db'}"
    config = _alembic_config(database_url)

    command.upgrade(config, "head")
    engine = create_engine(database_url)
    try:
        with engine.begin() as connection:
            connection.execute(
                text(
                    "INSERT INTO projects "
                    "(id, revision, schema_version, name, status) "
                    "VALUES ('project-1', 1, '1.0.0', 'Demo', 'draft')"
                )
            )
            connection.execute(
                text(
                    "INSERT INTO episodes "
                    "(id, revision, schema_version, project_id, display_number, status, title) "
                    "VALUES ('episode-1', 1, '1.0.0', 'project-1', 1, 'draft', 'Opening')"
                )
            )
            with pytest.raises(IntegrityError):
                connection.execute(
                    text(
                        "INSERT INTO episodes "
                        "(id, revision, schema_version, project_id, display_number, status, title) "
                        "VALUES ('episode-duplicate', 1, '1.0.0', 'project-1', 1, "
                        "'draft', 'Duplicate')"
                    )
                )
            with pytest.raises(IntegrityError):
                connection.execute(
                    text(
                        "INSERT INTO episodes "
                        "(id, revision, schema_version, project_id, display_number, status, title) "
                        "VALUES ('episode-invalid', 1, '1.0.0', 'project-1', 0, 'draft', 'Invalid')"
                    )
                )
        assert "title" in {column["name"] for column in inspect(engine).get_columns("episodes")}
    finally:
        engine.dispose()

    command.downgrade(config, "0002_version_contract_alignment")
    engine = create_engine(database_url)
    try:
        assert "title" not in {column["name"] for column in inspect(engine).get_columns("episodes")}
    finally:
        engine.dispose()

    command.upgrade(config, "head")
    engine = create_engine(database_url)
    try:
        assert "title" in {column["name"] for column in inspect(engine).get_columns("episodes")}
    finally:
        engine.dispose()
