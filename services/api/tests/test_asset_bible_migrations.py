from __future__ import annotations

import json
from pathlib import Path
from uuid import uuid4

import pytest
from alembic.config import Config
from sqlalchemy import create_engine, inspect, text
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from alembic import command
from video_agent_api.adapters.sqlalchemy import _encode_phase_one, make_sqlalchemy_uow_factory
from video_agent_api.application.asset_bible import (
    AcceptImpactCommand,
    AssetBibleService,
    AssignContinuityCommand,
    CreateEntryCommand,
    PreviewImpactCommand,
    UpdateEntryCommand,
)
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.scenes import CreateSceneCommand, CreateShotCommand, ScenesService
from video_agent_api.domain.asset_bible import AssetBible, AssetBibleEntry
from video_agent_api.domain.errors import RevisionConflictError

API_ROOT = Path(__file__).parents[1]
ASSET_BIBLE_MIGRATION = "0014_asset_bible_owner"
CURRENT_HEAD = "0023_export_dispatch_owner"
ACTOR_UUID = "00000000-0000-4000-8000-000000000701"
ASSET_BIBLE_TABLES = {
    "asset_bibles",
    "asset_bible_entries",
    "asset_bible_entry_versions",
    "asset_bible_relationships",
    "asset_bible_assignments",
    "resolved_continuity_snapshots",
    "continuity_impact_analyses",
    "asset_bible_accept_decisions",
    "continuity_revision_tasks",
    "asset_bible_handoff_acks",
}
ASSET_BIBLE_DOCUMENT_COLLECTIONS = {
    "asset_bible_entries",
    "asset_bibles_by_project",
    "asset_bible_by_project",
    "asset_bible_assignments",
    "asset_bible_relationships",
    "asset_bible_snapshots",
    "asset_bible_tasks",
    "asset_bible_impacts",
    "asset_bible_impact_payloads",
    "asset_bible_decisions",
    "asset_bible_handoff_acks",
}


def _config(database_url: str) -> Config:
    config = Config(str(API_ROOT / "alembic.ini"))
    config.set_main_option("script_location", str(API_ROOT / "alembic"))
    config.set_main_option("sqlalchemy.url", database_url)
    return config


def test_asset_bible_migration_cycle_and_constraints(tmp_path: Path) -> None:
    database_url = f"sqlite:///{tmp_path / 'asset-bible-owner.db'}"
    config = _config(database_url)
    command.upgrade(config, "head")
    engine = create_engine(database_url)
    assert ASSET_BIBLE_TABLES <= set(inspect(engine).get_table_names())
    with engine.begin() as connection:
        connection.execute(text("PRAGMA foreign_keys = ON"))
        connection.execute(
            text(
                "INSERT INTO projects (id, revision, schema_version, name, status) "
                "VALUES ('project-ab', 1, '1.0.0', 'Asset Bible', 'draft')"
            )
        )
        connection.execute(
            text(
                "INSERT INTO asset_bibles "
                "(id, revision, schema_version, project_id, current_version_map) "
                "VALUES ('bible-ab', 1, '1.0.0', 'project-ab', '{}')"
            )
        )
        connection.execute(
            text(
                "INSERT INTO asset_bible_entries "
                "(id, revision, schema_version, asset_bible_id, project_id, entry_type, "
                "disabled, current_version_id) VALUES "
                "('entry-ab', 1, '1.0.0', 'bible-ab', 'project-ab', 'prop', 0, NULL)"
            )
        )
        valid_version = {
            "id": "version-ab",
            "revision": 1,
            "schema_version": "1.0.0",
            "entry_id": "entry-ab",
            "project_id": "project-ab",
            "entry_type": "prop",
            "payload": '{"name":"key"}',
            "version_number": 1,
            "actor_uuid": ACTOR_UUID,
            "reference_asset_version_refs": "[]",
            "generation_spec_refs": "[]",
            "content_hash": "a" * 64,
        }
        columns = ", ".join(valid_version)
        values = ", ".join(f":{key}" for key in valid_version)
        connection.execute(
            text(f"INSERT INTO asset_bible_entry_versions ({columns}) VALUES ({values})"),
            valid_version,
        )
        with pytest.raises(IntegrityError):
            connection.execute(
                text(f"INSERT INTO asset_bible_entry_versions ({columns}) VALUES ({values})"),
                {**valid_version, "id": "version-bad-hash", "content_hash": "g" * 64},
            )
        with pytest.raises(IntegrityError):
            connection.execute(
                text(f"INSERT INTO asset_bible_entry_versions ({columns}) VALUES ({values})"),
                {**valid_version, "id": "version-bad-revision", "revision": 2},
            )
        with pytest.raises(IntegrityError):
            connection.execute(
                text(
                    "INSERT INTO asset_bible_relationships "
                    "(id, schema_version, project_id, source_entry_id, target_entry_id, kind) "
                    "VALUES ('rel-self', '1.0.0', 'project-ab', 'entry-ab', 'entry-ab', 'related')"
                )
            )
    with engine.connect() as connection:
        assert (
            connection.execute(text("SELECT version_num FROM alembic_version")).scalar_one()
            == CURRENT_HEAD
        )
    command.downgrade(config, "0013_scenes_owner_repair")
    assert ASSET_BIBLE_TABLES.isdisjoint(inspect(engine).get_table_names())
    command.upgrade(config, "head")
    assert ASSET_BIBLE_TABLES <= set(inspect(engine).get_table_names())
    engine.dispose()


