from __future__ import annotations

from typing import Annotated, cast

from fastapi import APIRouter, Depends, Request
from pydantic import BaseModel, ConfigDict, Field

from video_agent_api.application.scenes import (
    AttachContinuityCommand,
    CreateSceneCommand,
    CreateShotCommand,
    ReorderScenesCommand,
    ReorderShotsCommand,
    ReviewMediaCommand,
    ScenesService,
    scene_projection,
    shot_projection,
)
from video_agent_api.domain.errors import DatabaseUnavailableError, ValidationDomainError
from video_agent_api.domain.scenes import SCHEMA_VERSION, SceneShotBatchHandoff

router = APIRouter(tags=["scenes"])


class _DTO(BaseModel):
    model_config = ConfigDict(
        alias_generator=lambda v: (
            v.split("_")[0] + "".join(x.capitalize() for x in v.split("_")[1:])
        ),
        populate_by_name=True,
        extra="forbid",
    )


class ReorderRequest(_DTO):
    scene_ids: list[str] = Field(alias="sceneIds", min_length=1)
    expected_revision: int = Field(alias="expectedRevision", ge=1)


class ReviewRequest(_DTO):
    decision: str
    candidate: dict[str, object] | None = None
    expected_shot_revision: int = Field(alias="expectedShotRevision", ge=1)


class CreateOwnerRequest(_DTO):
    schema_version: str = Field(alias="schemaVersion")


class HandoffRequest(_DTO):
    handoff_id: str = Field(alias="handoffId")
    batch_revision: int = Field(alias="batchRevision", ge=1)
    correlation_id: str = Field(alias="correlationId", min_length=1)
    payload_hash: str = Field(alias="payloadHash", min_length=64, max_length=64)
    accepted: bool
    scenes: list[dict[str, object]] = Field(min_length=1)
    schema_version: str = Field(alias="schemaVersion")


class UnsupportedRequest(_DTO):
    operation: str


class ShotReorderRequest(_DTO):
    shot_ids: list[str] = Field(alias="shotIds", min_length=1)
    expected_revision: int = Field(alias="expectedRevision", ge=1)


class SpecRequest(_DTO):
    payload: dict[str, object]
    shot_id: str | None = Field(default=None, alias="shotId")


class ContinuityRequest(_DTO):
    snapshot_id: str = Field(alias="snapshotId", min_length=1)
    snapshot_revision: int = Field(alias="snapshotRevision", ge=1)
    snapshot_hash: str = Field(alias="snapshotHash", min_length=64, max_length=64)
    expected_shot_revision: int = Field(alias="expectedShotRevision", ge=1)


def _service(request: Request) -> ScenesService:
    service = getattr(request.app.state, "scenes_service", None)
    if service is None:
        raise DatabaseUnavailableError("scenes service is not configured")
    return cast(ScenesService, service)


@router.post("/v1/projects/{project_id}/episodes/{episode_id}/scenes", status_code=201)
async def create_scene(
    project_id: str,
    episode_id: str,
    body: CreateOwnerRequest,
    service: Annotated[ScenesService, Depends(_service)],
) -> object:
    if body.schema_version != SCHEMA_VERSION:
        raise ValidationDomainError("unsupported schemaVersion")
    return scene_projection(await service.create_scene(CreateSceneCommand(project_id, episode_id)))


@router.get("/v1/projects/{project_id}/episodes/{episode_id}/storyboard")
async def storyboard(
    project_id: str, episode_id: str, service: Annotated[ScenesService, Depends(_service)]
) -> list[dict[str, object]]:
    return await service.list_episode(project_id, episode_id)


@router.post("/v1/projects/{project_id}/episodes/{episode_id}/scenes/reorder")
async def reorder(
    project_id: str,
    episode_id: str,
    body: ReorderRequest,
    service: Annotated[ScenesService, Depends(_service)],
) -> object:
    return await service.reorder_scenes(
        ReorderScenesCommand(project_id, episode_id, body.scene_ids, body.expected_revision)
    )


