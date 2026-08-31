"""FastAPI 入口；健康探测保持兼容，业务路由由显式 UoW 注入。"""

import os
from collections.abc import Callable
from pathlib import Path
from typing import cast

from fastapi import Depends, FastAPI, Header, HTTPException, Request
from fastapi.encoders import jsonable_encoder
from fastapi.exceptions import RequestValidationError
from fastapi.responses import JSONResponse
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from video_agent_api.adapters.ffmpeg import SubprocessFfmpegRenderAdapter
from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
from video_agent_api.application.agent_edit import AgentEditService
from video_agent_api.application.asset_bible import AssetBibleService
from video_agent_api.application.assets import AssetsService
from video_agent_api.application.catalog import CatalogService
from video_agent_api.application.creative import CreativeService
from video_agent_api.application.exports import ExportService
from video_agent_api.application.image_generation import ImageGenerationService
from video_agent_api.application.media import MediaOwnerService
from video_agent_api.application.ports import (
    AssetBibleUnitOfWorkFactory,
    AssetsUnitOfWorkFactory,
    UnitOfWorkFactory,
)
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.runs import RunsService
from video_agent_api.application.runtime_composition import (
    CatalogRuntimeComposition,
    CatalogRuntimeResolver,
)
from video_agent_api.application.scenes import ScenesService
from video_agent_api.application.skill_routing import SkillRoutingService
from video_agent_api.application.source_material import SourceMaterialService
from video_agent_api.application.storage_profiles import StorageProfileService
from video_agent_api.application.text_generation import TextGenerationService
from video_agent_api.application.timeline import TimelineService
from video_agent_api.application.video_generation import AgnesVideoService
from video_agent_api.db import ReadinessAssessment, default_readiness_assessment
from video_agent_api.domain.errors import DomainError, ValidationDomainError
from video_agent_api.interfaces.http.agent_edit import router as agent_edit_router
from video_agent_api.interfaces.http.assets import router as assets_router
from video_agent_api.interfaces.http.catalog import router as catalog_router
from video_agent_api.interfaces.http.creative import router as creative_router
from video_agent_api.interfaces.http.image_generation import router as image_generation_router
from video_agent_api.interfaces.http.phase_one import router as phase_one_router
from video_agent_api.interfaces.http.projects_episodes import router as projects_episodes_router
from video_agent_api.interfaces.http.scenes import router as scenes_router
from video_agent_api.interfaces.http.text_generation import router as text_generation_router
from video_agent_api.interfaces.http.video_generation import router as video_generation_router
from video_agent_api.logging import log_event
from video_agent_api.observability import InMemoryTelemetry, TraceMiddleware
from video_agent_api.ports.storage import LocalOpaqueReadGrantIssuer, LocalWorkspaceAdapter
from video_agent_api.resilience import (
    OperationsResilienceCoordinator,
    capacity_snapshot,
    probe_resources,
)
from video_agent_api.runtime import RuntimeSettings, build_runtime
from video_agent_api.skills.registry import SkillRegistry


