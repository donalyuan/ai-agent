"""Project Asset Center HTTP contracts with safe owner projections."""

from __future__ import annotations

from typing import Annotated, Any, cast

from fastapi import APIRouter, Depends, Header, Path, Query, Request, status
from pydantic import BaseModel, ConfigDict, Field, field_validator
from starlette.responses import StreamingResponse

from video_agent_api.application.assets import (
    AssetsService,
    CancelReservationCommand,
    CompleteReservationCommand,
    CreateAssetCommand,
    CreateReservationCommand,
    UpdateAssetMetadataCommand,
)
from video_agent_api.application.media import MediaDispatchAdmission, MediaOwnerService
from video_agent_api.application.storage_handoffs import asset_upload_intent
from video_agent_api.application.storage_profiles import StorageProfileService
from video_agent_api.domain.assets import ASSET_KINDS, Asset, AssetVersion, StorageObject
from video_agent_api.domain.errors import (
    AssetNotFoundError,
    AssetVersionConflictError,
    AssetVersionNotFoundError,
    DatabaseUnavailableError,
    DomainError,
    ProjectAccessForbiddenError,
    ProjectNotFoundError,
    StorageProfileNotFoundError,
    StorageProfileRevisionConflictError,
    ValidationDomainError,
)
from video_agent_api.interfaces.http.project_scope import project_scope
from video_agent_api.ports.contracts import (
    AdapterNotConfiguredError,
    PartReceipt,
    PortError,
    StorageValidationError,
    StorageWriteIntent,
    StoredObjectRef,
)

router = APIRouter(tags=["assets"])


def _camel(value: str) -> str:
    head, *tail = value.split("_")
    return "".join([head, *[part.capitalize() for part in tail]])


class _DTO(BaseModel):
    model_config = ConfigDict(alias_generator=_camel, populate_by_name=False, extra="forbid")


class AssetCreateRequest(_DTO):
    kind: str
    name: str = Field(min_length=1)
    source_type: str = "imported"
    catalog_role: str | None = None
    tags: list[str] = Field(default_factory=list, max_length=32)
    authorization_status: str = "unknown"
    copyright_owner: str | None = None
    license_label: str | None = None
    license_reference: str | None = None
    schema_version: str

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


class AssetMetadataRequest(_DTO):
    tags: list[str] | None = Field(default=None, max_length=32)
    source_type: str | None = None
    catalog_role: str | None = None
    authorization_status: str | None = None
    copyright_owner: str | None = None
    license_label: str | None = None
    license_reference: str | None = None
    expected_revision: int = Field(ge=1)
    schema_version: str


class ReservationCreateRequest(_DTO):
    fingerprint: str = Field(min_length=64, max_length=64)
    expected_asset_revision: int = Field(ge=1)
    declared_kind: str
    declared_mime_type: str
    declared_size_bytes: int = Field(ge=0)
    declared_checksum: str = Field(min_length=64, max_length=64)
    storage_profile_id: str = Field(min_length=1)
    storage_profile_revision: int = Field(ge=1)
    storage_profile_snapshot_hash: str = Field(min_length=64, max_length=64)
    part_size_bytes: int = Field(ge=1, le=64 * 1024 * 1024)
    schema_version: str


class UploadAdmissionRequest(_DTO):
    storage_profile_id: str = Field(min_length=1)
    storage_profile_revision: int = Field(ge=1)
    declared_mime_type: str = Field(min_length=1)
    declared_size_bytes: int = Field(ge=0)
    part_size_bytes: int = Field(ge=1, le=64 * 1024 * 1024)
    schema_version: str


class UploadProfileResponse(_DTO):
    storage_profile_id: str
    revision: int
    name: str
    adapter_key: str
    enabled: bool


class UploadAdmissionResponse(_DTO):
    schema_version: str = "1.0.0"
    storage_profile_id: str
    storage_profile_revision: int
    storage_profile_snapshot_hash: str
    min_part_size_bytes: int
    max_part_size_bytes: int
    max_part_count: int
    max_object_size_bytes: int
    warning: str | None = None


class ReservationMutationRequest(_DTO):
    expected_revision: int = Field(ge=1)
    session_id: str | None = None
    correlation_id: str = "asset-center"
    schema_version: str


class ReservationResumeRequest(_DTO):
    correlation_id: str = Field(min_length=1, max_length=255)
    schema_version: str


class MultipartSessionResponse(_DTO):
    schema_version: str = "1.0.0"
    reservation_id: str
    session_id: str
    operation_key: str
    status: str
    expected_size_bytes: int
    expected_checksum: str
    expected_mime_type: str


class MultipartPartResponse(_DTO):
    schema_version: str = "1.0.0"
    reservation_id: str
    session_id: str
    part_number: int
    checksum: str
    e_tag: str
    size_bytes: int


