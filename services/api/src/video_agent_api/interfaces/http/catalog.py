from __future__ import annotations

from typing import Annotated, cast

from fastapi import APIRouter, Depends, Header, Query, Request
from fastapi.encoders import jsonable_encoder
from pydantic import BaseModel, ConfigDict, Field

from video_agent_api.application.catalog import (
    AppendSkillRevisionCommand,
    CatalogService,
    CreateModelCommand,
    CreateProfileCommand,
    CreateProviderCommand,
    ModelSyncCommand,
    ReplaceCredentialCommand,
    SetQuotaCommand,
    UpdateCatalogCommand,
)
from video_agent_api.domain.errors import (
    DatabaseUnavailableError,
    ProjectAccessForbiddenError,
    RevisionConflictError,
    ValidationDomainError,
)

router = APIRouter(tags=["catalog"])


class _DTO(BaseModel):
    model_config = ConfigDict(
        alias_generator=lambda v: (
            v.split("_")[0] + "".join(x.capitalize() for x in v.split("_")[1:])
        ),
        populate_by_name=True,
        extra="forbid",
    )


class ProviderRequest(_DTO):
    name: str
    adapter_key: str


class ProfileRequest(_DTO):
    provider_id: str
    name: str
    adapter_identity: str = "local_workspace"


class ModelRequest(_DTO):
    profile_id: str
    model_key: str


class QuotaRequest(_DTO):
    profile_id: str
    operation: str
    status: str
    remaining: int | None = None
    reset_at: str | None = None
    source: str = "local"


class AdmissionRequest(_DTO):
    operation: str
    live: bool = False


class UpdateRequest(_DTO):
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    changes: dict[str, object]
    schema_version: str = Field(default="1.0.0", alias="schemaVersion")


class CredentialRequest(_DTO):
    credential_id: str = Field(alias="credentialId", min_length=1)
    value: str = Field(min_length=1)
    expected_revision: int = Field(alias="expectedRevision", ge=1)


class ModelSyncRequest(_DTO):
    remote_models: list[str] = Field(alias="remoteModels")
    expected_revision: int | None = Field(default=None, alias="expectedRevision", ge=1)
    source: str = "explicit_input"


class ModelSyncDecisionRequest(_DTO):
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    decision: str


class SkillRevisionRequest(_DTO):
    name: str
    version: str
    expected_revision: int = Field(alias="expectedRevision", ge=0)
    source_identity: str = Field(alias="sourceIdentity")
    digest: str = Field(min_length=64, max_length=64)
    source_type: str = Field(alias="sourceType")
    license_status: str = Field(alias="licenseStatus")
    capabilities: list[str] = Field(default_factory=list)


class ProbeRequest(_DTO):
    operation: str
    expected_revision: int | None = Field(default=None, alias="expectedRevision", ge=1)


class LifecycleRequest(_DTO):
    expected_revision: int = Field(alias="expectedRevision", ge=1)


def _camel(value: str) -> str:
    head, *tail = value.split("_")
    return head + "".join(item.capitalize() for item in tail)


def _response(value: object) -> object:
    encoded = jsonable_encoder(value)
    if isinstance(encoded, dict):
        return {_camel(str(key)): _response(item) for key, item in encoded.items()}
    if isinstance(encoded, list):
        return [_response(item) for item in encoded]
    return encoded


def _expected(body: int, header: str | None) -> int:
    if header is None:
        return body
    try:
        value = int(header.strip('"'))
    except ValueError as error:
        raise ValidationDomainError("If-Match must be an integer revision") from error
    if value != body:
        raise RevisionConflictError("If-Match", body, value)
    return body


def _service(request: Request) -> CatalogService:
    service = getattr(request.app.state, "catalog_service", None)
    if service is None:
        raise DatabaseUnavailableError("catalog service is not configured")
    return cast(CatalogService, service)


@router.get("/v1/catalog")
async def projection(service: Annotated[CatalogService, Depends(_service)]) -> dict[str, object]:
    return cast(dict[str, object], _response(await service.projection()))


