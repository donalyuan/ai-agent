"""Media Worker queue entry; export activities require explicit adapter composition."""

from __future__ import annotations

import argparse
import asyncio
import os

from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine
from temporalio.client import Client
from workers.runtime import health as read_health
from workers.runtime import serve

from video_agent_api.adapters.export_temporal import (
    EpisodeExportWorkflow,
    TemporalExportStarter,
    configure_episode_export_activity,
    episode_export_execute,
)
from video_agent_api.adapters.ffmpeg import SubprocessFfmpegRenderAdapter
from video_agent_api.adapters.media_temporal import (
    MEDIA_ACTIVITIES as MEDIA_RUNTIME_ACTIVITIES,
)
from video_agent_api.adapters.media_temporal import (
    MEDIA_WORKFLOWS as MEDIA_RUNTIME_WORKFLOWS,
)
from video_agent_api.adapters.media_temporal import (
    MediaActivityDependencies,
    TemporalMediaStarter,
    configure_media_activities,
)
from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
from video_agent_api.application.assets import AssetsService
from video_agent_api.application.catalog import CatalogService
from video_agent_api.application.export_dispatch import ExportDispatchService
from video_agent_api.application.export_worker import EpisodeExportWorker
from video_agent_api.application.media import MediaDispatchService, MediaOwnerService
from video_agent_api.application.runtime_composition import (
    CatalogRuntimeComposition,
    CatalogRuntimeResolver,
)
from video_agent_api.db import default_readiness_assessment
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.ports.storage import LocalWorkspaceAdapter
from video_agent_api.providers.media_inspect import LocalMediaInspector
from video_agent_api.resilience import (
    OperationsResilienceCoordinator,
    capacity_snapshot,
    probe_resources,
)
from video_agent_api.runtime import RuntimeSettings, build_runtime

TASK_QUEUE = os.environ.get("MEDIA_TASK_QUEUE", "media-tasks")
MEDIA_WORKFLOWS = (EpisodeExportWorkflow, *MEDIA_RUNTIME_WORKFLOWS)
# Keep the historical public export-only tuple stable; the worker uses the full tuple below.
MEDIA_ACTIVITIES = (episode_export_execute,)
MEDIA_ALL_ACTIVITIES = (episode_export_execute, *MEDIA_RUNTIME_ACTIVITIES)


def health():
    return read_health(TASK_QUEUE)


async def _run() -> None:
    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        raise RuntimeError("DATABASE_URL is required for the Media Worker")
    assessment = await asyncio.to_thread(default_readiness_assessment)
    if not assessment.ready:
        raise RuntimeError(
            "Media Worker readiness failed: "
            + ", ".join((assessment.status, *assessment.diagnostics))
        )
    engine = create_async_engine(database_url)
    uow_factory = make_sqlalchemy_uow_factory(async_sessionmaker(engine, expire_on_commit=False))
    settings = RuntimeSettings.from_env()
    _runtime = build_runtime(settings)
    # Do not expose the runtime's generic storage placeholder to activities. Local
    # storage is an explicit offline adapter; live/TOS must be composed per profile.
    storage_port = (
        LocalWorkspaceAdapter(settings.workspace_root)
        if settings.storage_mode == "local_workspace"
        else None
    )
    catalog = CatalogService(uow_factory)
    runtime_composition = CatalogRuntimeComposition(CatalogRuntimeResolver(uow_factory), catalog)
    # Paths alone are not an admission. Keep renderer unconfigured until a persisted
    # profile composition succeeds; never execute an environment-only fallback.
    renderer = SubprocessFfmpegRenderAdapter(None, None)
    renderer_identity: dict[str, object] | None = None
    if settings.renderer_profile_id:
        try:
            composed_renderer = await runtime_composition.resolve_renderer(
                profile_id=settings.renderer_profile_id,
                ffmpeg_path=settings.ffmpeg_path,
                ffprobe_path=settings.ffprobe_path,
                renderer_factory=SubprocessFfmpegRenderAdapter,
            )
            renderer = composed_renderer.port
            renderer_identity = {
                "profileId": composed_renderer.profile_id,
                "profileRevision": composed_renderer.revision,
                "capabilitySnapshotId": composed_renderer.capability_snapshot_id,
                "capabilityRevision": composed_renderer.capability_revision,
                "snapshotId": composed_renderer.snapshot_id,
            }
        except ValidationDomainError as error:
            # Do not continue with an unconfigured renderer after an explicit profile
            # failed composition; startup must remain fail-closed and diagnosable.
            raise RuntimeError(
                f"Media Worker renderer composition failed: {type(error).__name__}"
            ) from error
    resource = probe_resources(settings.workspace_root)
    resilience = OperationsResilienceCoordinator(resource, capacity_snapshot(resource, "*"))
    worker = EpisodeExportWorker(
        uow_factory,
        renderer,
        storage_port,
        resilience=resilience,
        renderer_identity=renderer_identity,
    )
    configure_media_activities(
        MediaActivityDependencies(
            MediaOwnerService(uow_factory, resilience=resilience),
            storage_port,
            LocalMediaInspector(workspace_root=settings.workspace_root),
            worker._renderer,
            assets=AssetsService(uow_factory),
            exports=worker._exports,
        )
    )
    configure_episode_export_activity(worker, settings.workspace_root / "exports")
    dispatcher = ExportDispatchService(uow_factory)
    media_dispatcher = MediaDispatchService(uow_factory)

    async def dispatch_loop(client: Client) -> None:
        starter = TemporalExportStarter(client, TASK_QUEUE)
        media_starter = TemporalMediaStarter(client, TASK_QUEUE)
        while True:
            await dispatcher.dispatch_pending(starter)
            await media_dispatcher.dispatch_pending(media_starter)
            await asyncio.sleep(1)

    try:
        await serve(
            TASK_QUEUE,
            os.environ.get("TEMPORAL_ADDRESS", "localhost:7233"),
            workflows=MEDIA_WORKFLOWS,
            activities=MEDIA_ALL_ACTIVITIES,
            background_services=(dispatch_loop,),
        )
    finally:
        await engine.dispose()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--health", action="store_true")
    args = parser.parse_args()
    if args.health:
        raise SystemExit(0 if health().status == "ready" else 1)
    asyncio.run(_run())


if __name__ == "__main__":
    main()