def create_app(
    readiness_probe: Callable[[], bool] | None = None,
    settings: RuntimeSettings | None = None,
    projects_episodes_service: ProjectsEpisodesService | None = None,
    assets_service: AssetsService | None = None,
) -> FastAPI:
    """构建健康端点和 projects/episodes；未配置数据库时业务端点显式 503。"""
    app = FastAPI(title="Video Agent API", version="0.1.0")

    async def require_project_scope(
        request: Request,
        project_scope: str | None = Header(default=None, alias="X-Project-Scope"),
    ) -> None:
        """Reject project-scoped requests before an application service reads owner data."""
        path_project = request.path_params.get("project_id") or request.path_params.get("projectId")
        if request.url.path.startswith("/v1/asset-media-grants/"):
            return
        if not project_scope or not project_scope.strip():
            raise HTTPException(status_code=403, detail="X-Project-Scope is required")
        project_scope = project_scope.strip()
        if path_project is not None and str(path_project) != project_scope:
            raise HTTPException(status_code=403, detail="project scope is forbidden")
        request.state.project_scope = project_scope

    app.state.telemetry = InMemoryTelemetry()
    app.add_middleware(TraceMiddleware, telemetry=app.state.telemetry)
    runtime_settings = settings or RuntimeSettings.from_env()
    app.state.runtime_settings = runtime_settings
    app.state.runtime = build_runtime(runtime_settings)
    # A local port is selected only by the explicit local profile. Live/TOS
    # requests must resolve StorageProfile/BucketBinding per frozen operation;
    # exposing the unresolved runtime placeholder would bypass that contract.
    # Local is an explicit catalog choice; live/TOS has no process-global storage port.
    # Project-scoped routes revalidate the profile/bucket binding before each operation.
    app.state.storage_port = (
        LocalWorkspaceAdapter(runtime_settings.workspace_root)
        if runtime_settings.storage_mode == "local_workspace"
        else None
    )
    # One immutable coordinator is shared by all paid/media owners so admission
    # snapshots use a single resource/capacity observation and revision.
    shared_resource_snapshot = probe_resources(runtime_settings.workspace_root)
    shared_resilience = OperationsResilienceCoordinator(
        shared_resource_snapshot,
        capacity_snapshot(shared_resource_snapshot, "*"),
    )
    asset_resilience_by_scope: dict[str, OperationsResilienceCoordinator] = {}

    def asset_resilience(scope: str) -> OperationsResilienceCoordinator:
        coordinator = asset_resilience_by_scope.get(scope)
        if coordinator is None:
            resource = probe_resources(runtime_settings.workspace_root)
            coordinator = OperationsResilienceCoordinator(
                resource, capacity_snapshot(resource, scope)
            )
            asset_resilience_by_scope[scope] = coordinator
        return coordinator

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
            assets = AssetsService(
                cast(AssetsUnitOfWorkFactory, uow_factory),
                resilience_factory=asset_resilience,
            )
    app.state.projects_episodes_service = service
    app.state.assets_service = assets
    app.state.creative_service = (
        CreativeService(cast(object, service._uow_factory)) if service is not None else None
    )
    media_owner_service = (
        MediaOwnerService(cast(object, service._uow_factory), resilience=shared_resilience)
        if service is not None
        else None
    )
    scenes_service = (
        ScenesService(cast(object, service._uow_factory), media_owner=media_owner_service)
        if service is not None
        else None
    )
    app.state.scenes_service = scenes_service
    app.state.catalog_service = (
        CatalogService(cast(object, service._uow_factory)) if service is not None else None
    )
    app.state.runtime_composition = (
        CatalogRuntimeComposition(
            CatalogRuntimeResolver(cast(object, service._uow_factory)),
            app.state.catalog_service,
        )
        if service is not None and app.state.catalog_service is not None
        else None
    )
    app.state.live_runtime_composition = (
        app.state.runtime_composition if runtime_settings.provider_mode == "live" else None
    )
    app.state.timeline_service = (
        TimelineService(cast(object, service._uow_factory)) if service is not None else None
    )
    app.state.agent_edit_service = (
        AgentEditService(cast(object, service._uow_factory), scenes_service)
        if service is not None
        else None
    )
    app.state.storage_profile_service = (
        StorageProfileService(
            cast(object, service._uow_factory), storage_mode=app.state.runtime.storage_mode
        )
        if service is not None
        else None
    )
    app.state.media_owner_service = media_owner_service
    app.state.opaque_read_grants = LocalOpaqueReadGrantIssuer()
    # Environment paths do not establish a frozen renderer profile. Startup resolves
    # the durable catalog identity after bootstrap; until then exports fail closed.
    renderer = SubprocessFfmpegRenderAdapter(None, None)
    app.state.renderer_composition_error = None
    app.state.export_service = (
        ExportService(
            cast(object, service._uow_factory),
            renderer,
            app.state.opaque_read_grants,
            app.state.storage_port,
            shared_resilience,
            renderer_identity=None,
        )
        if service is not None
        else None
    )
    app.state.asset_bible_service = (
        AssetBibleService(cast(AssetBibleUnitOfWorkFactory, service._uow_factory))
        if service is not None
        else None
    )
    app.state.runs_service = (
        RunsService(cast(object, service._uow_factory), scenes_service)
        if service is not None
        else None
    )
    app.state.text_generation_service = (
        TextGenerationService(
            cast(object, service._uow_factory),
            app.state.runtime.provider,
            cast(CatalogService, app.state.catalog_service),
            resilience=shared_resilience,
            live_composition=app.state.live_runtime_composition,
        )
        if service is not None
        else None
    )
    app.state.video_generation_service = (
        AgnesVideoService(
            cast(object, service._uow_factory),
            app.state.runtime.provider,
            cast(CatalogService, app.state.catalog_service),
            app.state.storage_port,
            assets,
            resilience=shared_resilience,
            live_composition=app.state.live_runtime_composition,
            media_owner=media_owner_service,
        )
        if service is not None and app.state.catalog_service is not None
        else None
    )
    app.state.image_generation_service = (
        ImageGenerationService(
            cast(object, service._uow_factory),
            app.state.runtime.provider,
            app.state.storage_port,
            cast(CatalogService, app.state.catalog_service),
            assets,
            resilience=shared_resilience,
            live_composition=app.state.live_runtime_composition,
        )
        if service is not None and assets is not None and app.state.catalog_service is not None
        else None
    )
    skill_registry = SkillRegistry(Path(__file__).resolve().parents[2] / "skill_registry")
    skill_registry.load()
    app.state.skill_registry = skill_registry
    app.state.skill_routing_service = (
        SkillRoutingService(cast(object, service._uow_factory), skill_registry)
        if service is not None and not skill_registry.errors
        else None
    )
    app.state.source_material_service = (
        SourceMaterialService(cast(object, service._uow_factory)) if service is not None else None
    )
    probe = readiness_probe or default_readiness_assessment

    @app.exception_handler(DomainError)
    async def handle_domain_error(request: Request, error: DomainError) -> JSONResponse:
        status_code = (
            503
            if error.code
            in {
                "database_unavailable",
                "credential_master_key_unavailable",
                "asset_edit_unconfigured",
                "renderer_unconfigured",
                "renderer_capability_unsupported",
            }
            else 422
        )
        if error.code in {
            "project_not_found",
            "episode_not_found",
            "workflow_run_not_found",
            "storage_profile_not_found",
            "asset_edit_not_found",
        }:
            status_code = 404
        elif error.code == "project_access_forbidden":
            status_code = 403
        elif error.code in {
            "episode_number_conflict",
            "revision_conflict",
            "workflow_version_unavailable",
            "workflow_source_conflict",
            "workflow_run_conflict",
            "base_version_conflict",
            "continuity_stale",
            "storage_profile_revision_conflict",
        }:
            status_code = 409
        trace_id = getattr(getattr(request.state, "trace_context", None), "trace_id", None)
        return JSONResponse(
            status_code=status_code,
            content={"detail": {"type": error.code, "message": str(error), "traceId": trace_id}},
        )

    @app.exception_handler(RequestValidationError)
    async def handle_request_validation(
        request: Request, error: RequestValidationError
    ) -> JSONResponse:
        return JSONResponse(
            status_code=422,
            content={
                "detail": {
                    "type": "validation",
                    "message": "request validation failed",
                    "errors": jsonable_encoder(error.errors()),
                    "traceId": getattr(
                        getattr(request.state, "trace_context", None), "trace_id", None
                    ),
                }
            },
        )

    @app.get("/v1/health/live")
    def live() -> dict[str, str]:
        log_event("api.health.live")
        return {"status": "live"}

    @app.get("/v1/health/ready")
    def ready() -> dict[str, str]:
        result = probe()
        assessment = (
            result
            if isinstance(result, ReadinessAssessment)
            else ReadinessAssessment("ready" if result else "not_ready", ())
        )
        if not assessment.ready:
            log_event("api.health.ready", status=assessment.status)
            raise HTTPException(
                status_code=503,
                detail={"status": assessment.status, "diagnostics": assessment.diagnostics},
            )
        log_event("api.health.ready", status="ready")
        return {"status": "ready"}

    app.include_router(projects_episodes_router)
    app.include_router(assets_router, dependencies=[Depends(require_project_scope)])
    app.include_router(creative_router, dependencies=[Depends(require_project_scope)])
    app.include_router(scenes_router, dependencies=[Depends(require_project_scope)])
    app.include_router(catalog_router)
    app.include_router(phase_one_router)
    app.include_router(text_generation_router, dependencies=[Depends(require_project_scope)])
    app.include_router(image_generation_router, dependencies=[Depends(require_project_scope)])
    app.include_router(video_generation_router, dependencies=[Depends(require_project_scope)])
    app.include_router(agent_edit_router, dependencies=[Depends(require_project_scope)])

    @app.on_event("startup")
    async def bootstrap_catalog() -> None:
        if app.state.catalog_service is not None:
            await app.state.catalog_service.bootstrap()
        if (
            runtime_settings.renderer_profile_id
            and app.state.runtime_composition is not None
            and app.state.export_service is not None
        ):
            try:
                composed_renderer = await app.state.runtime_composition.resolve_renderer(
                    profile_id=runtime_settings.renderer_profile_id,
                    ffmpeg_path=runtime_settings.ffmpeg_path,
                    ffprobe_path=runtime_settings.ffprobe_path,
                    renderer_factory=SubprocessFfmpegRenderAdapter,
                )
                app.state.export_service.configure_renderer(
                    composed_renderer.port,
                    {
                        "profileId": composed_renderer.profile_id,
                        "profileRevision": composed_renderer.revision,
                        "capabilitySnapshotId": composed_renderer.capability_snapshot_id,
                        "capabilityRevision": composed_renderer.capability_revision,
                        "snapshotId": composed_renderer.snapshot_id,
                    },
                )
            except ValidationDomainError as error:
                # Keep the adapter explicitly unconfigured and retain a bounded diagnostic;
                # a failed catalog composition must never look like a valid renderer.
                app.state.renderer_composition_error = f"{type(error).__name__}: {error}"
                log_event(
                    "api.renderer.composition_failed",
                    diagnostic=app.state.renderer_composition_error,
                )

    return app


app = create_app()
