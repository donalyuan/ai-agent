"""Deterministic phase-one Temporal workflow and orchestration-only activity."""

from __future__ import annotations

from datetime import timedelta

from temporalio import activity, workflow


def _required_string(payload: dict[str, object], key: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{key} is required")
    return value


@activity.defn(name="phase_one_operation_checkpoint")
async def phase_one_operation_checkpoint(payload: dict[str, object]) -> dict[str, str]:
    """Execute text generation in the activity boundary, retaining a checkpoint for other ops."""
    logical_operation = _required_string(payload, "logicalOperation")
    if logical_operation.startswith("text.generate") and payload.get("projectId"):
        return await _execute_text_generation(payload)
    return {
        "status": "ready",
        "runId": _required_string(payload, "runId"),
        "nodeRunId": _required_string(payload, "nodeRunId"),
        "logicalOperation": logical_operation,
    }


async def _execute_text_generation(payload: dict[str, object]) -> dict[str, str]:
    """Load owner snapshots in the Worker and close the text review gate idempotently."""
    import os

    from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

    from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
    from video_agent_api.application.catalog import CatalogService
    from video_agent_api.application.runs import RunsService
    from video_agent_api.application.text_generation import (
        GenerateTextBatchCommand,
        TextGenerationService,
    )
    from video_agent_api.ports.contracts import ModelSelection
    from video_agent_api.runtime import build_runtime_from_env

    database_url = os.environ.get("DATABASE_URL")
    if not database_url:
        raise RuntimeError("DATABASE_URL is required for text activity")
    run_id = _required_string(payload, "runId")
    node_run_id = _required_string(payload, "nodeRunId")
    project_id = _required_string(payload, "projectId")
    engine = create_async_engine(database_url)
    try:
        uow_factory = make_sqlalchemy_uow_factory(
            async_sessionmaker(engine, expire_on_commit=False)
        )
        runtime = build_runtime_from_env()
        catalog = CatalogService(uow_factory)
        text_service = TextGenerationService(uow_factory, runtime.provider, catalog)
        async with uow_factory() as uow:
            project = await uow.projects.get(project_id)
            run = uow.workflow_runs.get(run_id)
            if project is None or run is None or run.project_id != project_id:
                raise ValueError("text activity Project/Run scope is invalid")
            node = next((item for item in run.nodes if item.id == node_run_id), None)
            brief = getattr(project, "creative_brief_current", None) or (
                uow.creative_brief_current.get(project_id)
            )
            if node is None or brief is None:
                raise ValueError("text activity owner snapshot is unavailable")
            source = getattr(project, "source_binding_current", None) or (
                uow.source_bindings_current.get(project_id)
            )
            selection_snapshot = dict(run.selection_snapshot)
            selection = ModelSelection(
                _required_string(selection_snapshot, "providerId"),
                _required_string(selection_snapshot, "profileId"),
                _required_string(selection_snapshot, "modelId"),
                _required_string(selection_snapshot, "adapterKey"),
            )
            expected_node_revision = node.revision
            command = GenerateTextBatchCommand(
                project_id=project_id,
                run_id=run_id,
                brief_revision=brief.revision,
                selection=selection,
                brief_snapshot=brief,
                source_binding_snapshot=source,
                scope_ids=(project_id,),
                correlation_id=run_id,
            )
        batch = await text_service.generate(command)
        await RunsService(uow_factory, None).enter_text_review(
            run_id, node_run_id, batch.id, expected_node_revision
        )
        return {
            "status": "waiting_review",
            "runId": run_id,
            "nodeRunId": node_run_id,
            "logicalOperation": _required_string(payload, "logicalOperation"),
            "batchId": batch.id,
        }
    finally:
        await engine.dispose()


def _agnes_activity_payload(action: str, payload: dict[str, object]) -> dict[str, str]:
    """Post-commit boundary: activities carry an idempotency envelope only."""
    run_id = _required_string(payload, "runId")
    logical_operation = _required_string(payload, "logicalOperation")
    return {
        "status": "ready",
        "action": action,
        "runId": run_id,
        "logicalOperation": logical_operation,
    }


@activity.defn(name="agnes_video_submit")
async def agnes_video_submit(payload: dict[str, object]) -> dict[str, str]:
    return _agnes_activity_payload("submit", payload)


@activity.defn(name="agnes_video_poll")
async def agnes_video_poll(payload: dict[str, object]) -> dict[str, str]:
    return _agnes_activity_payload("poll", payload)


@activity.defn(name="agnes_video_cancel")
async def agnes_video_cancel(payload: dict[str, object]) -> dict[str, str]:
    return _agnes_activity_payload("cancel", payload)


@activity.defn(name="agnes_video_result")
async def agnes_video_result(payload: dict[str, object]) -> dict[str, str]:
    return _agnes_activity_payload("result", payload)


@workflow.defn(name="phase_one_run")
class PhaseOneRunWorkflow:
    def __init__(self) -> None:
        self._cancel_requested = False

    @workflow.signal(name="cancel")
    async def cancel(self) -> None:
        self._cancel_requested = True

    @workflow.run
    async def run(self, payload: dict[str, object]) -> dict[str, str]:
        run_id = _required_string(payload, "runId")
        logical_operation = _required_string(payload, "logicalOperation")
        selection = payload.get("selectionSnapshot")
        if not isinstance(selection, dict) or (
            selection.get("provider") != "mock"
            or selection.get("profile") != "local-test-offline"
            or selection.get("adapterIdentity") != "local_workspace"
        ):
            return {
                "status": "unconfigured",
                "runId": run_id,
                "logicalOperation": logical_operation,
            }
        if self._cancel_requested:
            return {
                "status": "cancelled",
                "runId": run_id,
                "logicalOperation": logical_operation,
            }
        return await workflow.execute_activity(
            phase_one_operation_checkpoint,
            payload,
            activity_id=f"{run_id}:{logical_operation}",
            start_to_close_timeout=timedelta(seconds=30),
        )


WORKFLOWS = (PhaseOneRunWorkflow,)
ACTIVITIES = (
    phase_one_operation_checkpoint,
    agnes_video_submit,
    agnes_video_poll,
    agnes_video_cancel,
    agnes_video_result,
)
