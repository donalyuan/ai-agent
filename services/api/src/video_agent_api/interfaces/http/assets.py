"""Assets HTTP adapter; transport aliases are explicit and persistence-free."""

from __future__ import annotations

from typing import Annotated, cast

from fastapi import APIRouter, Depends, Path, Request, status
from pydantic import BaseModel, ConfigDict, Field, field_validator

from video_agent_api.application.assets import (
    AppendAssetVersionCommand,
    AssetsService,
    CreateAssetCommand,
)
from video_agent_api.domain.assets import ASSET_KINDS, Asset, AssetVersion, StorageObject
from video_agent_api.domain.errors import (
    AssetNotFoundError,
    AssetVersionConflictError,
    AssetVersionNotFoundError,
    DatabaseUnavailableError,
    DomainError,
    ProjectNotFoundError,
    ValidationDomainError,
)

router = APIRouter(tags=["assets"])


class _DTO(BaseModel):
    model_config = ConfigDict(
        alias_generator=lambda value: "".join(
            [value.split("_")[0], *[part.capitalize() for part in value.split("_")[1:]]]
        ),
        populate_by_name=True,
        extra="forbid",
    )


class AssetCreateRequest(_DTO):
    kind: str
    name: str = Field(min_length=1)

    @field_validator("kind")
    @classmethod
    def shared_kind(cls, value: str) -> str:
        if value not in ASSET_KINDS:
            raise ValueError("kind must be one of image, video, audio, text, document")
        return value

    @field_validator("name")
    @classmethod
    def non_blank(cls, value: str) -> str:
        if not value.strip():
            raise ValueError("name must not be blank")
        return value


class MediaRequest(_DTO):
    duration_ms: int | None = Field(default=None, ge=0)
    width: int | None = Field(default=None, ge=1)
    height: int | None = Field(default=None, ge=1)


class StorageObjectRequest(_DTO):
    storage_provider: str = Field(min_length=1)
    bucket: str = Field(min_length=1)
    region: str | None = Field(default=None, min_length=1)
    object_key: str = Field(min_length=1)
    e_tag: str | None = Field(default=None, min_length=1)
    checksum: str = Field(min_length=64, max_length=64)
    mime_type: str
    size_bytes: int = Field(ge=0)
    media: MediaRequest | None = None


class AssetVersionCreateRequest(_DTO):
    storage_object: StorageObjectRequest
    content_hash: str = Field(min_length=64, max_length=64)


class AssetResponse(_DTO):
    id: str
    schema_version: str = Field(alias="schema_version")
    revision: int
    status: str
    project_id: str
    kind: str
    name: str


class StorageObjectResponse(StorageObjectRequest):
    pass


class AssetVersionResponse(_DTO):
    id: str
    schema_version: str = Field(alias="schema_version")
    revision: int
    status: str
    project_id: str
    asset_id: str
    version_number: int
    content_hash: str
    storage_object: StorageObjectResponse


def _asset_response(value: Asset) -> AssetResponse:
    return AssetResponse.model_validate(
        {
            "id": value.id,
            "schema_version": value.schema_version,
            "revision": value.revision,
            "status": value.status,
            "project_id": value.project_id,
            "kind": value.kind,
            "name": value.name,
        }
    )


def _storage_response(value: StorageObject) -> StorageObjectResponse:
    return StorageObjectResponse.model_validate(
        {
            "storage_provider": value.storage_provider,
            "bucket": value.bucket,
            "region": value.region,
            "object_key": value.object_key,
            "e_tag": value.e_tag,
            "checksum": value.checksum,
            "mime_type": value.mime_type,
            "size_bytes": value.size_bytes,
            "media": dict(value.media) if value.media else None,
        }
    )


def _version_response(value: AssetVersion) -> AssetVersionResponse:
    return AssetVersionResponse.model_validate(
        {
            "id": value.id,
            "schema_version": value.schema_version,
            "revision": value.revision,
            "status": value.status,
            "project_id": value.project_id,
            "asset_id": value.asset_id,
            "version_number": value.version_number,
            "content_hash": value.content_hash,
            "storage_object": _storage_response(value.storage_object),
        }
    )


