from __future__ import annotations

import asyncio
import json
from collections.abc import AsyncIterator
from typing import Annotated, Literal, cast

from fastapi import APIRouter, Depends, Header, Query, Request
from fastapi.responses import StreamingResponse
from pydantic import BaseModel, ConfigDict, Field, model_validator

from video_agent_api.application.agent_edit import AgentEditService
from video_agent_api.domain.agent_edit import (
    AssetEditSelection,
    AssetVersionRef,
    ContinuitySnapshotRef,
)
from video_agent_api.domain.errors import AssetEditUnconfiguredError, ValidationDomainError
from video_agent_api.interfaces.http.project_scope import project_scope

router = APIRouter(tags=["agent-asset-edit"])


class _DTO(BaseModel):
    model_config = ConfigDict(
        alias_generator=lambda v: (
            v.split("_")[0] + "".join(x.capitalize() for x in v.split("_")[1:])
        ),
        populate_by_name=True,
        extra="forbid",
    )

    @model_validator(mode="before")
    @classmethod
    def reject_conflicting_schema_fields(cls, value: object) -> object:
        if isinstance(value, dict) and "schemaVersion" in value and "schema_version" in value:
            if value["schemaVersion"] != value["schema_version"]:
                raise ValueError("schemaVersion conflicts with schema_version")
        return value


class VersionRefRequest(_DTO):
    id: str
    revision: int = Field(ge=0)
    content_hash: str = Field(alias="contentHash", min_length=64, max_length=64)
    kind: str
    project_id: str | None = Field(alias="projectId", default=None)
    mime_type: str | None = Field(alias="mimeType", default=None)


class PlanRequest(_DTO):
    episode_id: str = Field(alias="episodeId")
    kind: str
    base: VersionRefRequest
    references: list[VersionRefRequest] = Field(default_factory=list)
    instruction: str
    turn_id: str = Field(alias="turnId")
    target_id: str = Field(alias="targetId", default="")
    schema_version: str = Field(alias="schemaVersion", default="1.0.0")
    mask: object | None = None
    schema_version_canonical: str | None = Field(alias="schema_version", default=None)


class SessionRequest(_DTO):
    episode_id: str = Field(alias="episodeId")
    target_id: str = Field(alias="targetId")
    continuity: dict[str, object]
    primary: VersionRefRequest
    references: list[VersionRefRequest] = Field(default_factory=list)
    schema_version: str = Field(alias="schemaVersion", default="1.0.0")


class MessageRequest(_DTO):
    content_hash: str = Field(alias="contentHash", min_length=64, max_length=64)
    correlation_id: str = Field(alias="correlationId", min_length=1)
    expected_revision: int | None = Field(alias="expectedRevision", default=None, ge=1)


class AgentReplyRequest(MessageRequest):
    expected_turn_revision: int = Field(alias="expectedTurnRevision", ge=1)
    status: Literal["complete", "failed"] = "complete"


class GeneratePlanRequest(PlanRequest):
    session_id: str = Field(alias="sessionId")
    turn_id: str = Field(alias="turnId")
    conversation_id: str = Field(alias="conversationId")
    run_id: str = Field(alias="runId")
    node_run_id: str = Field(alias="nodeRunId")
    logical_operation: str = Field(alias="logicalOperation")
    correlation_id: str = Field(alias="correlationId")


class TargetReferenceRequest(_DTO):
    reference_id: str = Field(alias="referenceId")
    expected_revision: int = Field(alias="expectedRevision", ge=1)


class ReviewRequest(_DTO):
    action: Literal["accept", "reject", "retake"]
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    expected_base_version_id: str | None = Field(alias="expectedBaseVersionId", default=None)
    scope: list[str] = Field(default_factory=list)
    candidate_facts: dict[str, object] | None = Field(alias="candidateFacts", default=None)
    references: list[TargetReferenceRequest] = Field(default_factory=list)
    logical_operation: str | None = Field(alias="logicalOperation", default=None)


class ReconcileRequest(_DTO):
    status: Literal[
        "queued",
        "running",
        "waiting_reconciliation",
        "succeeded",
        "failed",
        "submission_unknown",
        "cancel_requested",
        "cancelled",
    ]
    provider_request_id: str | None = Field(alias="providerRequestId", default=None)


