from __future__ import annotations

from pathlib import Path

import pytest
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine
from temporalio.api.common.v1 import Payload
from temporalio.exceptions import WorkflowAlreadyStartedError

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.adapters.sqlalchemy import SQLAlchemyUnitOfWork
from video_agent_api.adapters.sqlalchemy_models import Base
from video_agent_api.adapters.temporal import TemporalLaunch, TemporalRunStarter
from video_agent_api.adapters.temporal_workflow import (
    ACTIVITIES,
    WORKFLOWS,
    PhaseOneRunWorkflow,
    agnes_video_cancel,
    agnes_video_poll,
    agnes_video_result,
    agnes_video_submit,
    phase_one_operation_checkpoint,
)
from video_agent_api.application.catalog import CatalogService
from video_agent_api.application.creative import CreativeService, SaveCreativeBriefCommand
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.run_dispatch import RunDispatchService
from video_agent_api.domain.errors import WorkflowUnconfiguredError
from video_agent_api.domain.runs import (
    NodeRun,
    ProjectDefaultWorkflowBinding,
    TemporalStart,
    WorkflowRun,
    WorkflowVersion,
    temporal_workflow_id,
)


class FakeTemporalClient:
    def __init__(self, already_started: bool = False) -> None:
        self.already_started = already_started
        self.calls: list[dict[str, object]] = []

    async def start_workflow(
        self,
        workflow: str,
        arg: object,
        *,
        id: str,
        task_queue: str,
        headers: dict[str, Payload],
    ) -> object:
        self.calls.append(
            {
                "workflow": workflow,
                "arg": arg,
                "id": id,
                "taskQueue": task_queue,
                "headers": headers,
            }
        )
        if self.already_started:
            raise WorkflowAlreadyStartedError(id, workflow, run_id="existing-run")
        return object()


def _selection() -> dict[str, object]:
    return {
        "selectionSnapshotId": "selection-1",
        "provider": "mock",
        "providerId": "mock-provider",
        "profile": "local-test-offline",
        "profileId": "local-profile",
        "modelId": "mock-model",
        "adapterKey": "mock",
        "adapterIdentity": "local_workspace",
        "profileRevision": 1,
        "capabilitySnapshotId": "text-capability",
        "capabilityRevision": 1,
        "capabilityOperation": "text.generate",
        "capabilitySnapshots": {"text.generate": {"id": "text-capability", "revision": 1}},
        "skills": ["drama-skills"],
        "skillRevisionIds": ["drama-skills@1.0.0"],
        "skillDigests": ["d" * 64],
        "decision": "fixed",
        "decisionRevision": 1,
        "routeStatus": "selected",
        "source": "explicit-local-profile",
    }


def _launch(selection: dict[str, object] | None = None) -> TemporalLaunch:
    project_id = "project-1"
    workflow_version_id = "workflow-version-1"
    run_id = "run-1"
    logical_operation = "text.generate:operation-1"
    return TemporalLaunch(
        project_id,
        workflow_version_id,
        TemporalStart(
            run_id,
            "node-run-1",
            logical_operation,
            temporal_workflow_id(
                project_id,
                workflow_version_id,
                run_id,
                logical_operation,
            ),
            "f" * 64,
        ),
        selection or _selection(),
        "00-" + "a" * 32 + "-" + "b" * 16 + "-01",
    )


async def test_temporal_starter_uses_stable_identity_and_exact_operation() -> None:
    client = FakeTemporalClient()
    launch = _launch()
    result = await TemporalRunStarter(client).start(launch)

    assert result.status == "started" and result.workflow_id == launch.start.workflow_id
    assert client.calls == [
        {
            "workflow": "phase_one_run",
            "arg": {
                "projectId": launch.project_id,
                "workflowVersionId": launch.workflow_version_id,
                "runId": launch.start.run_id,
                "nodeRunId": launch.start.node_run_id,
                "logicalOperation": launch.start.logical_operation,
                "requestFingerprint": launch.start.request_fingerprint,
                "selectionSnapshot": _selection(),
                "schemaVersion": "1.0.0",
            },
            "id": launch.start.workflow_id,
            "taskQueue": "agent-tasks",
            "headers": {
                "traceparent": Payload(
                    metadata={"encoding": b"text/plain"},
                    data=("00-" + "a" * 32 + "-" + "b" * 16 + "-01").encode(),
                )
            },
        }
    ]


async def test_temporal_already_started_is_reused_without_fallback() -> None:
    client = FakeTemporalClient(already_started=True)
    result = await TemporalRunStarter(client).start(_launch())
    assert result.status == "already_started"
    assert len(client.calls) == 1

    unconfigured = {**_selection(), "profile": "live-profile"}
    before = len(client.calls)
    with pytest.raises(WorkflowUnconfiguredError, match="unconfigured"):
        await TemporalRunStarter(client).start(_launch(unconfigured))
    assert len(client.calls) == before


