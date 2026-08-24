from __future__ import annotations

import json
from pathlib import Path

import pytest
from alembic.config import Config
from alembic.script import ScriptDirectory
from sqlalchemy import create_engine, inspect, text
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import Session

from alembic import command
from video_agent_api.adapters.sqlalchemy import _version_from_model
from video_agent_api.adapters.sqlalchemy_models import AssetVersion as AssetVersionModel

API_ROOT = Path(__file__).parents[1]
CURRENT_HEAD_REVISION = "0023_export_dispatch_owner"
OBJECT_KEY_CORPUS = json.loads(
    (
        API_ROOT.parents[1] / "packages/contracts/tests/fixtures/object-key-contract-corpus.json"
    ).read_text(encoding="utf-8")
)


def _alembic_config(database_url: str) -> Config:
    config = Config(str(API_ROOT / "alembic.ini"))
    config.set_main_option("script_location", str(API_ROOT / "alembic"))
    config.set_main_option("sqlalchemy.url", database_url)
    return config


def test_assets_migration_cycle_and_columns(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'assets.db'}"
    config = _alembic_config(database_url)
    script_directory = ScriptDirectory.from_config(config)
    assert script_directory.get_current_head() == CURRENT_HEAD_REVISION
    assert len(script_directory.get_current_head()) <= 32

    command.upgrade(config, "head")
    engine = create_engine(database_url)
    with engine.connect() as connection:
        revision = connection.execute(text("SELECT version_num FROM alembic_version")).scalar_one()
    assert revision == CURRENT_HEAD_REVISION
    assert len(revision) <= 32
    columns = {column["name"] for column in inspect(engine).get_columns("asset_versions")}
    assert {"object_key", "mime_type", "size_bytes", "media_metadata"} <= columns
    asset_columns = {column["name"] for column in inspect(engine).get_columns("assets")}
    assert {
        "source_type",
        "catalog_role",
        "tags",
        "authorization_status",
        "copyright_owner",
        "license_label",
        "license_reference",
    } <= asset_columns
    assert "asset_version_reservations" in inspect(engine).get_table_names()
    reservation_columns = {
        column["name"] for column in inspect(engine).get_columns("asset_version_reservations")
    }
    assert "upload_key" in reservation_columns
    command.downgrade(config, "0003_projects_episodes_slice")
    columns = {column["name"] for column in inspect(engine).get_columns("asset_versions")}
    assert "object_key" not in columns
    engine.dispose()


def _insert_legacy_asset_version(
    database_url: str,
    *,
    checksum: str | None,
    storage_ref: str = "projects/legacy/original.mp4",
) -> None:
    engine = create_engine(database_url)
    try:
        with engine.begin() as connection:
            connection.execute(
                text(
                    "INSERT INTO projects "
                    "(id, revision, schema_version, name, status) "
                    "VALUES ('project-legacy', 1, '1.0.0', 'Legacy', 'draft')"
                )
            )
            connection.execute(
                text(
                    "INSERT INTO assets "
                    "(id, revision, schema_version, project_id, kind, status) "
                    "VALUES ('asset-legacy', 1, '1.0.0', 'project-legacy', 'video', 'draft')"
                )
            )
            connection.execute(
                text(
                    "INSERT INTO asset_versions "
                    "(id, asset_id, version_number, revision, status, schema_version, "
                    "storage_ref, checksum, metadata_json) "
                    "VALUES ('version-legacy', 'asset-legacy', 1, 0, 'draft', '1.0.0', "
                    ":storage_ref, :checksum, '{}')"
                ),
                {"checksum": checksum, "storage_ref": storage_ref},
            )
    finally:
        engine.dispose()


