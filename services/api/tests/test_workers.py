from __future__ import annotations

import importlib.util
import sys
from pathlib import Path

import pytest


@pytest.mark.parametrize(
    ("folder", "queue"),
    [("agent", "agent-tasks"), ("generation", "generation-tasks"), ("media", "media-tasks")],
)
def test_workers_declare_distinct_temporal_queues(folder: str, queue: str) -> None:
    repository_root = Path(__file__).parents[3]
    sys.path.insert(0, str(repository_root))
    path = repository_root / "workers" / folder / "main.py"
    spec = importlib.util.spec_from_file_location(f"{folder}_worker", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    assert module.health().task_queue == queue
    assert module.health().status == "unavailable"