def service_dependency(request: Request) -> AssetsService:
    service = getattr(request.app.state, "assets_service", None)
    if service is None:
        raise DatabaseUnavailableError("business database is not configured")
    return cast(AssetsService, service)


Service = Annotated[AssetsService, Depends(service_dependency)]


def _error(error: DomainError) -> Exception:
    if isinstance(error, (AssetNotFoundError, AssetVersionNotFoundError, ProjectNotFoundError)):
        http_status = status.HTTP_404_NOT_FOUND
    elif isinstance(error, AssetVersionConflictError):
        http_status = status.HTTP_409_CONFLICT
    elif isinstance(error, DatabaseUnavailableError):
        http_status = status.HTTP_503_SERVICE_UNAVAILABLE
    elif isinstance(error, ValidationDomainError):
        http_status = status.HTTP_422_UNPROCESSABLE_ENTITY
    else:
        http_status = status.HTTP_422_UNPROCESSABLE_ENTITY
    from fastapi import HTTPException

    return HTTPException(http_status, detail={"type": error.code, "message": str(error)})


def _storage(value: StorageObjectRequest) -> StorageObject:
    return StorageObject(
        storage_provider=value.storage_provider,
        bucket=value.bucket,
        region=value.region,
        object_key=value.object_key,
        e_tag=value.e_tag,
        checksum=value.checksum,
        mime_type=value.mime_type,
        size_bytes=value.size_bytes,
        media=value.media.model_dump(by_alias=False, exclude_none=True) if value.media else None,
    )


@router.post("/v1/projects/{projectId}/assets", response_model=AssetResponse, status_code=201)
async def create_asset(
    project_id: Annotated[str, Path(alias="projectId")],
    payload: AssetCreateRequest,
    service: Service,
) -> AssetResponse:
    try:
        return _asset_response(
            await service.create_asset(CreateAssetCommand(project_id, payload.name, payload.kind))
        )
    except DomainError as error:
        raise _error(error) from error


@router.get("/v1/projects/{projectId}/assets", response_model=list[AssetResponse])
async def list_assets(
    project_id: Annotated[str, Path(alias="projectId")], service: Service
) -> list[AssetResponse]:
    try:
        return [_asset_response(value) for value in await service.list_assets(project_id)]
    except DomainError as error:
        raise _error(error) from error


@router.get("/v1/assets/{assetId}", response_model=AssetResponse)
async def get_asset(
    asset_id: Annotated[str, Path(alias="assetId")], service: Service
) -> AssetResponse:
    try:
        return _asset_response(await service.get_asset(asset_id))
    except DomainError as error:
        raise _error(error) from error


@router.post(
    "/v1/assets/{assetId}/versions",
    response_model=AssetVersionResponse,
    response_model_exclude_none=True,
    status_code=201,
)
async def append_version(
    asset_id: Annotated[str, Path(alias="assetId")],
    payload: AssetVersionCreateRequest,
    service: Service,
) -> AssetVersionResponse:
    try:
        return _version_response(
            await service.append_version(
                AppendAssetVersionCommand(
                    asset_id, _storage(payload.storage_object), payload.content_hash
                )
            )
        )
    except DomainError as error:
        raise _error(error) from error


@router.get(
    "/v1/assets/{assetId}/versions",
    response_model=list[AssetVersionResponse],
    response_model_exclude_none=True,
)
async def list_versions(
    asset_id: Annotated[str, Path(alias="assetId")], service: Service
) -> list[AssetVersionResponse]:
    try:
        return [_version_response(value) for value in await service.list_versions(asset_id)]
    except DomainError as error:
        raise _error(error) from error


@router.get(
    "/v1/asset-versions/{versionId}",
    response_model=AssetVersionResponse,
    response_model_exclude_none=True,
)
async def get_version(
    version_id: Annotated[str, Path(alias="versionId")], service: Service
) -> AssetVersionResponse:
    try:
        return _version_response(await service.get_version(version_id))
    except DomainError as error:
        raise _error(error) from error
