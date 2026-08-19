"""共享状态的内存 UoW，仅用于测试和显式注入。"""

from __future__ import annotations

from video_agent_api.domain.assets import Asset, AssetVersion
from video_agent_api.domain.entities import Episode, Project


class _State:
    def __init__(self) -> None:
        self.projects: dict[str, Project] = {}
        self.episodes: dict[str, Episode] = {}
        self.assets: dict[str, Asset] = {}
        self.asset_versions: dict[str, AssetVersion] = {}


class InMemoryProjectRepository:
    def __init__(self, state: _State) -> None:
        self._state = state

    async def get(self, project_id: str) -> Project | None:
        return self._state.projects.get(project_id)

    async def list(self) -> list[Project]:
        return list(self._state.projects.values())

    async def add(self, project: Project) -> None:
        self._state.projects[project.id] = project

    async def save(self, project: Project) -> None:
        self._state.projects[project.id] = project


class InMemoryEpisodeRepository:
    def __init__(self, state: _State) -> None:
        self._state = state

    async def get(self, episode_id: str) -> Episode | None:
        return self._state.episodes.get(episode_id)

    async def list_by_project(self, project_id: str) -> list[Episode]:
        return [value for value in self._state.episodes.values() if value.project_id == project_id]

    async def add(self, episode: Episode) -> None:
        self._state.episodes[episode.id] = episode

    async def save(self, episode: Episode) -> None:
        self._state.episodes[episode.id] = episode


class InMemoryAssetRepository:
    def __init__(self, state: _State) -> None:
        self._state = state

    async def get(self, asset_id: str) -> Asset | None:
        return self._state.assets.get(asset_id)

    async def list_by_project(self, project_id: str) -> list[Asset]:
        return [value for value in self._state.assets.values() if value.project_id == project_id]

    async def add(self, asset: Asset) -> None:
        self._state.assets[asset.id] = asset


class InMemoryAssetVersionRepository:
    def __init__(self, state: _State) -> None:
        self._state = state

    async def get(self, version_id: str) -> AssetVersion | None:
        return self._state.asset_versions.get(version_id)

    async def list_by_asset(self, asset_id: str) -> list[AssetVersion]:
        return [
            value for value in self._state.asset_versions.values() if value.asset_id == asset_id
        ]

    async def next_version_number(self, asset_id: str) -> int:
        versions = await self.list_by_asset(asset_id)
        return max((value.version_number for value in versions), default=0) + 1

    async def add(self, version: AssetVersion) -> None:
        self._state.asset_versions[version.id] = version


class InMemoryUnitOfWork:
    def __init__(self, state: _State | None = None) -> None:
        self.state = state or _State()
        self.projects = InMemoryProjectRepository(self.state)
        self.episodes = InMemoryEpisodeRepository(self.state)
        self.assets = InMemoryAssetRepository(self.state)
        self.asset_versions = InMemoryAssetVersionRepository(self.state)
        self.commits = 0

    def __call__(self) -> InMemoryUnitOfWork:
        return self

    async def __aenter__(self) -> InMemoryUnitOfWork:
        return self

    async def __aexit__(self, exc_type: object, exc: object, tb: object) -> None:
        if exc_type is not None:
            await self.rollback()

    async def commit(self) -> None:
        self.commits += 1

    async def rollback(self) -> None:
        return None


def in_memory_uow_factory() -> InMemoryUnitOfWork:
    # Factory closure is useful to callers that need one shared state per app.
    return InMemoryUnitOfWork()
