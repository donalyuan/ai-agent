"""Phase-one owner-scoped HTTP commands and read projections."""

from __future__ import annotations

import json
from collections.abc import AsyncIterator
from typing import Annotated, Any, cast

from fastapi import APIRouter, Depends, Header, Request
from fastapi.encoders import jsonable_encoder
from fastapi.responses import JSONResponse, StreamingResponse
from pydantic import BaseModel, ConfigDict, Field

from video_agent_api.application.asset_bible import (
    AcceptImpactCommand,
    AssetBibleService,
    AssignContinuityCommand,
    CreateEntryCommand,
    CreateRelationshipCommand,
    DisableEntryCommand,
    PreviewImpactCommand,
    UpdateEntryCommand,
)
from video_agent_api.application.exports import (
    ExportService,
    export_batch_projection,
    export_job_projection,
)
from video_agent_api.application.runs import (
    BudgetGateCommand,
    EnsureWorkflowCommand,
    HistoricalRerunCommand,
    ReviewSignalCommand,
    RunsService,
    SuccessorRunCommand,
)
from video_agent_api.application.source_material import (
    AppendSourceMaterialCommand,
    CreateSourceMaterialCommand,
    SourceMaterialService,
)
from video_agent_api.application.storage_profiles import (
    CreateStorageProfileCommand,
    StorageProfileService,
)
from video_agent_api.application.timeline import (
    TimelineService,
    timeline_cut_projection,
    timeline_version_projection,
)
from video_agent_api.domain.asset_bible import ContinuityImpactTarget, OwnerReference
from video_agent_api.domain.errors import (
    DatabaseUnavailableError,
    ProjectAccessForbiddenError,
    RevisionConflictError,
    ValidationDomainError,
)
from video_agent_api.ports.contracts import StoredObjectRef
from video_agent_api.resilience import admit, probe_resources

router = APIRouter(tags=["phase-one"])


class DTO(BaseModel):
    model_config = ConfigDict(
        alias_generator=lambda v: (
            v.split("_")[0] + "".join(x.capitalize() for x in v.split("_")[1:])
        ),
        populate_by_name=True,
        extra="forbid",
    )


class EntryRequest(DTO):
    entry_type: str
    schema_version: str = Field(alias="schemaVersion")


class OwnerReferenceRequest(DTO):
    owner_id: str = Field(alias="ownerId")
    revision: int = Field(ge=0)
    content_hash: str = Field(alias="contentHash", min_length=64, max_length=64)
    purpose: str = Field(min_length=1)


class EntryVersionRequest(DTO):
    payload: dict[str, object]
    expected_revision: int = Field(ge=1)
    actor_uuid: str = Field(alias="actorUuid")
    reference_asset_version_refs: list[OwnerReferenceRequest] = Field(
        default_factory=list, alias="referenceAssetVersionRefs"
    )
    generation_spec_refs: list[OwnerReferenceRequest] = Field(
        default_factory=list, alias="generationSpecRefs"
    )
    schema_version: str = Field(alias="schemaVersion")


class DisableEntryRequest(DTO):
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")


class RunStartRequest(DTO):
    workflow_version_id: str
    node_keys: list[str] = Field(min_length=1)
    scope_refs: list[dict[str, object]] = Field(default_factory=list)
    owner_refs: list[dict[str, object]] = Field(default_factory=list)
    selection_snapshot: dict[str, object] | None = None
    idempotency_key: str | None = None
    route_decision_id: str | None = Field(default=None, alias="routeDecisionId")
    expected_binding_revision: int = Field(alias="expectedBindingRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")


class EnsureWorkflowRequest(DTO):
    schema_version: str = Field(alias="schemaVersion")


class RunRevisionRequest(DTO):
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")


class ReviewSignalRequest(DTO):
    node_run_id: str = Field(alias="nodeRunId")
    expected_node_revision: int = Field(alias="expectedNodeRevision", ge=1)
    decision: str
    correlation_id: str = Field(alias="correlationId")
    actor_uuid: str = Field(alias="actorUuid")
    schema_version: str = Field(alias="schemaVersion")


class BudgetGateRequest(DTO):
    node_run_id: str = Field(alias="nodeRunId")
    logical_operation: str = Field(alias="logicalOperation")
    request_fingerprint: str = Field(alias="requestFingerprint")
    operation_kind: str = Field(alias="operationKind")
    batch_size: int = Field(alias="batchSize", ge=1)
    cost_status: str = Field(alias="costStatus")
    estimated_cost: str | None = Field(default=None, alias="estimatedCost")
    currency: str | None = None
    threshold_snapshot_id: str | None = Field(default=None, alias="thresholdSnapshotId")
    threshold_revision: int | None = Field(default=None, alias="thresholdRevision")
    expected_node_revision: int = Field(alias="expectedNodeRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")


class BudgetConfirmRequest(DTO):
    logical_operation: str = Field(alias="logicalOperation")
    request_fingerprint: str = Field(alias="requestFingerprint")
    confirmation_id: str = Field(alias="confirmationId")
    user_uuid: str = Field(alias="userUuid")
    expected_gate_revision: int = Field(alias="expectedGateRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")


class UnsupportedWorkflowRequest(DTO):
    operation: str
    schema_version: str = Field(alias="schemaVersion")


class TimelineEditRequest(DTO):
    model_config = ConfigDict(extra="forbid", populate_by_name=False)
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    command: str
    payload: dict[str, object] = Field(default_factory=dict)
    schema_version: str = Field(alias="schemaVersion")


class TimelinePublishRequest(DTO):
    model_config = ConfigDict(extra="forbid", populate_by_name=False)
    name: str
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")


