from __future__ import annotations

import ast
from pathlib import Path

import pytest

from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.domain.runs import (
    NODE_RUN_STATUSES,
    NODE_RUN_TRANSITIONS,
    RUN_STATUSES,
    RUN_TERMINAL,
    RUN_TRANSITIONS,
    NodeRun,
    WorkflowRun,
)

ROOT = Path(__file__).parents[3]
API_PACKAGE = ROOT / "services/api/src/video_agent_api"


def test_workflows_change_traces_plan_and_shared_exit_requirements() -> None:
    archive = ROOT / "openspec/changes/archive"
    plan_path = next(archive.glob("*-plan-phase-one-drama-mvp-a/tasks.md"))
    change_path = next(archive.glob("*-implement-workflows-runs-slice"))
    plan = plan_path.read_text()
    tasks = (change_path / "tasks.md").read_text()
    spec = (change_path / "specs/workflows-runs/spec.md").read_text()
    assert "2.2 实施 `implement-workflows-runs-slice`" in plan
    for shared in ("5.1", "5.2", "5.3", "5.5"):
        assert f"`{shared}`" in tasks
    for excluded_owner in (
        "ProviderCall",
        "真实 Provider SDK",
        "AgentScope",
        "FFmpeg",
        "Timeline",
        "Temporal 内部表",
    ):
        assert excluded_owner in tasks
    assert "RunEvent" in spec and "workflows/runs" in spec


def test_run_and_node_state_tables_are_total_and_terminal() -> None:
    assert set(RUN_TRANSITIONS) == RUN_STATUSES
    assert set(NODE_RUN_TRANSITIONS) == NODE_RUN_STATUSES
    assert all(not RUN_TRANSITIONS[state] for state in RUN_TERMINAL)
    assert all(
        not NODE_RUN_TRANSITIONS[state] for state in {"succeeded", "failed", "cancelled", "skipped"}
    )
    run = WorkflowRun("project", "workflow")
    node = NodeRun(run.id, "text.generate")
    run.nodes = [node]
    run.transition("running")
    node.transition("running")
    node.transition("waiting_review")
    run.recompute_from_nodes()
    assert run.status == "waiting_review"
    node.transition("succeeded")
    run.recompute_from_nodes()
    assert run.status == "succeeded"
    with pytest.raises(ValidationDomainError, match="invalid workflow run transition"):
        run.transition("running")


def test_workflows_layers_and_run_event_owner_boundary() -> None:
    interface = ast.parse((API_PACKAGE / "interfaces/http/phase_one.py").read_text())
    imports = {
        node.module
        for node in ast.walk(interface)
        if isinstance(node, ast.ImportFrom) and node.module is not None
    }
    assert "video_agent_api.application.runs" in imports
    assert "video_agent_api.domain.runs" not in imports

    application = ast.parse((API_PACKAGE / "application/runs.py").read_text())
    app_imports = {
        node.module
        for node in ast.walk(application)
        if isinstance(node, ast.ImportFrom) and node.module is not None
    }
    assert "video_agent_api.domain.runs" in app_imports
    assert not any(module.startswith("video_agent_api.interfaces") for module in app_imports)

    for folder in ("application", "domain"):
        for path in (API_PACKAGE / folder).glob("*.py"):
            if path.name == "runs.py":
                continue
            source = path.read_text()
            assert "RunEvent" not in source, path
            assert ".run_events" not in source, path

    run_application = (API_PACKAGE / "application/runs.py").read_text()
    for forbidden in ("ProviderCall", "AgentScope", "FFmpeg", "TimelineDocument"):
        assert forbidden not in run_application
