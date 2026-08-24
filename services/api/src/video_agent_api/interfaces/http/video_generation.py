from __future__ import annotations

from typing import Annotated, cast

from fastapi import APIRouter, Depends, Request
from pydantic import BaseModel, ConfigDict, Field

from video_agent_api.application.video_generation import (
    AgnesVideoService,
    PollVideoCommand,
    ReconcileVideoCommand,
    SubmitVideoCommand,
)
from video_agent_api.domain.errors import DatabaseUnavailableError
from video_agent_api.interfaces.http.project_scope import project_scope

router = APIRouter(tags=["video-generation"])


class _DTO(BaseModel):
    model_config = ConfigDict(
        alias_generator=lambda v: (
            v.split("_")[0] + "".join(x.capitalize() for x in v.split("_")[1:])
        ),
        populate_by_name=True,
        extra="forbid",
    )


class SubmitRequest(_DTO):
    scene_id: str = Field(alias="sceneId")
    run_id: str = Field(alias="runId")
    logical_operation: str = Field(alias="logicalOperation")
    provider_id: str = Field(alias="providerId")
    profile_id: str = Field(alias="profileId")
    model_id: str = Field(alias="modelId")
    capability_snapshot_id: str = Field(alias="capabilitySnapshotId")
    capability_revision: int = Field(alias="capabilityRevision", ge=1)
    source_asset_version_id: str = Field(alias="sourceAssetVersionId")
    source_asset_version_revision: int = Field(alias="sourceAssetVersionRevision", ge=0)
    source_asset_version_hash: str = Field(
        alias="sourceAssetVersionHash", min_length=64, max_length=64
    )
    shot_spec_id: str = Field(alias="shotSpecId")
    shot_spec_revision: int = Field(alias="shotSpecRevision", ge=1)
    shot_spec_hash: str = Field(alias="shotSpecHash", min_length=64, max_length=64)
    duration_seconds: float = Field(alias="durationSeconds", gt=0)
    aspect_ratio: str = Field(alias="aspectRatio", min_length=1)
    parameters: dict[str, object] = Field(default_factory=dict)
    prompt: str = ""
    source_candidate_id: str | None = Field(default=None, alias="sourceCandidateId")
    source_provenance: str | None = Field(default=None, alias="sourceProvenance")
    source_schema_version: str | None = Field(default=None, alias="sourceSchemaVersion")
    schema_version: str | None = Field(default=None, alias="schemaVersion")


class PollRequest(_DTO):
    run_id: str = Field(alias="runId")
    logical_operation: str = Field(alias="logicalOperation")


class ReconcileRequest(_DTO):
    run_id: str = Field(alias="runId")
    logical_operation: str = Field(alias="logicalOperation")
    provider_request_id: str | None = Field(default=None, alias="providerRequestId")


def _service(request: Request) -> AgnesVideoService:
    service = getattr(request.app.state, "video_generation_service", None)
    if service is None:
        raise DatabaseUnavailableError("video generation service is not configured")
    return cast(AgnesVideoService, service)


@router.post("/v1/projects/{project_id}/episodes/{episode_id}/shots/{shot_id}/video-operations")
async def submit_video(
    project_id: str,
    episode_id: str,
    shot_id: str,
    body: SubmitRequest,
    service: Annotated[AgnesVideoService, Depends(_service)],
) -> object:
    operation = await service.submit(
        SubmitVideoCommand(
            project_id=project_id,
            episode_id=episode_id,
            scene_id=body.scene_id,
            shot_id=shot_id,
            run_id=body.run_id,
            logical_operation=body.logical_operation,
            provider_id=body.provider_id,
            profile_id=body.profile_id,
            model_id=body.model_id,
            capability_snapshot_id=body.capability_snapshot_id,
            capability_revision=body.capability_revision,
            source_asset_version_id=body.source_asset_version_id,
            source_asset_version_revision=body.source_asset_version_revision,
            source_asset_version_hash=body.source_asset_version_hash,
            shot_spec_id=body.shot_spec_id,
            shot_spec_revision=body.shot_spec_revision,
            shot_spec_hash=body.shot_spec_hash,
            duration_seconds=body.duration_seconds,
            aspect_ratio=body.aspect_ratio,
            parameters=body.parameters,
            prompt=body.prompt,
            source_candidate_id=body.source_candidate_id,
            source_provenance=body.source_provenance,
            source_schema_version=body.source_schema_version,
            schema_version=body.schema_version,
        )
    )
    return {
        "id": operation.id,
        "runId": operation.run_id,
        "logicalOperation": operation.logical_operation,
        "status": operation.status,
        "revision": operation.revision,
        "providerRequestId": operation.provider_request_id,
    }


@router.post("/v1/video-operations/poll")
async def poll_video(
    body: PollRequest,
    service: Annotated[AgnesVideoService, Depends(_service)],
    request: Request,
) -> object:
    operation = await service.poll(
        PollVideoCommand(body.run_id, body.logical_operation),
        project_scope=project_scope(request),
    )
    return {
        "runId": operation.run_id,
        "logicalOperation": operation.logical_operation,
        "status": operation.status,
        "revision": operation.revision,
        "providerRequestId": operation.provider_request_id,
    }


@router.post("/v1/video-operations/cancel")
async def cancel_video(
    body: PollRequest,
    service: Annotated[AgnesVideoService, Depends(_service)],
    request: Request,
) -> object:
    return {
        "runId": body.run_id,
        "logicalOperation": body.logical_operation,
        "status": await service.cancel(
            body.run_id,
            body.logical_operation,
            project_scope=project_scope(request),
        ),
    }


@router.post("/v1/video-operations/reconcile")
async def reconcile_video(
    body: ReconcileRequest,
    service: Annotated[AgnesVideoService, Depends(_service)],
    request: Request,
) -> object:
    operation = await service.reconcile(
        ReconcileVideoCommand(body.run_id, body.logical_operation, body.provider_request_id),
        project_scope=project_scope(request),
    )
    return {
        "runId": operation.run_id,
        "logicalOperation": operation.logical_operation,
        "status": operation.status,
        "revision": operation.revision,
        "providerRequestId": operation.provider_request_id,
    }