class MultipartPartManifest(_DTO):
    part_number: int = Field(ge=1)
    checksum: str = Field(min_length=64, max_length=64)
    e_tag: str = Field(min_length=1, max_length=255)
    size_bytes: int = Field(ge=0)


class MultipartCompleteRequest(_DTO):
    session_id: str = Field(min_length=1)
    parts: list[MultipartPartManifest] = Field(min_length=1, max_length=10000)
    correlation_id: str = Field(min_length=1, max_length=255)
    schema_version: str


class TimelineSelectionRequest(_DTO):
    episode_id: str = Field(min_length=1)
    schema_version: str


class MediaGrantRequest(_DTO):
    ttl_seconds: int = Field(default=120, ge=1, le=300)
    schema_version: str


class AssetResponse(_DTO):
    id: str
    schema_version: str
    revision: int
    status: str
    project_id: str
    kind: str
    name: str
    source_type: str
    catalog_role: str | None
    tags: tuple[str, ...]
    authorization_status: str
    copyright_owner: str | None
    license_label: str | None
    license_reference: str | None
    updated_at: str


class AssetVersionResponse(_DTO):
    id: str
    schema_version: str
    revision: int
    status: str
    project_id: str
    asset_id: str
    version_number: int
    content_hash: str
    checksum: str
    mime_type: str
    size_bytes: int
    duration_ms: int | None = None
    width: int | None = None
    height: int | None = None


class AssetCatalogItemResponse(AssetResponse):
    version_count: int
    processing_status: str
    latest_version: AssetVersionResponse | None


class AssetCatalogResponse(_DTO):
    schema_version: str = "1.0.0"
    items: list[AssetCatalogItemResponse]
    next_cursor: str | None


class ReservationResponse(_DTO):
    id: str
    schema_version: str
    revision: int
    project_id: str
    asset_id: str
    operation_key: str
    fingerprint: str
    status: str
    registered_version_id: str | None
    expected_asset_revision: int
    declared_kind: str
    declared_mime_type: str
    declared_size_bytes: int
    declared_checksum: str
    storage_profile_id: str
    storage_profile_revision: int
    storage_profile_snapshot_hash: str
    diagnostic: str | None


def _asset_response(value: Asset) -> AssetResponse:
    return AssetResponse.model_validate(
        {
            "id": value.id,
            "schemaVersion": value.schema_version,
            "revision": value.revision,
            "status": value.status,
            "projectId": value.project_id,
            "kind": value.kind,
            "name": value.name,
            "sourceType": value.source_type,
            "catalogRole": value.catalog_role,
            "tags": value.tags,
            "authorizationStatus": value.authorization_status,
            "copyrightOwner": value.copyright_owner,
            "licenseLabel": value.license_label,
            "licenseReference": value.license_reference,
            "updatedAt": value.updated_at,
        }
    )


def _version_response(value: AssetVersion) -> AssetVersionResponse:
    media = dict(value.storage_object.media or {})
    return AssetVersionResponse.model_validate(
        {
            "id": value.id,
            "schemaVersion": value.schema_version,
            "revision": value.revision,
            "status": value.status,
            "projectId": value.project_id,
            "assetId": value.asset_id,
            "versionNumber": value.version_number,
            "contentHash": cast(str, value.content_hash),
            "checksum": value.storage_object.checksum,
            "mimeType": value.storage_object.mime_type,
            "sizeBytes": value.storage_object.size_bytes,
            "durationMs": media.get("duration_ms"),
            "width": media.get("width"),
            "height": media.get("height"),
        }
    )


def _reservation_response(value: Any) -> ReservationResponse:
    return ReservationResponse.model_validate(
        {
            "id": value.id,
            "schemaVersion": value.schema_version,
            "revision": value.revision,
            "projectId": value.project_id,
            "assetId": value.asset_id,
            "operationKey": value.operation_key,
            "fingerprint": value.fingerprint,
            "status": value.status,
            "registeredVersionId": value.registered_version_id,
            "expectedAssetRevision": value.expected_asset_revision,
            "declaredKind": value.declared_kind,
            "declaredMimeType": value.declared_mime_type,
            "declaredSizeBytes": value.declared_size_bytes,
            "declaredChecksum": value.declared_checksum,
            "storageProfileId": value.storage_profile_id,
            "storageProfileRevision": value.storage_profile_revision,
            "storageProfileSnapshotHash": value.storage_profile_snapshot_hash,
            "diagnostic": value.diagnostic,
        }
    )


def service_dependency(request: Request) -> AssetsService:
    service = getattr(request.app.state, "assets_service", None)
    if service is None:
        raise DatabaseUnavailableError("business database is not configured")
    return cast(AssetsService, service)


