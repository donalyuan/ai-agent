"""Minimal owner-command consumer for persisted Generation image intents."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Protocol, cast

from video_agent_api.application.image_generation import (
    ImageGenerationService,
    command_from_payload,
    frozen_admission_from_payload,
)
from video_agent_api.domain.errors import ValidationDomainError

GENERATION_SCHEMA_VERSION = "1.0.0"
GENERATION_ROUTE = "generation"
GENERATION_TASK_QUEUE = "generation-tasks"


def generation_temporal_workflow_id(project_id: str, run_id: str, logical_operation: str) -> str:
    if not all(
        isinstance(value, str) and value for value in (project_id, run_id, logical_operation)
    ):
        raise ValidationDomainError("generation workflow identity is incomplete")
    return f"generation:{project_id}:{run_id}:{logical_operation}"


@dataclass(frozen=True, slots=True)
class GenerationLaunch:
    project_id: str
    run_id: str
    logical_operation: str
    workflow_type: str
    request_fingerprint: str
    schema_version: str = GENERATION_SCHEMA_VERSION
    execution_route: str = GENERATION_ROUTE
    task_queue: str = GENERATION_TASK_QUEUE
    command: object | None = None
    resource_admission: dict[str, object] | None = None
    node_run_id: str | None = None
    node_revision: int | None = None

    @property
    def workflow_id(self) -> str:
        return generation_temporal_workflow_id(self.project_id, self.run_id, self.logical_operation)


class GenerationStarter(Protocol):
    async def start(self, launch: GenerationLaunch) -> object: ...


class GenerationOutboxDispatcher:
    """Dispatch only Generation-route outbox rows with a stable workflow id."""

    def __init__(self, uow_factory: Any) -> None:
        self._uow_factory = uow_factory

    async def dispatch_pending(
        self, starter: GenerationStarter, *, limit: int = 100
    ) -> dict[str, int]:
        async with self._uow_factory() as uow:
            pending = [
                dict(event)
                for event in uow.outbox_events
                if event.get("status") == "pending"
                and event.get("executionRoute") == GENERATION_ROUTE
                and event.get("workflowType")
                in {"text-generation", "image-generation", "video-generation"}
            ][:limit]
        dispatched = 0
        failed = 0
        for event in pending:
            event_id = event.get("eventId")
            project_id = event.get("projectId")
            run_id = event.get("runId")
            logical_operation = event.get("logicalOperation")
            fingerprint = event.get("requestFingerprint")
            # Route, queue, and schema are frozen owner identity. A worker must
            # reject drift before it can start a workflow or alter delivery state.
            if (
                event.get("taskQueue") != GENERATION_TASK_QUEUE
                or event.get("schemaVersion") != GENERATION_SCHEMA_VERSION
            ):
                failed += 1
                continue
            # The activity must never compensate for a missing admission by
            # probing current resources.  A delivery without the owner-frozen
            # record stays pending for owner reconciliation.
            if not isinstance(event.get("resourceAdmission"), dict):
                failed += 1
                continue
            if not await self._owner_admission_matches(event):
                failed += 1
                continue
            if not all(
                isinstance(value, str) and value
                for value in (event_id, project_id, run_id, logical_operation, fingerprint)
            ):
                failed += 1
                continue
            project_id = cast(str, project_id)
            run_id = cast(str, run_id)
            logical_operation = cast(str, logical_operation)
            fingerprint = cast(str, fingerprint)
            launch = GenerationLaunch(
                project_id,
                run_id,
                logical_operation,
                str(event["workflowType"]),
                fingerprint,
                str(event.get("schemaVersion", "1.0.0")),
                str(event.get("executionRoute", GENERATION_ROUTE)),
                str(event.get("taskQueue", "generation-tasks")),
                event.get("command"),
                dict(cast(dict[str, object], event["resourceAdmission"])),
                (str(event["nodeRunId"]) if event.get("nodeRunId") else None),
                (
                    int(event["nodeRevision"])
                    if isinstance(event.get("nodeRevision"), int)
                    else None
                ),
            )
            try:
                await starter.start(launch)
            except Exception:
                failed += 1
                continue
            async with self._uow_factory() as uow:
                for index, current in enumerate(uow.outbox_events):
                    if current.get("eventId") == event_id and current.get("status") == "pending":
                        uow.outbox_events[index] = {**current, "status": "dispatched"}
                        await uow.commit()
                        break
            dispatched += 1
        return {"dispatched": dispatched, "failed": failed}

    async def _owner_admission_matches(self, event: dict[str, object]) -> bool:
        """Accept only the admission record already owned by this operation.

        The outbox is a delivery record, not a source of authority.  Image and
        video have a ProviderCall before dispatch; text has the frozen NodeRun.
        An event without its matching owner record must stay pending rather than
        being made runnable by a worker-local resource probe.
        """
        admission = event.get("resourceAdmission")
        run_id = event.get("runId")
        logical_operation = event.get("logicalOperation")
        workflow_type = event.get("workflowType")
        project_id = event.get("projectId")
        fingerprint = event.get("requestFingerprint")
        if not (
            isinstance(admission, dict)
            and isinstance(project_id, str)
            and project_id
            and isinstance(run_id, str)
            and run_id
            and isinstance(logical_operation, str)
            and logical_operation
            and isinstance(fingerprint, str)
            and fingerprint
            and admission.get("scope") == project_id
        ):
            return False
        async with self._uow_factory() as uow:
            if workflow_type == "text-generation":
                node_run_id = event.get("nodeRunId")
                node_revision = event.get("nodeRevision")
                run = uow.workflow_runs.get(run_id)
                node = next(
                    (
                        item
                        for item in getattr(run, "nodes", ())
                        if getattr(item, "id", None) == node_run_id
                        and getattr(item, "logical_operation", None) == logical_operation
                    ),
                    None,
                )
                return (
                    run is not None
                    and getattr(run, "project_id", None) == project_id
                    and getattr(run, "logical_operations", {}).get(logical_operation) == fingerprint
                    and node is not None
                    and isinstance(node_revision, int)
                    and getattr(node, "revision", None) == node_revision
                    and getattr(node, "execution_route", None) == GENERATION_ROUTE
                    and getattr(node, "workflow_type", None) == workflow_type
                    and getattr(node, "task_queue", None) == GENERATION_TASK_QUEUE
                    and getattr(node, "admission_refs", None) == admission
                )
            call_id = uow.provider_call_keys.get((run_id, logical_operation))
            call = uow.provider_calls.get(call_id) if call_id is not None else None
            expected_operation_prefix = {
                "image-generation": "image.",
                "video-generation": "video.",
            }.get(str(workflow_type))
            return (
                call is not None
                and expected_operation_prefix is not None
                and getattr(call, "project_id", None) == project_id
                and getattr(call, "run_id", None) == run_id
                and getattr(call, "logical_operation", None) == logical_operation
                and getattr(call, "request_fingerprint", None) == fingerprint
                and str(getattr(call, "operation", "")).startswith(expected_operation_prefix)
                and getattr(call, "admission_refs", None) == admission
            )


class GenerationCommandConsumer:
    """Execute only the image commands already frozen in the owner outbox."""

    def __init__(self, uow_factory: Any, image_service: ImageGenerationService) -> None:
        self._uow_factory = uow_factory
        self._image_service = image_service

    async def dispatch_pending(self, *, limit: int = 100) -> dict[str, int]:
        async with self._uow_factory() as uow:
            pending = [
                dict(event)
                for event in uow.outbox_events
                if event.get("type") == "image.generation.requested"
                and event.get("status") == "pending"
            ][:limit]

        dispatched = 0
        failed = 0
        for event in pending:
            event_id = event.get("eventId")
            if not isinstance(event_id, str):
                raise ValidationDomainError("image generation outbox identity is invalid")
            try:
                raw_admission = event.get("resourceAdmission")
                admission = (
                    frozen_admission_from_payload(raw_admission)
                    if raw_admission is not None
                    else None
                )
                await self._image_service.execute(
                    command_from_payload(event.get("command")),
                    frozen_admission=admission,
                )
            except Exception:
                # A ProviderCall records whether the side effect is retryable or must
                # reconcile; this delivery record only prevents implicit re-dispatch.
                await self._mark(event_id, "reconciliation_required")
                failed += 1
                continue
            await self._mark(event_id, "dispatched")
            dispatched += 1
        return {"dispatched": dispatched, "failed": failed}

    async def _mark(self, event_id: str, status: str) -> None:
        async with self._uow_factory() as uow:
            for index, event in enumerate(uow.outbox_events):
                if event.get("eventId") == event_id and event.get("status") == "pending":
                    updated = dict(event)
                    updated["status"] = status
                    uow.outbox_events[index] = updated
                    await uow.commit()
                    return
