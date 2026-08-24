from __future__ import annotations

from pathlib import Path

import pytest
from export_test_support import create_persisted_generated_export_asset
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from video_agent_api.adapters.ffmpeg import MockFfmpegRenderAdapter
from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
from video_agent_api.adapters.sqlalchemy_models import Base
from video_agent_api.application.catalog import CatalogService, SetQuotaCommand
from video_agent_api.application.exports import ExportService
from video_agent_api.application.projects_episodes import (
    CreateEpisodeCommand,
    ProjectsEpisodesService,
)
from video_agent_api.application.scenes import CreateSceneCommand, ScenesService
from video_agent_api.application.timeline import TimelineService
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.domain.timeline import TimelineCut
from video_agent_api.ports.storage import LocalWorkspaceAdapter


@pytest.mark.asyncio
async def test_scene_spec_and_provider_admission_are_owner_facts() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    scenes = ScenesService(lambda: uow)
    catalog = CatalogService(lambda: uow)
    project = await projects.create_project("Phase one")
    episode = await projects.create_episode(CreateEpisodeCommand(project.id, "E1", 1))
    scene = await scenes.create_scene(CreateSceneCommand(project.id, episode.id))
    spec = await scenes.append_spec(project.id, episode.id, scene.id, {"beats": ["arrival"]})
    assert spec.project_id == project.id
    assert scene.spec_ref is not None
    await catalog.bootstrap()
    profile = next(iter(uow.profiles.values()))
    await catalog.update_operation_policy(
        profile.id,
        "image.generate",
        profile.revision,
        {"maxConcurrency": 1, "rateLimit": 1, "rateWindowSeconds": 60},
    )
    await catalog.admit_operation(profile.id, "image.generate", now=10)
    with pytest.raises(ValidationDomainError, match="concurrency"):
        await catalog.admit_operation(profile.id, "image.generate", now=11)
    await catalog.release_operation(profile.id, "image.generate")
    await catalog.set_quota(
        SetQuotaCommand(profile.id, "image.generate", "exhausted", 0, None, "local")
    )
    with pytest.raises(ValidationDomainError, match="quota"):
        await catalog.admit_operation(profile.id, "image.generate", now=12)


@pytest.mark.asyncio
async def test_export_batch_idempotency_and_normalized_owner_restart(tmp_path: Path) -> None:
    database = tmp_path / "phase-one.db"
    engine = create_async_engine(f"sqlite+aiosqlite:///{database}")
    async with engine.begin() as connection:
        await connection.run_sync(Base.metadata.create_all)
    factory = make_sqlalchemy_uow_factory(async_sessionmaker(engine, expire_on_commit=False))
    projects = ProjectsEpisodesService(factory)
    project = await projects.create_project("Durable")
    episode = await projects.create_episode(CreateEpisodeCommand(project.id, "E1", 1))
    storage = LocalWorkspaceAdapter(tmp_path / "storage")
    asset_version = await create_persisted_generated_export_asset(
        factory, storage, project.id, episode.id, label="phase-one-export"
    )
    timeline = TimelineService(factory)
    cut = TimelineCut(episode.id, project.id)
    cut.clips.append(
        {
            "id": "clip-1",
            "assetVersionId": asset_version.id,
            "assetVersionRevision": asset_version.revision,
            "assetVersionHash": asset_version.content_hash,
            "derivativeFingerprint": "b" * 64,
            "inFrame": 0,
            "outFrame": 30,
            "durationFrames": 30,
            "timelineStart": 0,
            "derivativeStatus": "ready",
            "transition": {"type": "cut", "durationFrames": 0},
        }
    )
    async with factory() as uow:
        uow.timeline_cuts[episode.id] = cut
        await uow.commit()
    version = await timeline.publish(episode.id, "Published", 1, project.id)
    exports = ExportService(factory, MockFfmpegRenderAdapter(), storage=storage)
    selection = [
        {
            "episodeId": episode.id,
            "timelineVersionId": version.id,
            "timelineVersionRevision": 1,
            "outputBaseName": "episode-01",
        }
    ]
    first = await exports.create_batch(
        project.id,
        selection,
        "light",
        "export-1",
        storage_profile_id="local-test-offline",
        storage_profile_revision=1,
    )
    second = await exports.create_batch(
        project.id,
        selection,
        "light",
        "export-1",
        storage_profile_id="local-test-offline",
        storage_profile_revision=1,
    )
    assert first.id == second.id
    assert len(first.jobs) == 1
    # A fresh UoW reads the normalized batch/job/artifact owner tables.
    fresh = ExportService(
        make_sqlalchemy_uow_factory(async_sessionmaker(engine, expire_on_commit=False)),
        MockFfmpegRenderAdapter(),
    )
    projection = await fresh.projection(project.id, first.id)
    assert projection["projectId"] == project.id
    assert projection["status"] == "queued"
    async with factory() as uow:
        events = list(uow.export_dispatch_outbox.values())
        assert len(events) == 1
        assert events[0].job_id == first.jobs[0].id
        assert events[0].status == "pending"
    await engine.dispose()
