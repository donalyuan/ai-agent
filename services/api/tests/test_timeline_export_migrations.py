from __future__ import annotations

import json
from pathlib import Path

import pytest
from alembic.config import Config
from export_test_support import create_persisted_generated_export_asset
from sqlalchemy import create_engine, inspect, text
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from alembic import command
from video_agent_api.adapters.ffmpeg import MockFfmpegRenderAdapter
from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
from video_agent_api.application.exports import ExportService
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.timeline import TimelineService
from video_agent_api.domain.timeline import TimelineCut
from video_agent_api.ports.storage import LocalWorkspaceAdapter

API_ROOT = Path(__file__).parents[1]
HEAD = "0029_lookup_binding"
OWNER_TABLES = {
    "timeline_current_cuts",
    "timeline_clips",
    "timeline_sound_cues",
    "timeline_captions",
    "episode_timeline_versions",
    "media_inspections",
    "media_derivatives",
    "timeline_preview_artifacts",
    "episode_export_batches",
    "episode_export_members",
    "episode_export_jobs",
    "export_artifacts",
    "export_diagnostic_targets",
    "export_dispatch_outbox",
}


def _config(url: str) -> Config:
    config = Config(str(API_ROOT / "alembic.ini"))
    config.set_main_option("script_location", str(API_ROOT / "alembic"))
    config.set_main_option("sqlalchemy.url", url)
    return config


def test_timeline_export_migration_cycle_constraints_and_no_draft_table(tmp_path: Path) -> None:
    url = f"sqlite:///{tmp_path / 'timeline-export-cycle.db'}"
    config = _config(url)
    command.upgrade(config, "head")
    engine = create_engine(url)
    tables = set(inspect(engine).get_table_names())
    assert OWNER_TABLES <= tables
    assert "timeline_drafts" not in tables and "mutable_timeline_cuts" not in tables
    with engine.begin() as connection:
        connection.execute(text("PRAGMA foreign_keys = ON"))
        connection.execute(
            text(
                "INSERT INTO projects (id, revision, schema_version, name, status) "
                "VALUES ('project-tl', 1, '1.0.0', 'Timeline', 'draft')"
            )
        )
        connection.execute(
            text(
                "INSERT INTO episodes "
                "(id, project_id, display_number, title, revision, schema_version, status) "
                "VALUES ('episode-tl', 'project-tl', 1, 'E1', 1, '1.0.0', 'draft')"
            )
        )
        values = {
            "id": "cut-one",
            "project_id": "project-tl",
            "episode_id": "episode-tl",
            "revision": 1,
            "schema_version": "1.0.0",
            "payload": json.dumps({}),
        }
        connection.execute(
            text(
                "INSERT INTO timeline_current_cuts "
                "(id, project_id, episode_id, revision, schema_version, payload) "
                "VALUES (:id, :project_id, :episode_id, :revision, :schema_version, :payload)"
            ),
            values,
        )
        with pytest.raises(IntegrityError):
            connection.execute(
                text(
                    "INSERT INTO timeline_current_cuts "
                    "(id, project_id, episode_id, revision, schema_version, payload) "
                    "VALUES ('cut-two', :project_id, :episode_id, 1, '1.0.0', :payload)"
                ),
                values,
            )
    with engine.connect() as connection:
        assert (
            connection.execute(text("SELECT version_num FROM alembic_version")).scalar_one() == HEAD
        )
    command.downgrade(config, "0019_storage_owner")
    assert OWNER_TABLES.isdisjoint(inspect(engine).get_table_names())
    command.upgrade(config, "head")
    assert OWNER_TABLES <= set(inspect(engine).get_table_names())
    engine.dispose()


async def test_normalized_timeline_export_round_trip_and_document_boundary(
    tmp_path: Path,
) -> None:
    sync_url = f"sqlite:///{tmp_path / 'timeline-export-round-trip.db'}"
    command.upgrade(_config(sync_url), "head")
    async_engine = create_async_engine(sync_url.replace("sqlite://", "sqlite+aiosqlite://"))
    factory = make_sqlalchemy_uow_factory(async_sessionmaker(async_engine, expire_on_commit=False))
    projects = ProjectsEpisodesService(factory)
    timeline = TimelineService(factory)
    storage = LocalWorkspaceAdapter(tmp_path / "storage")
    exports = ExportService(factory, MockFfmpegRenderAdapter(), storage=storage)
    project = await projects.create_project("P")
    episode = await projects.create_episode((project.id, "E", 1))
    version = await create_persisted_generated_export_asset(
        factory, storage, project.id, episode.id, label="round-trip"
    )
    cut = TimelineCut(episode.id, project.id)
    cut.clips.append(
        {
            "id": "clip-round-trip",
            "assetVersionId": version.id,
            "assetVersionRevision": version.revision,
            "assetVersionHash": version.content_hash,
            "derivativeFingerprint": "c" * 64,
            "derivativeStatus": "ready",
            "inFrame": 0,
            "outFrame": 30,
            "durationFrames": 30,
            "timelineStart": 0,
            "transition": {"type": "cut", "durationFrames": 0},
        }
    )
    async with factory() as uow:
        uow.timeline_cuts[episode.id] = cut
        await uow.commit()
    timeline_version = await timeline.publish(episode.id, "Published", 1, project.id)
    batch = await exports.create_batch(
        project.id,
        [
            {
                "episodeId": episode.id,
                "timelineVersionId": timeline_version.id,
                "timelineVersionRevision": 1,
                "outputBaseName": "episode-01",
            }
        ],
        "light",
        "round-trip",
        storage_profile_id="local-test-offline",
        storage_profile_revision=1,
    )
    fresh_timeline = TimelineService(factory)
    fresh_exports = ExportService(factory, MockFfmpegRenderAdapter())
    assert (await fresh_timeline.get_cut(episode.id, project.id)).id == cut.id
    assert (
        await fresh_timeline.get_version(project.id, episode.id, timeline_version.id)
    ).name == "Published"
    assert (await fresh_exports.projection(project.id, batch.id))["id"] == batch.id
    await async_engine.dispose()

    engine = create_engine(sync_url)
    with engine.connect() as connection:
        counts = {
            table: connection.execute(text(f"SELECT COUNT(*) FROM {table}")).scalar_one()
            for table in OWNER_TABLES
        }
        collections = set(
            connection.execute(
                text("SELECT collection FROM phase_one_documents WHERE owner = 'phase-one'")
            ).scalars()
        )
    assert counts["timeline_current_cuts"] == 1
    assert counts["timeline_clips"] == 1
    assert counts["episode_timeline_versions"] == 1
    assert counts["episode_export_batches"] == 1
    assert counts["episode_export_members"] == 1
    assert counts["episode_export_jobs"] == 1
    assert counts["export_artifacts"] == 3
    assert counts["export_dispatch_outbox"] == 1
    assert collections.isdisjoint(
        {"timeline_cuts", "timeline_versions", "export_batches", "export_jobs"}
    )
    engine.dispose()
