from __future__ import annotations

import pytest
from temporalio.exceptions import WorkflowAlreadyStartedError

from video_agent_api.adapters.generation_temporal import (
    GenerationActivityDependencies,
    GenerationLaunch,
    TemporalGenerationStarter,
    configure_generation_activities,
    generation_image_generate,
    generation_image_reconcile,
    generation_temporal_workflow_id,
    generation_text_reconcile,
)
from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.application.catalog import CatalogService
from video_agent_api.application.generation_dispatch import GenerationOutboxDispatcher
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.domain.provider_ops import ProviderCall
from video_agent_api.ports.contracts import FrozenRemoteLookup, PortResult


class _Client:
    def __init__(self) -> None:
        self.calls: list[tuple[object, object, str, str]] = []

    async def start_workflow(
        self, workflow: object, arg: object, *, id: str, task_queue: str
    ) -> object:
        self.calls.append((workflow, arg, id, task_queue))
        raise WorkflowAlreadyStartedError(id, str(workflow))


@pytest.mark.asyncio
async def test_generation_workflow_id_and_already_started_are_stable() -> None:
    client = _Client()
    launch = GenerationLaunch("project", "run", "image.generate:1", "image-generation", "f" * 64)
    result = await TemporalGenerationStarter(client).start(launch)
    assert result.status == "already_started"
    assert result.workflow_id == generation_temporal_workflow_id(
        "project", "run", "image.generate:1"
    )
    assert client.calls[0][2] == result.workflow_id
    assert client.calls[0][3] == "generation-tasks"
    assert client.calls[0][1]["requestFingerprint"] == "f" * 64


@pytest.mark.asyncio
async def test_generation_outbox_dispatches_once_and_drains_legacy() -> None:
    uow = InMemoryUnitOfWork()
    uow.outbox_events.extend(
        [
            {
                "eventId": "generation-event",
                "status": "pending",
                "executionRoute": "generation",
                "workflowType": "image-generation",
                "taskQueue": "generation-tasks",
                "schemaVersion": "1.0.0",
                "projectId": "project",
                "runId": "run",
                "logicalOperation": "image.generate:1",
                "requestFingerprint": "f" * 64,
                "resourceAdmission": {
                    "operationKey": "image.generate:1",
                    "scope": "project",
                    "operation": "image.generate",
                    "reference": "frozen",
                    "resourceRevision": 1,
                    "capacityRevision": 1,
                    "resourceHash": "resource",
                    "capacityHash": "capacity",
                    "allowed": True,
                },
            },
            {
                "eventId": "legacy-event",
                "status": "pending",
                "executionRoute": "legacy",
                "workflowType": "phase_one_run",
            },
        ]
    )
    call = ProviderCall(
        "project",
        "run",
        None,
        "image.generate:1",
        "image.generate",
        "provider",
        "profile",
        "model",
        "capability",
        "f" * 64,
        "pending",
        admission_refs=dict(uow.outbox_events[0]["resourceAdmission"]),
    )
    uow.provider_calls[call.id] = call
    uow.provider_call_keys[(call.run_id, call.logical_operation)] = call.id

    class _Starter:
        def __init__(self) -> None:
            self.launches: list[GenerationLaunch] = []

        async def start(self, launch: GenerationLaunch) -> None:
            self.launches.append(launch)

    starter = _Starter()
    dispatcher = GenerationOutboxDispatcher(lambda: uow)
    assert await dispatcher.dispatch_pending(starter) == {"dispatched": 1, "failed": 0}
    assert len(starter.launches) == 1
    assert await dispatcher.dispatch_pending(starter) == {"dispatched": 0, "failed": 0}
    assert (
        next(item for item in uow.outbox_events if item["eventId"] == "legacy-event")["status"]
        == "pending"
    )