@router.post(
    "/v1/projects/{project_id}/episodes/{episode_id}/scenes/{scene_id}/shots", status_code=201
)
async def create_shot(
    project_id: str,
    episode_id: str,
    scene_id: str,
    body: CreateOwnerRequest,
    service: Annotated[ScenesService, Depends(_service)],
) -> object:
    if body.schema_version != SCHEMA_VERSION:
        raise ValidationDomainError("unsupported schemaVersion")
    return shot_projection(
        await service.create_shot(CreateShotCommand(project_id, episode_id, scene_id))
    )


@router.post("/v1/projects/{project_id}/episodes/{episode_id}/shots/{shot_id}/media-review")
async def review(
    project_id: str,
    episode_id: str,
    shot_id: str,
    body: ReviewRequest,
    service: Annotated[ScenesService, Depends(_service)],
) -> object:
    return shot_projection(
        await service.review_media(
            ReviewMediaCommand(
                project_id,
                episode_id,
                shot_id,
                body.decision,
                body.candidate,
                body.expected_shot_revision,
            )
        )
    )


@router.put("/v1/projects/{project_id}/episodes/{episode_id}/shots/{shot_id}/continuity")
async def attach_continuity(
    project_id: str,
    episode_id: str,
    shot_id: str,
    body: ContinuityRequest,
    service: Annotated[ScenesService, Depends(_service)],
) -> object:
    return shot_projection(
        await service.attach_continuity(
            AttachContinuityCommand(
                project_id,
                episode_id,
                shot_id,
                body.snapshot_id,
                body.snapshot_revision,
                body.snapshot_hash,
                body.expected_shot_revision,
            )
        )
    )


@router.post("/v1/shots/{shot_id}/video-review", deprecated=True)
async def legacy_review(
    shot_id: str, body: ReviewRequest, service: Annotated[ScenesService, Depends(_service)]
) -> object:
    return await service.review_video(shot_id, body.decision, body.candidate)


@router.post("/v1/projects/{project_id}/episodes/{episode_id}/scenes/{scene_id}/shots/reorder")
async def reorder_shots(
    project_id: str,
    episode_id: str,
    scene_id: str,
    body: ShotReorderRequest,
    service: Annotated[ScenesService, Depends(_service)],
) -> object:
    return await service.reorder_shots(
        ReorderShotsCommand(project_id, episode_id, scene_id, body.shot_ids, body.expected_revision)
    )


@router.post(
    "/v1/projects/{project_id}/episodes/{episode_id}/scenes/{scene_id}/specs", status_code=201
)
async def append_spec(
    project_id: str,
    episode_id: str,
    scene_id: str,
    body: SpecRequest,
    service: Annotated[ScenesService, Depends(_service)],
) -> object:
    return await service.append_spec(project_id, episode_id, scene_id, body.payload, body.shot_id)


@router.get("/v1/projects/{project_id}/episodes/{episode_id}/workflow-scope")
async def workflow_scope(
    project_id: str,
    episode_id: str,
    service: Annotated[ScenesService, Depends(_service)],
) -> dict[str, object]:
    return await service.workflow_scope(project_id, episode_id)


@router.post("/v1/projects/{project_id}/episodes/{episode_id}/scene-shot-handoffs", status_code=201)
async def apply_text_handoff(
    project_id: str,
    episode_id: str,
    body: HandoffRequest,
    service: Annotated[ScenesService, Depends(_service)],
) -> object:
    handoff = SceneShotBatchHandoff(
        body.handoff_id,
        project_id,
        episode_id,
        body.batch_revision,
        body.correlation_id,
        body.payload_hash,
        body.accepted,
        tuple(body.scenes),
        body.schema_version,
    )
    return await service.apply_text_handoff(handoff)


@router.post("/v1/projects/{project_id}/episodes/{episode_id}/storyboard/structure")
async def unsupported_structure_edit(
    project_id: str,
    episode_id: str,
    body: UnsupportedRequest,
    service: Annotated[ScenesService, Depends(_service)],
) -> None:
    del project_id, episode_id, body
    await service.unsupported_structure_edit()