class ExecuteRequest(_DTO):
    plan_revision: int = Field(alias="planRevision", ge=1)
    run_id: str = Field(alias="runId")
    node_run_id: str = Field(alias="nodeRunId")
    logical_operation: str = Field(alias="logicalOperation")
    correlation_id: str = Field(alias="correlationId")
    request_fingerprint: str = Field(alias="requestFingerprint", min_length=64, max_length=64)


def _service(request: Request) -> AgentEditService:
    service = getattr(request.app.state, "agent_edit_service", None)
    if service is None:
        raise AssetEditUnconfiguredError("agent edit service is not configured")
    return cast(AgentEditService, service)


@router.post("/v1/projects/{project_id}/asset-edit-plans")
async def create_plan(
    project_id: str,
    body: PlanRequest,
    service: Annotated[AgentEditService, Depends(_service)],
) -> object:
    if (
        body.schema_version_canonical is not None
        and body.schema_version_canonical != body.schema_version
    ):
        raise ValidationDomainError("schemaVersion conflicts with schema_version")
    if body.schema_version != "1.0.0":
        raise ValidationDomainError("unsupported schemaVersion")
    if body.base.kind not in {"image", "video"} or any(
        item.kind not in {"image", "video"} for item in body.references
    ):
        raise ValidationDomainError("unsupported_feature")
    base_ref = AssetVersionRef(
        body.base.id,
        body.base.revision,
        body.base.content_hash,
        cast(Literal["image", "video"], body.base.kind),
        body.base.project_id or "",
        body.base.mime_type or "",
    )
    reference_refs = tuple(
        AssetVersionRef(
            item.id,
            item.revision,
            item.content_hash,
            cast(Literal["image", "video"], item.kind),
            item.project_id or "",
            item.mime_type or "",
        )
        for item in body.references
    )
    plan = await service.create_plan(
        project_id,
        body.episode_id,
        body.kind,
        base_ref,
        reference_refs,
        body.instruction,
        body.turn_id,
        target_id=body.target_id,
        mask=body.mask,
        schema_version=body.schema_version,
    )
    return {
        "id": plan.id,
        "revision": plan.revision,
        "schemaVersion": plan.schema_version,
        "status": plan.status,
    }


@router.post("/v1/asset-edit-plans/{plan_id}/execute")
async def execute_plan(
    plan_id: str,
    body: ExecuteRequest,
    service: Annotated[AgentEditService, Depends(_service)],
    request: Request,
) -> object:
    execution = await service.execute(
        plan_id,
        body.plan_revision,
        body.run_id,
        body.node_run_id,
        body.logical_operation,
        body.correlation_id,
        body.request_fingerprint,
        project_scope=project_scope(request),
    )
    return {"id": execution.id, "status": execution.status, "revision": execution.revision}


@router.post("/v1/projects/{project_id}/asset-edit-sessions", status_code=201)
async def create_session(
    project_id: str,
    body: SessionRequest,
    service: Annotated[AgentEditService, Depends(_service)],
) -> object:
    continuity = ContinuitySnapshotRef(
        str(body.continuity["id"]),
        int(str(body.continuity["revision"])),
        str(body.continuity["contentHash"]),
        str(body.continuity["targetId"]),
    )
    if body.schema_version != "1.0.0":
        raise ValidationDomainError("unsupported schemaVersion")
    primary = AssetVersionRef(
        body.primary.id,
        body.primary.revision,
        body.primary.content_hash,
        cast(Literal["image", "video"], body.primary.kind),
        body.primary.project_id or "",
        body.primary.mime_type or "",
    )
    refs = tuple(
        AssetVersionRef(
            item.id,
            item.revision,
            item.content_hash,
            cast(Literal["image", "video"], item.kind),
            item.project_id or "",
            item.mime_type or "",
        )
        for item in body.references
    )
    session = await service.create_session(
        project_id,
        body.episode_id,
        AssetEditSelection(project_id, body.episode_id, body.target_id, primary, refs),
        continuity,
    )
    return {"id": session.id, "revision": session.revision, "status": session.status}


