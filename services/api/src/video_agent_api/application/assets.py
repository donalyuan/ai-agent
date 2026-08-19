"""Assets/asset versions command and query services."""

from __future__ import annotations

from dataclasses import dataclass

from video_agent_api.domain.assets import Asset, AssetVersion, StorageObject
from video_agent_api.domain.errors import (
    AssetNotFoundError,
    AssetVersionNotFoundError,
    ProjectNotFoundError,
)

from .ports import AssetsUnitOfWorkFactory


@dataclass(frozen=True, slots=True)
class CreateAssetCommand:
    project_id: str
    name: str
    kind: str


@dataclass(frozen=True, slots=True)
class AppendAssetVersionCommand:
    asset_id: str
    storage_object: StorageObject
    content_hash: str | None = None


class AssetsService:
    """Each command owns one UoW; versions are append-only and never updated in place."""

    def __init__(self, uow_factory: AssetsUnitOfWorkFactory) -> None:
        self._uow_factory = uow_factory

    async def create_asset(self, command: CreateAssetCommand) -> Asset:
        async with self._uow_factory() as uow:
            if await uow.projects.get(command.project_id) is None:
                raise ProjectNotFoundError(command.project_id)
            asset = Asset(command.project_id, command.kind, command.name)
            await uow.assets.add(asset)
            await uow.commit()
            return asset

    async def get_asset(self, asset_id: str) -> Asset:
        async with self._uow_factory() as uow:
            asset = await uow.assets.get(asset_id)
        if asset is None:
            raise AssetNotFoundError(asset_id)
        return asset

    async def list_assets(self, project_id: str) -> list[Asset]:
        async with self._uow_factory() as uow:
            if await uow.projects.get(project_id) is None:
                raise ProjectNotFoundError(project_id)
            assets = await uow.assets.list_by_project(project_id)
        return sorted(assets, key=lambda value: value.id)

    async def append_version(self, command: AppendAssetVersionCommand) -> AssetVersion:
        async with self._uow_factory() as uow:
            asset = await uow.assets.get(command.asset_id)
            if asset is None:
                raise AssetNotFoundError(command.asset_id)
            version_number = await uow.asset_versions.next_version_number(asset.id)
            version = AssetVersion(
                asset_id=asset.id,
                project_id=asset.project_id,
                version_number=version_number,
                storage_object=command.storage_object,
                content_hash=command.content_hash,
            )
            await uow.asset_versions.add(version)
            await uow.commit()
            return version

    async def get_version(self, version_id: str) -> AssetVersion:
        async with self._uow_factory() as uow:
            version = await uow.asset_versions.get(version_id)
        if version is None:
            raise AssetVersionNotFoundError(version_id)
        return version

    async def list_versions(self, asset_id: str) -> list[AssetVersion]:
        async with self._uow_factory() as uow:
            if await uow.assets.get(asset_id) is None:
                raise AssetNotFoundError(asset_id)
            versions = await uow.asset_versions.list_by_asset(asset_id)
        return sorted(versions, key=lambda value: (value.version_number, value.id))
