from __future__ import annotations

from typing import Annotated, cast

from fastapi import APIRouter, Depends, Request
from fastapi.encoders import jsonable_encoder
from pydantic import BaseModel, ConfigDict, Field

from video_agent_api.application.skill_routing import (
    ResolveSkillRouteCommand,
    SelectSkillRouteCommand,
    SkillRoutingService,
)
from video_agent_api.application.text_generation import (
    GenerateTextBatchCommand,
    RegenerateTextCandidateCommand,
    TextGenerationService,
)
from video_agent_api.domain.creative import (
    CreativeBriefSourceBindingSnapshot,
    CreativeBriefVersion,
)
from video_agent_api.domain.errors import DatabaseUnavailableError
from video_agent_api.interfaces.http.project_scope import project_scope
from video_agent_api.ports.contracts import ModelSelection

router = APIRouter(tags=["text-generation"])


class DTO(BaseModel):
    model_config = ConfigDict(
        alias_generator=lambda value: (
            value.split("_")[0] + "".join(part.capitalize() for part in value.split("_")[1:])
        ),
        populate_by_name=True,
        extra="forbid",
    )


class CreativeBriefSnapshotRequest(DTO):
    creative_brief_id: str = Field(alias="creativeBriefId")
    subject: str
    genre: str
    audience: str
    character_premise: str = Field(alias="characterPremise")
    style: str
    episode_duration_seconds: int = Field(alias="episodeDurationSeconds", ge=1)
    episode_count: int = Field(alias="episodeCount", ge=1)
    scenes_per_episode: int = Field(alias="scenesPerEpisode", ge=1)
    shots_per_scene: int = Field(alias="shotsPerScene", ge=1)
    revision: int = Field(ge=1)
    schema_version: str = Field(alias="schemaVersion")
    payload_hash: str = Field(alias="payloadHash", min_length=64, max_length=64)


class SourceBindingSnapshotRequest(DTO):
    source_material_id: str = Field(alias="sourceMaterialId")
    source_material_revision: int = Field(alias="sourceMaterialRevision", ge=1)
    source_content_hash: str = Field(alias="sourceContentHash", min_length=64, max_length=64)
    creative_brief_id: str = Field(alias="creativeBriefId")
    creative_brief_revision: int = Field(alias="creativeBriefRevision", ge=1)
    creative_brief_payload_hash: str = Field(
        alias="creativeBriefPayloadHash", min_length=64, max_length=64
    )
    parse_status: str = Field(alias="parseStatus")
    validation_status: str = Field(alias="validationStatus")
    binding_status: str = Field(alias="bindingStatus")
    binding_version: str = Field(alias="bindingVersion")
    schema_version: str = Field(alias="schemaVersion")


class GenerateRequest(DTO):
    run_id: str
    brief_revision: int = Field(alias="briefRevision", ge=1)
    provider_id: str = Field(alias="providerId")
    profile_id: str = Field(alias="profileId")
    model_id: str = Field(alias="modelId")
    adapter_key: str = Field(alias="adapterKey")
    creative_brief: CreativeBriefSnapshotRequest = Field(alias="creativeBrief")
    source_binding: SourceBindingSnapshotRequest | None = Field(default=None, alias="sourceBinding")
    requested_kinds: list[str] = Field(alias="requestedKinds", min_length=1)
    scope_ids: list[str] = Field(alias="scopeIds", min_length=1)
    schema_version: str = Field(default="1.0.0", alias="schemaVersion")


def _camel(value: str) -> str:
    head, *tail = value.split("_")
    return head + "".join(part.capitalize() for part in tail)


def _response(value: object) -> object:
    encoded = jsonable_encoder(value)
    if isinstance(encoded, dict):
        return {_camel(str(key)): _response(item) for key, item in encoded.items()}
    if isinstance(encoded, list):
        return [_response(item) for item in encoded]
    return encoded