def test_asset_center_migration_backfills_metadata_without_changing_versions(
    tmp_path: Path,
) -> None:
    database_url = f"sqlite:///{tmp_path / 'asset-center-backfill.db'}"
    config = _alembic_config(database_url)
    command.upgrade(config, "0003_projects_episodes_slice")
    checksum = "a" * 64
    _insert_legacy_asset_version(database_url, checksum=checksum)
    command.upgrade(config, "0021_catalog_owner_column_repair")

    engine = create_engine(database_url)
    try:
        with engine.connect() as connection:
            before = (
                connection.execute(text("SELECT * FROM asset_versions WHERE id = 'version-legacy'"))
                .mappings()
                .one()
            )
        command.upgrade(config, "head")
        with engine.connect() as connection:
            metadata = connection.execute(
                text(
                    "SELECT source_type, tags, authorization_status "
                    "FROM assets WHERE id = 'asset-legacy'"
                )
            ).one()
            after = (
                connection.execute(text("SELECT * FROM asset_versions WHERE id = 'version-legacy'"))
                .mappings()
                .one()
            )
        assert metadata.source_type == "imported"
        assert json.loads(metadata.tags) == []
        assert metadata.authorization_status == "unknown"
        assert dict(after) == dict(before)

        command.downgrade(config, "0021_catalog_owner_column_repair")
        asset_columns = {column["name"] for column in inspect(engine).get_columns("assets")}
        assert "source_type" not in asset_columns
        assert "asset_version_reservations" not in inspect(engine).get_table_names()
    finally:
        engine.dispose()


def test_assets_migration_backfills_legacy_rows_and_preserves_hash(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'legacy.db'}"
    config = _alembic_config(database_url)
    command.upgrade(config, "0003_projects_episodes_slice")
    checksum = "a" * 64
    _insert_legacy_asset_version(database_url, checksum=checksum)

    command.upgrade(config, "head")
    engine = create_engine(database_url)
    try:
        with engine.connect() as connection:
            row = connection.execute(
                text(
                    "SELECT project_id, content_hash, checksum, storage_provider, object_key "
                    "FROM asset_versions WHERE id = 'version-legacy'"
                )
            ).one()
        assert row.project_id == "project-legacy"
        assert row.content_hash == checksum
        assert row.checksum == checksum
        assert row.storage_provider == "legacy"
        assert row.object_key == "projects/legacy/original.mp4"
    finally:
        engine.dispose()


def test_assets_migration_normalizes_local_workspace_reference(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'workspace-reference.db'}"
    config = _alembic_config(database_url)
    command.upgrade(config, "0003_projects_episodes_slice")
    _insert_legacy_asset_version(
        database_url,
        checksum="a" * 64,
        storage_ref=OBJECT_KEY_CORPUS["validWorkspaceReferences"][0],
    )

    command.upgrade(config, "head")
    engine = create_engine(database_url)
    try:
        with engine.connect() as connection:
            row = connection.execute(
                text(
                    "SELECT storage_provider, bucket, object_key "
                    "FROM asset_versions WHERE id = 'version-legacy'"
                )
            ).one()
        assert row.storage_provider == "local_workspace"
        assert row.bucket == "workspace"
        assert row.object_key == "projects/a/v1.mp4"
        with Session(engine) as session:
            model = session.get(AssetVersionModel, "version-legacy")
            assert model is not None
            loaded = _version_from_model(model)
        assert loaded.storage_object.object_key == "projects/a/v1.mp4"
    finally:
        engine.dispose()


def test_assets_0006_repairs_already_applied_workspace_reference(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'applied-workspace-reference.db'}"
    config = _alembic_config(database_url)
    command.upgrade(config, "0003_projects_episodes_slice")
    _insert_legacy_asset_version(
        database_url,
        checksum="a" * 64,
        storage_ref="workspace://projects/legacy/original.mp4",
    )
    command.upgrade(config, "0005_assets_integrity_repair")
    engine = create_engine(database_url)
    try:
        with engine.begin() as connection:
            connection.execute(
                text(
                    "UPDATE asset_versions SET storage_provider = 'legacy', "
                    "bucket = 'legacy', object_key = storage_ref "
                    "WHERE id = 'version-legacy'"
                )
            )

        command.upgrade(config, "head")
        with engine.connect() as connection:
            repaired = connection.execute(
                text(
                    "SELECT storage_provider, bucket, object_key "
                    "FROM asset_versions WHERE id = 'version-legacy'"
                )
            ).one()
        assert repaired == (
            "local_workspace",
            "workspace",
            "projects/legacy/original.mp4",
        )

        command.downgrade(config, "0005_assets_integrity_repair")
        with engine.connect() as connection:
            downgraded_key = connection.execute(
                text("SELECT object_key FROM asset_versions WHERE id = 'version-legacy'")
            ).scalar_one()
        assert downgraded_key == "workspace://projects/legacy/original.mp4"

        command.upgrade(config, "head")
        with engine.connect() as connection:
            upgraded_key = connection.execute(
                text("SELECT object_key FROM asset_versions WHERE id = 'version-legacy'")
            ).scalar_one()
        assert upgraded_key == "projects/legacy/original.mp4"
    finally:
        engine.dispose()


