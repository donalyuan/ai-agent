from __future__ import annotations

from dataclasses import FrozenInstanceError

import pytest

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.application.assets import (
    AppendAssetVersionCommand,
    AssetsService,
    CreateAssetCommand,
)
from video_agent_api.domain.assets import StorageObject
from video_agent_api.domain.entities import Project
from video_agent_api.domain.errors import AssetVersionNotFoundError, ProjectNotFoundError

HASH = "b" * 64


def storage() -> StorageObject:
    return StorageObject("local", "workspace", "assets/a/v1.mp4", "video/mp4", 3, HASH)


@pytest.mark.asyncio
async def test_assets_application_create_get_list_and_append_versions() -> None:
    uow = InMemoryUnitOfWork()
    async with uow:
        project = Project("Demo")
        await uow.projects.add(project)
        await uow.commit()
    service = AssetsService(lambda: uow)
    asset = await service.create_asset(CreateAssetCommand(project.id, "Video", "video"))
    first = await service.append_version(AppendAssetVersionCommand(asset.id, storage()))
    second = await service.append_version(AppendAssetVersionCommand(asset.id, storage()))
    assert (first.version_number, second.version_number) == (1, 2)
    assert [v.version_number for v in await service.list_versions(asset.id)] == [1, 2]
    with pytest.raises(AssetVersionNotFoundError):
        await service.get_version("missing")


@pytest.mark.asyncio
async def test_assets_application_requires_project() -> None:
    service = AssetsService(lambda: InMemoryUnitOfWork())
    with pytest.raises(ProjectNotFoundError):
        await service.create_asset(CreateAssetCommand("missing", "Video", "video"))


@pytest.mark.asyncio
async def test_in_memory_application_returns_deeply_immutable_version() -> None:
    uow = InMemoryUnitOfWork()
    async with uow:
        project = Project("Demo")
        await uow.projects.add(project)
        await uow.commit()
    service = AssetsService(lambda: uow)
    asset = await service.create_asset(CreateAssetCommand(project.id, "Video", "video"))
    returned = await service.append_version(
        AppendAssetVersionCommand(
            asset.id,
            StorageObject(
                "local",
                "workspace",
                "assets/a/v1.mp4",
                "video/mp4",
                3,
                HASH,
                media={"duration_ms": 1000, "width": 1920, "height": 1080},
            ),
        )
    )

    with pytest.raises(FrozenInstanceError):
        returned.version_number = 2
    assert returned.storage_object.media is not None
    with pytest.raises(TypeError):
        returned.storage_object.media["width"] = 1  # type: ignore[index]

    stored = await service.get_version(returned.id)
    assert stored.version_number == 1
    assert dict(stored.storage_object.media or {}) == {
        "duration_ms": 1000,
        "width": 1920,
        "height": 1080,
    }