@router.get("/v1/projects/{project_id}/asset-edit-sessions")
async def list_sessions(
    project_id: str,
    service: Annotated[AgentEditService, Depends(_service)],
    episode_id: str | None = Query(default=None, alias="episodeId"),
) -> object:
    sessions = await service.list_sessions(project_id, episode_id)
    return {
        "schemaVersion": "1.0.0",
        "items": [
            {
                "id": session.id,
                "revision": session.revision,
                "projectId": session.project_id,
                "episodeId": session.episode_id,
                "targetId": session.selection.target_id,
                "status": session.status,
            }
            for session in sessions
        ],
    }


@router.get("/v1/projects/{project_id}/asset-edit-sessions/{session_id}")
async def get_session_projection(
    project_id: str,
    session_id: str,
    service: Annotated[AgentEditService, Depends(_service)],
) -> object:
    return await service.get_session_projection(project_id, session_id)


@router.get("/v1/projects/{project_id}/asset-edit-plans/{plan_id}")
async def get_plan_projection(
    project_id: str,
    plan_id: str,
    service: Annotated[AgentEditService, Depends(_service)],
) -> object:
    return await service.get_plan_projection(project_id, plan_id)


@router.post("/v1/asset-edit-sessions/{session_id}/messages")
async def append_message(
    session_id: str,
    body: MessageRequest,
    service: Annotated[AgentEditService, Depends(_service)],
    request: Request,
) -> object:
    turn = await service.append_user_message(
        session_id,
        body.content_hash,
        body.correlation_id,
        body.expected_revision,
        project_scope=project_scope(request),
    )
    return {
        "id": turn.id,
        "sequence": turn.sequence,
        "status": turn.status,
        "revision": turn.revision,
    }


@router.post("/v1/asset-edit-sessions/{session_id}/turns/{turn_id}/reply")
async def append_reply(
    session_id: str,
    turn_id: str,
    body: AgentReplyRequest,
    service: Annotated[AgentEditService, Depends(_service)],
    request: Request,
) -> object:
    message = await service.append_agent_reply(
        session_id,
        turn_id,
        body.content_hash,
        body.correlation_id,
        body.expected_turn_revision,
        body.status,
        project_scope=project_scope(request),
    )
    return {"id": message.id, "sequence": message.sequence, "status": message.status}


