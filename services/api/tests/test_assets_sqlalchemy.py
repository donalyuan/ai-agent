from __future__ import annotations

import pytest
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from video_agent_api.adapters.sqlalchemy import SQLAlchemyUnitOfWork
from video_agent_api.adapters.sqlalchemy_models import AssetVersion as AssetVersionModel
from video_agent_api.adapters.sqlalchemy_models import Base
from video_agent_api.application.assets import (
    AppendAssetVersionCommand,
    AssetsService,
    CancelReservationCommand,
    CreateAssetCommand,
    CreateReservationCommand,
    UpdateAssetMetadataCommand,
)
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.domain.assets import StorageObject

HASH = "d" * 64
CONTENT_HASH = "e" * 64


@pytest.mark.asyncio
async def test_assets_sqlalchemy_adapter_maps_storage_metadata_and_appends() -> None:
    engine = create_async_engine("sqlite+aiosqlite:///:memory:")
    try:
        async with engine.begin() as connection:
            await connection.run_sync(Base.metadata.create_all)
        factory = async_sessionmaker(engine, expire_on_commit=False)
        uow = SQLAlchemyUnitOfWork(factory)
        projects = ProjectsEpisodesService(lambda: uow)
        project = await projects.create_project("Demo")
        service = AssetsService(lambda: uow)
        asset = await service.create_asset(CreateAssetCommand(project.id, "Video", "video"))
        version = await service.append_version(
            AppendAssetVersionCommand(
                asset.id,
                StorageObject("local", "workspace", "assets/a/v1.mp4", "video/mp4", 4, HASH),
                content_hash=CONTENT_HASH,
            )
        )
        assert version.version_number == 1
        assert version.content_hash == CONTENT_HASH
        async with factory() as session:
            model = await session.get(AssetVersionModel, version.id)
            assert model is not None
            assert model.object_key == "assets/a/v1.mp4"
            assert model.size_bytes == 4
            assert model.checksum == HASH
            assert model.content_hash == CONTENT_HASH
        loaded = await service.get_version(version.id)
        assert loaded.content_hash == CONTENT_HASH
        assert loaded.storage_object.checksum == HASH
    finally:
        await engine.dispose()


@pytest.mark.asyncio
async def test_asset_version_number_unique_constraint_is_final_guard() -> None:
    engine = create_async_engine("sqlite+aiosqlite:///:memory:")
    try:
        async with engine.begin() as connection:
            await connection.run_sync(Base.metadata.create_all)
        factory = async_sessionmaker(engine, expire_on_commit=False)
        projects = ProjectsEpisodesService(lambda: SQLAlchemyUnitOfWork(factory))
        project = await projects.create_project("Demo")
        service = AssetsService(lambda: SQLAlchemyUnitOfWork(factory))
        asset = await service.create_asset(CreateAssetCommand(project.id, "Video", "video"))
        version = await service.append_version(
            AppendAssetVersionCommand(
                asset.id,
                StorageObject("local", "workspace", "assets/a/v1.mp4", "video/mp4", 4, HASH),
            )
        )
        async with factory() as session:
            session.add(
                AssetVersionModel(
                    id="duplicate-version",
                    asset_id=asset.id,
                    project_id=project.id,
                    version_number=version.version_number,
                    revision=0,
                    status="draft",
                    schema_version="1.0.0",
                    storage_ref="assets/a/duplicate.mp4",
                    checksum=HASH,
                    content_hash=HASH,
                    storage_provider="local",
                    bucket="workspace",
                    object_key="assets/a/duplicate.mp4",
                    mime_type="video/mp4",
                    size_bytes=4,
                    metadata_json={},
                )
            )
            with pytest.raises(IntegrityError):
                await session.commit()
    finally:
        await engine.dispose()


@pytest.mark.asyncio
async def test_asset_metadata_and_reservation_survive_fresh_uow() -> None:
    engine = create_async_engine("sqlite+aiosqlite:///:memory:")
    try:
        async with engine.begin() as connection:
            await connection.run_sync(Base.metadata.create_all)
        factory = async_sessionmaker(engine, expire_on_commit=False)
        projects = ProjectsEpisodesService(lambda: SQLAlchemyUnitOfWork(factory))
        assets = AssetsService(lambda: SQLAlchemyUnitOfWork(factory))
        project = await projects.create_project("Asset center")
        asset = await assets.create_asset(
            CreateAssetCommand(
                project.id,
                "Dialogue",
                "audio",
                source_type="user_upload",
                catalog_role="dialogue",
                authorization_status="verified",
                license_label="Owned",
            )
        )

        updated = await assets.update_metadata(
            UpdateAssetMetadataCommand(
                asset.id,
                asset.revision,
                tags=("lead", "episode-1"),
                copyright_owner="Studio",
            )
        )
        reloaded = await assets.get_asset(asset.id)
        assert reloaded.revision == updated.revision == 2
        assert reloaded.tags == ("lead", "episode-1")
        assert reloaded.catalog_role == "dialogue"
        assert reloaded.authorization_status == "verified"
        assert reloaded.copyright_owner == "Studio"
        assert reloaded.license_label == "Owned"

        reservation = await assets.create_reservation(
            CreateReservationCommand(
                project.id,
                asset.id,
                "b" * 64,
                reloaded.revision,
                "audio",
                "audio/wav",
                4,
                HASH,
                "local-test-offline",
                1,
                "c" * 64,
            )
        )
        after_restart = await assets.get_reservation(project.id, reservation.id)
        assert after_restart.operation_key == reservation.operation_key
        assert after_restart.upload_key.endswith("/original.wav")
        same = await assets.create_reservation(
            CreateReservationCommand(
                project.id,
                asset.id,
                "b" * 64,
                reloaded.revision,
                "audio",
                "audio/wav",
                4,
                HASH,
                "local-test-offline",
                1,
                "c" * 64,
            )
        )
        assert same.id == reservation.id
        cancelled = await assets.cancel_reservation(
            project.id, CancelReservationCommand(reservation.id, reservation.revision)
        )
        assert cancelled.status == "cancelled"
        assert (await assets.get_reservation(project.id, reservation.id)).status == "cancelled"
    finally:
        await engine.dispose()