class SourceCreateRequest(DTO):
    material_type: str
    input_mode: str


class SourceAppendRequest(DTO):
    expected_revision: int = Field(ge=1)
    input_mode: str
    content: str | None = None
    content_hash: str | None = None
    asset_version_id: str | None = None


class AssignmentRequest(DTO):
    level: str
    target_id: str = Field(alias="targetId")
    entry_id: str = Field(alias="entryId")
    version_id: str = Field(alias="versionId")
    version_revision: int = Field(alias="versionRevision", ge=1)
    content_hash: str = Field(alias="contentHash", min_length=64, max_length=64)
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")


class RelationshipRequest(DTO):
    source_entry_id: str = Field(alias="sourceEntryId")
    target_entry_id: str = Field(alias="targetEntryId")
    kind: str


class ResolutionRequest(DTO):
    target_id: str = Field(alias="targetId")
    scope_ids: dict[str, str] = Field(alias="scopeIds")
    persist: bool = False


class ImpactPreviewRequest(DTO):
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    payload: dict[str, object]
    actor_uuid: str = Field(alias="actorUuid")
    reference_asset_version_refs: list[OwnerReferenceRequest] = Field(
        default_factory=list, alias="referenceAssetVersionRefs"
    )
    generation_spec_refs: list[OwnerReferenceRequest] = Field(
        default_factory=list, alias="generationSpecRefs"
    )
    owner_projection_complete: bool = Field(default=True, alias="ownerProjectionComplete")
    diagnostic: str | None = None
    schema_version: str = Field(alias="schemaVersion")


class ImpactTargetRequest(DTO):
    target_type: str = Field(alias="targetType")
    target_id: str = Field(alias="targetId")
    target_revision: int = Field(alias="targetRevision", ge=1)
    reason: str = Field(min_length=1)
    snapshot_id: str = Field(alias="snapshotId", min_length=1)
    snapshot_hash: str = Field(alias="snapshotHash", min_length=64, max_length=64)
    suggested_action: str = Field(default="review", alias="suggestedAction")


class ImpactAcceptRequest(DTO):
    analysis_id: str = Field(alias="analysisId")
    expected_analysis_revision: int = Field(alias="expectedAnalysisRevision", ge=1)
    expected_entry_revision: int = Field(alias="expectedEntryRevision", ge=1)
    expected_asset_bible_revision: int = Field(alias="expectedAssetBibleRevision", ge=1)
    candidate_payload_hash: str = Field(alias="candidatePayloadHash", min_length=64, max_length=64)
    target_refs: list[ImpactTargetRequest] = Field(alias="targetRefs")
    target_set_hash: str = Field(alias="targetSetHash", min_length=64, max_length=64)
    actor_uuid: str = Field(alias="actorUuid", min_length=1)
    correlation_id: str = Field(alias="correlationId", min_length=1)
    schema_version: str = Field(alias="schemaVersion")


class TaskTransitionRequest(DTO):
    target: str
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")


class HistoricalRerunRequest(DTO):
    expected_snapshot_revision: int = Field(alias="expectedSnapshotRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")


class SuccessorRunRequest(DTO):
    expected_predecessor_revision: int = Field(alias="expectedPredecessorRevision", ge=1)
    reuse_node_ids: list[str] = Field(default_factory=list, alias="reuseNodeIds")
    selection_snapshot: dict[str, object] | None = Field(default=None, alias="selectionSnapshot")
    schema_version: str = Field(alias="schemaVersion")


class StorageProfileRequest(DTO):
    name: str = ""
    endpoint: str
    bucket: str
    region: str
    adapter_key: str = Field(alias="adapterKey", default="tos")
    private_bucket: bool = Field(alias="privateBucket", default=True)
    bucket_binding_id: str = Field(alias="bucketBindingId", default="")
    credential_ref: str | None = Field(alias="credentialRef", default=None)
    enabled: bool = False
    connect_timeout_ms: int = Field(alias="connectTimeoutMs", default=10000, ge=1)
    read_timeout_ms: int = Field(alias="readTimeoutMs", default=30000, ge=1)
    write_timeout_ms: int = Field(alias="writeTimeoutMs", default=60000, ge=1)
    presign_max_ttl_seconds: int = Field(alias="presignMaxTtlSeconds", default=900, ge=1)
    project_scope: list[str] = Field(alias="projectScope", default_factory=list)
    expected_revision: int | None = Field(alias="expectedRevision", default=None, ge=1)


class GlobalStorageProfileRequest(StorageProfileRequest):
    project_id: str = Field(alias="projectId", min_length=1)


class StorageProfileMutationRequest(DTO):
    expected_revision: int = Field(alias="expectedRevision", ge=1)


class StorageConnectionTestRequest(StorageProfileMutationRequest):
    probe_correlation_id: str = Field(alias="probeCorrelationId", min_length=1)


class ExportBatchRequest(DTO):
    model_config = ConfigDict(extra="forbid", populate_by_name=False)
    selections: list[dict[str, object]] = Field(min_length=1)
    export_profile: str = Field(alias="exportProfile", pattern="^(light|portable)$")
    idempotency_key: str = Field(alias="idempotencyKey", min_length=1)
    storage_profile_id: str = Field(alias="storageProfileId", min_length=1)
    storage_profile_revision: int = Field(alias="storageProfileRevision", ge=1)
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    settings: dict[str, object]
    schema_version: str = Field(alias="schemaVersion")


class ExportRetryRequest(DTO):
    model_config = ConfigDict(extra="forbid", populate_by_name=False)
    episode_ids: list[str] = Field(alias="episodeIds", min_length=1)
    logical_operation: str = Field(alias="logicalOperation", min_length=1)
    schema_version: str = Field(alias="schemaVersion")


