from __future__ import annotations

import pytest

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.application.projects_episodes import (
    CreateEpisodeCommand,
    CreateProjectCommand,
    ProjectsEpisodesService,
    UpdateEpisodeCommand,
    UpdateProjectCommand,
)
from video_agent_api.domain.errors import (
    EpisodeNumberConflictError,
    ProjectNotFoundError,
    RevisionConflictError,
)


@pytest.fixture
def service() -> ProjectsEpisodesService:
    uow = InMemoryUnitOfWork()
    return ProjectsEpisodesService(lambda: uow)


async def test_commands_queries_and_scoped_episode_order(service: ProjectsEpisodesService) -> None:
    first = await service.create_project(CreateProjectCommand("First"))
    second = await service.create_project("Second")
    episode_two = await service.create_episode(CreateEpisodeCommand(first.id, "Two", 2))
    episode_one = await service.create_episode((first.id, "One", 1))
    await service.create_episode(CreateEpisodeCommand(second.id, "Other", 1))

    assert [item.id for item in await service.list_projects()] == sorted([first.id, second.id])
    assert [item.id for item in await service.list_episodes(first.id)] == [
        episode_one.id,
        episode_two.id,
    ]
    assert await service.get_project(first.id) == first


async def test_missing_parent_and_duplicate_number_are_stable_errors(
    service: ProjectsEpisodesService,
) -> None:
    with pytest.raises(ProjectNotFoundError) as missing:
        await service.create_episode(("missing", "Nope", 1))
    assert missing.value.code == "project_not_found"

    project = await service.create_project("Demo")
    await service.create_episode((project.id, "One", 1))
    with pytest.raises(EpisodeNumberConflictError) as conflict:
        await service.create_episode((project.id, "Duplicate", 1))
    assert conflict.value.code == "episode_number_conflict"


async def test_updates_require_revision_and_do_not_silently_overwrite(
    service: ProjectsEpisodesService,
) -> None:
    project = await service.create_project("Demo")
    await service.update_project(UpdateProjectCommand(project.id, 1, "Updated"))
    with pytest.raises(RevisionConflictError):
        await service.update_project(UpdateProjectCommand(project.id, 1, "Stale"))
    assert (await service.get_project(project.id)).name == "Updated"

    episode = await service.create_episode((project.id, "One", 1))
    await service.update_episode(UpdateEpisodeCommand(episode.id, 1, title="Changed"))
    with pytest.raises(RevisionConflictError):
        await service.update_episode(UpdateEpisodeCommand(episode.id, 1, title="Stale"))
