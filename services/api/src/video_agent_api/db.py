"""数据库就绪探针；未配置数据库时明确报告未就绪。"""

from __future__ import annotations

import asyncio
import os

from sqlalchemy import text
from sqlalchemy.exc import SQLAlchemyError
from sqlalchemy.ext.asyncio import create_async_engine


async def check_database(database_url: str) -> bool:
    """执行无副作用的 `SELECT 1`，成功后释放临时引擎。"""
    engine = create_async_engine(database_url)
    try:
        async with engine.connect() as connection:
            await connection.execute(text("SELECT 1"))
        return True
    finally:
        await engine.dispose()


def default_readiness_probe() -> bool:
    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        return False
    try:
        return asyncio.run(check_database(database_url))
    except (OSError, RuntimeError, SQLAlchemyError):
        return False