@pytest.mark.asyncio
async def test_generation_outbox_rejects_frozen_queue_drift_before_workflow_start() -> None:
    uow = InMemoryUnitOfWork()
    uow.outbox_events.append(
        {
            "eventId": "queue-drift",
            "status": "pending",
            "executionRoute": "generation",
            "workflowType": "image-generation",
            "taskQueue": "foreign-generation-tasks",
            "schemaVersion": "1.0.0",
            "projectId": "project",
            "runId": "run",
            "logicalOperation": "image.generate:1",
            "requestFingerprint": "f" * 64,
        }
    )

    class _Starter:
        def __init__(self) -> None:
            self.launches: list[GenerationLaunch] = []

        async def start(self, launch: GenerationLaunch) -> None:
            self.launches.append(launch)

    starter = _Starter()
    result = await GenerationOutboxDispatcher(lambda: uow).dispatch_pending(starter)

    assert result == {"dispatched": 0, "failed": 1}
    assert starter.launches == []
    assert uow.outbox_events[0]["status"] == "pending"


@pytest.mark.asyncio
async def test_generation_outbox_rejects_missing_frozen_admission_before_workflow_start() -> None:
    uow = InMemoryUnitOfWork()
    uow.outbox_events.append(
        {
            "eventId": "missing-admission",
            "status": "pending",
            "executionRoute": "generation",
            "workflowType": "image-generation",
            "taskQueue": "generation-tasks",
            "schemaVersion": "1.0.0",
            "projectId": "project",
            "runId": "run",
            "logicalOperation": "image.generate:1",
            "requestFingerprint": "f" * 64,
        }
    )

    class _Starter:
        async def start(self, launch: GenerationLaunch) -> None:
            raise AssertionError(f"unexpected launch: {launch}")

    assert await GenerationOutboxDispatcher(lambda: uow).dispatch_pending(_Starter()) == {
        "dispatched": 0,
        "failed": 1,
    }
    assert uow.outbox_events[0]["status"] == "pending"


@pytest.mark.asyncio
async def test_generation_outbox_rejects_owner_fingerprint_drift_before_start() -> None:
    uow = InMemoryUnitOfWork()
    admission = {
        "operationKey": "image.generate:1",
        "scope": "project",
        "operation": "image.generate",
        "reference": "frozen",
        "resourceRevision": 1,
        "capacityRevision": 1,
        "resourceHash": "resource",
        "capacityHash": "capacity",
        "allowed": True,
    }
    uow.outbox_events.append(
        {
            "eventId": "owner-drift",
            "status": "pending",
            "executionRoute": "generation",
            "workflowType": "image-generation",
            "taskQueue": "generation-tasks",
            "schemaVersion": "1.0.0",
            "projectId": "project",
            "runId": "run",
            "logicalOperation": "image.generate:1",
            "requestFingerprint": "f" * 64,
            "resourceAdmission": admission,
        }
    )
    call = ProviderCall(
        "project",
        "run",
        None,
        "image.generate:1",
        "image.generate",
        "provider",
        "profile",
        "model",
        "capability",
        "e" * 64,
        "pending",
        admission_refs=dict(admission),
    )
    uow.provider_calls[call.id] = call
    uow.provider_call_keys[(call.run_id, call.logical_operation)] = call.id

    class _Starter:
        async def start(self, launch: GenerationLaunch) -> None:
            raise AssertionError(f"unexpected launch: {launch}")

    assert await GenerationOutboxDispatcher(lambda: uow).dispatch_pending(_Starter()) == {
        "dispatched": 0,
        "failed": 1,
    }
    assert uow.outbox_events[0]["status"] == "pending"


@pytest.mark.asyncio
async def test_generation_activity_rejects_frozen_legacy_route_before_dependency() -> None:
    with pytest.raises(ValidationDomainError, match="legacy route"):
        await generation_image_generate(
            {
                "executionRoute": "legacy",
                "schemaVersion": "1.0.0",
                "logicalOperation": "image.generate:1",
            }
        )


@pytest.mark.asyncio
async def test_generation_activity_rejects_missing_frozen_admission_before_dependency() -> None:
    with pytest.raises(ValidationDomainError, match="frozen admission"):
        await generation_image_generate(
            {
                "executionRoute": "generation",
                "schemaVersion": "1.0.0",
                "logicalOperation": "image.generate:1",
            }
        )


