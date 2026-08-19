"""projects/episodes 的 SQLAlchemy async adapter。"""

from __future__ import annotations

from collections.abc import Callable, Sequence
from types import TracebackType
from typing import Any, cast

from sqlalchemy import func, select, update
from sqlalchemy.engine import CursorResult
from sqlalchemy.exc import IntegrityError, SQLAlchemyError
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from video_agent_api.adapters.sqlalchemy_models import Asset as AssetModel
from video_agent_api.adapters.sqlalchemy_models import AssetVersion as AssetVersionModel
from video_agent_api.adapters.sqlalchemy_models import Episode as EpisodeModel
from video_agent_api.adapters.sqlalchemy_models import Project as ProjectModel
from video_agent_api.application.ports import (
    AssetRepository,
    AssetVersionRepository,
    EpisodeRepository,
    ProjectRepository,
)
from video_agent_api.domain.assets import Asset, AssetVersion, StorageObject
from video_agent_api.domain.entities import Episode, Project
from video_agent_api.domain.errors import (
    AssetVersionConflictError,
    DatabaseUnavailableError,
    EpisodeNumberConflictError,
    RevisionConflictError,
)


def _asset_from_model(value: AssetModel) -> Asset:
    return Asset(
        id=value.id,
        project_id=value.project_id,
        kind=value.kind,
        name=value.name,
        status=value.status,
        schema_version=value.schema_version,
        revision=value.revision,
    )


def _version_from_model(value: AssetVersionModel) -> AssetVersion:
    media_raw: object = value.media_metadata
    if media_raw is None and isinstance(value.metadata_json, dict):
        media_raw = value.metadata_json.get("media")
    media = (
        {key: item for key, item in media_raw.items() if isinstance(item, int)}
        if isinstance(media_raw, dict)
        else None
    )
    storage = StorageObject(
        storage_provider=value.storage_provider,
        bucket=value.bucket,
        region=value.region,
        object_key=value.object_key or value.storage_ref,
        e_tag=value.e_tag,
        mime_type=value.mime_type,
        size_bytes=value.size_bytes,
        checksum=value.checksum,
        media=media,
    )
    return AssetVersion(
        id=value.id,
        asset_id=value.asset_id,
        project_id=value.project_id,
        version_number=value.version_number,
        storage_object=storage,
        content_hash=value.content_hash,
        status=value.status,
        schema_version=value.schema_version,
        revision=value.revision,
    )


def _project_from_model(value: ProjectModel) -> Project:
    return Project(
        id=value.id,
        name=value.name,
        status=value.status,
        schema_version=value.schema_version,
        revision=value.revision,
    )


def _episode_from_model(value: EpisodeModel) -> Episode:
    return Episode(
        id=value.id,
        project_id=value.project_id,
        title=value.title,
        number=value.display_number,
        status=value.status,
        schema_version=value.schema_version,
        revision=value.revision,
    )


class SQLAlchemyProjectRepository(ProjectRepository):
    def __init__(self, session: AsyncSession) -> None:
        self.session = session

    async def get(self, project_id: str) -> Project | None:
        model = await self.session.get(ProjectModel, project_id)
        return _project_from_model(model) if model else None

    async def list(self) -> Sequence[Project]:
        result = await self.session.execute(select(ProjectModel).order_by(ProjectModel.id))
        return [_project_from_model(item) for item in result.scalars()]

    async def add(self, project: Project) -> None:
        self.session.add(
            ProjectModel(
                id=project.id,
                name=project.name,
                status=project.status,
                schema_version=project.schema_version,
                revision=project.revision,
            )
        )

    async def save(self, project: Project) -> None:
        result = cast(
            CursorResult[Any],
            await self.session.execute(
                update(ProjectModel)
                .where(
                    ProjectModel.id == project.id,
                    ProjectModel.revision == project.revision - 1,
                )
                .values(
                    name=project.name,
                    status=project.status,
                    schema_version=project.schema_version,
                    revision=project.revision,
                )
            ),
        )
        if result.rowcount != 1:
            current_revision = await self.session.scalar(
                select(ProjectModel.revision).where(ProjectModel.id == project.id)
            )
            if current_revision is None:
                raise KeyError(project.id)
            raise RevisionConflictError(project.id, project.revision - 1, current_revision)


class SQLAlchemyEpisodeRepository(EpisodeRepository):
    def __init__(self, session: AsyncSession) -> None:
        self.session = session

    async def get(self, episode_id: str) -> Episode | None:
        model = await self.session.get(EpisodeModel, episode_id)
        return _episode_from_model(model) if model else None

    async def list_by_project(self, project_id: str) -> Sequence[Episode]:
        result = await self.session.execute(
            select(EpisodeModel)
            .where(EpisodeModel.project_id == project_id)
            .order_by(EpisodeModel.display_number, EpisodeModel.id)
        )
        return [_episode_from_model(item) for item in result.scalars()]

    async def add(self, episode: Episode) -> None:
        self.session.add(
            EpisodeModel(
                id=episode.id,
                project_id=episode.project_id,
                display_number=episode.number,
                title=episode.title,
                status=episode.status,
                schema_version=episode.schema_version,
                revision=episode.revision,
            )
        )

    async def save(self, episode: Episode) -> None:
        result = cast(
            CursorResult[Any],
            await self.session.execute(
                update(EpisodeModel)
                .where(
                    EpisodeModel.id == episode.id,
                    EpisodeModel.revision == episode.revision - 1,
                )
                .values(
                    display_number=episode.number,
                    title=episode.title,
                    status=episode.status,
                    schema_version=episode.schema_version,
                    revision=episode.revision,
                )
            ),
        )
        if result.rowcount != 1:
            current_revision = await self.session.scalar(
                select(EpisodeModel.revision).where(EpisodeModel.id == episode.id)
            )
            if current_revision is None:
                raise KeyError(episode.id)
            raise RevisionConflictError(episode.id, episode.revision - 1, current_revision)