async def test_asset_bible_relational_uow_round_trip_and_document_boundary(
    tmp_path: Path,
) -> None:
    sync_url = f"sqlite:///{tmp_path / 'asset-bible-round-trip.db'}"
    command.upgrade(_config(sync_url), "head")
    async_engine = create_async_engine(sync_url.replace("sqlite://", "sqlite+aiosqlite://"))
    factory = make_sqlalchemy_uow_factory(async_sessionmaker(async_engine, expire_on_commit=False))
    projects = ProjectsEpisodesService(factory)
    scenes = ScenesService(factory)
    bible = AssetBibleService(factory)
    project = await projects.create_project("P")
    episode = await projects.create_episode((project.id, "E", 1))
    scene = await scenes.create_scene(CreateSceneCommand(project.id, episode.id))
    shot = await scenes.create_shot(CreateShotCommand(project.id, episode.id, scene.id))
    entry = await bible.create_entry(CreateEntryCommand(project.id, "prop"))
    version = await bible.update_entry(
        UpdateEntryCommand(project.id, entry.id, {"name": "key"}, entry.revision, ACTOR_UUID)
    )
    await bible.assign(
        AssignContinuityCommand(
            project.id,
            "shot",
            shot.id,
            entry.id,
            version.id,
            version.revision,
            version.content_hash,
            shot.revision,
        )
    )
    snapshot = await bible.resolve(project.id, shot.id)
    reloaded_entry = await bible.get_entry(project.id, entry.id)
    analysis = await bible.preview_impact(
        PreviewImpactCommand(
            project.id,
            entry.id,
            reloaded_entry.revision,
            {"name": "new key"},
            ACTOR_UUID,
        )
    )
    async with factory() as uow:
        aggregate_revision = uow.asset_bibles_by_project[project.id].revision
    decision, successor, tasks = await bible.accept_impact(
        AcceptImpactCommand(
            project_id=project.id,
            entry_id=entry.id,
            analysis_id=analysis.id,
            expected_analysis_revision=analysis.revision,
            expected_entry_revision=reloaded_entry.revision,
            expected_asset_bible_revision=aggregate_revision,
            candidate_payload_hash=analysis.candidate_payload_hash,
            target_refs=analysis.target_refs,
            target_set_hash=analysis.target_set_hash,
            actor_uuid=ACTOR_UUID,
            correlation_id="round-trip",
        )
    )
    assert decision.new_version_id == successor.id
    assert len(tasks) == 1 and tasks[0].snapshot_id == snapshot.id
    assert (await bible.get_entry(project.id, entry.id)).current == successor
    assert (await bible.get_snapshot(project.id, snapshot.id)).content_hash == snapshot.content_hash
    assert (await bible.list_tasks(project.id))[0].status == "pending"
    projection = await bible.consumer_projection(project.id, snapshot.id)
    assert projection["snapshotRef"] == {
        "id": snapshot.id,
        "revision": snapshot.revision,
        "hash": snapshot.content_hash,
    }
    await async_engine.dispose()

    engine = create_engine(sync_url)
    with engine.connect() as connection:
        counts = {
            table: connection.execute(text(f"SELECT COUNT(*) FROM {table}")).scalar_one()
            for table in ASSET_BIBLE_TABLES
        }
        collections = set(
            connection.execute(
                text("SELECT collection FROM phase_one_documents WHERE owner = 'phase-one'")
            ).scalars()
        )
    assert counts["asset_bibles"] == 1
    assert counts["asset_bible_entries"] == 1
    assert counts["asset_bible_entry_versions"] == 2
    assert counts["continuity_impact_analyses"] == 1
    assert counts["asset_bible_accept_decisions"] == 1
    assert counts["continuity_revision_tasks"] == 1
    assert collections.isdisjoint(ASSET_BIBLE_DOCUMENT_COLLECTIONS)
    engine.dispose()


