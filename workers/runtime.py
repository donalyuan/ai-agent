"""Temporal Worker 的最小可轮询阶段 0 入口。"""

from __future__ import annotations

import asyncio
import json
import os
from collections.abc import Awaitable, Callable, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from temporalio import activity
from temporalio.client import Client
from temporalio.worker import UnsandboxedWorkflowRunner, Worker
from video_agent_api.logging import log_event
from video_agent_api.observability import child_context, parse_traceparent
from video_agent_api.runtime import RuntimeComponents, build_runtime_from_env


@dataclass(frozen=True)
class WorkerHealth:
    status: str
    task_queue: str


@activity.defn(name="phase_zero_health")
async def phase_zero_health_activity() -> dict[str, str]:
    """无副作用活动证明 Worker 已注册可由 Temporal 轮询的入口。"""
    return {"status": "ready"}


def _marker_path() -> Path:
    return Path(
        os.environ.get("WORKER_HEALTH_MARKER", "/tmp/video-agent-worker-health.json")
    )


def health(task_queue: str) -> WorkerHealth:
    marker = _marker_path()
    try:
        payload = json.loads(marker.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return WorkerHealth(status="unavailable", task_queue=task_queue)
    if payload.get("task_queue") != task_queue or payload.get("status") != "ready":
        return WorkerHealth(status="unavailable", task_queue=task_queue)
    return WorkerHealth(status="ready", task_queue=task_queue)


def configure_runtime() -> RuntimeComponents:
    """Worker 与 API 共用同一严格配置装配，不各自选择 Adapter。"""
    return build_runtime_from_env()


async def serve(
    task_queue: str,
    temporal_address: str,
    *,
    workflows: Sequence[type[Any]] = (),
    activities: Sequence[Any] = (),
    background_services: Sequence[Callable[[Client], Awaitable[None]]] = (),
) -> None:
    """连接 Temporal 后才写入健康标记，然后开始轮询指定队列。"""
    configure_runtime()
    root = parse_traceparent(os.environ.get("TRACEPARENT"))
    worker_context = child_context(root)
    marker = _marker_path()
    marker.unlink(missing_ok=True)
    client = await Client.connect(temporal_address)
    marker.parent.mkdir(parents=True, exist_ok=True)
    marker.write_text(
        json.dumps({"status": "ready", "task_queue": task_queue}), encoding="utf-8"
    )
    log_event(
        "worker.temporal.connected",
        correlation_id=worker_context.trace_id,
        task_queue=task_queue,
        trace_id=worker_context.trace_id,
        span_id=worker_context.span_id,
    )
    worker = Worker(
        client,
        task_queue=task_queue,
        workflows=list(workflows),
        activities=[phase_zero_health_activity, *activities],
        workflow_runner=UnsandboxedWorkflowRunner(),
    )
    if not background_services:
        await worker.run()
        return
    await asyncio.gather(
        worker.run(), *(service(client) for service in background_services)
    )