def _service(request: Request) -> TextGenerationService:
    service = getattr(request.app.state, "text_generation_service", None)
    if service is None:
        raise DatabaseUnavailableError("text generation service is not configured")
    return cast(TextGenerationService, service)


def _skill_service(request: Request) -> SkillRoutingService:
    service = getattr(request.app.state, "skill_routing_service", None)
    if service is None:
        raise DatabaseUnavailableError("skill routing service is not configured")
    return cast(SkillRoutingService, service)


@router.post("/v1/projects/{project_id}/text-review-batches", status_code=201)
async def generate(
    project_id: str,
    body: GenerateRequest,
    service: Annotated[TextGenerationService, Depends(_service)],
) -> object:
    if body.schema_version != "1.0.0":
        from video_agent_api.domain.errors import ValidationDomainError

        raise ValidationDomainError("unsupported schemaVersion")
    brief = CreativeBriefVersion(
        body.creative_brief.creative_brief_id,
        project_id,
        body.creative_brief.subject,
        body.creative_brief.genre,
        body.creative_brief.audience,
        body.creative_brief.character_premise,
        body.creative_brief.style,
        body.creative_brief.episode_duration_seconds,
        body.creative_brief.episode_count,
        body.creative_brief.scenes_per_episode,
        body.creative_brief.shots_per_scene,
        body.creative_brief.revision,
        body.creative_brief.schema_version,
        payload_hash=body.creative_brief.payload_hash,
    )
    source = body.source_binding
    source_snapshot = (
        CreativeBriefSourceBindingSnapshot(
            project_id,
            source.source_material_id,
            source.source_material_revision,
            source.source_content_hash,
            source.creative_brief_id,
            source.creative_brief_revision,
            source.creative_brief_payload_hash,
            source.parse_status,
            source.validation_status,
            source.binding_status,
            source.binding_version,
            source.schema_version,
        )
        if source is not None
        else None
    )
    return _response(
        await service.generate(
            GenerateTextBatchCommand(
                project_id,
                body.run_id,
                body.brief_revision,
                ModelSelection(body.provider_id, body.profile_id, body.model_id, body.adapter_key),
                brief,
                source_snapshot,
                tuple(body.requested_kinds),
                tuple(body.scope_ids),
            )
        )
    )


class DecideRequest(DTO):
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    action: str


class HandoffAckRequest(DTO):
    owner: str
    owner_revision: int = Field(alias="ownerRevision", ge=1)
    fingerprint: str
    correlation_id: str = Field(alias="correlationId")


class RegenerateRequest(DTO):
    candidate_id: str = Field(alias="candidateId")
    expected_batch_revision: int = Field(alias="expectedBatchRevision", ge=1)
    expected_candidate_revision: int = Field(alias="expectedCandidateRevision", ge=1)
    payload: dict[str, object]
    source_candidate_ids: list[str] = Field(alias="sourceCandidateIds")
    source_hashes: list[str] = Field(alias="sourceHashes")


@router.post("/v1/text-review-batches/{batch_id}/decision")
async def decide(
    batch_id: str,
    body: DecideRequest,
    service: Annotated[TextGenerationService, Depends(_service)],
    request: Request,
) -> object:
    scope = project_scope(request)
    batch = await service.decide(batch_id, body.expected_revision, body.action, project_scope=scope)
    handoff = await service.handoff_for_batch(batch.id, project_scope=scope)
    return _response({"batch": batch, "handoff": handoff})


@router.get("/v1/projects/{project_id}/text-review-batches")
async def list_batches(
    project_id: str, service: Annotated[TextGenerationService, Depends(_service)]
) -> object:
    return _response(await service.list_batches(project_id))


@router.get("/v1/text-review-batches/{batch_id}")
async def get_batch(
    batch_id: str,
    service: Annotated[TextGenerationService, Depends(_service)],
    request: Request,
) -> object:
    return _response(await service.get_batch(batch_id, project_scope=project_scope(request)))


