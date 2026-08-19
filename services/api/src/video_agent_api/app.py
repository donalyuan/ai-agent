"""FastAPI 入口；健康探测保持兼容，业务路由由显式 UoW 注入。"""

import os
from collections.abc import Callable
from typing import cast

from fastapi import FastAPI, HTTPException
from fastapi.encoders import jsonable_encoder
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
from video_agent_api.application.assets import AssetsService
from video_agent_api.application.ports import AssetsUnitOfWorkFactory, UnitOfWorkFactory
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.db import default_readiness_probe
from video_agent_api.domain.errors import DomainError
from video_agent_api.interfaces.http.assets import router as assets_router
from video_agent_api.interfaces.http.projects_episodes import router as projects_episodes_router
from video_agent_api.logging import log_event
from video_agent_api.runtime import RuntimeSettings, build_runtime


def create_app(
    readiness_probe: Callable[[], bool] | None = None,
    settings: RuntimeSettings | None = None,
    projects_episodes_service: ProjectsEpisodesService | None = None,
    assets_service: AssetsService | None = None,
) -> FastAPI:
    """构建健康端点和 projects/episodes；未配置数据库时业务端点显式 503。"""
    app = FastAPI(title="Video Agent API", version="0.1.0")
    app.state.runtime = build_runtime(settings or RuntimeSettings.from_env())
    service = projects_episodes_service
    assets = assets_service
    database_url = os.environ.get("DATABASE_URL")
    if database_url and (service is None or assets is None):
        engine = create_async_engine(database_url)
        app.state.projects_episodes_engine = engine
        session_factory = async_sessionmaker(engine, expire_on_commit=False)
        uow_factory = cast(UnitOfWorkFactory, make_sqlalchemy_uow_factory(session_factory))
        if service is None:
            service = ProjectsEpisodesService(uow_factory)
        if assets is None:
            assets = AssetsService(cast(AssetsUnitOfWorkFactory, uow_factory))
    app.state.projects_episodes_service = service
    app.state.assets_service = assets
    probe = readiness_probe or default_readiness_probe

    @app.exception_handler(DomainError)
    async def handle_domain_error(_request: object, error: DomainError) -> JSONResponse:
        status_code = 503 if error.code == "database_unavailable" else 422
        if error.code in {"project_not_found", "episode_not_found"}:
            status_code = 404
        elif error.code in {"episode_number_conflict", "revision_conflict"}:
            status_code = 409
        return JSONResponse(
            status_code=status_code,
            content={"detail": {"type": error.code, "message": str(error)}},
        )

    @app.exception_handler(RequestValidationError)
    async def handle_request_validation(
        _request: object, error: RequestValidationError
    ) -> JSONResponse:
        return JSONResponse(
            status_code=422,
            content={
                "detail": {
                    "type": "validation",
                    "message": "request validation failed",
                    "errors": jsonable_encoder(error.errors()),
                }
            },
        )

    @app.get("/v1/health/live")
    def live() -> dict[str, str]:
        log_event("api.health.live")
        return {"status": "live"}

    @app.get("/v1/health/ready")
    def ready() -> dict[str, str]:
        if not probe():
            log_event("api.health.ready", status="unavailable")
            raise HTTPException(status_code=503, detail="database is not ready")
        log_event("api.health.ready", status="ready")
        return {"status": "ready"}

    app.include_router(projects_episodes_router)
    app.include_router(assets_router)
    return app


app = create_app()