@pytest.mark.asyncio
async def test_generation_text_and_image_reconcile_keep_ambiguous_owner_calls_unknown() -> None:
    uow = InMemoryUnitOfWork()
    catalog = CatalogService(lambda: uow)
    calls = [
        ProviderCall(
            "project",
            "run",
            None,
            "text.generate:1",
            "text.generate",
            "p",
            "profile",
            "m",
            None,
            "a" * 64,
            "unknown",
        ),
        ProviderCall(
            "project",
            "run",
            None,
            "image.generate:1",
            "image.generate",
            "p",
            "profile",
            "m",
            None,
            "b" * 64,
            "unknown",
        ),
    ]
    for call in calls:
        uow.provider_calls[call.id] = call
        uow.provider_call_keys[(call.run_id, call.logical_operation)] = call.id

    class Owner:
        def __init__(self) -> None:
            self._catalog = catalog

    configure_generation_activities(GenerationActivityDependencies(text=Owner(), image=Owner()))
    base = {
        "executionRoute": "generation",
        "schemaVersion": "1.0.0",
        "runId": "run",
    }
    text = await generation_text_reconcile({**base, "logicalOperation": "text.generate:1"})
    image = await generation_image_reconcile({**base, "logicalOperation": "image.generate:1"})
    repeated = await generation_image_reconcile({**base, "logicalOperation": "image.generate:1"})

    assert text == {
        "status": "unknown",
        "runId": "run",
        "logicalOperation": "text.generate:1",
        "lookupOutcome": "unsupported",
    }
    assert image == repeated
    assert len(uow.provider_calls) == 2


@pytest.mark.asyncio
async def test_generation_image_reconcile_accepts_edit_owner_operation() -> None:
    uow = InMemoryUnitOfWork()
    catalog = CatalogService(lambda: uow)
    call = ProviderCall(
        "project",
        "run",
        None,
        "image.edit:1",
        "image.edit",
        "p",
        "profile",
        "m",
        None,
        "c" * 64,
        "unknown",
    )
    uow.provider_calls[call.id] = call
    uow.provider_call_keys[(call.run_id, call.logical_operation)] = call.id

    class Owner:
        def __init__(self) -> None:
            self._catalog = catalog

    configure_generation_activities(GenerationActivityDependencies(image=Owner()))
    result = await generation_image_reconcile(
        {
            "executionRoute": "generation",
            "schemaVersion": "1.0.0",
            "runId": "run",
            "logicalOperation": "image.edit:1",
        }
    )

    assert result == {
        "status": "unknown",
        "runId": "run",
        "logicalOperation": "image.edit:1",
        "lookupOutcome": "unsupported",
    }


@pytest.mark.asyncio
async def test_generation_reconcile_uses_only_matching_frozen_lookup_binding() -> None:
    uow = InMemoryUnitOfWork()
    catalog = CatalogService(lambda: uow)
    call = ProviderCall(
        "project",
        "run",
        None,
        "image.edit:1",
        "image.edit",
        "p",
        "profile",
        "m",
        "snapshot-1",
        "c" * 64,
        "unknown",
        remote_lookup_protocol="by-correlation",
        remote_lookup_binding={
            "profileId": "profile",
            "modelId": "m",
            "profileRevision": 1,
            "capabilitySnapshotId": "snapshot-1",
            "capabilityRevision": 1,
            "operation": "image.edit",
            "protocol": "by-correlation",
        },
        outbound_correlation="frozen-correlation",
    )
    uow.provider_calls[call.id] = call
    uow.provider_call_keys[(call.run_id, call.logical_operation)] = call.id
    seen: list[tuple[str, str]] = []

    class Lookup:
        def lookup_provider_request(self, correlation: str, protocol: str) -> PortResult:
            seen.append((correlation, protocol))
            return PortResult("remote-image", correlation, {"usage": {"images": 1}})

    class Owner:
        def __init__(self) -> None:
            self._catalog = catalog

    configure_generation_activities(
        GenerationActivityDependencies(
            image=Owner(),
            remote_lookups=(
                FrozenRemoteLookup(
                    "snapshot-1",
                    "image.edit",
                    "by-correlation",
                    Lookup(),
                    "profile",
                    "m",
                    1,
                    1,
                ),
            ),
        )
    )
    result = await generation_image_reconcile(
        {
            "executionRoute": "generation",
            "schemaVersion": "1.0.0",
            "runId": "run",
            "logicalOperation": "image.edit:1",
        }
    )

    assert result == {
        "status": "succeeded",
        "runId": "run",
        "logicalOperation": "image.edit:1",
        "lookupOutcome": "found",
    }
    assert seen == [("frozen-correlation", "by-correlation")]