@router.post("/v1/catalog/providers", status_code=201)
async def create_provider(
    body: ProviderRequest, service: Annotated[CatalogService, Depends(_service)]
) -> object:
    return _response(
        await service.create_provider(CreateProviderCommand(body.name, body.adapter_key))
    )


@router.post("/v1/catalog/profiles", status_code=201)
async def create_profile(
    body: ProfileRequest, service: Annotated[CatalogService, Depends(_service)]
) -> object:
    return _response(
        await service.create_profile(
            CreateProfileCommand(body.provider_id, body.name, body.adapter_identity)
        )
    )


@router.post("/v1/catalog/models", status_code=201)
async def create_model(
    body: ModelRequest, service: Annotated[CatalogService, Depends(_service)]
) -> object:
    return _response(
        await service.create_model(CreateModelCommand(body.profile_id, body.model_key))
    )


@router.post("/v1/catalog/quotas", status_code=201)
async def set_quota(
    body: QuotaRequest, service: Annotated[CatalogService, Depends(_service)]
) -> object:
    return await service.set_quota(
        SetQuotaCommand(
            body.profile_id,
            body.operation,
            body.status,
            body.remaining,
            body.reset_at,
            body.source,
        )
    )


@router.patch("/v1/catalog/providers/{provider_id}")
async def update_provider(
    provider_id: str,
    body: UpdateRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    if body.schema_version != "1.0.0":
        raise ValidationDomainError("unsupported schemaVersion")
    return _response(
        await service.update_provider(
            UpdateCatalogCommand(
                provider_id, _expected(body.expected_revision, if_match), body.changes
            )
        )
    )


@router.patch("/v1/catalog/profiles/{profile_id}")
async def update_profile(
    profile_id: str,
    body: UpdateRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return _response(
        await service.update_profile(
            UpdateCatalogCommand(
                profile_id, _expected(body.expected_revision, if_match), body.changes
            )
        )
    )


@router.patch("/v1/catalog/models/{model_id}")
async def update_model(
    model_id: str,
    body: UpdateRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return _response(
        await service.update_model(
            UpdateCatalogCommand(
                model_id, _expected(body.expected_revision, if_match), body.changes
            )
        )
    )


@router.post("/v1/catalog/providers/{provider_id}/enable")
async def enable_provider(
    provider_id: str,
    body: LifecycleRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return _response(
        await service.set_provider_enabled(
            provider_id, _expected(body.expected_revision, if_match), True
        )
    )


@router.post("/v1/catalog/providers/{provider_id}/disable")
async def disable_provider(
    provider_id: str,
    body: LifecycleRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return _response(
        await service.set_provider_enabled(
            provider_id, _expected(body.expected_revision, if_match), False
        )
    )


@router.post("/v1/catalog/profiles/{profile_id}/enable")
async def enable_profile(
    profile_id: str,
    body: LifecycleRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return _response(
        await service.set_profile_enabled(
            profile_id, _expected(body.expected_revision, if_match), True
        )
    )


@router.post("/v1/catalog/profiles/{profile_id}/disable")
async def disable_profile(
    profile_id: str,
    body: LifecycleRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return _response(
        await service.set_profile_enabled(
            profile_id, _expected(body.expected_revision, if_match), False
        )
    )


@router.post("/v1/catalog/models/{model_id}/enable")
async def enable_model(
    model_id: str,
    body: LifecycleRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return _response(
        await service.set_model_enabled(model_id, _expected(body.expected_revision, if_match), True)
    )


@router.post("/v1/catalog/models/{model_id}/disable")
async def disable_model(
    model_id: str,
    body: LifecycleRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return _response(
        await service.set_model_enabled(
            model_id, _expected(body.expected_revision, if_match), False
        )
    )


@router.put("/v1/catalog/profiles/{profile_id}/credential")
async def replace_credential(
    profile_id: str,
    body: CredentialRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> dict[str, str]:
    expected_revision = _expected(body.expected_revision, if_match)
    return await service.replace_credential(
        ReplaceCredentialCommand(profile_id, body.credential_id, body.value, expected_revision)
    )


@router.get("/v1/catalog/profiles/{profile_id}/credential")
async def credential_status(
    profile_id: str, service: Annotated[CatalogService, Depends(_service)]
) -> dict[str, str]:
    return await service.credential_status(profile_id)


@router.post("/v1/catalog/profiles/{profile_id}/model-syncs", status_code=201)
async def preview_model_sync(
    profile_id: str,
    body: ModelSyncRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    expected_revision = (
        None if body.expected_revision is None else _expected(body.expected_revision, if_match)
    )
    candidate = await service.preview_model_sync(
        ModelSyncCommand(
            profile_id,
            tuple(body.remote_models),
            expected_revision,
            body.source,
        )
    )
    payload = cast(dict[str, object], _response(candidate))
    payload["source"] = "explicit_input"
    payload["discovery"] = "not_performed"
    return payload


@router.post("/v1/catalog/model-syncs/{candidate_id}/decision")
async def decide_model_sync(
    candidate_id: str,
    body: ModelSyncDecisionRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return _response(
        await service.decide_model_sync(
            candidate_id, _expected(body.expected_revision, if_match), body.decision
        )
    )


@router.post("/v1/catalog/skill-revisions", status_code=201)
async def append_skill_revision(
    body: SkillRevisionRequest,
    service: Annotated[CatalogService, Depends(_service)],
) -> object:
    return _response(
        await service.append_skill_revision(
            AppendSkillRevisionCommand(
                body.name,
                body.version,
                body.expected_revision,
                body.source_identity,
                body.digest,
                body.source_type,
                body.license_status,
                tuple(body.capabilities),
            )
        )
    )


@router.post("/v1/catalog/skill-revisions/{skill_id}/enable")
@router.post("/v1/catalog/skills/{skill_id}/enable")
async def enable_skill(
    skill_id: str,
    body: LifecycleRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return _response(
        await service.set_skill_enabled(skill_id, _expected(body.expected_revision, if_match), True)
    )


@router.post("/v1/catalog/skill-revisions/{skill_id}/disable")
@router.post("/v1/catalog/skills/{skill_id}/disable")
async def disable_skill(
    skill_id: str,
    body: LifecycleRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    return _response(
        await service.set_skill_enabled(
            skill_id, _expected(body.expected_revision, if_match), False
        )
    )


@router.post("/v1/catalog/profiles/{profile_id}/probe")
async def probe_profile(
    profile_id: str,
    body: ProbeRequest,
    service: Annotated[CatalogService, Depends(_service)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    if body.expected_revision is None and if_match is None:
        raise ValidationDomainError("probe expectedRevision is required")
    expected_revision = (
        body.expected_revision
        if if_match is None
        else _expected(body.expected_revision or 0, if_match)
    )
    return _response(
        await service.snapshot(profile_id, body.operation, expected_revision=expected_revision)
    )


@router.get("/v1/projects/{project_id}/runs/{run_id}/provider-calls")
async def provider_calls(
    project_id: str,
    run_id: str,
    service: Annotated[CatalogService, Depends(_service)],
    node_run_id: Annotated[str | None, Query(alias="nodeRunId")] = None,
    logical_operation: Annotated[str | None, Query(alias="logicalOperation")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> list[dict[str, object]]:
    if project_scope != project_id:
        raise ProjectAccessForbiddenError(project_id)
    return await service.provider_call_summaries(
        project_id,
        run_id,
        node_run_id=node_run_id,
        logical_operation=logical_operation,
        project_scope=project_scope,
    )


@router.post("/v1/catalog/profiles/{profile_id}/admit")
async def admit_operation(
    profile_id: str,
    body: AdmissionRequest,
    service: Annotated[CatalogService, Depends(_service)],
) -> object:
    return await service.admit_operation(profile_id, body.operation, live=body.live)


@router.post("/v1/catalog/profiles/{profile_id}/release")
async def release_operation(
    profile_id: str,
    body: AdmissionRequest,
    service: Annotated[CatalogService, Depends(_service)],
) -> dict[str, str]:
    await service.release_operation(profile_id, body.operation)
    return {"status": "released"}