def test_assets_0006_keeps_existing_non_workspace_provider_and_bucket(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'ordinary-legacy-reference.db'}"
    config = _alembic_config(database_url)
    command.upgrade(config, "0003_projects_episodes_slice")
    _insert_legacy_asset_version(
        database_url,
        checksum="a" * 64,
        storage_ref=OBJECT_KEY_CORPUS["canonicalObjectKeys"][0],
    )
    command.upgrade(config, "0005_assets_integrity_repair")
    engine = create_engine(database_url)
    try:
        with engine.begin() as connection:
            connection.execute(
                text(
                    "UPDATE asset_versions SET storage_provider = 'archive', bucket = 'historical' "
                    "WHERE id = 'version-legacy'"
                )
            )

        command.upgrade(config, "head")
        with engine.connect() as connection:
            row = connection.execute(
                text(
                    "SELECT storage_provider, bucket, object_key "
                    "FROM asset_versions WHERE id = 'version-legacy'"
                )
            ).one()
        assert row == ("archive", "historical", OBJECT_KEY_CORPUS["canonicalObjectKeys"][0])
    finally:
        engine.dispose()


@pytest.mark.parametrize(
    "storage_ref",
    OBJECT_KEY_CORPUS["invalidObjectKeys"] + OBJECT_KEY_CORPUS["invalidLegacyReferences"],
)
def test_assets_migration_rejects_unsafe_legacy_reference(tmp_path: Path, storage_ref: str) -> None:
    database_url = f"sqlite:///{tmp_path / 'unsafe-reference.db'}"
    config = _alembic_config(database_url)
    command.upgrade(config, "0003_projects_episodes_slice")
    _insert_legacy_asset_version(
        database_url,
        checksum="a" * 64,
        storage_ref=storage_ref,
    )

    with pytest.raises(RuntimeError, match="object reference"):
        command.upgrade(config, "head")

    engine = create_engine(database_url)
    try:
        columns = {column["name"] for column in inspect(engine).get_columns("asset_versions")}
        assert "object_key" not in columns
    finally:
        engine.dispose()


@pytest.mark.parametrize(
    "storage_ref",
    OBJECT_KEY_CORPUS["invalidObjectKeys"] + OBJECT_KEY_CORPUS["invalidLegacyReferences"],
)
def test_assets_0006_rejects_unsafe_workspace_reference_before_repair(
    tmp_path: Path, storage_ref: str
) -> None:
    database_url = f"sqlite:///{tmp_path / 'unsafe-0006-reference.db'}"
    config = _alembic_config(database_url)
    command.upgrade(config, "0003_projects_episodes_slice")
    _insert_legacy_asset_version(
        database_url,
        checksum="a" * 64,
        storage_ref="workspace://projects/legacy/original.mp4",
    )
    command.upgrade(config, "0005_assets_integrity_repair")
    engine = create_engine(database_url)
    try:
        with engine.begin() as connection:
            connection.execute(
                text(
                    "UPDATE asset_versions SET storage_provider = 'legacy', bucket = 'legacy', "
                    "storage_ref = :storage_ref "
                    "WHERE id = 'version-legacy'"
                ),
                {"storage_ref": storage_ref},
            )

        with pytest.raises(RuntimeError, match="unsafe legacy object reference"):
            command.upgrade(config, "head")

        with engine.connect() as connection:
            revision = connection.execute(
                text("SELECT version_num FROM alembic_version")
            ).scalar_one()
            row = connection.execute(
                text(
                    "SELECT storage_provider, bucket, storage_ref, object_key "
                    "FROM asset_versions WHERE id = 'version-legacy'"
                )
            ).one()
        assert revision == "0005_assets_integrity_repair"
        assert row == ("legacy", "legacy", storage_ref, "projects/legacy/original.mp4")
    finally:
        engine.dispose()


def test_assets_migration_rejects_legacy_rows_without_checksum(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'missing-checksum.db'}"
    config = _alembic_config(database_url)
    command.upgrade(config, "0003_projects_episodes_slice")
    _insert_legacy_asset_version(database_url, checksum=None)

    with pytest.raises(RuntimeError, match="checksum"):
        command.upgrade(config, "head")

    engine = create_engine(database_url)
    try:
        columns = {column["name"] for column in inspect(engine).get_columns("asset_versions")}
        assert "content_hash" not in columns
    finally:
        engine.dispose()


