"""HTTP command boundary for immutable image-generation candidates."""

from __future__ import annotations

from typing import Annotated, Literal, cast

from fastapi import APIRouter, Depends, Request
from pydantic import BaseModel, ConfigDict, Field

from video_agent_api.application.image_generation import (
    GenerateImageCommand,
    ImageGenerationService,
)
from video_agent_api.domain.errors import DatabaseUnavailableError
from video_agent_api.domain.image_generation import ImageReference
from video_agent_api.interfaces.http.project_scope import project_scope

router = APIRouter(tags=["image-generation"])


class _DTO(BaseModel):
    model_config = ConfigDict(
        alias_generator=lambda value: (
            value.split("_")[0] + "".join(piece.capitalize() for piece in value.split("_")[1:])
        ),
        populate_by_name=True,
        extra="forbid",
    )


class ImageReferenceRequest(_DTO):
    asset_version_id: str
    asset_version_revision: int = Field(ge=0)
    asset_version_hash: str = Field(min_length=64, max_length=64)
    mime_type: str
    size_bytes: int = Field(ge=0)


class GenerateImageRequest(_DTO):
    episode_id: str
    target_id: str
    asset_id: str
    run_id: str
    logical_operation: str
    operation: Literal["generate", "edit"]
    prompt: str
    provider_id: str
    profile_id: str
    profile_revision: int = Field(ge=1)
    model_id: str
    capability_snapshot_id: str
    capability_revision: int = Field(ge=1)
    continuity_snapshot_id: str
    continuity_snapshot_revision: int = Field(ge=1)
    continuity_snapshot_hash: str = Field(min_length=64, max_length=64)
    target_revision: int = Field(ge=1)
    parameters: dict[str, object] = Field(default_factory=dict)
    references: list[ImageReferenceRequest] = Field(default_factory=list)
    mask_base64: str | None = None


def _service(request: Request) -> ImageGenerationService:
    service = getattr(request.app.state, "image_generation_service", None)
    if service is None:
        raise DatabaseUnavailableError("image generation service is not configured")
    return cast(ImageGenerationService, service)


@router.post("/v1/projects/{project_id}/image-candidates", status_code=202)
async def generate_image(
    project_id: str,
    body: GenerateImageRequest,
    service: Annotated[ImageGenerationService, Depends(_service)],
    request: Request,
) -> dict[str, object]:
    operation = await service.enqueue(
        GenerateImageCommand(
            project_id=project_id,
            episode_id=body.episode_id,
            target_id=body.target_id,
            asset_id=body.asset_id,
            run_id=body.run_id,
            logical_operation=body.logical_operation,
            operation=body.operation,
            prompt=body.prompt,
            provider_id=body.provider_id,
            profile_id=body.profile_id,
            profile_revision=body.profile_revision,
            model_id=body.model_id,
            capability_snapshot_id=body.capability_snapshot_id,
            capability_revision=body.capability_revision,
            continuity_snapshot_id=body.continuity_snapshot_id,
            continuity_snapshot_revision=body.continuity_snapshot_revision,
            continuity_snapshot_hash=body.continuity_snapshot_hash,
            target_revision=body.target_revision,
            parameters=body.parameters,
            references=tuple(
                ImageReference(
                    project_id,
                    value.asset_version_id,
                    value.asset_version_revision,
                    value.asset_version_hash,
                    value.mime_type,
                    value.size_bytes,
                )
                for value in body.references
            ),
            mask_base64=body.mask_base64,
        ),
        project_scope=project_scope(request),
    )
    return {
        "id": operation.id,
        "runId": operation.run_id,
        "logicalOperation": operation.logical_operation,
        "status": operation.status,
        "candidateId": operation.candidate.id if operation.candidate is not None else None,
        "candidateStatus": operation.candidate.status if operation.candidate is not None else None,
    }
