"""Temporal Worker 的最小可轮询阶段 0 入口。"""

from __future__ import annotations

import json
import os
from dataclasses import dataclass
from pathlib import Path

from temporalio import activity
from temporalio.client import Client
from temporalio.worker import Worker
from video_agent_api.logging import log_event
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


async def serve(task_queue: str, temporal_address: str) -> None:
    """连接 Temporal 后才写入健康标记，然后开始轮询指定队列。"""
    configure_runtime()
    marker = _marker_path()
    marker.unlink(missing_ok=True)
    client = await Client.connect(temporal_address)
    marker.parent.mkdir(parents=True, exist_ok=True)
    marker.write_text(
        json.dumps({"status": "ready", "task_queue": task_queue}), encoding="utf-8"
    )
    log_event("worker.temporal.connected", task_queue=task_queue)
    worker = Worker(
        client,
        task_queue=task_queue,
        workflows=[],
        activities=[phase_zero_health_activity],
    )
    await worker.run()
