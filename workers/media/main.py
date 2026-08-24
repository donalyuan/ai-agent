"""Media Worker queue entry; export activities require explicit adapter composition."""

from __future__ import annotations

import argparse
import asyncio
import os

from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine
from temporalio.client import Client
from video_agent_api.adapters.export_temporal import (
    EpisodeExportWorkflow,
    TemporalExportStarter,
    configure_episode_export_activity,
    episode_export_execute,
)
from video_agent_api.adapters.ffmpeg import SubprocessFfmpegRenderAdapter
from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
from video_agent_api.application.export_dispatch import ExportDispatchService
from video_agent_api.application.export_worker import EpisodeExportWorker
from video_agent_api.runtime import RuntimeSettings, build_runtime

from workers.runtime import health as read_health
from workers.runtime import serve

TASK_QUEUE = os.environ.get("MEDIA_TASK_QUEUE", "media-tasks")
MEDIA_WORKFLOWS = (EpisodeExportWorkflow,)
MEDIA_ACTIVITIES = (episode_export_execute,)


def health():
    return read_health(TASK_QUEUE)


async def _run() -> None:
    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        raise RuntimeError("DATABASE_URL is required for the Media Worker")
    engine = create_async_engine(database_url)
    uow_factory = make_sqlalchemy_uow_factory(
        async_sessionmaker(engine, expire_on_commit=False)
    )
    settings = RuntimeSettings.from_env()
    runtime = build_runtime(settings)
    worker = EpisodeExportWorker(
        uow_factory,
        SubprocessFfmpegRenderAdapter(
            os.environ.get("FFMPEG_PATH"), os.environ.get("FFPROBE_PATH")
        ),
        runtime.storage,
    )
    configure_episode_export_activity(worker, settings.workspace_root / "exports")
    dispatcher = ExportDispatchService(uow_factory)

    async def dispatch_loop(client: Client) -> None:
        starter = TemporalExportStarter(client, TASK_QUEUE)
        while True:
            await dispatcher.dispatch_pending(starter)
            await asyncio.sleep(1)

    try:
        await serve(
            TASK_QUEUE,
            os.environ.get("TEMPORAL_ADDRESS", "localhost:7233"),
            workflows=MEDIA_WORKFLOWS,
            activities=MEDIA_ACTIVITIES,
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