@router.get("/v1/asset-edit-sessions/{session_id}/events")
async def session_events(
    session_id: str,
    service: Annotated[AgentEditService, Depends(_service)],
    request: Request,
    last_event_id: Annotated[str | None, Header(alias="Last-Event-ID")] = None,
    accept: Annotated[str | None, Header()] = None,
) -> StreamingResponse:
    project_id = project_scope(request)
    try:
        cursor = int(last_event_id or "0")
    except ValueError as error:
        raise ValidationDomainError("Last-Event-ID must be a non-negative integer") from error
    async def replay() -> AsyncIterator[str]:
        nonlocal cursor
        continuous = accept is not None and "text/event-stream" in accept
        while True:
            projection = await service.get_session_projection(project_id, session_id)
            conversation = projection.get("conversation") if isinstance(projection, dict) else None
            messages = conversation.get("messages", []) if isinstance(conversation, dict) else []
            for message in messages:
                sequence = int(message.get("sequence", 0)) if isinstance(message, dict) else 0
                if sequence <= cursor:
                    continue
                payload = json.dumps({"sessionId": session_id, **message}, separators=(",", ":"))
                yield f"id: {sequence}\nevent: asset_edit.message\ndata: {payload}\n\n"
                cursor = sequence
            if not continuous or await request.is_disconnected():
                return
            yield ": keepalive\n\n"
            await asyncio.sleep(1)

    return StreamingResponse(
        replay(),
        media_type="text/event-stream",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


@router.post(
    "/v1/asset-edit-sessions/{session_id}/turns/{turn_id}/asset-edit-plans", status_code=201
)
async def generate_plan(
    session_id: str,
    turn_id: str,
    body: GeneratePlanRequest,
    service: Annotated[AgentEditService, Depends(_service)],
    request: Request,
) -> object:
    if body.session_id != session_id or body.turn_id != turn_id:
        raise ValidationDomainError("conversation turn scope is invalid")
    if body.conversation_id != session_id:
        raise ValidationDomainError("conversation scope is invalid")
    base = AssetVersionRef(
        body.base.id,
        body.base.revision,
        body.base.content_hash,
        cast(Literal["image", "video"], body.base.kind),
        body.base.project_id or "",
        body.base.mime_type or "",
    )
    refs = tuple(
        AssetVersionRef(
            item.id,
            item.revision,
            item.content_hash,
            cast(Literal["image", "video"], item.kind),
            item.project_id or "",
            item.mime_type or "",
        )
        for item in body.references
    )
    plan = await service.generate_plan_from_turn(
        session_id,
        turn_id,
        base,
        refs,
        body.instruction,
        kind=body.kind,
        target_id=body.target_id,
        run_id=body.run_id,
        node_run_id=body.node_run_id,
        logical_operation=body.logical_operation,
        correlation_id=body.correlation_id,
        project_scope=project_scope(request),
    )
    return {
        "id": plan.id,
        "revision": plan.revision,
        "schemaVersion": plan.schema_version,
        "status": plan.status,
    }


@router.get("/v1/asset-edit-plans/{plan_id}/candidates")
async def list_candidates(
    plan_id: str,
    service: Annotated[AgentEditService, Depends(_service)],
    request: Request,
) -> list[object]:
    return [
        {
            "id": item.id,
            "revision": item.revision,
            "status": item.status,
            "assetVersionId": item.asset_version.id,
            "assetVersionRevision": item.asset_version.revision,
            "assetVersionHash": item.asset_version.content_hash,
            "provenance": item.provenance,
        }
        for item in await service.list_candidates(plan_id, project_scope=project_scope(request))
    ]


@router.get("/v1/asset-edit-candidates/{candidate_id}/compare")
async def compare_candidate(
    candidate_id: str,
    service: Annotated[AgentEditService, Depends(_service)],
    request: Request,
) -> object:
    return await service.compare_candidate(candidate_id, project_scope=project_scope(request))


@router.post("/v1/asset-edit-candidates/{candidate_id}/review")
async def review_candidate(
    candidate_id: str,
    body: ReviewRequest,
    service: Annotated[AgentEditService, Depends(_service)],
    request: Request,
) -> object:
    reference_ids = [item.reference_id for item in body.references]
    if len(set(reference_ids)) != len(reference_ids):
        raise ValidationDomainError("accept references must be unique")
    if body.action == "accept" and (not reference_ids or set(reference_ids) != set(body.scope)):
        raise ValidationDomainError("accept references must exactly match scope")
    if body.action == "retake" and not body.logical_operation:
        raise ValidationDomainError("retake logicalOperation is required")
    candidate = await service.decide(
        candidate_id,
        body.action,
        body.expected_revision,
        expected_base_version_id=body.expected_base_version_id,
        scope=tuple(body.scope),
        candidate_facts=body.candidate_facts,
        logical_operation=body.logical_operation,
        project_scope=project_scope(request),
    )
    return {"id": candidate.id, "revision": candidate.revision, "status": candidate.status}


@router.get("/v1/asset-edit-executions/{execution_id}")
async def execution_status(
    execution_id: str,
    service: Annotated[AgentEditService, Depends(_service)],
    request: Request,
) -> object:
    execution = await service.get_execution(execution_id, project_scope=project_scope(request))
    return {
        "id": execution.id,
        "status": execution.status,
        "revision": execution.revision,
        "providerRequestId": execution.provider_request_id,
    }


@router.post("/v1/asset-edit-executions/{execution_id}/reconcile")
async def reconcile_execution(
    execution_id: str,
    body: ReconcileRequest,
    service: Annotated[AgentEditService, Depends(_service)],
    request: Request,
) -> object:
    execution = await service.reconcile_execution(
        execution_id,
        body.status,
        provider_request_id=body.provider_request_id,
        project_scope=project_scope(request),
    )
    return {
        "id": execution.id,
        "status": execution.status,
        "revision": execution.revision,
        "providerRequestId": execution.provider_request_id,
    }
