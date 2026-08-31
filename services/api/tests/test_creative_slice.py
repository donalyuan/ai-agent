from __future__ import annotations

from pathlib import Path

import pytest
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.adapters.sqlalchemy import SQLAlchemyUnitOfWork
from video_agent_api.adapters.sqlalchemy_models import Base
from video_agent_api.application.creative import (
    BindCreativeSourceCommand,
    CreativeService,
    ProjectEpisodeTextHandoff,
    SaveCreativeBriefCommand,
    SaveCreativeSettingsCommand,
)
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.source_material import (
    AppendSourceMaterialCommand,
    CreateSourceMaterialCommand,
    SourceMaterialService,
)
from video_agent_api.domain.creative import CreativeBriefSourceBindingSnapshot
from video_agent_api.domain.errors import RevisionConflictError, ValidationDomainError


@pytest.fixture
def services() -> tuple[ProjectsEpisodesService, CreativeService]:
    uow = InMemoryUnitOfWork()
    return ProjectsEpisodesService(lambda: uow), CreativeService(lambda: uow)


async def test_original_brief_is_immutable_successor_and_projection_hides_text(services):
    projects, creative = services
    project = await projects.create_project("Drama")
    fields = {
        "subject": "A",
        "genre": "Drama",
        "audience": "Adult",
        "characterPremise": "C",
        "style": "Real",
        "episodeDurationSeconds": 60,
        "episodeCount": 1,
        "scenesPerEpisode": 1,
        "shotsPerScene": 1,
    }
    brief = await creative.save_brief(SaveCreativeBriefCommand(project.id, "original", fields, 1))
    assert brief.revision == 1
    with pytest.raises(RevisionConflictError):
        await creative.save_brief(SaveCreativeBriefCommand(project.id, "original", fields, 1, 1))
    projection = await creative.get_projection(project.id)
    assert projection["creativeBrief"]["payload_hash"] == brief.payload_hash
    assert "subject" in projection["creativeBrief"]


async def test_adaptation_binding_rejects_original_and_brief_mismatch(services):
    projects, creative = services
    project = await projects.create_project("Drama")
    fields = {
        "subject": "A",
        "genre": "Drama",
        "audience": "Adult",
        "characterPremise": "C",
        "style": "Real",
        "episodeDurationSeconds": 60,
        "episodeCount": 1,
        "scenesPerEpisode": 1,
        "shotsPerScene": 1,
    }
    brief = await creative.save_brief(SaveCreativeBriefCommand(project.id, "original", fields, 1))
    snapshot = CreativeBriefSourceBindingSnapshot(
        project.id,
        "source",
        1,
        "a" * 64,
        brief.creative_brief_id,
        brief.revision,
        brief.payload_hash,
        "parsed",
        "valid",
        "bound",
        "1",
    )
    with pytest.raises(ValidationDomainError):
        await creative.bind_source(
            __import__(
                "video_agent_api.application.creative", fromlist=["BindCreativeSourceCommand"]
            ).BindCreativeSourceCommand(project.id, snapshot, 2, brief.revision)
        )


async def test_settings_threshold_uses_non_negative_decimal(services):
    projects, creative = services
    project = await projects.create_project("Drama")
    settings = await creative.save_settings(
        SaveCreativeSettingsCommand(project.id, {"amount": "1.20", "currency": "CNY"}, 1)
    )
    assert settings.text_cost_confirmation_threshold == {"amount": "1.20", "currency": "CNY"}
    with pytest.raises(ValidationDomainError):
        await creative.save_settings(
            SaveCreativeSettingsCommand(project.id, {"amount": "-1", "currency": "CNY"}, 2, 1)
        )


