from __future__ import annotations

import pytest
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from video_agent_api.adapters.sqlalchemy import (
    SQLAlchemyProjectRepository,
    SQLAlchemyUnitOfWork,
)
from video_agent_api.adapters.sqlalchemy_models import Base
from video_agent_api.application.projects_episodes import (
    CreateEpisodeCommand,
    CreateProjectCommand,
    ProjectsEpisodesService,
    UpdateProjectCommand,
)
from video_agent_api.domain.errors import RevisionConflictError


async def test_sqlalchemy_adapter_maps_title_filters_parent_and_updates_revision() -> None:
    engine = create_async_engine("sqlite+aiosqlite:///:memory:")
    try:
        async with engine.begin() as connection:
            await connection.run_sync(Base.metadata.create_all)
        factory = async_sessionmaker(engine, expire_on_commit=False)
        service = ProjectsEpisodesService(lambda: SQLAlchemyUnitOfWork(factory))
        project = await service.create_project(CreateProjectCommand("Demo"))
        await service.create_episode(CreateEpisodeCommand(project.id, "Opening", 1))
        assert (await service.list_episodes(project.id))[0].title == "Opening"
        updated = await service.update_project(UpdateProjectCommand(project.id, 1, "Updated"))
        assert updated.revision == 2
        assert (await service.get_project(project.id)).name == "Updated"
    finally:
        await engine.dispose()


async def test_sqlalchemy_repository_rejects_concurrent_revision_update() -> None:
    engine = create_async_engine("sqlite+aiosqlite:///:memory:")
    try:
        async with engine.begin() as connection:
            await connection.run_sync(Base.metadata.create_all)
        factory = async_sessionmaker(engine, expire_on_commit=False)
        service = ProjectsEpisodesService(lambda: SQLAlchemyUnitOfWork(factory))
        project = await service.create_project(CreateProjectCommand("Demo"))

        async with factory() as first_session, factory() as second_session:
            first_repository = SQLAlchemyProjectRepository(first_session)
            second_repository = SQLAlchemyProjectRepository(second_session)
            first = await first_repository.get(project.id)
            second = await second_repository.get(project.id)
            assert first is not None and second is not None

            first.update(expected_revision=1, name="First")
            await first_repository.save(first)
            await first_session.commit()

            second.update(expected_revision=1, name="Second")
            with pytest.raises(RevisionConflictError) as conflict:
                await second_repository.save(second)
            assert conflict.value.current_revision == 2

        assert (await service.get_project(project.id)).name == "First"
    finally:
        await engine.dispose()