Service = Annotated[AssetsService, Depends(service_dependency)]


def _error(error: DomainError) -> Exception:
    if isinstance(
        error,
        (
            AssetNotFoundError,
            AssetVersionNotFoundError,
            ProjectNotFoundError,
            StorageProfileNotFoundError,
        ),
    ):
        http_status = status.HTTP_404_NOT_FOUND
    elif isinstance(error, ProjectAccessForbiddenError):
        http_status = status.HTTP_403_FORBIDDEN
    elif isinstance(error, (AssetVersionConflictError, StorageProfileRevisionConflictError)):
        http_status = status.HTTP_409_CONFLICT
    elif isinstance(error, DatabaseUnavailableError):
        http_status = status.HTTP_503_SERVICE_UNAVAILABLE
    else:
        http_status = status.HTTP_422_UNPROCESSABLE_ENTITY
    from fastapi import HTTPException

    return HTTPException(http_status, detail={"type": error.code, "message": str(error)})


def _expected_revision(if_match: str, body_revision: int) -> int:
    from fastapi import HTTPException

    normalized = if_match.strip().strip('"')
    try:
        header_revision = int(normalized)
    except ValueError as error:
        raise HTTPException(
            status.HTTP_422_UNPROCESSABLE_ENTITY,
            detail={"type": "validation_error", "message": "If-Match must be a revision"},
        ) from error
    if header_revision != body_revision:
        raise HTTPException(
            status.HTTP_422_UNPROCESSABLE_ENTITY,
            detail={
                "type": "schema_alias_conflict",
                "message": "If-Match and expectedRevision conflict",
            },
        )
    return body_revision


def _storage(request: Request) -> Any:
    # A request-scoped resolved port is authoritative. The app-level local adapter
    # remains only for legacy test fixtures that do not have a profile owner.
    storage = getattr(request.state, "resolved_storage", None)
    if storage is not None:
        return getattr(storage, "port", storage)
    storage = getattr(request.app.state, "storage_port", None)
    if storage is None:
        raise DatabaseUnavailableError("storage owner is not configured")
    return storage


def _storage_profiles(request: Request) -> StorageProfileService:
    service = getattr(request.app.state, "storage_profile_service", None)
    if service is None:
        raise DatabaseUnavailableError("storage profile owner is not configured")
    return cast(StorageProfileService, service)


async def _resolve_upload_profile(
    request: Request,
    project_id: str,
    profile_id: str,
    profile_revision: int,
) -> tuple[Any, Any, str]:
    profile = await _storage_profiles(request).resolve_upload_profile(
        project_id, profile_id, profile_revision, project_scope(request)
    )
    composition = getattr(request.app.state, "runtime_composition", None)
    if composition is not None:
        composed = await composition.resolve_storage(
            _storage_profiles(request),
            project_id=project_id,
            profile_id=profile.id,
            expected_profile_revision=profile.revision,
            expected_bucket_binding_id=profile.bucket_binding_id,
            expected_identity={
                "adapterKey": profile.adapter_key,
                "profileId": profile.id,
                "projectId": profile.project_id,
                "revision": profile.revision,
                "bucketBindingId": profile.bucket_binding_id,
                "bucket": profile.bucket,
                "endpoint": profile.endpoint,
                "region": profile.region,
                "credentialRef": profile.credential_ref,
            },
            local_workspace_root=getattr(
                getattr(request.app.state, "runtime_settings", None),
                "workspace_root",
                None,
            ),
        )
        request.state.resolved_storage = composed
        capability_payload = composed.identity.capability
        from video_agent_api.ports.contracts import StorageCapability

        capability = StorageCapability(
            profile_revision=int(capability_payload["profileRevision"]),
            min_part_size_bytes=int(capability_payload["minPartSizeBytes"]),
            max_part_size_bytes=int(capability_payload["maxPartSizeBytes"]),
            max_part_count=int(capability_payload["maxPartCount"]),
            max_object_size_bytes=int(capability_payload["maxObjectSizeBytes"]),
        )
    else:
        storage_owner = _storage(request)
        capability_reader = getattr(storage_owner, "capability", None)
        if capability_reader is None:
            raise AdapterNotConfiguredError("storage capability is unconfigured")
        capability = capability_reader(profile.revision)
    snapshot_hash = StorageProfileService.snapshot_hash(profile, capability)
    return profile, capability, snapshot_hash


async def _resolve_reservation_storage(request: Request, reservation: Any) -> Any:
    """Resolve the reservation's complete profile identity before storage I/O."""
    profile, _capability, _snapshot_hash = await _resolve_upload_profile(
        request,
        reservation.project_id,
        reservation.storage_profile_id,
        reservation.storage_profile_revision,
    )
    if profile.bucket_binding_id == "":
        raise ValidationDomainError("storage bucket binding is required")
    return _storage(request)


