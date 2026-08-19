"""Agent Worker 的阶段 0 Temporal 队列入口，不注册真实产品 workflow。"""

from __future__ import annotations

import argparse
import asyncio
import os

from workers.runtime import health as read_health
from workers.runtime import serve

TASK_QUEUE = os.environ.get("AGENT_TASK_QUEUE", "agent-tasks")


def health():
    return read_health(TASK_QUEUE)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--health", action="store_true")
    args = parser.parse_args()
    if args.health:
        raise SystemExit(0 if health().status == "ready" else 1)
    asyncio.run(serve(TASK_QUEUE, os.environ.get("TEMPORAL_ADDRESS", "localhost:7233")))


if __name__ == "__main__":
    main()