@router.post("/v1/text-review-batches/{batch_id}/regenerate", status_code=201)
async def regenerate(
    batch_id: str,
    body: RegenerateRequest,
    service: Annotated[TextGenerationService, Depends(_service)],
    request: Request,
) -> object:
    return _response(
        await service.regenerate(
            RegenerateTextCandidateCommand(
                batch_id,
                body.candidate_id,
                body.expected_batch_revision,
                body.expected_candidate_revision,
                body.payload,
                tuple(body.source_candidate_ids),
                tuple(body.source_hashes),
            ),
            project_scope=project_scope(request),
        )
    )


@router.post("/v1/text-handoffs/{handoff_id}/acks", status_code=201)
async def ack_handoff(
    handoff_id: str,
    body: HandoffAckRequest,
    service: Annotated[TextGenerationService, Depends(_service)],
    request: Request,
) -> object:
    return _response(
        await service.ack_handoff(
            handoff_id,
            body.owner,
            body.owner_revision,
            body.fingerprint,
            body.correlation_id,
            project_scope=project_scope(request),
        )
    )


@router.get("/v1/text-handoffs/{handoff_id}/media-gate")
async def media_gate(
    handoff_id: str,
    service: Annotated[TextGenerationService, Depends(_service)],
    request: Request,
) -> dict[str, object]:
    return await service.media_gate(handoff_id, project_scope=project_scope(request))


class ResolveSkillRouteRequest(DTO):
    node_key: str = Field(alias="nodeKey")
    launch_id: str = Field(alias="launchId")
    project_type: str = Field(alias="projectType")
    stage: str
    target_model: str = Field(alias="targetModel")
    query: str
    allowed_tools: list[str] = Field(alias="allowedTools", min_length=1)
    allowed_licenses: list[str] = Field(alias="allowedLicenses", min_length=1)
    allowed_skills: list[str] = Field(alias="allowedSkills", min_length=1)
    required_capabilities: list[str] = Field(alias="requiredCapabilities", min_length=1)
    selection_mode: str = Field(alias="selectionMode")


class SelectSkillRouteRequest(DTO):
    skill_name: str = Field(alias="skillName")
    skill_version: str = Field(alias="skillVersion")
    actor_uuid: str = Field(alias="actorUuid")
    expected_revision: int = Field(alias="expectedRevision", ge=1)


@router.post("/v1/projects/{project_id}/skill-route-decisions", status_code=201)
async def resolve_skill_route(
    project_id: str,
    body: ResolveSkillRouteRequest,
    service: Annotated[SkillRoutingService, Depends(_skill_service)],
) -> object:
    return _response(
        await service.resolve(
            ResolveSkillRouteCommand(
                project_id,
                body.node_key,
                body.launch_id,
                body.project_type,
                body.stage,
                body.target_model,
                body.query,
                frozenset(body.allowed_tools),
                frozenset(body.allowed_licenses),
                frozenset(body.allowed_skills),
                frozenset(body.required_capabilities),
                body.selection_mode,
            )
        )
    )


@router.get("/v1/projects/{project_id}/skill-route-decisions")
async def list_skill_routes(
    project_id: str,
    service: Annotated[SkillRoutingService, Depends(_skill_service)],
) -> object:
    return _response(await service.list(project_id))


@router.get("/v1/skill-route-decisions/{decision_id}")
async def get_skill_route(
    decision_id: str,
    service: Annotated[SkillRoutingService, Depends(_skill_service)],
    request: Request,
) -> object:
    return _response(await service.get(decision_id, project_scope=project_scope(request)))


@router.post("/v1/skill-route-decisions/{decision_id}/selection", status_code=201)
async def select_skill_route(
    decision_id: str,
    body: SelectSkillRouteRequest,
    service: Annotated[SkillRoutingService, Depends(_skill_service)],
    request: Request,
) -> object:
    return _response(
        await service.select(
            SelectSkillRouteCommand(
                decision_id,
                body.skill_name,
                body.skill_version,
                body.actor_uuid,
                body.expected_revision,
            ),
            project_scope=project_scope(request),
        )
    )