async def _admit_upload(
    request: Request,
    project_id: str,
    profile_id: str,
    profile_revision: int,
    size_bytes: int,
    part_size_bytes: int,
) -> tuple[Any, Any, str]:
    profile, capability, snapshot_hash = await _resolve_upload_profile(
        request, project_id, profile_id, profile_revision
    )
    storage_owner = _storage(request)
    admission = getattr(storage_owner, "admit_upload", None)
    if admission is None:
        raise AdapterNotConfiguredError("storage capacity admission is unconfigured")
    admission(size_bytes, part_size_bytes, profile_revision=profile.revision)
    return profile, capability, snapshot_hash


def _storage_http_error(error: Exception) -> Exception:
    from fastapi import HTTPException

    code = getattr(error, "code", "storage_unconfigured")
    status_code = (
        status.HTTP_503_SERVICE_UNAVAILABLE
        if isinstance(error, AdapterNotConfiguredError)
        else status.HTTP_422_UNPROCESSABLE_ENTITY
    )
    return HTTPException(
        status_code,
        detail={"type": code, "message": str(error) or type(error).__name__},
    )


def _reservation_intent(value: Any) -> Any:
    return asset_upload_intent(
        value,
        value.storage_profile_id,
        value.upload_key,
        expected_size_bytes=value.declared_size_bytes,
        expected_checksum=value.declared_checksum,
        expected_mime_type=value.declared_mime_type,
    )


def _reconciliation_intent(value: Any) -> StorageWriteIntent:
    return StorageWriteIntent(
        value.operation_key,
        value.project_id,
        value.storage_profile_id,
        value.upload_key,
        value.declared_size_bytes,
        value.declared_checksum,
        value.declared_mime_type,
    )


def _safe_session(value: Any, reservation_id: str) -> MultipartSessionResponse:
    return MultipartSessionResponse.model_validate(
        {
            "reservationId": reservation_id,
            "sessionId": value.session_id,
            "operationKey": value.operation_key,
            "status": value.status,
            "expectedSizeBytes": value.expected_size_bytes,
            "expectedChecksum": value.expected_checksum,
            "expectedMimeType": value.expected_mime_type,
        }
    )


@router.post("/v1/projects/{projectId}/assets", response_model=AssetResponse, status_code=201)
async def create_asset(
    project_id: Annotated[str, Path(alias="projectId")],
    payload: AssetCreateRequest,
    service: Service,
) -> AssetResponse:
    try:
        return _asset_response(
            await service.create_asset(
                CreateAssetCommand(
                    project_id,
                    payload.name,
                    payload.kind,
                    payload.source_type,
                    payload.catalog_role,
                    tuple(payload.tags),
                    payload.authorization_status,
                    payload.copyright_owner,
                    payload.license_label,
                    payload.license_reference,
                    payload.schema_version,
                )
            )
        )
    except DomainError as error:
        raise _error(error) from error


@router.get(
    "/v1/projects/{projectId}/asset-upload-profiles",
    response_model=list[UploadProfileResponse],
)
async def list_upload_profiles(
    project_id: Annotated[str, Path(alias="projectId")], request: Request
) -> list[UploadProfileResponse]:
    try:
        profiles = await _storage_profiles(request).list_upload_profiles(
            project_id, project_scope(request)
        )
        return [
            UploadProfileResponse.model_validate(
                {
                    "storageProfileId": profile.id,
                    "revision": profile.revision,
                    "name": profile.name,
                    "adapterKey": profile.adapter_key,
                    "enabled": profile.enabled,
                }
            )
            for profile in profiles
        ]
    except DomainError as error:
        raise _error(error) from error


@router.post(
    "/v1/projects/{projectId}/asset-upload-admissions",
    response_model=UploadAdmissionResponse,
)
async def admit_asset_upload(
    project_id: Annotated[str, Path(alias="projectId")],
    payload: UploadAdmissionRequest,
    request: Request,
) -> UploadAdmissionResponse:
    try:
        AssetsService._require_schema_version(payload.schema_version)
        profile, capability, snapshot_hash = await _admit_upload(
            request,
            project_id,
            payload.storage_profile_id,
            payload.storage_profile_revision,
            payload.declared_size_bytes,
            payload.part_size_bytes,
        )
        return UploadAdmissionResponse.model_validate(
            {
                "storageProfileId": profile.id,
                "storageProfileRevision": profile.revision,
                "storageProfileSnapshotHash": snapshot_hash,
                "minPartSizeBytes": capability.min_part_size_bytes,
                "maxPartSizeBytes": capability.max_part_size_bytes,
                "maxPartCount": capability.max_part_count,
                "maxObjectSizeBytes": capability.max_object_size_bytes,
            }
        )
    except PortError as error:
        raise _storage_http_error(error) from error
    except DomainError as error:
        raise _error(error) from error