class SQLAlchemyAssetRepository(AssetRepository):
    def __init__(self, session: AsyncSession) -> None:
        self.session = session

    async def get(self, asset_id: str) -> Asset | None:
        model = await self.session.get(AssetModel, asset_id)
        return _asset_from_model(model) if model else None

    async def list_by_project(self, project_id: str) -> Sequence[Asset]:
        result = await self.session.execute(
            select(AssetModel).where(AssetModel.project_id == project_id).order_by(AssetModel.id)
        )
        return [_asset_from_model(item) for item in result.scalars()]

    async def add(self, asset: Asset) -> None:
        self.session.add(
            AssetModel(
                id=asset.id,
                project_id=asset.project_id,
                kind=asset.kind,
                name=asset.name,
                status=asset.status,
                schema_version=asset.schema_version,
                revision=asset.revision,
            )
        )


class SQLAlchemyAssetVersionRepository(AssetVersionRepository):
    def __init__(self, session: AsyncSession) -> None:
        self.session = session

    async def get(self, version_id: str) -> AssetVersion | None:
        model = await self.session.get(AssetVersionModel, version_id)
        return _version_from_model(model) if model else None

    async def list_by_asset(self, asset_id: str) -> Sequence[AssetVersion]:
        result = await self.session.execute(
            select(AssetVersionModel)
            .where(AssetVersionModel.asset_id == asset_id)
            .order_by(AssetVersionModel.version_number, AssetVersionModel.id)
        )
        return [_version_from_model(item) for item in result.scalars()]

    async def next_version_number(self, asset_id: str) -> int:
        current = await self.session.scalar(
            select(func.max(AssetVersionModel.version_number)).where(
                AssetVersionModel.asset_id == asset_id
            )
        )
        return int(current or 0) + 1

    async def add(self, version: AssetVersion) -> None:
        obj = version.storage_object
        self.session.add(
            AssetVersionModel(
                id=version.id,
                asset_id=version.asset_id,
                project_id=version.project_id,
                version_number=version.version_number,
                revision=version.revision,
                status=version.status,
                schema_version=version.schema_version,
                storage_ref=obj.object_key,
                checksum=obj.checksum,
                content_hash=version.content_hash,
                storage_provider=obj.storage_provider,
                bucket=obj.bucket,
                region=obj.region,
                object_key=obj.object_key,
                e_tag=obj.e_tag,
                mime_type=obj.mime_type,
                size_bytes=obj.size_bytes,
                media_metadata=dict(obj.media) if obj.media else None,
                metadata_json={"media": dict(obj.media)} if obj.media else {},
            )
        )


class SQLAlchemyUnitOfWork:
    """一个 command 一个 session；adapter 不向 application 暴露 AsyncSession。"""

    def __init__(self, session_factory: async_sessionmaker[AsyncSession]) -> None:
        self._session_factory = session_factory
        self.session: AsyncSession | None = None
        self.projects: SQLAlchemyProjectRepository
        self.episodes: SQLAlchemyEpisodeRepository
        self.assets: SQLAlchemyAssetRepository
        self.asset_versions: SQLAlchemyAssetVersionRepository

    def __call__(self) -> SQLAlchemyUnitOfWork:
        return type(self)(self._session_factory)

    async def __aenter__(self) -> SQLAlchemyUnitOfWork:
        self.session = self._session_factory()
        self.projects = SQLAlchemyProjectRepository(self.session)
        self.episodes = SQLAlchemyEpisodeRepository(self.session)
        self.assets = SQLAlchemyAssetRepository(self.session)
        self.asset_versions = SQLAlchemyAssetVersionRepository(self.session)
        return self

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        if self.session is None:
            return
        database_error = isinstance(exc, (OSError, SQLAlchemyError))
        try:
            if exc_type is not None:
                await self.rollback()
        finally:
            await self.session.close()
        if database_error and exc is not None:
            raise DatabaseUnavailableError("business database is unavailable") from exc

    async def commit(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        try:
            await self.session.commit()
        except IntegrityError as error:
            await self.session.rollback()
            # The unique constraint is the final concurrency guard.
            # Unknown DB errors remain visible.
            if "episode" in str(error).lower() and "number" in str(error).lower():
                raise EpisodeNumberConflictError("unknown", 0) from error
            if "asset_version" in str(error).lower() or "asset version" in str(error).lower():
                raise AssetVersionConflictError("unknown", 0) from error
            raise

    async def rollback(self) -> None:
        if self.session is not None:
            await self.session.rollback()


SqlAlchemyUnitOfWork = SQLAlchemyUnitOfWork


def make_sqlalchemy_uow_factory(
    session_factory: async_sessionmaker[AsyncSession],
) -> Callable[[], SQLAlchemyUnitOfWork]:
    return lambda: SQLAlchemyUnitOfWork(session_factory)