def test_assets_migration_rejects_legacy_rows_with_non_hex_checksum(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'malformed-checksum.db'}"
    config = _alembic_config(database_url)
    command.upgrade(config, "0003_projects_episodes_slice")
    _insert_legacy_asset_version(database_url, checksum="g" * 64)

    with pytest.raises(RuntimeError, match="checksum"):
        command.upgrade(config, "head")

    engine = create_engine(database_url)
    try:
        columns = {column["name"] for column in inspect(engine).get_columns("asset_versions")}
        assert "content_hash" not in columns
    finally:
        engine.dispose()


def test_assets_migration_enforces_asset_and_version_constraints(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'constraints.db'}"
    config = _alembic_config(database_url)
    command.upgrade(config, "head")
    engine = create_engine(database_url)
    checksum = "b" * 64
    try:
        with engine.begin() as connection:
            connection.execute(text("PRAGMA foreign_keys = ON"))
            connection.execute(
                text(
                    "INSERT INTO projects "
                    "(id, revision, schema_version, name, status) "
                    "VALUES ('project-constraints', 1, '1.0.0', 'Demo', 'draft')"
                )
            )
            connection.execute(
                text(
                    "INSERT INTO assets "
                    "(id, revision, schema_version, project_id, kind, name, status) "
                    "VALUES ('asset-constraints', 1, '1.0.0', 'project-constraints', "
                    "'video', 'Video', 'draft')"
                )
            )
            valid = {
                "id": "version-constraints",
                "asset_id": "asset-constraints",
                "project_id": "project-constraints",
                "version_number": 1,
                "revision": 0,
                "status": "draft",
                "schema_version": "1.0.0",
                "storage_ref": "projects/constraints/original.mp4",
                "checksum": checksum,
                "content_hash": checksum,
                "storage_provider": "local",
                "bucket": "workspace",
                "object_key": "projects/constraints/original.mp4",
                "mime_type": "video/mp4",
                "size_bytes": 1,
                "metadata_json": "{}",
            }
            columns = ", ".join(valid)
            values = ", ".join(f":{key}" for key in valid)
            connection.execute(
                text(f"INSERT INTO asset_versions ({columns}) VALUES ({values})"), valid
            )
            for name, value in (("kind", "sound"), ("name", "   ")):
                with pytest.raises(IntegrityError):
                    connection.execute(
                        text(f"UPDATE assets SET {name} = :value WHERE id = 'asset-constraints'"),
                        {"value": value},
                    )
            for field, value in (("version_number", 0), ("size_bytes", -1)):
                with pytest.raises(IntegrityError):
                    connection.execute(
                        text(f"INSERT INTO asset_versions ({columns}) VALUES ({values})"),
                        {**valid, "id": f"invalid-{field}", field: value},
                    )
            with pytest.raises(IntegrityError):
                connection.execute(
                    text(f"INSERT INTO asset_versions ({columns}) VALUES ({values})"),
                    {**valid, "id": "invalid-project", "project_id": None},
                )
            with pytest.raises(IntegrityError):
                connection.execute(
                    text(f"INSERT INTO asset_versions ({columns}) VALUES ({values})"),
                    {
                        **valid,
                        "id": "invalid-cross-project",
                        "project_id": "project-other",
                    },
                )
            connection.execute(
                text(
                    "INSERT INTO projects "
                    "(id, revision, schema_version, name, status) "
                    "VALUES ('project-other', 1, '1.0.0', 'Other', 'draft')"
                )
            )
            with pytest.raises(IntegrityError):
                connection.execute(
                    text(f"INSERT INTO asset_versions ({columns}) VALUES ({values})"),
                    {
                        **valid,
                        "id": "invalid-cross-project-existing",
                        "project_id": "project-other",
                    },
                )
            with pytest.raises(IntegrityError):
                connection.execute(
                    text(f"INSERT INTO asset_versions ({columns}) VALUES ({values})"),
                    {**valid, "id": "invalid-checksum", "checksum": "g" * 64},
                )
            with pytest.raises(IntegrityError):
                connection.execute(
                    text(f"INSERT INTO asset_versions ({columns}) VALUES ({values})"),
                    {**valid, "id": "invalid-content-hash", "content_hash": "g" * 64},
                )
    finally:
        engine.dispose()
