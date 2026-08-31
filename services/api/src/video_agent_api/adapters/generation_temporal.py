"""Temporal boundary for Generation Worker operations.

The workflow is intentionally orchestration-only.  Every provider, database,
storage and owner handoff call is performed by an activity supplied through
``GenerationActivityDependencies``.  Payloads carry the frozen route and
operation identity, so a retry cannot silently cross to the legacy Agent route.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import timedelta
from typing import Any, Literal, Protocol, cast

from temporalio import activity, workflow
from temporalio.exceptions import WorkflowAlreadyStartedError

from video_agent_api.application.generation_dispatch import (
    GENERATION_ROUTE,
    GENERATION_SCHEMA_VERSION,
    GENERATION_TASK_QUEUE,
    GenerationLaunch,
)
from video_agent_api.application.generation_dispatch import (
    generation_temporal_workflow_id as _generation_temporal_workflow_id,
)
from video_agent_api.application.image_generation import (
    GenerateImageCommand,
    ImageGenerationService,
    frozen_admission_from_payload,
)
from video_agent_api.application.image_generation import (
    command_from_payload as image_command_from_payload,
)
from video_agent_api.application.text_generation import (
    GenerateTextBatchCommand,
    TextGenerationService,
)
from video_agent_api.application.video_generation import (
    AgnesVideoService,
    PollVideoCommand,
    ReconcileVideoCommand,
    SubmitVideoCommand,
)
from video_agent_api.application.video_generation import (
    command_from_payload as video_command_from_payload,
)
from video_agent_api.domain.creative import CreativeBriefSourceBindingSnapshot, CreativeBriefVersion
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.ports.contracts import FrozenRemoteLookup, ModelSelection


def generation_temporal_workflow_id(project_id: str, run_id: str, logical_operation: str) -> str:
    return _generation_temporal_workflow_id(project_id, run_id, logical_operation)


class TemporalGenerationClient(Protocol):
    async def start_workflow(
        self, workflow: str, arg: object, *, id: str, task_queue: str
    ) -> object: ...


@dataclass(frozen=True, slots=True)
class GenerationLaunchResult:
    workflow_id: str
    status: Literal["started", "already_started"]


class TemporalGenerationStarter:
    """Start only frozen Generation launches; AlreadyStarted is success."""

    def __init__(
        self, client: TemporalGenerationClient, task_queue: str = GENERATION_TASK_QUEUE
    ) -> None:
        self._client = client
        self._task_queue = task_queue

    async def start(self, launch: GenerationLaunch) -> GenerationLaunchResult:
        if launch.execution_route != GENERATION_ROUTE or launch.task_queue != self._task_queue:
            raise ValidationDomainError("generation launch route is stale")
        if launch.schema_version != GENERATION_SCHEMA_VERSION:
            raise ValidationDomainError("generation launch schemaVersion is unsupported")
        workflow_name = launch.workflow_type
        if workflow_name not in {"text-generation", "image-generation", "video-generation"}:
            raise ValidationDomainError("generation workflow type is unsupported")
        payload = {
            "projectId": launch.project_id,
            "runId": launch.run_id,
            "logicalOperation": launch.logical_operation,
            "requestFingerprint": launch.request_fingerprint,
            "workflowType": workflow_name,
            "executionRoute": launch.execution_route,
            "taskQueue": launch.task_queue,
            "schemaVersion": launch.schema_version,
            "command": launch.command,
            "resourceAdmission": launch.resource_admission,
            "nodeRunId": launch.node_run_id,
            "nodeRevision": launch.node_revision,
        }
        try:
            await self._client.start_workflow(
                workflow_name,
                payload,
                id=launch.workflow_id,
                task_queue=self._task_queue,
            )
        except WorkflowAlreadyStartedError:
            return GenerationLaunchResult(launch.workflow_id, "already_started")
        return GenerationLaunchResult(launch.workflow_id, "started")


@dataclass(slots=True)
class GenerationActivityDependencies:
    """Injected owner services; activities do not construct alternate ledgers."""

    text: TextGenerationService | None = None
    image: ImageGenerationService | None = None
    video: AgnesVideoService | None = None
    remote_lookups: tuple[FrozenRemoteLookup, ...] = ()


_dependencies: GenerationActivityDependencies | None = None


def configure_generation_activities(dependencies: GenerationActivityDependencies) -> None:
    global _dependencies
    _dependencies = dependencies


def _deps() -> GenerationActivityDependencies:
    if _dependencies is None:
        raise RuntimeError("generation activities are unconfigured")
    return _dependencies


def _payload_string(payload: dict[str, object], key: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or not value:
        raise ValidationDomainError(f"generation payload {key} is required")
    return value


def _validate_route(payload: dict[str, object], *, require_admission: bool = False) -> None:
    if payload.get("executionRoute") != GENERATION_ROUTE:
        raise ValidationDomainError("generation activity cannot consume legacy route")
    if payload.get("schemaVersion", GENERATION_SCHEMA_VERSION) != GENERATION_SCHEMA_VERSION:
        raise ValidationDomainError("generation activity schemaVersion is unsupported")
    if require_admission and not isinstance(payload.get("resourceAdmission"), dict):
        raise ValidationDomainError("generation activity frozen admission is required")


def _text_command_from_payload(payload: object) -> GenerateTextBatchCommand:
    if not isinstance(payload, dict):
        raise ValidationDomainError("text generation activity command is invalid")
    try:
        brief_payload = payload["brief"]
        selection_payload = payload["selection"]
        if not isinstance(brief_payload, dict) or not isinstance(selection_payload, dict):
            raise TypeError("brief/selection")
        brief = CreativeBriefVersion(
            str(brief_payload["creativeBriefId"]),
            str(brief_payload["projectId"]),
            str(brief_payload["subject"]),
            str(brief_payload["genre"]),
            str(brief_payload["audience"]),
            str(brief_payload["characterPremise"]),
            str(brief_payload["style"]),
            int(brief_payload["episodeDurationSeconds"]),
            int(brief_payload["episodeCount"]),
            int(brief_payload["scenesPerEpisode"]),
            int(brief_payload["shotsPerScene"]),
            int(brief_payload["revision"]),
            str(brief_payload.get("schemaVersion", GENERATION_SCHEMA_VERSION)),
            payload_hash=str(brief_payload.get("payloadHash", "")),
        )
        source_payload = payload.get("source")
        source = None
        if source_payload is not None:
            if not isinstance(source_payload, dict):
                raise TypeError("source")
            source = CreativeBriefSourceBindingSnapshot(
                str(source_payload["projectId"]),
                str(source_payload["sourceMaterialId"]),
                int(source_payload["sourceMaterialRevision"]),
                str(source_payload["sourceContentHash"]),
                str(source_payload["creativeBriefId"]),
                int(source_payload["creativeBriefRevision"]),
                str(source_payload["creativeBriefPayloadHash"]),
                str(source_payload["parseStatus"]),
                str(source_payload["validationStatus"]),
                str(source_payload["bindingStatus"]),
                str(source_payload["bindingVersion"]),
                str(source_payload.get("schemaVersion", GENERATION_SCHEMA_VERSION)),
            )
        kinds = payload["requestedKinds"]
        scopes = payload["scopeIds"]
        if not isinstance(kinds, list) or not isinstance(scopes, list):
            raise TypeError("requestedKinds/scopeIds")
        return GenerateTextBatchCommand(
            project_id=str(payload["projectId"]),
            run_id=str(payload["runId"]),
            brief_revision=int(payload["briefRevision"]),
            selection=ModelSelection(
                str(selection_payload["providerId"]),
                str(selection_payload["profileId"]),
                str(selection_payload["modelId"]),
                str(selection_payload["adapterKey"]),
                dict(selection_payload.get("defaultParameters", {})),
            ),
            brief_snapshot=brief,
            source_binding_snapshot=source,
            requested_kinds=tuple(str(item) for item in kinds),
            scope_ids=tuple(str(item) for item in scopes),
            correlation_id=str(payload.get("correlationId", "generation-text")),
        )
    except (KeyError, TypeError, ValueError, ValidationDomainError) as error:
        raise ValidationDomainError("text generation activity command is invalid") from error


@activity.defn(name="generation_text_generate")
async def generation_text_generate(payload: dict[str, object]) -> dict[str, object]:
    _validate_route(payload, require_admission=True)
    service = _deps().text
    if service is None:
        raise RuntimeError("text generation activity is unconfigured")
    command = payload.get("command")
    if not isinstance(command, GenerateTextBatchCommand):
        command = _text_command_from_payload(command)
    batch = await service.generate(command)
    node_run_id = payload.get("nodeRunId")
    if isinstance(node_run_id, str) and node_run_id:
        from video_agent_api.application.runs import RunsService

        expected_revision = payload.get("nodeRevision")
        if not isinstance(expected_revision, int):
            expected_revision = 1
        await RunsService(service._uow_factory, None).enter_text_review(
            command.run_id, node_run_id, batch.id, expected_revision
        )
    return {
        "status": "waiting_review",
        "batchId": batch.id,
        "runId": command.run_id,
        "logicalOperation": _payload_string(payload, "logicalOperation"),
    }


@activity.defn(name="generation_image_generate")
async def generation_image_generate(payload: dict[str, object]) -> dict[str, object]:
    _validate_route(payload, require_admission=True)
    service = _deps().image
    if service is None:
        raise RuntimeError("image generation activity is unconfigured")
    command = payload.get("command")
    if not isinstance(command, GenerateImageCommand):
        command = image_command_from_payload(command)
    raw_admission = payload.get("resourceAdmission")
    if not isinstance(raw_admission, dict):  # guarded above; keeps the type boundary explicit.
        raise ValidationDomainError("generation activity frozen admission is required")
    candidate = await service.execute(
        command,
        frozen_admission=frozen_admission_from_payload(raw_admission),
    )
    return {
        "status": "candidate_ready",
        "candidateId": candidate.id,
        "runId": command.run_id,
        "logicalOperation": command.logical_operation,
    }


async def _reconcile_provider_owner(
    service: TextGenerationService | ImageGenerationService,
    payload: dict[str, object],
    operation: str | tuple[str, ...],
) -> dict[str, object]:
    """Reconcile only the existing durable attempt, never a new provider submission."""
    _validate_route(payload)
    catalog = cast(Any, service._catalog)
    if catalog is None:
        raise RuntimeError("generation reconcile activity is unconfigured")
    call = await catalog.reconcile_provider_call(
        _payload_string(payload, "runId"),
        _payload_string(payload, "logicalOperation"),
        lookups=_deps().remote_lookups,
    )
    allowed_operations = {operation} if isinstance(operation, str) else set(operation)
    if call.operation not in allowed_operations:
        raise ValidationDomainError("generation reconcile operation is stale")
    return {
        "status": call.status,
        "runId": call.run_id,
        "logicalOperation": call.logical_operation,
        "lookupOutcome": call.lookup_outcome,
    }


@activity.defn(name="generation_text_reconcile")
async def generation_text_reconcile(payload: dict[str, object]) -> dict[str, object]:
    service = _deps().text
    if service is None:
        raise RuntimeError("text generation activity is unconfigured")
    return await _reconcile_provider_owner(service, payload, "text.generate")


@activity.defn(name="generation_image_reconcile")
async def generation_image_reconcile(payload: dict[str, object]) -> dict[str, object]:
    service = _deps().image
    if service is None:
        raise RuntimeError("image generation activity is unconfigured")
    return await _reconcile_provider_owner(service, payload, ("image.generate", "image.edit"))


@activity.defn(name="generation_video_submit")
async def generation_video_submit(payload: dict[str, object]) -> dict[str, object]:
    _validate_route(payload, require_admission=True)
    service = _deps().video
    if service is None:
        raise RuntimeError("video generation activity is unconfigured")
    command = payload.get("command")
    if not isinstance(command, SubmitVideoCommand):
        command = video_command_from_payload(command)
    raw_admission = payload.get("resourceAdmission")
    if not isinstance(raw_admission, dict):
        raise ValidationDomainError("generation activity frozen admission is required")
    operation = await service.execute(
        command,
        frozen_admission=frozen_admission_from_payload(raw_admission),
    )
    return {
        "status": operation.status,
        "operationId": operation.id,
        "runId": operation.run_id,
        "logicalOperation": operation.logical_operation,
    }


@activity.defn(name="generation_video_poll")
async def generation_video_poll(payload: dict[str, object]) -> dict[str, object]:
    _validate_route(payload)
    service = _deps().video
    if service is None:
        raise RuntimeError("video generation activity is unconfigured")
    command = payload.get("command")
    if not isinstance(command, PollVideoCommand):
        raise ValidationDomainError("video poll activity requires a frozen command")
    operation = await service.poll(command)
    return {
        "status": operation.status,
        "runId": operation.run_id,
        "logicalOperation": operation.logical_operation,
    }


@activity.defn(name="generation_video_cancel")
async def generation_video_cancel(payload: dict[str, object]) -> dict[str, object]:
    _validate_route(payload)
    service = _deps().video
    if service is None:
        raise RuntimeError("video generation activity is unconfigured")
    run_id = _payload_string(payload, "runId")
    logical_operation = _payload_string(payload, "logicalOperation")
    status = await service.cancel(run_id, logical_operation)
    return {"status": status, "runId": run_id, "logicalOperation": logical_operation}


@activity.defn(name="generation_video_result_registration")
async def generation_video_result_registration(payload: dict[str, object]) -> dict[str, object]:
    _validate_route(payload)
    service = _deps().video
    if service is None:
        raise RuntimeError("video generation activity is unconfigured")
    required = (
        "runId",
        "logicalOperation",
        "assetVersionId",
        "assetVersionRevision",
        "assetVersionHash",
    )
    values = {key: payload.get(key) for key in required}
    run_id = _payload_string(payload, "runId")
    logical_operation = _payload_string(payload, "logicalOperation")
    if (
        not isinstance(values["assetVersionId"], str)
        or not isinstance(values["assetVersionRevision"], int)
        or not isinstance(values["assetVersionHash"], str)
    ):
        raise ValidationDomainError("video result asset version identity is invalid")
    candidate = await service.register_result(
        run_id,
        logical_operation,
        asset_version_id=values["assetVersionId"],
        asset_version_revision=values["assetVersionRevision"],
        asset_version_hash=values["assetVersionHash"],
        provider_request_id=(
            str(payload["providerRequestId"]) if payload.get("providerRequestId") else None
        ),
    )
    return {
        "status": "pending_review",
        "candidateId": candidate.id,
        "runId": run_id,
        "logicalOperation": logical_operation,
    }


@activity.defn(name="generation_video_reconcile")
async def generation_video_reconcile(payload: dict[str, object]) -> dict[str, object]:
    _validate_route(payload)
    service = _deps().video
    if service is None:
        raise RuntimeError("video generation activity is unconfigured")
    command = payload.get("command")
    if not isinstance(command, ReconcileVideoCommand):
        raise ValidationDomainError("video reconcile activity requires a frozen command")
    operation = await service.reconcile(command)
    return {
        "status": operation.status,
        "runId": operation.run_id,
        "logicalOperation": operation.logical_operation,
    }


@workflow.defn(name="text-generation")
class TextGenerationWorkflow:
    @workflow.run
    async def run(self, payload: dict[str, object]) -> dict[str, object]:
        action = payload.get("action", "generate")
        if action not in {"generate", "reconcile"}:
            raise ValueError("unsupported text generation action")
        return await workflow.execute_activity(
            generation_text_reconcile if action == "reconcile" else generation_text_generate,
            payload,
            activity_id=f"{_payload_string(payload, 'logicalOperation')}:{action}",
            start_to_close_timeout=timedelta(hours=1),
        )


@workflow.defn(name="image-generation")
class ImageGenerationWorkflow:
    @workflow.run
    async def run(self, payload: dict[str, object]) -> dict[str, object]:
        action = payload.get("action", "generate")
        if action not in {"generate", "reconcile"}:
            raise ValueError("unsupported image generation action")
        return await workflow.execute_activity(
            generation_image_reconcile if action == "reconcile" else generation_image_generate,
            payload,
            activity_id=f"{_payload_string(payload, 'logicalOperation')}:{action}",
            start_to_close_timeout=timedelta(hours=1),
        )


@workflow.defn(name="video-generation")
class VideoGenerationWorkflow:
    @workflow.run
    async def run(self, payload: dict[str, object]) -> dict[str, object]:
        action = payload.get("action", "submit")
        if not isinstance(action, str):
            raise ValueError("unsupported video generation action")
        activities = {
            "submit": generation_video_submit,
            "poll": generation_video_poll,
            "cancel": generation_video_cancel,
            "result": generation_video_result_registration,
            "reconcile": generation_video_reconcile,
        }
        selected = activities.get(action)
        if selected is None:
            raise ValueError("unsupported video generation action")
        return await workflow.execute_activity(
            selected,
            payload,
            activity_id=f"{_payload_string(payload, 'logicalOperation')}:{action}",
            start_to_close_timeout=timedelta(hours=1),
        )


GENERATION_WORKFLOWS = (TextGenerationWorkflow, ImageGenerationWorkflow, VideoGenerationWorkflow)
GENERATION_ACTIVITIES = (
    generation_text_generate,
    generation_image_generate,
    generation_text_reconcile,
    generation_image_reconcile,
    generation_video_submit,
    generation_video_poll,
    generation_video_cancel,
    generation_video_result_registration,
    generation_video_reconcile,
)

# Stable semantic aliases keep the activity contract readable to worker wiring
# and make the owner action explicit in tests and diagnostics.
text_generation_activity = generation_text_generate
image_generation_activity = generation_image_generate
text_reconcile_activity = generation_text_reconcile
image_reconcile_activity = generation_image_reconcile
video_submit_activity = generation_video_submit
video_poll_activity = generation_video_poll
video_cancel_activity = generation_video_cancel
video_result_registration_activity = generation_video_result_registration
video_reconcile_activity = generation_video_reconcile
