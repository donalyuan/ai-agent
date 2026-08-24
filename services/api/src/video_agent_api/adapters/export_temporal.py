"""Temporal transport for persistent Episode export dispatches."""

from __future__ import annotations

from datetime import timedelta
from pathlib import Path
from typing import Any, Protocol

from temporalio import activity, workflow
from temporalio.exceptions import ApplicationError, WorkflowAlreadyStartedError

from video_agent_api.application.export_dispatch import (
    TemporalExportLaunch,
    TemporalExportLaunchResult,
)
from video_agent_api.application.export_worker import ExecuteExportJobCommand
from video_agent_api.domain.errors import DomainError, ValidationDomainError
from video_agent_api.domain.exports import export_temporal_workflow_id
from video_agent_api.ports.contracts import PortError, StorageRetryableError


class TemporalClient(Protocol):
    async def start_workflow(
        self,
        workflow: str,
        arg: object,
        *,
        id: str,
        task_queue: str,
    ) -> object: ...


class TemporalExportStarter:
    def __init__(self, client: TemporalClient, task_queue: str = "media-tasks") -> None:
        self._client = client
        self._task_queue = task_queue

    async def start(self, launch: TemporalExportLaunch) -> TemporalExportLaunchResult:
        expected = export_temporal_workflow_id(
            launch.project_id,
            launch.batch_id,
            launch.job_id,
            launch.logical_operation,
        )
        if launch.workflow_id != expected:
            raise ValidationDomainError("export Temporal workflow identity is stale")
        payload = {
            "projectId": launch.project_id,
            "batchId": launch.batch_id,
            "jobId": launch.job_id,
            "logicalOperation": launch.logical_operation,
            "schemaVersion": launch.schema_version,
        }
        try:
            await self._client.start_workflow(
                "episode_export",
                payload,
                id=launch.workflow_id,
                task_queue=self._task_queue,
            )
        except WorkflowAlreadyStartedError:
            return TemporalExportLaunchResult(launch.workflow_id, "already_started")
        return TemporalExportLaunchResult(launch.workflow_id, "started")


_episode_export_worker: Any | None = None
_episode_export_workspace_root: Path | None = None


def configure_episode_export_activity(worker: Any, workspace_root: Path) -> None:
    global _episode_export_worker, _episode_export_workspace_root
    _episode_export_worker = worker
    _episode_export_workspace_root = workspace_root.resolve()


def _required(payload: dict[str, object], key: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or not value:
        raise ValueError(f"{key} is required")
    return value


@activity.defn(name="episode_export_execute")
async def episode_export_execute(payload: dict[str, object]) -> dict[str, object]:
    if _episode_export_worker is None or _episode_export_workspace_root is None:
        raise RuntimeError("episode export activity is unconfigured")
    project_id = _required(payload, "projectId")
    job_id = _required(payload, "jobId")
    try:
        return await _episode_export_worker.execute(
            ExecuteExportJobCommand(
                project_id=project_id,
                job_id=job_id,
                workspace=_episode_export_workspace_root / job_id,
                correlation_id=activity.info().activity_id,
            )
        )
    except StorageRetryableError:
        raise
    except (DomainError, PortError) as error:
        raise ApplicationError(
            str(error),
            type=getattr(error, "code", type(error).__name__),
            non_retryable=True,
        ) from error


@workflow.defn(name="episode_export")
class EpisodeExportWorkflow:
    @workflow.run
    async def run(self, payload: dict[str, object]) -> dict[str, object]:
        job_id = _required(payload, "jobId")
        logical_operation = _required(payload, "logicalOperation")
        return await workflow.execute_activity(
            episode_export_execute,
            payload,
            activity_id=f"{job_id}:{logical_operation}",
            start_to_close_timeout=timedelta(hours=6),
        )
