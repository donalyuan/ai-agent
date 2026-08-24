"""projects/episodes 的 Repository 与 Unit of Work 端口。"""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any, Protocol

from video_agent_api.domain.asset_bible import (
    AssetBible,
    AssetBibleAcceptDecision,
    AssetBibleEntry,
    AssetBibleHandoffAck,
    AssetBibleRelationship,
    AssetBibleVersion,
    ContinuityAssignment,
    ContinuityImpactAnalysis,
    ContinuityRevisionTask,
    ResolvedContinuitySnapshot,
)
from video_agent_api.domain.assets import Asset, AssetVersion, AssetVersionReservation
from video_agent_api.domain.entities import Episode, Project
from video_agent_api.domain.scenes import Scene, Shot


class ProjectRepository(Protocol):
    async def get(self, project_id: str) -> Project | None: ...

    async def list(self) -> Sequence[Project]: ...

    async def add(self, project: Project) -> None: ...

    async def save(self, project: Project) -> None: ...


class EpisodeRepository(Protocol):
    async def get(self, episode_id: str) -> Episode | None: ...

    async def list_by_project(self, project_id: str) -> Sequence[Episode]: ...

    async def add(self, episode: Episode) -> None: ...

    async def save(self, episode: Episode) -> None: ...


class AssetRepository(Protocol):
    async def get(self, asset_id: str) -> Asset | None: ...

    async def list_by_project(self, project_id: str) -> Sequence[Asset]: ...

    async def add(self, asset: Asset) -> None: ...

    async def save(self, asset: Asset) -> None: ...


class AssetVersionRepository(Protocol):
    async def get(self, version_id: str) -> AssetVersion | None: ...

    async def list_by_asset(self, asset_id: str) -> Sequence[AssetVersion]: ...

    async def next_version_number(self, asset_id: str) -> int: ...

    async def add(self, version: AssetVersion) -> None: ...


class ProjectsEpisodesUnitOfWork(Protocol):
    projects: ProjectRepository
    episodes: EpisodeRepository

    async def __aenter__(self) -> ProjectsEpisodesUnitOfWork: ...

    async def __aexit__(self, exc_type: object, exc: object, tb: object) -> None: ...

    async def commit(self) -> None: ...

    async def rollback(self) -> None: ...


class AssetsUnitOfWork(Protocol):
    projects: ProjectRepository
    episodes: EpisodeRepository
    assets: AssetRepository
    asset_versions: AssetVersionRepository
    asset_reservations: dict[str, AssetVersionReservation]
    audit_events: list[dict[str, object]]
    outbox_events: list[dict[str, object]]
    media_inspections: dict[str, Any]
    media_derivatives: dict[str, Any]
    source_materials: dict[str, Any]
    shots: dict[str, Shot]
    asset_edit_candidates: dict[str, Any]
    timeline_cuts: dict[str, Any]
    timeline_versions: dict[str, Any]
    export_jobs: dict[str, Any]
    export_dispatch_outbox: dict[str, Any]

    async def __aenter__(self) -> AssetsUnitOfWork: ...

    async def __aexit__(self, exc_type: object, exc: object, tb: object) -> None: ...

    async def commit(self) -> None: ...

    async def rollback(self) -> None: ...


class UnitOfWorkFactory(Protocol):
    def __call__(self) -> ProjectsEpisodesUnitOfWork: ...


class AssetsUnitOfWorkFactory(Protocol):
    def __call__(self) -> AssetsUnitOfWork: ...


class MediaCurrentOwner(Protocol):
    async def accept_current_media_in_transaction(
        self,
        uow: Any,
        *,
        project_id: str,
        episode_id: str,
        shot_id: str,
        candidate: dict[str, object],
        expected_shot_revision: int,
    ) -> Shot: ...

    async def update_derivative_in_transaction(
        self,
        uow: Any,
        *,
        project_id: str,
        shot_id: str,
        candidate_id: str,
        derivative_status: str,
    ) -> Shot: ...


class AssetBibleUnitOfWork(Protocol):
    projects: ProjectRepository
    episodes: EpisodeRepository
    scenes: dict[str, Scene]
    shots: dict[str, Shot]
    asset_bible_entries: dict[str, AssetBibleEntry]
    asset_bibles_by_project: dict[str, AssetBible]
    asset_bible_by_project: dict[str, list[AssetBibleEntry]]
    asset_bible_assignments: list[ContinuityAssignment]
    asset_bible_relationships: list[AssetBibleRelationship]
    asset_bible_snapshots: dict[str, ResolvedContinuitySnapshot]
    asset_bible_tasks: dict[str, ContinuityRevisionTask]
    asset_bible_impacts: dict[str, ContinuityImpactAnalysis]
    asset_bible_decisions: dict[
        str, tuple[AssetBibleAcceptDecision, AssetBibleVersion, tuple[str, ...]]
    ]
    asset_bible_handoff_acks: dict[str, tuple[str, AssetBibleHandoffAck]]
    audit_events: list[dict[str, object]]
    outbox_events: list[dict[str, object]]
    provider_calls: dict[str, object]

    async def __aenter__(self) -> AssetBibleUnitOfWork: ...

    async def __aexit__(self, exc_type: object, exc: object, tb: object) -> None: ...

    async def commit(self) -> None: ...

    async def rollback(self) -> None: ...


class AssetBibleUnitOfWorkFactory(Protocol):
    def __call__(self) -> AssetBibleUnitOfWork: ...
