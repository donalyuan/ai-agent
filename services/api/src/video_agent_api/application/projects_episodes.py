"""projects/episodes command/query services。"""

from __future__ import annotations

from dataclasses import dataclass

from video_agent_api.domain.entities import Episode, Project
from video_agent_api.domain.errors import (
    EpisodeNotFoundError,
    EpisodeNumberConflictError,
    ProjectNotFoundError,
)

from .ports import UnitOfWorkFactory


@dataclass(frozen=True, slots=True)
class CreateProjectCommand:
    name: str


@dataclass(frozen=True, slots=True)
class UpdateProjectCommand:
    project_id: str
    expected_revision: int
    name: str | None = None


@dataclass(frozen=True, slots=True)
class CreateEpisodeCommand:
    project_id: str
    title: str
    number: int


@dataclass(frozen=True, slots=True)
class UpdateEpisodeCommand:
    episode_id: str
    expected_revision: int
    title: str | None = None
    number: int | None = None


class ProjectsEpisodesService:
    """每个 command 自己建立一个 UoW，保证读、领域变更和 commit 同属一个事务。"""

    def __init__(self, uow_factory: UnitOfWorkFactory) -> None:
        self._uow_factory = uow_factory

    async def create_project(self, command: CreateProjectCommand | str) -> Project:
        name = command if isinstance(command, str) else command.name
        project = Project(name)
        async with self._uow_factory() as uow:
            await uow.projects.add(project)
            await uow.commit()
        return project

    async def get_project(self, project_id: str) -> Project:
        async with self._uow_factory() as uow:
            project = await uow.projects.get(project_id)
        if project is None:
            raise ProjectNotFoundError(project_id)
        return project

    async def list_projects(self) -> list[Project]:
        async with self._uow_factory() as uow:
            projects = await uow.projects.list()
        return sorted(projects, key=lambda value: value.id)

    async def update_project(self, command: UpdateProjectCommand, **kwargs: object) -> Project:
        if kwargs:
            command = UpdateProjectCommand(command.project_id, command.expected_revision, **kwargs)  # type: ignore[arg-type]
        async with self._uow_factory() as uow:
            project = await uow.projects.get(command.project_id)
            if project is None:
                raise ProjectNotFoundError(command.project_id)
            project.update(expected_revision=command.expected_revision, name=command.name)
            await uow.projects.save(project)
            await uow.commit()
            return project

    async def create_episode(self, command: CreateEpisodeCommand | tuple[str, str, int]) -> Episode:
        if isinstance(command, tuple):
            project_id, title, number = command
        else:
            project_id, title, number = command.project_id, command.title, command.number
        async with self._uow_factory() as uow:
            if await uow.projects.get(project_id) is None:
                raise ProjectNotFoundError(project_id)
            existing = await uow.episodes.list_by_project(project_id)
            if any(episode.number == number for episode in existing):
                raise EpisodeNumberConflictError(project_id, number)
            episode = Episode(project_id, title, number)
            await uow.episodes.add(episode)
            await uow.commit()
            return episode

    async def get_episode(self, episode_id: str) -> Episode:
        async with self._uow_factory() as uow:
            episode = await uow.episodes.get(episode_id)
        if episode is None:
            raise EpisodeNotFoundError(episode_id)
        return episode

    async def list_episodes(self, project_id: str) -> list[Episode]:
        async with self._uow_factory() as uow:
            if await uow.projects.get(project_id) is None:
                raise ProjectNotFoundError(project_id)
            episodes = await uow.episodes.list_by_project(project_id)
        return sorted(episodes, key=lambda value: (value.number, value.id))

    async def update_episode(self, command: UpdateEpisodeCommand) -> Episode:
        async with self._uow_factory() as uow:
            episode = await uow.episodes.get(command.episode_id)
            if episode is None:
                raise EpisodeNotFoundError(command.episode_id)
            if command.number is not None:
                siblings = await uow.episodes.list_by_project(episode.project_id)
                if any(
                    item.id != episode.id and item.number == command.number for item in siblings
                ):
                    raise EpisodeNumberConflictError(episode.project_id, command.number)
            episode.update(
                expected_revision=command.expected_revision,
                title=command.title,
                number=command.number,
            )
            await uow.episodes.save(episode)
            await uow.commit()
            return episode