class ExportPhaseRequest(DTO):
    model_config = ConfigDict(extra="forbid", populate_by_name=False)
    phase: str
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")


class ExportTransitionRequest(DTO):
    model_config = ConfigDict(extra="forbid", populate_by_name=False)
    target: str
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")


class StoredObjectRequest(DTO):
    model_config = ConfigDict(extra="forbid", populate_by_name=False)
    project_id: str = Field(alias="projectId", min_length=1)
    profile_id: str = Field(alias="profileId", min_length=1)
    bucket: str = Field(min_length=1)
    object_key: str = Field(alias="objectKey", min_length=1)
    size_bytes: int = Field(alias="sizeBytes", ge=0)
    checksum: str = Field(min_length=64, max_length=64)
    mime_type: str = Field(alias="mimeType", min_length=3)
    etag: str | None = None
    operation_key: str = Field(alias="operationKey", min_length=1)
    verified: bool


class ArtifactRequest(DTO):
    model_config = ConfigDict(extra="forbid", populate_by_name=False)
    artifact_type: str = Field(alias="artifactType")
    size_bytes: int = Field(alias="sizeBytes", ge=0)
    checksum: str = Field(min_length=64, max_length=64)
    verified: bool
    expected_revision: int = Field(alias="expectedRevision", ge=1)
    schema_version: str = Field(alias="schemaVersion")
    storage_profile_revision: int = Field(alias="storageProfileRevision", ge=1)
    stored_object: StoredObjectRequest = Field(alias="storedObject")


class ArtifactDownloadRequest(DTO):
    model_config = ConfigDict(extra="forbid", populate_by_name=False)
    ttl_seconds: int = Field(alias="ttlSeconds", ge=1, le=300)
    schema_version: str = Field(alias="schemaVersion")


def _state(request: Request, name: str) -> object:
    service = getattr(request.app.state, name, None)
    if service is None:
        raise DatabaseUnavailableError(f"{name} is not configured")
    return service


def _camel_key(value: str) -> str:
    head, *tail = value.split("_")
    return head + "".join(part.capitalize() for part in tail)


def _owner_response(value: object) -> object:
    encoded = jsonable_encoder(value)
    if isinstance(encoded, dict):
        return {_camel_key(str(key)): _owner_response(item) for key, item in encoded.items()}
    if isinstance(encoded, list):
        return [_owner_response(item) for item in encoded]
    return encoded


def _if_match(body_revision: int, value: str | None) -> int:
    if value is None:
        raise RevisionConflictError("If-Match", body_revision, 0)
    try:
        header_revision = int(value.strip('"'))
    except ValueError as error:
        raise ValidationDomainError("If-Match must be an integer revision") from error
    if header_revision != body_revision:
        raise RevisionConflictError("If-Match", body_revision, header_revision)
    return body_revision


def _schema(value: str) -> None:
    if value != "1.0.0":
        raise ValidationDomainError("unsupported schemaVersion")


def _project_access(project_id: str, project_scope: str | None) -> None:
    if project_scope != project_id:
        raise ProjectAccessForbiddenError(project_id)


def _require_project_scope(project_scope: str | None) -> str:
    if not project_scope:
        raise ProjectAccessForbiddenError("missing-project-scope")
    return project_scope


def _timeline_payload(command: str, payload: dict[str, object]) -> dict[str, object]:
    """Map one strict public typed command to its internal domain argument names."""
    mappings: dict[str, tuple[str, dict[str, str]]] = {
        "AddClip": ("add_clip", {"clip": "clip"}),
        "TrimClip": (
            "trim_clip",
            {"clipId": "clip_id", "inFrame": "in_frame", "outFrame": "out_frame"},
        ),
        "SplitClip": (
            "split_clip",
            {"clipId": "clip_id", "splitFrame": "split_frame"},
        ),
        "ReorderClips": ("reorder_clips", {"clipIds": "clip_ids"}),
        "DeleteClip": ("delete_clip", {"clipId": "clip_id"}),
        "ReplaceClipSource": (
            "replace_clip_source",
            {"clipId": "clip_id", "oldSource": "old_source", "newSource": "new_source"},
        ),
        "SetClipTransform": (
            "set_clip_transform",
            {"clipId": "clip_id", "transform": "transform"},
        ),
        "AddSoundCue": ("add_sound_cue", {"cue": "cue"}),
        "RemoveSoundCue": ("remove_sound_cue", {"cueId": "cue_id"}),
        "SetDuckingPolicy": ("set_ducking", {"ducking": "ducking"}),
        "UpsertManualCaption": ("upsert_caption", {"caption": "caption"}),
    }
    if command == "SetSoundCueMix":
        allowed = {
            "cueId": "cue_id",
            "gainDb": "gain_db",
            "mute": "mute",
            "solo": "solo",
            "fadeInFrames": "fade_in_frames",
            "fadeOutFrames": "fade_out_frames",
        }
        if "cueId" not in payload or set(payload) - set(allowed):
            raise ValidationDomainError("SetSoundCueMix payload is incomplete or aliased")
        return {
            "__operation__": "set_sound_cue_mix",
            **{target: payload[source] for source, target in allowed.items() if source in payload},
        }
    selected = mappings.get(command)
    if selected is None:
        raise ValidationDomainError("unsupported timeline command")
    operation, fields = selected
    if set(payload) != set(fields):
        raise ValidationDomainError(f"{command} payload is incomplete or aliased")
    return {
        "__operation__": operation,
        **{target: payload[source] for source, target in fields.items()},
    }


