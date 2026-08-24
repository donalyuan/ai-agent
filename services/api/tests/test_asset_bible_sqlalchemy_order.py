from __future__ import annotations

import pytest
from sqlalchemy import insert
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from video_agent_api.adapters.sqlalchemy import SQLAlchemyUnitOfWork
from video_agent_api.adapters.sqlalchemy_models import Base, Project
from video_agent_api.application.asset_bible import AssetBibleService, CreateEntryCommand


@pytest.mark.asyncio
async def test_new_asset_bible_is_flushed_before_entry_foreign_key(
    tmp_path,
) -> None:
    engine = create_async_engine(f"sqlite+aiosqlite:///{tmp_path / 'asset-bible-order.db'}")
    async with engine.begin() as connection:
        await connection.run_sync(Base.metadata.create_all)
    factory = async_sessionmaker(engine, expire_on_commit=False)
    service = AssetBibleService(lambda: SQLAlchemyUnitOfWork(factory))
    # Existing fixture project is supplied by the PostgreSQL integration fixture.
    project_id = "asset-bible-order-project"
    async with engine.begin() as connection:
        await connection.execute(
            insert(Project).values(
                id=project_id,
                revision=1,
                schema_version="1.0.0",
                status="draft",
                name="asset bible order",
            )
        )
    entry = await service.create_entry(CreateEntryCommand(project_id, "character"))
    assert entry.project_id == project_id
    await engine.dispose()