async def test_persisted_temporal_start_is_dispatched_and_marked_started() -> None:
    uow = InMemoryUnitOfWork()
    run = WorkflowRun(
        "project-1",
        "workflow-version-1",
        status="running",
        selection_snapshot=_selection(),
    )
    node = NodeRun(run.id, "text.generate", logical_operation="text.generate:1")
    run.nodes = [node]
    start = TemporalStart(
        run.id,
        node.id,
        node.logical_operation,
        temporal_workflow_id(
            run.project_id, run.workflow_version_id, run.id, node.logical_operation
        ),
        "f" * 64,
    )
    uow.workflow_runs[run.id] = run
    uow.temporal_starts[start.workflow_id] = start
    client = FakeTemporalClient()

    result = await RunDispatchService(lambda: uow).dispatch_pending(TemporalRunStarter(client))

    assert result == {"dispatched": 1, "failed": 0}
    assert uow.temporal_starts[start.workflow_id].status == "started"
    assert run.nodes[0].status == "running"
    assert len(client.calls) == 1


async def test_registered_workflow_and_activity_are_orchestration_only() -> None:
    assert WORKFLOWS == (PhaseOneRunWorkflow,)
    assert ACTIVITIES == (
        phase_one_operation_checkpoint,
        agnes_video_submit,
        agnes_video_poll,
        agnes_video_cancel,
        agnes_video_result,
    )
    result = await phase_one_operation_checkpoint(
        {
            "runId": "run-1",
            "nodeRunId": "node-1",
            "logicalOperation": "text.generate:operation-1",
        }
    )
    assert result == {
        "status": "ready",
        "runId": "run-1",
        "nodeRunId": "node-1",
        "logicalOperation": "text.generate:operation-1",
    }
    assert all(
        getattr(item, "__name__", "").startswith(("phase_one", "agnes_video_"))
        for item in ACTIVITIES
    )


async def test_text_activity_generates_batch_and_enters_waiting_review(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    database_url = f"sqlite+aiosqlite:///{tmp_path / 'text-activity.db'}"
    engine = create_async_engine(database_url)
    async with engine.begin() as connection:
        await connection.run_sync(Base.metadata.create_all)
    factory = async_sessionmaker(engine, expire_on_commit=False)

    def uow_factory() -> SQLAlchemyUnitOfWork:
        return SQLAlchemyUnitOfWork(factory)

    try:
        projects = ProjectsEpisodesService(uow_factory)
        project = await projects.create_project("Temporal text")
        brief = await CreativeService(uow_factory).save_brief(
            SaveCreativeBriefCommand(
                project.id,
                "original",
                {
                    "subject": "Witness",
                    "genre": "Drama",
                    "audience": "Adult",
                    "characterPremise": "A witness chooses whether to speak",
                    "style": "Grounded",
                    "episodeDurationSeconds": 60,
                    "episodeCount": 1,
                    "scenesPerEpisode": 1,
                    "shotsPerScene": 1,
                },
                project.revision,
            )
        )
        await CatalogService(uow_factory).bootstrap()
        async with uow_factory() as uow:
            provider = next(item for item in uow.providers.values() if item.adapter_key == "mock")
            profile = next(
                item for item in uow.profiles.values() if item.provider_id == provider.id
            )
            model = next(item for item in uow.models.values() if item.profile_id == profile.id)
            capability = profile.capability_snapshots["text.generate"]
            selection = {
                **_selection(),
                "providerId": provider.id,
                "profileId": profile.id,
                "modelId": model.id,
                "profileRevision": profile.revision,
                "capabilitySnapshotId": capability.id,
                "capabilityRevision": capability.revision,
                "capabilitySnapshots": {
                    "text.generate": {"id": capability.id, "revision": capability.revision}
                },
            }
            workflow = WorkflowVersion(
                project.id,
                scope_ids=(project.id,),
                definition={"nodes": [{"key": "text.generate"}]},
            )
            binding = ProjectDefaultWorkflowBinding(project.id, workflow.id, workflow.content_hash)
            run = WorkflowRun(
                project.id, workflow.id, status="running", selection_snapshot=selection
            )
            node = NodeRun(
                run.id,
                "text.generate",
                status="running",
                logical_operation="text.generate:activity",
            )
            run.nodes = [node]
            uow.workflow_by_project[project.id] = workflow
            uow.workflow_bindings[project.id] = binding
            uow.workflow_runs[run.id] = run
            await uow.commit()

        monkeypatch.setenv("DATABASE_URL", database_url)
        monkeypatch.setenv("PROVIDER_MODE", "mock")
        monkeypatch.setenv("STORAGE_MODE", "local_workspace")
        monkeypatch.setenv("WORKSPACE_ROOT", str(tmp_path / "workspace"))
        result = await phase_one_operation_checkpoint(
            {
                "projectId": project.id,
                "runId": run.id,
                "nodeRunId": node.id,
                "logicalOperation": node.logical_operation,
            }
        )

        assert result["status"] == "waiting_review"
        async with uow_factory() as reloaded:
            persisted_run = reloaded.workflow_runs[run.id]
            assert persisted_run.status == "waiting_review"
            assert persisted_run.nodes[0].status == "waiting_review"
            assert len(reloaded.text_review_batches) == 1
            call = next(iter(reloaded.provider_calls.values()))
            assert call.status == "succeeded"
            assert call.node_run_id == node.id
            reloaded_batch = next(iter(reloaded.text_review_batches.values()))
            assert reloaded_batch.brief_revision == brief.revision
    finally:
        await engine.dispose()
