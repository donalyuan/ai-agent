"""Generation Worker: DB-backed owner activities on the Generation route."""

from __future__ import annotations

import argparse
import asyncio
import os

from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine
from temporalio.client import Client
from video_agent_api.adapters.generation_temporal import (
    GENERATION_ACTIVITIES,
    GENERATION_WORKFLOWS,
    GenerationActivityDependencies,
    TemporalGenerationStarter,
    configure_generation_activities,
)
from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
from video_agent_api.application.assets import AssetsService
from video_agent_api.application.catalog import CatalogService
from video_agent_api.application.generation_dispatch import (
    GENERATION_TASK_QUEUE,
    GenerationOutboxDispatcher,
)
from video_agent_api.application.image_generation import ImageGenerationService
from video_agent_api.application.runtime_composition import (
    CatalogRuntimeComposition,
    CatalogRuntimeResolver,
)
from video_agent_api.application.text_generation import TextGenerationService
from video_agent_api.application.video_generation import AgnesVideoService
from video_agent_api.db import default_readiness_assessment
from video_agent_api.ports.storage import LocalWorkspaceAdapter
from video_agent_api.resilience import (
    OperationsResilienceCoordinator,
    capacity_snapshot,
    probe_resources,
)
from video_agent_api.runtime import RuntimeSettings, build_runtime

from workers.runtime import health as read_health
from workers.runtime import serve

TASK_QUEUE = os.environ.get("GENERATION_TASK_QUEUE", GENERATION_TASK_QUEUE)
WORKFLOWS = GENERATION_WORKFLOWS
ACTIVITIES = GENERATION_ACTIVITIES


def health():
    return read_health(TASK_QUEUE)


async def _run() -> None:
    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        raise RuntimeError("DATABASE_URL is required for the Generation Worker")
    assessment = await asyncio.to_thread(default_readiness_assessment)
    if not assessment.ready:
        raise RuntimeError(
            "Generation Worker readiness failed: "
            + ", ".join((assessment.status, *assessment.diagnostics))
        )
    engine = create_async_engine(database_url)
    uow_factory = make_sqlalchemy_uow_factory(
        async_sessionmaker(engine, expire_on_commit=False)
    )
    settings = RuntimeSettings.from_env()
    runtime = build_runtime(settings)
    # Storage is selected explicitly by the local catalog profile; live/TOS remains
    # unconfigured until a project-scoped frozen profile is admitted.
    storage_port = (
        LocalWorkspaceAdapter(settings.workspace_root)
        if settings.storage_mode == "local_workspace"
        else None
    )
    resource = probe_resources(settings.workspace_root)
    resilience = OperationsResilienceCoordinator(
        resource, capacity_snapshot(resource, "*")
    )
    catalog = CatalogService(uow_factory)
    assets = AssetsService(uow_factory)
    live_composition = (
        CatalogRuntimeComposition(CatalogRuntimeResolver(uow_factory), catalog)
        if settings.provider_mode == "live"
        else None
    )
    remote_lookups = ()
    if live_composition is not None:
        # Composition reads frozen catalog contracts only.  It never probes a
        # remote provider and injects no port when the lookup protocol is absent.
        remote_lookups = await live_composition.resolve_remote_lookups(
            await catalog.remote_lookup_bindings()
        )
    configure_generation_activities(
        GenerationActivityDependencies(
            text=TextGenerationService(
                uow_factory,
                runtime.provider,
                catalog,
                resilience=resilience,
                live_composition=live_composition,
            ),
            image=ImageGenerationService(
                uow_factory,
                runtime.provider,
                storage_port,
                catalog,
                assets,
                resilience=resilience,
                live_composition=live_composition,
            ),
            video=AgnesVideoService(
                uow_factory,
                runtime.provider,
                catalog,
                storage_port,
                assets,
                resilience=resilience,
                live_composition=live_composition,
            ),
            remote_lookups=remote_lookups,
        )
    )
    dispatcher = GenerationOutboxDispatcher(uow_factory)

    async def dispatch_loop(client: Client) -> None:
        starter = TemporalGenerationStarter(client, TASK_QUEUE)
        while True:
            await dispatcher.dispatch_pending(starter)
            await asyncio.sleep(1)

    try:
        await serve(
            TASK_QUEUE,
            os.environ.get("TEMPORAL_ADDRESS", "localhost:7233"),
            workflows=GENERATION_WORKFLOWS,
            activities=GENERATION_ACTIVITIES,
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
