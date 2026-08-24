from __future__ import annotations

from typing import Annotated, cast

from fastapi import APIRouter, Depends, Header, Request
from pydantic import BaseModel, ConfigDict, Field

from video_agent_api.application.creative import (
    BindCreativeSourceCommand,
    CreativeService,
    SaveCreativeBriefCommand,
    SaveCreativeSettingsCommand,
)
from video_agent_api.domain.creative import (
    CreationMode,
    CreativeBriefSourceBindingSnapshot,
    ProjectEpisodeTextHandoff,
)
from video_agent_api.domain.errors import DomainError, RevisionConflictError

router = APIRouter(tags=["creative"])


class _DTO(BaseModel):
    model_config = ConfigDict(
        alias_generator=lambda v: (
            v.split("_")[0] + "".join(x.capitalize() for x in v.split("_")[1:])
        ),
        populate_by_name=True,
        extra="forbid",
    )


class BriefRequest(_DTO):
    creation_mode: str = Field(alias="creationMode", pattern="^(original|adaptation)$")
    subject: str
    genre: str
    audience: str
    character_premise: str = Field(alias="characterPremise")
    style: str
    episode_duration_seconds: int = Field(alias="episodeDurationSeconds", ge=1)
    episode_count: int = Field(alias="episodeCount", ge=1)
    scenes_per_episode: int = Field(alias="scenesPerEpisode", ge=1)
    shots_per_scene: int = Field(alias="shotsPerScene", ge=1)
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    expected_brief_revision: int | None = Field(default=None, alias="expectedBriefRevision", ge=1)
    schema_version: str = Field(default="1.0.0", alias="schemaVersion", pattern=r"^\d+\.\d+\.\d+$")


class SettingsRequest(_DTO):
    threshold: dict[str, str] | None = None
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    expected_settings_revision: int | None = Field(
        default=None, alias="expectedSettingsRevision", ge=1
    )
    schema_version: str = Field(default="1.0.0", alias="schemaVersion", pattern=r"^\d+\.\d+\.\d+$")


class SourceBindingRequest(_DTO):
    source_material_id: str = Field(alias="sourceMaterialId")
    source_material_revision: int = Field(alias="sourceMaterialRevision", ge=1)
    source_content_hash: str = Field(alias="sourceContentHash")
    creative_brief_id: str = Field(alias="creativeBriefId")
    creative_brief_revision: int = Field(alias="creativeBriefRevision", ge=1)
    creative_brief_payload_hash: str = Field(alias="creativeBriefPayloadHash")
    parse_status: str = Field(alias="parseStatus")
    validation_status: str = Field(alias="validationStatus")
    binding_status: str = Field(alias="bindingStatus")
    binding_version: str = Field(alias="bindingVersion")
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    schema_version: str = Field(default="1.0.0", alias="schemaVersion", pattern=r"^\d+\.\d+\.\d+$")


class HandoffRequest(_DTO):
    handoff_id: str = Field(alias="handoffId")
    project_revision: int = Field(alias="projectRevision", ge=1)
    batch_revision: int = Field(alias="batchRevision", ge=1)
    story_spec_id: str = Field(alias="storySpecId")
    story_spec_revision: int = Field(alias="storySpecRevision", ge=1)
    story_spec_hash: str = Field(alias="storySpecHash", min_length=64, max_length=64)
    episode_script_refs: list[dict[str, object]] = Field(alias="episodeScriptRefs", min_length=1)
    payload_hash: str = Field(alias="payloadHash", min_length=64, max_length=64)
    correlation_id: str = Field(alias="correlationId")
    schema_version: str = Field(default="1.0.0", alias="schemaVersion", pattern=r"^\d+\.\d+\.\d+$")


def _service(request: Request) -> CreativeService:
    service = getattr(request.app.state, "creative_service", None)
    if service is None:
        raise DomainError("creative service is not configured")
    return cast(CreativeService, service)


def _expected(body: int, if_match: str | None) -> int:
    if if_match is None or not if_match.isdecimal() or int(if_match) != body:
        raise RevisionConflictError("project", body, 0)
    return body


@router.get("/v1/projects/{project_id}/creative")
async def get_creative(
    project_id: str, service: Annotated[CreativeService, Depends(_service)]
) -> dict[str, object]:
    return await service.get_projection(project_id)


@router.put("/v1/projects/{project_id}/creative/brief")
async def save_brief(
    project_id: str,
    body: BriefRequest,
    service: Annotated[CreativeService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return await service.save_brief(
        SaveCreativeBriefCommand(
            project_id,
            cast(CreationMode, body.creation_mode),
            body.model_dump(by_alias=True),
            _expected(body.expected_revision, if_match),
            body.expected_brief_revision,
        )
    )


@router.put("/v1/projects/{project_id}/creative/settings")
async def save_settings(
    project_id: str,
    body: SettingsRequest,
    service: Annotated[CreativeService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return await service.save_settings(
        SaveCreativeSettingsCommand(
            project_id,
            body.threshold,
            _expected(body.expected_revision, if_match),
            body.expected_settings_revision,
        )
    )


@router.put("/v1/projects/{project_id}/creative/source-binding")
async def bind_source(
    project_id: str,
    body: SourceBindingRequest,
    service: Annotated[CreativeService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    snapshot = CreativeBriefSourceBindingSnapshot(
        project_id=project_id, **body.model_dump(exclude={"expected_revision"})
    )
    return await service.bind_source(
        BindCreativeSourceCommand(
            project_id,
            snapshot,
            _expected(body.expected_revision, if_match),
            body.creative_brief_revision,
        )
    )


@router.get("/v1/projects/{project_id}/creative/episodes")
async def creative_episodes(
    project_id: str, service: Annotated[CreativeService, Depends(_service)]
) -> list[dict[str, object]]:
    return await service.list_episodes_projection(project_id)


@router.post("/v1/projects/{project_id}/creative/text-handoff")
async def apply_text_handoff(
    project_id: str,
    body: HandoffRequest,
    service: Annotated[CreativeService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    _expected(body.project_revision, if_match)
    if body.schema_version != "1.0.0":
        raise DomainError("unsupported schema_version")
    handoff = ProjectEpisodeTextHandoff(
        handoff_id=body.handoff_id,
        project_id=project_id,
        project_revision=body.project_revision,
        batch_revision=body.batch_revision,
        story_spec_id=body.story_spec_id,
        story_spec_revision=body.story_spec_revision,
        story_spec_hash=body.story_spec_hash,
        episode_script_refs=tuple(body.episode_script_refs),
        payload_hash=body.payload_hash,
        correlation_id=body.correlation_id,
        schema_version=body.schema_version,
    )
    return await service.apply_handoff(handoff)