async def test_adaptation_binding_and_handoff_are_atomic_and_idempotent(services):
    projects, creative = services
    project = await projects.create_project("Drama")
    episode = await projects.create_episode((project.id, "Pilot", 1))
    fields = {
        "subject": "A",
        "genre": "Drama",
        "audience": "Adult",
        "characterPremise": "C",
        "style": "Real",
        "episodeDurationSeconds": 60,
        "episodeCount": 1,
        "scenesPerEpisode": 1,
        "shotsPerScene": 1,
    }
    brief = await creative.save_brief(SaveCreativeBriefCommand(project.id, "adaptation", fields, 1))
    source = await SourceMaterialService(creative._uow_factory).create(
        CreateSourceMaterialCommand(project.id, "novel", "inline_text", project.id)
    )
    version = await SourceMaterialService(creative._uow_factory).append(
        AppendSourceMaterialCommand(
            source.id, source.revision, "inline_text", content=b"source", project_scope=project.id
        )
    )
    project_after_source = await projects.get_project(project.id)
    snapshot = CreativeBriefSourceBindingSnapshot(
        project.id,
        source.id,
        source.revision,
        version.content_hash,
        brief.creative_brief_id,
        brief.revision,
        brief.payload_hash,
        "parsed",
        "valid",
        "bound",
        "1",
    )
    bound = await creative.bind_source(
        BindCreativeSourceCommand(project.id, snapshot, project_after_source.revision, 1)
    )
    assert bound.source_material_id == source.id
    bound_project = await projects.get_project(project.id)
    handoff = ProjectEpisodeTextHandoff(
        "handoff",
        project.id,
        bound_project.revision,
        1,
        "story",
        1,
        "b" * 64,
        (
            {
                "episodeId": episode.id,
                "number": 1,
                "expectedRevision": 1,
                "scriptSpecRef": {"id": "script", "revision": 1, "hash": "c" * 64},
            },
        ),
        "d" * 64,
        "corr",
    )
    ack = await creative.apply_handoff(handoff)
    retry = await creative.apply_handoff(handoff)
    assert ack.id == retry.id
    projection = await creative.get_projection(project.id)
    assert projection["storySpecRef"]["id"] == "story"
    assert len(services[0]._uow_factory().state.audit_events) == 3


async def test_sqlalchemy_uow_exposes_creative_projection_ports() -> None:
    engine = create_async_engine("sqlite+aiosqlite:///:memory:")
    async with engine.begin() as connection:
        await connection.run_sync(Base.metadata.create_all)
    factory = async_sessionmaker(engine, expire_on_commit=False)
    try:
        projects = ProjectsEpisodesService(lambda: SQLAlchemyUnitOfWork(factory))
        creative = CreativeService(lambda: SQLAlchemyUnitOfWork(factory))
        project = await projects.create_project("SQL creative")
        projection = await creative.get_projection(project.id)
        assert projection["projectId"] == project.id
        assert projection["creationMode"] is None
        assert projection["creativeBrief"] is None
    finally:
        await engine.dispose()


async def test_phase_one_document_concurrent_update_uses_loaded_revision_cas(
    tmp_path: Path,
) -> None:
    engine = create_async_engine(f"sqlite+aiosqlite:///{tmp_path / 'phase-one-cas.db'}")
    async with engine.begin() as connection:
        await connection.run_sync(Base.metadata.create_all)
    factory = async_sessionmaker(engine, expire_on_commit=False)
    try:
        async with SQLAlchemyUnitOfWork(factory) as initial:
            initial.audit_events.append({"type": "seed"})
            await initial.commit()

        async with SQLAlchemyUnitOfWork(factory) as first:
            async with SQLAlchemyUnitOfWork(factory) as second:
                first.audit_events.append({"type": "first"})
                second.audit_events.append({"type": "second"})
                await first.commit()
                with pytest.raises(RevisionConflictError):
                    await second.commit()

        async with SQLAlchemyUnitOfWork(factory) as reloaded:
            assert {item["type"] for item in reloaded.audit_events} == {"seed", "first"}
    finally:
        await engine.dispose()
