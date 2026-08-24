"""Temporal starter adapter for committed workflow operations."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Literal, Protocol

from temporalio.api.common.v1 import Payload
from temporalio.exceptions import WorkflowAlreadyStartedError

from video_agent_api.application.run_dispatch import TemporalRunLaunch
from video_agent_api.domain.errors import ValidationDomainError, WorkflowUnconfiguredError
from video_agent_api.domain.runs import temporal_workflow_id


class TemporalClient(Protocol):
    async def start_workflow(
        self,
        workflow: str,
        arg: object,
        *,
        id: str,
        task_queue: str,
        headers: dict[str, Payload],
    ) -> object: ...


TemporalLaunch = TemporalRunLaunch


@dataclass(frozen=True, slots=True)
class TemporalLaunchResult:
    workflow_id: str
    status: Literal["started", "already_started"]


class TemporalRunStarter:
    def __init__(self, client: TemporalClient, task_queue: str = "agent-tasks") -> None:
        self._client = client
        self._task_queue = task_queue

    async def start(self, launch: TemporalLaunch) -> TemporalLaunchResult:
        start = launch.start
        expected_id = temporal_workflow_id(
            launch.project_id,
            launch.workflow_version_id,
            start.run_id,
            start.logical_operation,
        )
        if start.workflow_id != expected_id:
            raise ValidationDomainError("temporal workflow identity is stale or mismatched")
        selection = launch.selection_snapshot
        if (
            selection.get("provider") != "mock"
            or selection.get("profile") != "local-test-offline"
            or selection.get("adapterIdentity") != "local_workspace"
            or selection.get("routeStatus") != "selected"
        ):
            raise WorkflowUnconfiguredError("workflow temporal selection is unconfigured")
        payload: dict[str, Any] = {
            "projectId": launch.project_id,
            "workflowVersionId": launch.workflow_version_id,
            "runId": start.run_id,
            "nodeRunId": start.node_run_id,
            "logicalOperation": start.logical_operation,
            "requestFingerprint": start.request_fingerprint,
            "selectionSnapshot": dict(selection),
            "schemaVersion": start.schema_version,
        }
        headers: dict[str, Payload] = {}
        if launch.traceparent:
            headers["traceparent"] = Payload(
                metadata={"encoding": b"text/plain"}, data=launch.traceparent.encode()
            )
        if launch.tracestate:
            headers["tracestate"] = Payload(
                metadata={"encoding": b"text/plain"}, data=launch.tracestate.encode()
            )
        try:
            await self._client.start_workflow(
                "phase_one_run",
                payload,
                id=start.workflow_id,
                task_queue=self._task_queue,
                headers=headers,
            )
        except WorkflowAlreadyStartedError:
            return TemporalLaunchResult(start.workflow_id, "already_started")
        return TemporalLaunchResult(start.workflow_id, "started")
