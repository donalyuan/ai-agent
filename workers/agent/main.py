"""Agent Worker runtime composition; model/provider work remains behind owned ports."""

from __future__ import annotations

import argparse
import asyncio
import os

from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine
from temporalio.client import Client
from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
from video_agent_api.adapters.temporal import TemporalRunStarter
from video_agent_api.adapters.temporal_workflow import ACTIVITIES, WORKFLOWS
from video_agent_api.agent_runtime import compose_agent_runtime
from video_agent_api.application.run_dispatch import RunDispatchService

from workers.runtime import health as read_health
from workers.runtime import serve

TASK_QUEUE = os.environ.get("AGENT_TASK_QUEUE", "agent-tasks")


def health():
    return read_health(TASK_QUEUE)


async def _run() -> None:
    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        raise RuntimeError("DATABASE_URL is required for the Agent Worker")
    engine = create_async_engine(database_url)
    uow_factory = make_sqlalchemy_uow_factory(
        async_sessionmaker(engine, expire_on_commit=False)
    )
    dispatcher = RunDispatchService(uow_factory)

    async def dispatch_loop(client: Client) -> None:
        starter = TemporalRunStarter(client, TASK_QUEUE)
        while True:
            await dispatcher.dispatch_pending(starter)
            await asyncio.sleep(1)

    try:
        await serve(
            TASK_QUEUE,
            os.environ.get("TEMPORAL_ADDRESS", "localhost:7233"),
            workflows=WORKFLOWS,
            activities=ACTIVITIES,
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
    compose_agent_runtime()
    asyncio.run(_run())


if __name__ == "__main__":
    main()