def _owner_refs(values: list[OwnerReferenceRequest]) -> tuple[OwnerReference, ...]:
    return tuple(
        OwnerReference(item.owner_id, item.revision, item.content_hash, item.purpose)
        for item in values
    )


def _impact_targets(values: list[ImpactTargetRequest]) -> tuple[ContinuityImpactTarget, ...]:
    return tuple(
        ContinuityImpactTarget(
            target_type=cast(Any, item.target_type),
            target_id=item.target_id,
            target_revision=item.target_revision,
            reason=item.reason,
            snapshot_id=item.snapshot_id,
            snapshot_hash=item.snapshot_hash,
            suggested_action=cast(Any, item.suggested_action),
        )
        for item in values
    )


def bible(request: Request) -> AssetBibleService:
    return cast(AssetBibleService, _state(request, "asset_bible_service"))


def runs(request: Request) -> RunsService:
    return cast(RunsService, _state(request, "runs_service"))


def timeline(request: Request) -> TimelineService:
    return cast(TimelineService, _state(request, "timeline_service"))


def source_material(request: Request) -> SourceMaterialService:
    return cast(SourceMaterialService, _state(request, "source_material_service"))


def exports(request: Request) -> ExportService:
    return cast(ExportService, _state(request, "export_service"))


def storage_profiles(request: Request) -> StorageProfileService:
    return cast(StorageProfileService, _state(request, "storage_profile_service"))