async def test_asset_bible_row_level_cas_rejects_second_concurrent_uow(tmp_path: Path) -> None:
    sync_url = f"sqlite:///{tmp_path / 'asset-bible-cas.db'}"
    command.upgrade(_config(sync_url), "head")
    async_engine = create_async_engine(sync_url.replace("sqlite://", "sqlite+aiosqlite://"))
    factory = make_sqlalchemy_uow_factory(async_sessionmaker(async_engine, expire_on_commit=False))
    projects = ProjectsEpisodesService(factory)
    bible_service = AssetBibleService(factory)
    project = await projects.create_project("CAS")
    entry = await bible_service.create_entry(CreateEntryCommand(project.id, "prop"))
    await bible_service.update_entry(
        UpdateEntryCommand(project.id, entry.id, {"name": "key"}, 1, ACTOR_UUID)
    )

    first = factory()
    second = factory()
    await first.__aenter__()
    await second.__aenter__()
    try:
        for uow, name in ((first, "first"), (second, "second")):
            loaded_entry = uow.asset_bible_entries[entry.id]
            aggregate = uow.asset_bibles_by_project[project.id]
            successor = loaded_entry.successor({"name": name}, loaded_entry.revision, ACTOR_UUID)
            aggregate.set_current(loaded_entry.id, successor.id, aggregate.revision)
        await first.commit()
        with pytest.raises(RevisionConflictError):
            await second.commit()
        await second.rollback()
    finally:
        await first.__aexit__(None, None, None)
        await second.__aexit__(None, None, None)
    loaded = await bible_service.get_entry(project.id, entry.id)
    assert loaded.current is not None and loaded.current.payload == {"name": "first"}
    assert len(loaded.versions) == 2
    await async_engine.dispose()


async def test_asset_bible_legacy_document_fallback_is_migrated_once(tmp_path: Path) -> None:
    sync_url = f"sqlite:///{tmp_path / 'asset-bible-legacy-document.db'}"
    config = _config(sync_url)
    command.upgrade(config, "0013_scenes_owner_repair")
    project_id = str(uuid4())
    bible = AssetBible(project_id)
    entry = AssetBibleEntry(project_id, bible.id, "prop")
    version = entry.successor({"name": "legacy key"}, 1, ACTOR_UUID)
    bible.set_current(entry.id, version.id, bible.revision)
    legacy = {
        "asset_bible_entries": {entry.id: entry},
        "asset_bibles_by_project": {project_id: bible},
        "asset_bible_by_project": {project_id: [entry]},
        "asset_bible_assignments": [],
        "asset_bible_relationships": [],
        "asset_bible_snapshots": {},
        "asset_bible_tasks": {},
        "asset_bible_impacts": {},
        "asset_bible_impact_payloads": {},
        "asset_bible_decisions": {},
        "asset_bible_handoff_acks": {},
    }
    engine = create_engine(sync_url)
    with engine.begin() as connection:
        connection.execute(
            text(
                "INSERT INTO projects (id, revision, schema_version, name, status) "
                "VALUES (:id, 1, '1.0.0', 'Legacy', 'draft')"
            ),
            {"id": project_id},
        )
        for collection, value in legacy.items():
            connection.execute(
                text(
                    "INSERT INTO phase_one_documents "
                    "(id, owner, collection, revision, document, retention_policy, "
                    "retention_version, hold) VALUES "
                    "(:id, 'phase-one', :collection, 0, :document, 'phase-one', '1', 0)"
                ),
                {
                    "id": str(uuid4()),
                    "collection": collection,
                    "document": json.dumps({"payload": _encode_phase_one(value)}),
                },
            )
    engine.dispose()
    command.upgrade(config, "head")

    async_engine = create_async_engine(sync_url.replace("sqlite://", "sqlite+aiosqlite://"))
    factory = make_sqlalchemy_uow_factory(async_sessionmaker(async_engine, expire_on_commit=False))
    async with factory() as uow:
        assert uow.asset_bible_entries[entry.id].current is not None
        await uow.commit()
    await async_engine.dispose()

    engine = create_engine(sync_url)
    with engine.connect() as connection:
        assert connection.execute(text("SELECT COUNT(*) FROM asset_bibles")).scalar_one() == 1
        assert (
            connection.execute(text("SELECT COUNT(*) FROM asset_bible_entries")).scalar_one() == 1
        )
        collections = set(
            connection.execute(
                text("SELECT collection FROM phase_one_documents WHERE owner = 'phase-one'")
            ).scalars()
        )
        assert collections.isdisjoint(ASSET_BIBLE_DOCUMENT_COLLECTIONS)
    engine.dispose()