@router.get("/v1/projects/{projectId}/assets", response_model=AssetCatalogResponse)
async def list_assets(
    project_id: Annotated[str, Path(alias="projectId")],
    service: Service,
    cursor: str | None = Query(default=None),
    limit: int = Query(default=50, ge=1, le=100),
    kind: str | None = Query(default=None),
    catalog_role: str | None = Query(default=None, alias="catalogRole"),
    tag: str | None = Query(default=None),
    source_type: str | None = Query(default=None, alias="sourceType"),
    authorization_status: str | None = Query(default=None, alias="authorizationStatus"),
    processing_status: str | None = Query(default=None, alias="processingStatus"),
) -> AssetCatalogResponse:
    filters = {
        key: value
        for key, value in {
            "kind": kind,
            "catalogRole": catalog_role,
            "tag": tag,
            "sourceType": source_type,
            "authorizationStatus": authorization_status,
            "processingStatus": processing_status,
        }.items()
        if value is not None
    }
    try:
        page = await service.catalog(project_id, cursor=cursor, limit=limit, filters=filters)
        items: list[AssetCatalogItemResponse] = []
        for entry in page.items:
            item = _asset_response(entry.asset).model_dump(by_alias=True)
            items.append(
                AssetCatalogItemResponse.model_validate(
                    {
                        **item,
                        "versionCount": entry.version_count,
                        "processingStatus": entry.processing_status,
                        "latestVersion": (
                            _version_response(entry.latest_version).model_dump(by_alias=True)
                            if entry.latest_version
                            else None
                        ),
                    }
                )
            )
        return AssetCatalogResponse.model_validate({"items": items, "nextCursor": page.next_cursor})
    except DomainError as error:
        raise _error(error) from error


@router.get("/v1/assets/{assetId}", response_model=AssetResponse)
async def get_asset(
    asset_id: Annotated[str, Path(alias="assetId")], service: Service, request: Request
) -> AssetResponse:
    try:
        return _asset_response(
            await service.get_asset(asset_id, project_scope=project_scope(request))
        )
    except DomainError as error:
        raise _error(error) from error