@router.post("/v1/projects/{project_id}/asset-bible/entries", status_code=201)
async def create_entry(
    project_id: str,
    body: EntryRequest,
    service: Annotated[AssetBibleService, Depends(bible)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return _owner_response(
        await service.create_entry(CreateEntryCommand(project_id, body.entry_type))
    )


@router.get("/v1/projects/{project_id}/asset-bible/entries")
async def list_bible_entries(
    project_id: str,
    service: Annotated[AssetBibleService, Depends(bible)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return _owner_response(await service.list_entries(project_id))


@router.get("/v1/projects/{project_id}/asset-bible/entries/{entry_id}")
async def get_bible_entry(
    project_id: str,
    entry_id: str,
    service: Annotated[AssetBibleService, Depends(bible)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return _owner_response(await service.get_entry(project_id, entry_id))


@router.post("/v1/projects/{project_id}/asset-bible/entries/{entry_id}/versions", status_code=201)
async def update_entry(
    project_id: str,
    entry_id: str,
    body: EntryVersionRequest,
    service: Annotated[AssetBibleService, Depends(bible)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return _owner_response(
        await service.update_entry(
            UpdateEntryCommand(
                project_id,
                entry_id,
                body.payload,
                _if_match(body.expected_revision, if_match),
                body.actor_uuid,
                _owner_refs(body.reference_asset_version_refs),
                _owner_refs(body.generation_spec_refs),
            )
        )
    )


@router.post("/v1/projects/{project_id}/asset-bible/entries/{entry_id}/disable")
async def disable_bible_entry(
    project_id: str,
    entry_id: str,
    body: DisableEntryRequest,
    service: Annotated[AssetBibleService, Depends(bible)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return _owner_response(
        await service.disable_entry(
            DisableEntryCommand(project_id, entry_id, _if_match(body.expected_revision, if_match))
        )
    )


@router.post("/v1/projects/{project_id}/workflow/default/ensure")
async def ensure_workflow(
    project_id: str,
    body: EnsureWorkflowRequest,
    service: Annotated[RunsService, Depends(runs)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    await service.ensure_workflow(EnsureWorkflowCommand(project_id, scope_ids=(project_id,)))
    return await service.get_default_workflow_projection(project_id)


@router.get("/v1/projects/{project_id}/workflow/default")
async def get_default_workflow(
    project_id: str,
    service: Annotated[RunsService, Depends(runs)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return await service.get_default_workflow_projection(project_id)


@router.post("/v1/projects/{project_id}/runs", status_code=201)
async def start_run(
    project_id: str,
    body: RunStartRequest,
    service: Annotated[RunsService, Depends(runs)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return _owner_response(
        await service.start_run(
            project_id,
            body.workflow_version_id,
            body.node_keys,
            selection_snapshot=body.selection_snapshot,
            scope_refs=tuple(body.scope_refs),
            owner_refs=tuple(body.owner_refs),
            idempotency_key=body.idempotency_key,
            route_decision_id=body.route_decision_id,
            expected_binding_revision=_if_match(body.expected_binding_revision, if_match),
        )
    )


@router.post("/v1/runs/{run_id}/cancel")
async def cancel_run(
    run_id: str,
    body: RunRevisionRequest,
    service: Annotated[RunsService, Depends(runs)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _schema(body.schema_version)
    _require_project_scope(project_scope)
    detail = await service.detail(run_id, project_scope)
    _project_access(str(detail["projectId"]), project_scope)
    return _owner_response(
        await service.cancel(run_id, _if_match(body.expected_revision, if_match))
    )


@router.post("/v1/runs/{run_id}/cancel-ack")
async def acknowledge_cancel(
    run_id: str,
    body: RunRevisionRequest,
    service: Annotated[RunsService, Depends(runs)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _schema(body.schema_version)
    _require_project_scope(project_scope)
    detail = await service.detail(run_id, project_scope)
    _project_access(str(detail["projectId"]), project_scope)
    return _owner_response(
        await service.acknowledge_cancel(run_id, _if_match(body.expected_revision, if_match))
    )


@router.get("/v1/runs/{run_id}")
async def run_detail(
    run_id: str,
    service: Annotated[RunsService, Depends(runs)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> dict[str, object]:
    _require_project_scope(project_scope)
    return await service.detail(run_id, project_scope)


@router.post("/v1/runs/{run_id}/review-signals")
async def signal_run_review(
    run_id: str,
    body: ReviewSignalRequest,
    service: Annotated[RunsService, Depends(runs)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _schema(body.schema_version)
    _require_project_scope(project_scope)
    await service.detail(run_id, project_scope)
    return _owner_response(
        await service.signal_review(
            ReviewSignalCommand(
                run_id,
                body.node_run_id,
                _if_match(body.expected_node_revision, if_match),
                body.decision,
                body.correlation_id,
                body.actor_uuid,
            )
        )
    )


@router.post("/v1/runs/{run_id}/budget-gates", status_code=201)
async def create_budget_gate(
    run_id: str,
    body: BudgetGateRequest,
    service: Annotated[RunsService, Depends(runs)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _schema(body.schema_version)
    _require_project_scope(project_scope)
    await service.detail(run_id, project_scope)
    return _owner_response(
        await service.create_budget_gate(
            BudgetGateCommand(
                run_id,
                body.node_run_id,
                body.logical_operation,
                body.request_fingerprint,
                body.operation_kind,
                body.batch_size,
                body.cost_status,
                body.estimated_cost,
                body.currency,
                body.threshold_snapshot_id,
                body.threshold_revision,
                _if_match(body.expected_node_revision, if_match),
            )
        )
    )


@router.post("/v1/runs/{run_id}/budget-confirmations")
async def confirm_budget(
    run_id: str,
    body: BudgetConfirmRequest,
    service: Annotated[RunsService, Depends(runs)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _schema(body.schema_version)
    _require_project_scope(project_scope)
    await service.detail(run_id, project_scope)
    return _owner_response(
        await service.confirm_budget(
            run_id,
            body.logical_operation,
            body.request_fingerprint,
            body.confirmation_id,
            body.user_uuid,
            _if_match(body.expected_gate_revision, if_match),
        )
    )


@router.get("/v1/runs/{run_id}/events")
async def run_events(
    run_id: str,
    service: Annotated[RunsService, Depends(runs)],
    last_event_id: Annotated[str | None, Header(alias="Last-Event-ID")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> StreamingResponse:
    _require_project_scope(project_scope)
    try:
        cursor = int(last_event_id or "0")
    except ValueError as error:
        raise ValidationDomainError("Last-Event-ID must be a non-negative integer") from error
    events = await service.events(run_id, cursor, project_scope)

    async def replay() -> AsyncIterator[str]:
        for event in events:
            payload = json.dumps(_owner_response(event), separators=(",", ":"))
            yield f"id: {event.sequence}\nevent: {event.event_type}\ndata: {payload}\n\n"

    return StreamingResponse(replay(), media_type="text/event-stream")


@router.post("/v1/projects/{project_id}/workflow/mutations")
async def unsupported_workflow_mutation(
    project_id: str,
    body: UnsupportedWorkflowRequest,
    service: Annotated[RunsService, Depends(runs)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> None:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    await service.unsupported_mutation()


@router.post("/v1/projects/{project_id}/runs/{run_id}/successor", status_code=201)
async def create_successor_run(
    project_id: str,
    run_id: str,
    body: SuccessorRunRequest,
    service: Annotated[RunsService, Depends(runs)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return _owner_response(
        await service.create_successor_from_failure(
            SuccessorRunCommand(
                project_id,
                run_id,
                _if_match(body.expected_predecessor_revision, if_match),
                tuple(body.reuse_node_ids),
                body.selection_snapshot,
            )
        )
    )


@router.get("/v1/projects/{project_id}/run-input-snapshots")
async def list_run_input_snapshots(
    project_id: str,
    service: Annotated[RunsService, Depends(runs)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return await service.list_input_snapshots(project_id)


@router.get("/v1/projects/{project_id}/run-input-snapshots/{snapshot_id}")
async def get_run_input_snapshot(
    project_id: str,
    snapshot_id: str,
    service: Annotated[RunsService, Depends(runs)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return await service.get_input_snapshot(project_id, snapshot_id)


@router.post(
    "/v1/projects/{project_id}/run-input-snapshots/{snapshot_id}/rerun",
    status_code=201,
)
async def historical_rerun(
    project_id: str,
    snapshot_id: str,
    body: HistoricalRerunRequest,
    service: Annotated[RunsService, Depends(runs)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return _owner_response(
        await service.create_run_from_historical_snapshot(
            HistoricalRerunCommand(
                project_id,
                snapshot_id,
                _if_match(body.expected_snapshot_revision, if_match),
            )
        )
    )


@router.get("/v1/projects/{project_id}/episodes/{episode_id}/timeline")
async def get_timeline(
    project_id: str,
    episode_id: str,
    service: Annotated[TimelineService, Depends(timeline)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return timeline_cut_projection(await service.get_cut(episode_id, project_id))


@router.post("/v1/projects/{project_id}/episodes/{episode_id}/timeline/commands")
async def edit_timeline(
    project_id: str,
    episode_id: str,
    body: TimelineEditRequest,
    service: Annotated[TimelineService, Depends(timeline)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    mapped = _timeline_payload(body.command, body.payload)
    operation = str(mapped.pop("__operation__"))
    try:
        cut = await service.edit(
            episode_id,
            body.expected_revision,
            operation,
            mapped,
            project_id=project_id,
        )
    except RevisionConflictError as error:
        authoritative = await service.get_cut(episode_id, project_id)
        return JSONResponse(
            status_code=409,
            content={
                "detail": {"type": error.code, "message": str(error)},
                "authoritative": timeline_cut_projection(authoritative),
            },
        )
    return timeline_cut_projection(cut)


@router.post("/v1/projects/{project_id}/episodes/{episode_id}/timeline/versions", status_code=201)
async def publish_timeline(
    project_id: str,
    episode_id: str,
    body: TimelinePublishRequest,
    service: Annotated[TimelineService, Depends(timeline)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return timeline_version_projection(
        await service.publish(episode_id, body.name, body.expected_revision, project_id)
    )


@router.post("/v1/projects/{project_id}/episodes/{episode_id}/timeline/versions/preflight")
async def preflight_publish_timeline(
    project_id: str,
    episode_id: str,
    body: TimelinePublishRequest,
    service: Annotated[TimelineService, Depends(timeline)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    """Run owner publication checks without appending a TimelineVersion."""
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    try:
        result = await service.preflight_publish(episode_id, body.expected_revision, project_id)
    except RevisionConflictError as error:
        authoritative = await service.get_cut(episode_id, project_id)
        return JSONResponse(
            status_code=409,
            content={
                "detail": {"type": error.code, "message": str(error)},
                "authoritative": timeline_cut_projection(authoritative),
            },
        )
    return {
        "cutId": result.cut_id,
        "expectedRevision": result.expected_revision,
        "timelineFingerprint": result.timeline_fingerprint,
        "name": body.name,
    }


@router.get("/v1/projects/{project_id}/episodes/{episode_id}/timeline/versions")
async def list_timeline_versions(
    project_id: str,
    episode_id: str,
    service: Annotated[TimelineService, Depends(timeline)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return [
        timeline_version_projection(item)
        for item in await service.list_versions(project_id, episode_id)
    ]


@router.get("/v1/projects/{project_id}/episodes/{episode_id}/timeline/versions/{version_id}")
async def get_timeline_version(
    project_id: str,
    episode_id: str,
    version_id: str,
    service: Annotated[TimelineService, Depends(timeline)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return timeline_version_projection(
        await service.get_version(project_id, episode_id, version_id)
    )


@router.get("/v1/operations/resources")
async def resources() -> dict[str, object]:
    from pathlib import Path

    snapshot = probe_resources(Path("/tmp"))
    admission = admit(snapshot)
    return {"snapshot": snapshot, "admission": admission}


@router.get("/v1/operations/telemetry")
async def telemetry_projection(request: Request) -> dict[str, object]:
    """返回本地、脱敏、有限量的 telemetry 证据；不作为业务事实源。"""
    telemetry = getattr(request.app.state, "telemetry", None)
    if telemetry is None:
        return {"status": "unavailable", "diagnostic": "telemetry_export_unavailable"}
    return {
        "status": "ready" if telemetry.exporter_available else "degraded",
        "diagnostics": list(telemetry.diagnostics),
        "logs": telemetry.logs[-100:],
        "spans": telemetry.spans[-100:],
        "metricCount": len(telemetry.metrics),
        "schemaVersion": "1.0.0",
    }


@router.post("/v1/projects/{project_id}/source-materials", status_code=201)
async def create_source_material(
    project_id: str,
    body: SourceCreateRequest,
    service: Annotated[SourceMaterialService, Depends(source_material)],
) -> object:
    return await service.create(
        CreateSourceMaterialCommand(project_id, body.material_type, body.input_mode)
    )


@router.post("/v1/source-materials/{source_material_id}/versions", status_code=201)
async def append_source_material(
    source_material_id: str,
    body: SourceAppendRequest,
    service: Annotated[SourceMaterialService, Depends(source_material)],
) -> object:
    return await service.append(
        AppendSourceMaterialCommand(
            source_material_id,
            body.expected_revision,
            body.input_mode,
            body.content.encode() if body.content is not None else None,
            body.content_hash,
            body.asset_version_id,
        )
    )


@router.post("/v1/projects/{project_id}/asset-bible/assignments", status_code=201)
async def assign_bible(
    project_id: str,
    body: AssignmentRequest,
    service: Annotated[AssetBibleService, Depends(bible)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return _owner_response(
        await service.assign(
            AssignContinuityCommand(
                project_id,
                body.level,
                body.target_id,
                body.entry_id,
                body.version_id,
                body.version_revision,
                body.content_hash,
                _if_match(body.expected_revision, if_match),
            )
        )
    )


@router.post("/v1/projects/{project_id}/asset-bible/relationships", status_code=201)
async def create_bible_relationship(
    project_id: str,
    body: RelationshipRequest,
    service: Annotated[AssetBibleService, Depends(bible)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return _owner_response(
        await service.create_relationship(
            CreateRelationshipCommand(
                project_id, body.source_entry_id, body.target_entry_id, body.kind
            )
        )
    )


@router.post("/v1/projects/{project_id}/asset-bible/resolutions")
async def resolve_bible(
    project_id: str,
    body: ResolutionRequest,
    service: Annotated[AssetBibleService, Depends(bible)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return _owner_response(
        await service.resolve(project_id, body.target_id, body.scope_ids, persist=body.persist)
    )


@router.post("/v1/projects/{project_id}/asset-bible/entries/{entry_id}/impact-previews")
async def preview_bible_impact(
    project_id: str,
    entry_id: str,
    body: ImpactPreviewRequest,
    service: Annotated[AssetBibleService, Depends(bible)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return _owner_response(
        await service.preview_impact(
            PreviewImpactCommand(
                project_id=project_id,
                entry_id=entry_id,
                expected_revision=_if_match(body.expected_revision, if_match),
                payload=body.payload,
                actor_uuid=body.actor_uuid,
                reference_asset_version_refs=_owner_refs(body.reference_asset_version_refs),
                generation_spec_refs=_owner_refs(body.generation_spec_refs),
                owner_projection_complete=body.owner_projection_complete,
                diagnostic=body.diagnostic,
            )
        )
    )


@router.post("/v1/projects/{project_id}/asset-bible/entries/{entry_id}/impact-accepts")
async def accept_bible_impact(
    project_id: str,
    entry_id: str,
    body: ImpactAcceptRequest,
    service: Annotated[AssetBibleService, Depends(bible)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    decision, version, tasks = await service.accept_impact(
        AcceptImpactCommand(
            project_id=project_id,
            entry_id=entry_id,
            analysis_id=body.analysis_id,
            expected_analysis_revision=body.expected_analysis_revision,
            expected_entry_revision=_if_match(body.expected_entry_revision, if_match),
            expected_asset_bible_revision=body.expected_asset_bible_revision,
            candidate_payload_hash=body.candidate_payload_hash,
            target_refs=_impact_targets(body.target_refs),
            target_set_hash=body.target_set_hash,
            actor_uuid=body.actor_uuid,
            correlation_id=body.correlation_id,
        )
    )
    return _owner_response({"decision": decision, "version": version, "tasks": tasks})


@router.post("/v1/projects/{project_id}/asset-bible/tasks/{task_id}/transition")
async def transition_bible_task(
    project_id: str,
    task_id: str,
    body: TaskTransitionRequest,
    service: Annotated[AssetBibleService, Depends(bible)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return _owner_response(
        await service.transition_task(
            project_id,
            task_id,
            body.target,
            _if_match(body.expected_revision, if_match),
        )
    )


@router.get("/v1/projects/{project_id}/asset-bible/impact/{entry_id}")
async def bible_impact(
    project_id: str,
    entry_id: str,
    service: Annotated[AssetBibleService, Depends(bible)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return await service.impact(project_id, entry_id)


@router.get("/v1/projects/{project_id}/asset-bible/tasks")
async def list_bible_tasks(
    project_id: str,
    service: Annotated[AssetBibleService, Depends(bible)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return _owner_response(await service.list_tasks(project_id))


@router.get("/v1/projects/{project_id}/asset-bible/snapshots/{snapshot_id}")
async def get_bible_snapshot(
    project_id: str,
    snapshot_id: str,
    service: Annotated[AssetBibleService, Depends(bible)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return _owner_response(await service.get_snapshot(project_id, snapshot_id))


@router.get("/v1/projects/{project_id}/asset-bible/snapshots/{snapshot_id}/consumer-projection")
async def get_bible_consumer_projection(
    project_id: str,
    snapshot_id: str,
    service: Annotated[AssetBibleService, Depends(bible)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return _owner_response(await service.consumer_projection(project_id, snapshot_id))


@router.get("/v1/projects/{project_id}/asset-bible/handoff-acks/{handoff_id}")
async def get_bible_handoff_ack(
    project_id: str,
    handoff_id: str,
    service: Annotated[AssetBibleService, Depends(bible)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return _owner_response(await service.get_handoff_ack(project_id, handoff_id))


@router.post("/v1/projects/{project_id}/storage-profiles", status_code=201)
async def create_storage_profile(
    project_id: str,
    body: StorageProfileRequest,
    service: Annotated[StorageProfileService, Depends(storage_profiles)],
) -> object:
    if body.adapter_key != "tos":
        raise ValidationDomainError("unsupported storage adapter")
    profile = await service.create(
        CreateStorageProfileCommand(
            project_id,
            body.endpoint,
            body.bucket,
            body.region,
            body.name,
            body.credential_ref,
            body.private_bucket,
            tuple(body.project_scope or [project_id]),
            body.enabled,
            body.adapter_key,
            body.bucket_binding_id,
            body.connect_timeout_ms,
            body.read_timeout_ms,
            body.write_timeout_ms,
            body.presign_max_ttl_seconds,
        )
    )
    return _storage_profile_response(profile)


@router.post("/v1/storage-profiles", status_code=201)
async def create_global_storage_profile(
    body: GlobalStorageProfileRequest,
    service: Annotated[StorageProfileService, Depends(storage_profiles)],
) -> object:
    return await create_storage_profile(body.project_id, body, service)


def _storage_profile_response(profile: object) -> object:
    encoded = jsonable_encoder(profile)
    if not isinstance(encoded, dict):
        return encoded
    encoded = {_camel_key(str(key)): value for key, value in encoded.items()}
    encoded["storageProfileId"] = encoded.pop("id", None)
    encoded["schemaVersion"] = "1.0.0"
    encoded["credentialStatus"] = getattr(profile, "credential_status", "unconfigured")
    encoded["credentialSummary"] = getattr(profile, "masked_credential_summary", None)
    return encoded


@router.get("/v1/storage-profiles/{profile_id}")
async def get_storage_profile(
    profile_id: str, service: Annotated[StorageProfileService, Depends(storage_profiles)]
) -> object:
    return _storage_profile_response(await service.get(profile_id))


@router.patch("/v1/storage-profiles/{profile_id}")
async def update_storage_profile(
    profile_id: str,
    body: StorageProfileRequest,
    service: Annotated[StorageProfileService, Depends(storage_profiles)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    if body.expected_revision is None:
        raise ValidationDomainError("expectedRevision is required")
    _if_match(body.expected_revision, if_match)
    profile = await service.update(
        profile_id,
        body.expected_revision,
        {
            "name": body.name,
            "adapter_key": body.adapter_key,
            "endpoint": body.endpoint,
            "bucket": body.bucket,
            "region": body.region,
            "private_bucket": body.private_bucket,
            "bucket_binding_id": body.bucket_binding_id,
            "credential_ref": body.credential_ref,
            "enabled": body.enabled,
            "connect_timeout_ms": body.connect_timeout_ms,
            "read_timeout_ms": body.read_timeout_ms,
            "write_timeout_ms": body.write_timeout_ms,
            "presign_max_ttl_seconds": body.presign_max_ttl_seconds,
            "project_scope": tuple(body.project_scope),
        },
    )
    return _storage_profile_response(profile)


@router.post("/v1/storage-profiles/{profile_id}/enable")
async def enable_storage_profile(
    profile_id: str,
    body: StorageProfileMutationRequest,
    service: Annotated[StorageProfileService, Depends(storage_profiles)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    _if_match(body.expected_revision, if_match)
    return _storage_profile_response(
        await service.set_enabled(profile_id, body.expected_revision, True)
    )


@router.post("/v1/storage-profiles/{profile_id}/disable")
async def disable_storage_profile(
    profile_id: str,
    body: StorageProfileMutationRequest,
    service: Annotated[StorageProfileService, Depends(storage_profiles)],
    if_match: Annotated[str | None, Header(alias="If-Match")] = None,
) -> object:
    _if_match(body.expected_revision, if_match)
    return _storage_profile_response(
        await service.set_enabled(profile_id, body.expected_revision, False)
    )


@router.post("/v1/storage-profiles/{profile_id}/connection-test")
async def storage_connection_test(
    profile_id: str,
    body: StorageConnectionTestRequest,
    service: Annotated[StorageProfileService, Depends(storage_profiles)],
) -> object:
    return await service.connection_test(
        profile_id, body.expected_revision, body.probe_correlation_id
    )


@router.post("/v1/projects/{project_id}/export-batches", status_code=201)
async def create_export_batch(
    project_id: str,
    body: ExportBatchRequest,
    service: Annotated[ExportService, Depends(exports)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    batch = await service.create_batch(
        project_id,
        body.selections,
        body.export_profile,
        body.idempotency_key,
        body.settings,
        body.expected_revision,
        body.storage_profile_id,
        body.storage_profile_revision,
    )
    return export_batch_projection(batch)


@router.get("/v1/projects/{project_id}/export-batches/{batch_id}")
async def export_batch(
    project_id: str,
    batch_id: str,
    service: Annotated[ExportService, Depends(exports)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return await service.projection(project_id, batch_id)


@router.post("/v1/projects/{project_id}/export-batches/{batch_id}/retries", status_code=201)
async def retry_export_batch(
    project_id: str,
    batch_id: str,
    body: ExportRetryRequest,
    service: Annotated[ExportService, Depends(exports)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return [
        export_job_projection(item)
        for item in await service.retry_failed_members(
            project_id, batch_id, body.episode_ids, body.logical_operation
        )
    ]


@router.get("/v1/projects/{project_id}/renderer/probe")
async def probe_export_renderer(
    project_id: str,
    service: Annotated[ExportService, Depends(exports)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    return _owner_response(await service.probe_renderer(project_id))


@router.post("/v1/projects/{project_id}/export-jobs/{job_id}/transition")
async def transition_export(
    project_id: str,
    job_id: str,
    body: ExportTransitionRequest,
    service: Annotated[ExportService, Depends(exports)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return export_job_projection(
        await service.transition_job(project_id, job_id, body.target, body.expected_revision)
    )


@router.post("/v1/projects/{project_id}/export-jobs/{job_id}/packaging")
async def package_export(
    project_id: str,
    job_id: str,
    body: ExportPhaseRequest,
    service: Annotated[ExportService, Depends(exports)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return export_job_projection(
        await service.set_packaging_phase(project_id, job_id, body.phase, body.expected_revision)
    )


@router.post("/v1/projects/{project_id}/export-jobs/{job_id}/artifacts", status_code=201)
async def register_export_artifact(
    project_id: str,
    job_id: str,
    body: ArtifactRequest,
    service: Annotated[ExportService, Depends(exports)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    stored = body.stored_object
    artifact = await service.register_artifact(
        project_id,
        job_id,
        body.artifact_type,
        body.size_bytes,
        body.checksum,
        body.verified,
        body.expected_revision,
        StoredObjectRef(
            project_id=stored.project_id,
            profile_id=stored.profile_id,
            bucket=stored.bucket,
            object_key=stored.object_key,
            size_bytes=stored.size_bytes,
            checksum=stored.checksum,
            mime_type=stored.mime_type,
            etag=stored.etag,
            operation_key=stored.operation_key,
            verified=stored.verified,
        ),
        body.storage_profile_revision,
    )
    return _owner_response(
        {
            "id": artifact.id,
            "artifactType": artifact.artifact_type,
            "status": artifact.status,
            "sizeBytes": artifact.size_bytes,
            "checksum": artifact.checksum,
            "mimeType": artifact.mime_type,
            "hold": artifact.hold,
        }
    )


@router.post(
    "/v1/projects/{project_id}/episodes/{episode_id}/timeline/versions/"
    "{timeline_version_id}/export-jobs/{job_id}/artifacts/{artifact_id}/download-grants"
)
async def export_artifact_download_grant(
    project_id: str,
    episode_id: str,
    timeline_version_id: str,
    job_id: str,
    artifact_id: str,
    body: ArtifactDownloadRequest,
    service: Annotated[ExportService, Depends(exports)],
    project_scope: Annotated[str | None, Header(alias="X-Project-Scope")] = None,
) -> object:
    _project_access(project_id, project_scope)
    _schema(body.schema_version)
    return await service.download_grant(
        project_id,
        episode_id,
        timeline_version_id,
        job_id,
        artifact_id,
        project_scope or project_id,
        body.ttl_seconds,
    )
