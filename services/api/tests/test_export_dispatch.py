from __future__ import annotations

import sys
from pathlib import Path
from types import SimpleNamespace

import pytest
from temporalio.exceptions import ApplicationError, WorkflowAlreadyStartedError

import video_agent_api.adapters.export_temporal as export_temporal

sys.path.insert(0, str(Path(__file__).parents[3]))

from workers.media.main import MEDIA_ACTIVITIES, MEDIA_WORKFLOWS

from video_agent_api.adapters.export_temporal import (
    EpisodeExportWorkflow,
    TemporalExportLaunch,
    TemporalExportStarter,
    episode_export_execute,
)
from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.application.export_dispatch import ExportDispatchService
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.domain.exports import ExportDispatchOutbox, export_temporal_workflow_id


class RecordingTemporalClient:
    def __init__(self, *, already_started: bool = False) -> None:
        self.already_started = already_started
        self.calls: list[dict[str, object]] = []

    async def start_workflow(
        self,
        workflow: str,
        arg: object,
        *,
        id: str,
        task_queue: str,
    ) -> object:
        self.calls.append(
            {
                "workflow": workflow,
                "arg": arg,
                "id": id,
                "taskQueue": task_queue,
            }
        )
        if self.already_started:
            raise WorkflowAlreadyStartedError(id, workflow)
        return object()


def _event() -> ExportDispatchOutbox:
    return ExportDispatchOutbox(
        project_id="project-1",
        batch_id="batch-1",
        job_id="job-1",
        logical_operation="export:batch-1:episode-1:version-1:initial",
    )


@pytest.mark.asyncio
@pytest.mark.parametrize(
    ("already_started", "expected_status"),
    [(False, "started"), (True, "already_started")],
)
async def test_temporal_export_starter_uses_stable_identity_and_treats_duplicates_as_success(
    already_started: bool, expected_status: str
) -> None:
    event = _event()
    client = RecordingTemporalClient(already_started=already_started)
    starter = TemporalExportStarter(client)

    result = await starter.start(TemporalExportLaunch.from_outbox(event))

    expected_id = export_temporal_workflow_id(
        event.project_id,
        event.batch_id,
        event.job_id,
        event.logical_operation,
    )
    assert result.status == expected_status
    assert result.workflow_id == expected_id == event.workflow_id
    assert client.calls == [
        {
            "workflow": "episode_export",
            "arg": {
                "projectId": event.project_id,
                "batchId": event.batch_id,
                "jobId": event.job_id,
                "logicalOperation": event.logical_operation,
                "schemaVersion": "1.0.0",
            },
            "id": expected_id,
            "taskQueue": "media-tasks",
        }
    ]


@pytest.mark.asyncio
async def test_dispatcher_persists_dispatched_state_after_temporal_accepts_event() -> None:
    uow = InMemoryUnitOfWork()
    event = _event()
    uow.export_dispatch_outbox[event.id] = event
    client = RecordingTemporalClient(already_started=True)

    result = await ExportDispatchService(lambda: uow).dispatch_pending(
        TemporalExportStarter(client)
    )

    assert result == {"dispatched": 1, "failed": 0}
    assert event.status == "dispatched"
    assert event.attempts == 1
    assert event.dispatched_at is not None


def test_media_worker_registers_real_episode_export_workflow_and_activity() -> None:
    assert EpisodeExportWorkflow in MEDIA_WORKFLOWS
    workflow_definition = EpisodeExportWorkflow.__temporal_workflow_definition
    assert workflow_definition.name == "episode_export"
    assert len(MEDIA_ACTIVITIES) == 1
    activity_definition = MEDIA_ACTIVITIES[0].__temporal_activity_definition
    assert activity_definition.name == "episode_export_execute"


@pytest.mark.asyncio
async def test_export_activity_marks_deterministic_domain_failures_non_retryable(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    class FailingWorker:
        async def execute(self, command: object) -> dict[str, object]:
            raise ValidationDomainError("frozen export input changed")

    monkeypatch.setattr(export_temporal, "_episode_export_worker", FailingWorker())
    monkeypatch.setattr(export_temporal, "_episode_export_workspace_root", tmp_path)
    monkeypatch.setattr(
        "video_agent_api.adapters.export_temporal.activity.info",
        lambda: SimpleNamespace(activity_id="activity-1"),
    )

    with pytest.raises(ApplicationError) as caught:
        await episode_export_execute(
            {
                "projectId": "project-1",
                "jobId": "job-1",
                "logicalOperation": "export:job-1",
            }
        )

    assert caught.value.non_retryable is True
    assert caught.value.type == "validation"