@router.patch("/v1/assets/{assetId}", response_model=AssetResponse)
async def update_asset(
    asset_id: Annotated[str, Path(alias="assetId")],
    payload: AssetMetadataRequest,
    service: Service,
    if_match: Annotated[str, Header(alias="If-Match")],
    request: Request,
) -> AssetResponse:
    expected = _expected_revision(if_match, payload.expected_revision)
    fields = frozenset(
        name
        for name in payload.model_fields_set
        if name not in {"expected_revision", "schema_version"}
    )
    try:
        return _asset_response(
            await service.update_metadata(
                UpdateAssetMetadataCommand(
                    asset_id=asset_id,
                    expected_revision=expected,
                    tags=tuple(payload.tags) if payload.tags is not None else None,
                    catalog_role=payload.catalog_role,
                    authorization_status=payload.authorization_status,
                    source_type=payload.source_type,
                    copyright_owner=payload.copyright_owner,
                    license_label=payload.license_label,
                    license_reference=payload.license_reference,
                    fields=fields,
                    schema_version=payload.schema_version,
                ),
                project_scope=project_scope(request),
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
    asset_id: Annotated[str, Path(alias="assetId")], service: Service, request: Request
) -> list[AssetVersionResponse]:
    try:
        return [
            _version_response(value)
            for value in await service.list_versions(asset_id, project_scope=project_scope(request))
        ]
    except DomainError as error:
        raise _error(error) from error


@router.get(
    "/v1/asset-versions/{versionId}",
    response_model=AssetVersionResponse,
    response_model_exclude_none=True,
)
async def get_version(
    version_id: Annotated[str, Path(alias="versionId")], service: Service, request: Request
) -> AssetVersionResponse:
    try:
        return _version_response(
            await service.get_version(version_id, project_scope=project_scope(request))
        )
    except DomainError as error:
        raise _error(error) from error


@router.post(
    "/v1/projects/{projectId}/assets/{assetId}/reservations",
    response_model=ReservationResponse,
    status_code=201,
)
async def create_reservation(
    project_id: Annotated[str, Path(alias="projectId")],
    asset_id: Annotated[str, Path(alias="assetId")],
    payload: ReservationCreateRequest,
    service: Service,
    request: Request,
) -> ReservationResponse:
    try:
        _profile, _capability, snapshot_hash = await _admit_upload(
            request,
            project_id,
            payload.storage_profile_id,
            payload.storage_profile_revision,
            payload.declared_size_bytes,
            payload.part_size_bytes,
        )
        if snapshot_hash != payload.storage_profile_snapshot_hash:
            raise StorageValidationError("storage profile snapshot is stale or forged")
        reservation = await service.create_reservation(
            CreateReservationCommand(
                project_id,
                asset_id,
                payload.fingerprint,
                payload.expected_asset_revision,
                payload.declared_kind,
                payload.declared_mime_type,
                payload.declared_size_bytes,
                payload.declared_checksum,
                payload.storage_profile_id,
                payload.storage_profile_revision,
                payload.storage_profile_snapshot_hash,
                schema_version=payload.schema_version,
            )
        )
        return _reservation_response(reservation)
    except PortError as error:
        raise _storage_http_error(error) from error
    except DomainError as error:
        raise _error(error) from error


@router.get(
    "/v1/projects/{projectId}/asset-reservations/{reservationId}",
    response_model=ReservationResponse,
)
async def get_reservation(
    project_id: Annotated[str, Path(alias="projectId")],
    reservation_id: Annotated[str, Path(alias="reservationId")],
    service: Service,
) -> ReservationResponse:
    try:
        return _reservation_response(await service.get_reservation(project_id, reservation_id))
    except DomainError as error:
        raise _error(error) from error


@router.post(
    "/v1/projects/{projectId}/asset-reservations/{reservationId}/uploads/resume",
    response_model=MultipartSessionResponse,
)
async def resume_upload(
    project_id: Annotated[str, Path(alias="projectId")],
    reservation_id: Annotated[str, Path(alias="reservationId")],
    payload: ReservationResumeRequest,
    service: Service,
    request: Request,
) -> MultipartSessionResponse:
    try:
        AssetsService._require_schema_version(payload.schema_version)
        reservation = await service.get_reservation(project_id, reservation_id)
        service.revalidate_reservation_admission(reservation)
        storage_owner = await _resolve_reservation_storage(request, reservation)
        session = storage_owner.resume_multipart(
            _reservation_intent(reservation), payload.correlation_id
        )
        return _safe_session(session, reservation.id)
    except PortError as error:
        raise _storage_http_error(error) from error
    except DomainError as error:
        raise _error(error) from error


@router.put(
    "/v1/projects/{projectId}/asset-reservations/{reservationId}/uploads/"
    "{sessionId}/parts/{partNumber}",
    response_model=MultipartPartResponse,
)
async def upload_part(
    project_id: Annotated[str, Path(alias="projectId")],
    reservation_id: Annotated[str, Path(alias="reservationId")],
    session_id: Annotated[str, Path(alias="sessionId")],
    part_number: Annotated[int, Path(alias="partNumber", ge=1)],
    request: Request,
    service: Service,
    part_checksum: Annotated[str, Header(alias="X-Part-Checksum")],
    part_etag: Annotated[str, Header(alias="X-Part-ETag")],
    correlation_id: Annotated[str, Header(alias="X-Correlation-ID")],
) -> MultipartPartResponse:
    try:
        reservation = await service.get_reservation(project_id, reservation_id)
        service.revalidate_reservation_admission(reservation)
        storage_owner = await _resolve_reservation_storage(request, reservation)
        session = storage_owner.resume_multipart(_reservation_intent(reservation), correlation_id)
        if session.session_id != session_id:
            raise StorageValidationError("multipart session does not match reservation")
        content = await request.body()
        capability = storage_owner.capability(reservation.storage_profile_revision)
        if len(content) > capability.max_part_size_bytes:
            raise StorageValidationError("multipart part exceeds profile limit")
        receipt = storage_owner.upload_part(
            session,
            PartReceipt(part_number, part_checksum, part_etag, len(content)),
            content,
            correlation_id,
        )
        return MultipartPartResponse.model_validate(
            {
                "reservationId": reservation.id,
                "sessionId": session.session_id,
                "partNumber": receipt.part_number,
                "checksum": receipt.checksum,
                "eTag": receipt.etag,
                "sizeBytes": receipt.size_bytes,
            }
        )
    except PortError as error:
        raise _storage_http_error(error) from error
    except DomainError as error:
        raise _error(error) from error


@router.post(
    "/v1/projects/{projectId}/asset-reservations/{reservationId}/uploads/complete",
    response_model=AssetVersionResponse,
    response_model_exclude_none=True,
    status_code=201,
)
async def complete_upload(
    project_id: Annotated[str, Path(alias="projectId")],
    reservation_id: Annotated[str, Path(alias="reservationId")],
    payload: MultipartCompleteRequest,
    service: Service,
    request: Request,
) -> AssetVersionResponse:
    try:
        AssetsService._require_schema_version(payload.schema_version)
        reservation = await service.get_reservation(project_id, reservation_id)
        if reservation.status == "registered" and reservation.registered_version_id:
            version = await service.get_version(reservation.registered_version_id)
            await _enqueue_verified_media(request, service, version)
            return _version_response(version)
        service.revalidate_reservation_admission(reservation)
        storage_owner = await _resolve_reservation_storage(request, reservation)
        session = storage_owner.resume_multipart(
            _reservation_intent(reservation), payload.correlation_id
        )
        if session.session_id != payload.session_id:
            raise StorageValidationError("multipart session does not match reservation")
        part_numbers = [item.part_number for item in payload.parts]
        if len(part_numbers) != len(set(part_numbers)):
            raise StorageValidationError("multipart manifest contains duplicate parts")
        manifest = tuple(
            PartReceipt(item.part_number, item.checksum, item.e_tag, item.size_bytes)
            for item in payload.parts
        )
        stored = storage_owner.complete_multipart(session, manifest, payload.correlation_id)
        if (
            stored.operation_key != reservation.operation_key
            or stored.project_id != project_id
            or not stored.verified
        ):
            raise StorageValidationError("verified StoredObjectRef scope is invalid")
        provider = "local_workspace" if stored.bucket == "workspace" else "tos"
        version = await service.complete_reservation(
            CompleteReservationCommand(
                reservation.id,
                StorageObject(
                    provider,
                    stored.bucket,
                    stored.object_key,
                    stored.mime_type,
                    stored.size_bytes,
                    stored.checksum,
                    e_tag=stored.etag,
                ),
                stored.checksum,
            )
        )
        await _enqueue_verified_media(request, service, version)
        return _version_response(version)
    except PortError as error:
        raise _storage_http_error(error) from error
    except DomainError as error:
        raise _error(error) from error


async def _enqueue_verified_media(
    request: Request, service: AssetsService, version: AssetVersion
) -> None:
    """Let the Media owner produce exactly one pipeline intent for eligible uploads."""
    asset = await service.get_asset(version.asset_id, project_scope=version.project_id)
    if (
        asset.authorization_status != "verified"
        or asset.source_type not in {"user_upload", "source_material", "imported"}
        or asset.kind not in {"audio", "video"}
    ):
        return
    owner = cast(MediaOwnerService | None, getattr(request.app.state, "media_owner_service", None))
    if owner is None:
        raise DatabaseUnavailableError("media dispatch owner is unconfigured")
    storage = version.storage_object
    asset_version_hash = version.content_hash
    if asset_version_hash is None:
        raise ValidationDomainError("media dispatch AssetVersion hash is unavailable")
    operation_key = f"media:{version.id}:{version.revision}:{asset_version_hash}"
    await owner.enqueue_dispatch(
        MediaDispatchAdmission(
            project_id=version.project_id,
            discriminator="uploaded_source",
            asset_version_id=version.id,
            asset_version_revision=version.revision,
            asset_version_hash=asset_version_hash,
            stored_object_ref=StoredObjectRef(
                project_id=version.project_id,
                profile_id=storage.storage_provider,
                bucket=storage.bucket,
                object_key=storage.object_key,
                size_bytes=storage.size_bytes,
                checksum=storage.checksum,
                mime_type=storage.mime_type,
                etag=storage.e_tag,
                operation_key=operation_key,
            ),
            operation_key=operation_key,
            technical_input={"operation": "pipeline", "steps": ["inspect", "derivative"]},
        )
    )


@router.post(
    "/v1/projects/{projectId}/asset-reservations/{reservationId}/cancel",
    response_model=ReservationResponse,
)
async def cancel_reservation(
    project_id: Annotated[str, Path(alias="projectId")],
    reservation_id: Annotated[str, Path(alias="reservationId")],
    payload: ReservationMutationRequest,
    service: Service,
    if_match: Annotated[str, Header(alias="If-Match")],
    request: Request,
) -> ReservationResponse:
    expected = _expected_revision(if_match, payload.expected_revision)
    try:
        reservation = await service.get_reservation(project_id, reservation_id)
        if payload.session_id is not None:
            storage_owner = await _resolve_reservation_storage(request, reservation)
            session = storage_owner.resume_multipart(
                _reservation_intent(reservation), payload.correlation_id
            )
            if session.session_id != payload.session_id:
                raise StorageValidationError("multipart session does not match reservation")
            storage_owner.abort_multipart(session, payload.correlation_id)
        return _reservation_response(
            await service.cancel_reservation(
                project_id,
                CancelReservationCommand(reservation_id, expected, payload.schema_version),
            )
        )
    except PortError as error:
        raise _storage_http_error(error) from error
    except DomainError as error:
        raise _error(error) from error


@router.post(
    "/v1/projects/{projectId}/asset-reservations/{reservationId}/reconcile",
    response_model=ReservationResponse,
)
async def reconcile_reservation(
    project_id: Annotated[str, Path(alias="projectId")],
    reservation_id: Annotated[str, Path(alias="reservationId")],
    payload: ReservationMutationRequest,
    service: Service,
    request: Request,
) -> ReservationResponse:
    try:
        AssetsService._require_schema_version(payload.schema_version)
        reservation = await service.get_reservation(project_id, reservation_id)
        if reservation.revision != payload.expected_revision:
            raise AssetVersionConflictError(reservation.id, payload.expected_revision)
        storage_owner = await _resolve_reservation_storage(request, reservation)
        stored = storage_owner.reconcile_multipart(
            _reconciliation_intent(reservation), payload.correlation_id
        )
        if stored is not None and reservation.status == "reserved":
            await service.complete_reservation(
                CompleteReservationCommand(
                    reservation.id,
                    StorageObject(
                        "local_workspace" if stored.bucket == "workspace" else "tos",
                        stored.bucket,
                        stored.object_key,
                        stored.mime_type,
                        stored.size_bytes,
                        stored.checksum,
                        e_tag=stored.etag,
                    ),
                    stored.checksum,
                )
            )
            reservation = await service.get_reservation(project_id, reservation_id)
        return _reservation_response(reservation)
    except PortError as error:
        raise _storage_http_error(error) from error
    except DomainError as error:
        raise _error(error) from error


@router.get("/v1/projects/{projectId}/asset-versions/{versionId}/media")
async def get_media_projection(
    project_id: Annotated[str, Path(alias="projectId")],
    version_id: Annotated[str, Path(alias="versionId")],
    service: Service,
) -> dict[str, object]:
    try:
        return await service.media_projection(project_id, version_id)
    except DomainError as error:
        raise _error(error) from error


@router.post("/v1/projects/{projectId}/asset-versions/{versionId}/media/{derivativeId}/grant")
async def issue_media_grant(
    project_id: Annotated[str, Path(alias="projectId")],
    version_id: Annotated[str, Path(alias="versionId")],
    derivative_id: Annotated[str, Path(alias="derivativeId")],
    payload: MediaGrantRequest,
    service: Service,
    request: Request,
) -> dict[str, object]:
    try:
        AssetsService._require_schema_version(payload.schema_version)
        source = await service.media_grant_source(project_id, version_id, derivative_id)
        issuer = request.app.state.opaque_read_grants
        grant = issuer.issue_read_grant(derivative_id, source, project_id, payload.ttl_seconds)
        return {
            "schemaVersion": "1.0.0",
            "action": grant.action,
            "expiresAt": grant.expires_at,
            "accessPath": f"/v1/asset-media-grants/{grant.token}",
        }
    except PortError as error:
        raise _storage_http_error(error) from error
    except DomainError as error:
        raise _error(error) from error


@router.get("/v1/asset-media-grants/{token}", response_class=StreamingResponse)
async def read_media_grant(
    token: str,
    request: Request,
) -> StreamingResponse:
    try:
        source = request.app.state.opaque_read_grants.resolve(token)
        storage_owner = _storage(request)
        if getattr(request.app.state.runtime, "storage_mode", "") != "local_workspace":
            raise AdapterNotConfiguredError("TOS media streaming is unconfigured")
        iter_chunks = getattr(storage_owner, "iter_chunks", None)
        if iter_chunks is None:
            raise AdapterNotConfiguredError("storage read stream is unconfigured")
        return StreamingResponse(
            iter_chunks(f"workspace://{source.object_key}"),
            media_type=source.mime_type,
            headers={"Cache-Control": "private, no-store", "X-Content-Type-Options": "nosniff"},
        )
    except PortError as error:
        raise _storage_http_error(error) from error


@router.get("/v1/projects/{projectId}/asset-versions/{versionId}/usage")
async def get_usage_projection(
    project_id: Annotated[str, Path(alias="projectId")],
    version_id: Annotated[str, Path(alias="versionId")],
    service: Service,
) -> dict[str, object]:
    try:
        return await service.usage_projection(project_id, version_id)
    except DomainError as error:
        raise _error(error) from error


@router.post("/v1/projects/{projectId}/asset-versions/{versionId}/timeline-selection")
async def create_timeline_selection(
    project_id: Annotated[str, Path(alias="projectId")],
    version_id: Annotated[str, Path(alias="versionId")],
    payload: TimelineSelectionRequest,
    service: Service,
) -> dict[str, object]:
    try:
        AssetsService._require_schema_version(payload.schema_version)
        return await service.timeline_selection(project_id, version_id, payload.episode_id)
    except DomainError as error:
        raise _error(error) from error
