"""projects/episodes 的 SQLAlchemy async adapter。"""

from __future__ import annotations

import importlib
import json
from collections.abc import Callable, Sequence
from dataclasses import asdict, fields, is_dataclass
from datetime import UTC, datetime
from types import TracebackType
from typing import Any, Literal, cast

from sqlalchemy import delete, func, select, update
from sqlalchemy.engine import CursorResult
from sqlalchemy.exc import IntegrityError, SQLAlchemyError
from sqlalchemy.ext.asyncio import AsyncSession, async_sessionmaker

from video_agent_api.adapters.sqlalchemy_models import (
    AcceptDecisionModel,
    AssetBibleAcceptDecisionModel,
    AssetBibleEntryModel,
    AssetBibleEntryVersionModel,
    AssetBibleHandoffAckModel,
    AssetBibleModel,
    AssetBibleRelationshipModel,
    AssetEditCandidateModel,
    AssetEditConversationModel,
    AssetEditExecutionModel,
    AssetEditMessageModel,
    AssetEditPlanModel,
    AssetEditSessionModel,
    AssetEditTurnModel,
    CapabilitySnapshotModel,
    ContinuityAssignmentModel,
    ContinuityImpactAnalysisModel,
    ContinuityRevisionTaskModel,
    CostConfirmationModel,
    CredentialMetadata,
    EditImpactModel,
    EpisodeExportBatchModel,
    EpisodeExportMemberModel,
    ExportArtifactModel,
    ExportDiagnosticTargetModel,
    ExportDispatchOutboxModel,
    ExportJobModel,
    MediaDerivativeModel,
    MediaInspectionModel,
    ModelSyncCandidateModel,
    PhaseOneDocument,
    ProjectDefaultWorkflowBindingModel,
    ProviderCallModel,
    ProviderOperationPolicyModel,
    ProviderQuotaSnapshotModel,
    PublishedWorkflowVersionModel,
    ResolvedContinuitySnapshotModel,
    SceneOrderState,
    SkillAccessAuditModel,
    SkillRevisionModel,
    StorageProfileModel,
    TimelineCaptionModel,
    TimelineClipModel,
    TimelineCurrentCutModel,
    TimelinePreviewArtifactModel,
    TimelineSoundCueModel,
    TimelineVersionModel,
    VideoOperationModel,
    VideoTakeCandidateModel,
    WorkflowBudgetGateModel,
    WorkflowIdempotencyKeyModel,
    WorkflowNodeRunModel,
    WorkflowOutboxEventModel,
    WorkflowRunEventModel,
    WorkflowRunInputSnapshotModel,
    WorkflowRunModel,
    WorkflowTemporalStartModel,
    new_id,
)
from video_agent_api.adapters.sqlalchemy_models import Asset as AssetModel
from video_agent_api.adapters.sqlalchemy_models import AssetVersion as AssetVersionModel
from video_agent_api.adapters.sqlalchemy_models import (
    AssetVersionReservation as AssetVersionReservationModel,
)
from video_agent_api.adapters.sqlalchemy_models import Episode as EpisodeModel
from video_agent_api.adapters.sqlalchemy_models import (
    Model as CatalogModel,
)
from video_agent_api.adapters.sqlalchemy_models import Project as ProjectModel
from video_agent_api.adapters.sqlalchemy_models import (
    Provider as ProviderModel,
)
from video_agent_api.adapters.sqlalchemy_models import (
    ProviderProfile as ProviderProfileModel,
)
from video_agent_api.adapters.sqlalchemy_models import Scene as SceneModel
from video_agent_api.adapters.sqlalchemy_models import (
    SceneShotHandoffAck as SceneShotHandoffAckModel,
)
from video_agent_api.adapters.sqlalchemy_models import Shot as ShotModel
from video_agent_api.application.ports import (
    AssetRepository,
    AssetVersionRepository,
    EpisodeRepository,
    ProjectRepository,
)
from video_agent_api.domain.asset_bible import (
    AssetBible,
    AssetBibleAcceptDecision,
    AssetBibleEntry,
    AssetBibleHandoffAck,
    AssetBibleRelationship,
    AssetBibleVersion,
    ContinuityAssignment,
    ContinuityImpactAnalysis,
    ContinuityImpactTarget,
    ContinuityRevisionTask,
    OwnerReference,
    ResolvedContinuitySnapshot,
)
from video_agent_api.domain.assets import (
    Asset,
    AssetVersion,
    AssetVersionReservation,
    StorageObject,
)
from video_agent_api.domain.catalog import (
    CapabilitySnapshot,
    ModelSyncCandidate,
    Provider,
    ProviderProfile,
    SkillAccessAudit,
    SkillRevisionRecord,
)
from video_agent_api.domain.catalog import (
    Model as CatalogDomainModel,
)
from video_agent_api.domain.conversation import (
    AgentConversation,
    ConversationMessage,
    ConversationTurn,
)
from video_agent_api.domain.creative import (
    CreativeBriefSourceBindingSnapshot,
    CreativeBriefVersion,
    ProjectCreativeSettingsVersion,
)
from video_agent_api.domain.entities import Episode, Project
from video_agent_api.domain.errors import (
    AssetVersionConflictError,
    DatabaseUnavailableError,
    EpisodeNumberConflictError,
    RevisionConflictError,
)
from video_agent_api.domain.media import MediaDerivative, MediaInspection, PreviewArtifact
from video_agent_api.domain.provider_ops import (
    CostConfirmation,
    ProviderCall,
    ProviderQuotaSnapshot,
)
from video_agent_api.domain.runs import (
    BudgetGate,
    NodeRun,
    ProjectDefaultWorkflowBinding,
    RunEvent,
    RunInputSnapshot,
    TemporalStart,
    WorkflowRun,
    WorkflowVersion,
)
from video_agent_api.domain.scenes import (
    AcceptedMediaEligibility,
    ImmutableOwnerRef,
    Scene,
    SceneShotOwnerAck,
    Shot,
    SpecVersion,
)
from video_agent_api.domain.timeline import TimelineCut, TimelineVersion
from video_agent_api.domain.video_generation import VideoOperation, VideoTakeCandidate
from video_agent_api.ports.credentials import CredentialEnvelope
from video_agent_api.ports.storage import StorageProfile

_ASSET_BIBLE_DOCUMENT_COLLECTIONS = {
    "asset_bible_entries",
    "asset_bibles_by_project",
    "asset_bible_by_project",
    "asset_bible_assignments",
    "asset_bible_relationships",
    "asset_bible_snapshots",
    "asset_bible_tasks",
    "asset_bible_impacts",
    "asset_bible_impact_payloads",
    "asset_bible_decisions",
    "asset_bible_handoff_acks",
}

_WORKFLOW_DOCUMENT_COLLECTIONS = {
    "workflow_by_project",
    "workflow_bindings",
    "workflow_runs",
    "workflow_run_keys",
    "workflow_run_key_fingerprints",
    "workflow_signal_keys",
    "run_events",
    "run_input_snapshots",
    "budget_gates",
    "temporal_starts",
}


def _encode_phase_one(value: object) -> str:
    """Encode owner objects as typed JSON, never as executable/pickle data."""

    def encode(item: object) -> object:
        if is_dataclass(item) and not isinstance(item, type):
            return {
                "__type__": f"{type(item).__module__}:{type(item).__qualname__}",
                "fields": {field.name: encode(getattr(item, field.name)) for field in fields(item)},
            }
        if isinstance(item, dict):
            return {"__dict__": [[encode(key), encode(value)] for key, value in item.items()]}
        if isinstance(item, (list, tuple)):
            if isinstance(item, tuple):
                return {"__tuple__": [encode(value) for value in item]}
            return [encode(value) for value in item]
        if isinstance(item, (set, frozenset)):
            return {"__set__": [encode(value) for value in sorted(item, key=str)]}
        if isinstance(item, (str, int, float, bool)) or item is None:
            return item
        raise TypeError(f"phase-one value is not JSON encodable: {type(item).__name__}")

    return json.dumps(encode(value), sort_keys=True, separators=(",", ":"), ensure_ascii=True)


def _decode_phase_one(raw: object) -> object:
    parsed = json.loads(raw) if isinstance(raw, str) else raw

    def decode(item: object) -> object:
        if isinstance(item, list):
            return [decode(value) for value in item]
        if isinstance(item, dict) and "__tuple__" in item:
            return tuple(decode(value) for value in item["__tuple__"])
        if isinstance(item, dict) and "__set__" in item:
            return frozenset(decode(value) for value in item["__set__"])
        if isinstance(item, dict) and "__dict__" in item:
            entries = item["__dict__"]
            return {decode(pair[0]): decode(pair[1]) for pair in entries}
        if isinstance(item, dict) and "__type__" in item:
            module_name, _, qualname = str(item["__type__"]).partition(":")
            if not module_name.startswith("video_agent_api."):
                raise ValueError("phase-one type module is not allowed")
            target: Any = importlib.import_module(module_name)
            for part in qualname.split("."):
                target = getattr(target, part)
            if not is_dataclass(target):
                raise ValueError("phase-one type is not a dataclass")
            values = item.get("fields")
            if not isinstance(values, dict):
                raise ValueError("phase-one dataclass fields are invalid")
            constructor = cast(Any, target)
            return constructor(**{key: decode(value) for key, value in values.items()})
        if isinstance(item, dict):
            return {key: decode(value) for key, value in item.items()}
        return item

    return decode(parsed)


def _asset_from_model(value: AssetModel) -> Asset:
    return Asset(
        id=value.id,
        project_id=value.project_id,
        kind=value.kind,
        name=value.name,
        status=value.status,
        schema_version=value.schema_version,
        revision=value.revision,
        source_type=value.source_type,
        catalog_role=value.catalog_role,
        tags=tuple(value.tags),
        authorization_status=value.authorization_status,
        copyright_owner=value.copyright_owner,
        license_label=value.license_label,
        license_reference=value.license_reference,
        updated_at=_utc_iso(value.updated_at),
    )


def _version_from_model(value: AssetVersionModel) -> AssetVersion:
    media_raw: object = value.media_metadata
    if media_raw is None and isinstance(value.metadata_json, dict):
        media_raw = value.metadata_json.get("media")
    media = (
        {key: item for key, item in media_raw.items() if isinstance(item, int)}
        if isinstance(media_raw, dict)
        else None
    )
    storage = StorageObject(
        storage_provider=value.storage_provider,
        bucket=value.bucket,
        region=value.region,
        object_key=value.object_key or value.storage_ref,
        e_tag=value.e_tag,
        mime_type=value.mime_type,
        size_bytes=value.size_bytes,
        checksum=value.checksum,
        media=media,
    )
    return AssetVersion(
        id=value.id,
        asset_id=value.asset_id,
        project_id=value.project_id,
        version_number=value.version_number,
        storage_object=storage,
        content_hash=value.content_hash,
        status=value.status,
        schema_version=value.schema_version,
        revision=value.revision,
    )


def _project_from_model(value: ProjectModel) -> Project:
    project = Project(
        id=value.id,
        name=value.name,
        status=value.status,
        schema_version=value.schema_version,
        revision=value.revision,
    )
    # SQLAlchemy adapter keeps creative documents attached to the same project owner.
    project.creation_mode = value.creation_mode
    project.creative_brief_current = _decode_brief(value.creative_brief_current)
    project.creative_brief_history = [_decode_brief(item) for item in value.creative_brief_history]
    project.creative_settings_current = _decode_settings(value.creative_settings_current)
    project.creative_settings_history = [
        _decode_settings(item) for item in value.creative_settings_history
    ]
    project.source_binding_current = _decode_binding(value.source_binding_current)
    project.source_binding_history = [
        _decode_binding(item) for item in value.source_binding_history
    ]
    project.story_spec_ref = value.story_spec_ref
    project.story_spec_history = value.story_spec_history
    project.source_materials = value.source_materials
    return project


def _project_creative_payload(value: Project, model: ProjectModel) -> None:
    """将 creative owner 文档映射回 ORM；正文仍只由 projects owner 保存。"""
    model.creation_mode = getattr(value, "creation_mode", None)


def _json_value(value: object) -> object:
    if is_dataclass(value):
        return asdict(cast(Any, value))
    if isinstance(value, list):
        return [_json_value(item) for item in value]
    if isinstance(value, dict):
        return {key: _json_value(item) for key, item in value.items()}
    return value


def _provider_from_model(value: ProviderModel) -> Provider:
    return Provider(
        name=value.name,
        adapter_key=value.adapter_key,
        approval=cast(Any, getattr(value, "approval", "pending")),
        feature_gate=cast(Any, getattr(value, "feature_gate", "MVP-A")),
        adapter_installed=bool(getattr(value, "adapter_installed", False)),
        enabled=bool(value.enabled),
        id=value.id,
        revision=int(getattr(value, "revision", 1)),
    )


def _profile_from_model(value: ProviderProfileModel) -> ProviderProfile:
    return ProviderProfile(
        provider_id=value.provider_id,
        name=value.name,
        adapter_identity=getattr(value, "adapter_identity", "local_workspace"),
        enabled=bool(value.enabled),
        explicit_live_opt_in=bool(getattr(value, "explicit_live_opt_in", False)),
        credential_status=getattr(value, "credential_status", "unconfigured"),
        revision=int(getattr(value, "revision", 1)),
        id=value.id,
    )


def _model_from_model(value: CatalogModel) -> CatalogDomainModel:
    return CatalogDomainModel(
        profile_id=value.profile_id,
        model_key=value.model_key,
        enabled=bool(value.enabled),
        revision=int(getattr(value, "revision", 1)),
        id=value.id,
    )


def _capability_from_model(value: CapabilitySnapshotModel) -> CapabilitySnapshot:
    return CapabilitySnapshot(
        provider_id=value.provider_id,
        profile_id=value.profile_id,
        operation=value.operation,
        revision=value.revision,
        runnable=value.runnable,
        capabilities=tuple(value.capabilities or ()),
        captured_at=value.captured_at,
        model_id=value.model_id,
        id=value.id,
        retention_policy=value.retention_policy,
        retention_version=value.retention_version,
        hold=value.hold,
    )


def _provider_call_from_model(value: ProviderCallModel) -> ProviderCall:
    return ProviderCall(
        project_id=value.project_id,
        run_id=value.run_id,
        node_run_id=value.node_run_id,
        logical_operation=value.logical_operation,
        operation=value.operation,
        provider_id=value.provider_id,
        profile_id=value.profile_id,
        model_id=value.model_id,
        capability_snapshot_id=value.capability_snapshot_id,
        request_fingerprint=value.request_fingerprint,
        status=cast(Any, value.status),
        cost_status=cast(Any, value.cost_status),
        cost_value=value.cost_value,
        cost_currency=value.cost_currency,
        cost_source=value.cost_source,
        provider_request_id=value.provider_request_id,
        native_usage=dict(value.native_usage) if isinstance(value.native_usage, dict) else None,
        failure_code=value.failure_code,
        id=value.id,
        revision=value.revision,
        retention_policy=value.retention_policy,
        retention_version=value.retention_version,
        hold=value.hold,
    )


def _video_operation_from_model(value: VideoOperationModel) -> VideoOperation:
    return VideoOperation(
        project_id=value.project_id,
        run_id=value.run_id,
        logical_operation=value.logical_operation,
        provider_id=value.provider_id,
        profile_id=value.profile_id,
        model_id=value.model_id,
        capability_snapshot_id=value.capability_snapshot_id,
        source_asset_version_id=value.source_asset_version_id,
        source_asset_version_revision=value.source_asset_version_revision,
        source_asset_version_hash=value.source_asset_version_hash,
        source_candidate_id=value.source_candidate_id,
        source_provenance=value.source_provenance,
        shot_spec_id=value.shot_spec_id,
        shot_spec_revision=value.shot_spec_revision,
        shot_spec_hash=value.shot_spec_hash,
        duration_seconds=value.duration_seconds,
        aspect_ratio=value.aspect_ratio,
        status=cast(Any, value.status),
        provider_request_id=value.provider_request_id,
        revision=value.revision,
        id=value.id,
        cancel_requested=bool(value.cancel_requested),
        episode_id=value.episode_id,
        target_id=value.target_id,
        asset_id=value.asset_id,
        observation_fingerprints=tuple(value.observation_fingerprints or ()),
    )


def _video_candidate_from_model(value: VideoTakeCandidateModel) -> VideoTakeCandidate:
    return VideoTakeCandidate(
        project_id=value.project_id,
        episode_id=value.episode_id,
        target_id=value.target_id,
        run_id=value.run_id,
        logical_operation=value.logical_operation,
        source_asset_version_id=value.source_asset_version_id,
        source_asset_version_revision=value.source_asset_version_revision,
        source_asset_version_hash=value.source_asset_version_hash,
        source_candidate_id=value.source_candidate_id,
        source_provenance=value.source_provenance or "agnes_video",
        shot_spec_id=value.shot_spec_id,
        shot_spec_revision=value.shot_spec_revision,
        shot_spec_hash=value.shot_spec_hash,
        duration_seconds=value.duration_seconds,
        aspect_ratio=value.aspect_ratio,
        asset_version_id=value.asset_version_id,
        asset_version_revision=value.asset_version_revision,
        asset_version_hash=value.asset_version_hash,
        provider_request_id=value.provider_request_id,
        status=cast(Any, value.status),
        revision=value.revision,
        id=value.id,
    )


def _cost_confirmation_from_model(value: CostConfirmationModel) -> CostConfirmation:
    return CostConfirmation(
        project_id=value.project_id,
        run_id=value.run_id,
        logical_operation=value.logical_operation,
        request_fingerprint=value.request_fingerprint,
        user_uuid=value.user_uuid,
        threshold_snapshot_id=value.threshold_snapshot_id,
        threshold_revision=value.threshold_revision,
        estimated_cost=value.estimated_cost,
        cost_status=cast(Any, value.cost_status),
        operation_kind=value.operation_kind,
        batch_size=value.batch_size,
        id=value.id,
        retention_policy=value.retention_policy,
        retention_version=value.retention_version,
        hold=value.hold,
    )


def _skill_from_model(value: SkillRevisionModel) -> SkillRevisionRecord:
    return SkillRevisionRecord(
        name=value.name,
        version=value.version,
        provenance=cast(Any, value.provenance),
        approval=cast(Any, value.approval),
        enabled=value.enabled,
        source_identity=value.source_identity,
        digest=value.digest,
        id=value.id,
        schema_version=value.schema_version,
        revision=value.revision,
        source_type=cast(Any, value.source_type),
        license_status=value.license_status,
        capabilities=tuple(value.capabilities or ()),
    )


def _sync_candidate_from_model(value: ModelSyncCandidateModel) -> ModelSyncCandidate:
    return ModelSyncCandidate(
        profile_id=value.profile_id,
        remote_models=tuple(value.remote_models or ()),
        added=tuple(value.added or ()),
        removed=tuple(value.removed or ()),
        changed=tuple(value.changed or ()),
        status=cast(Any, value.status),
        revision=value.revision,
        id=value.id,
    )


def _skill_access_from_model(value: SkillAccessAuditModel) -> SkillAccessAudit:
    return SkillAccessAudit(
        skill_revision_id=value.skill_revision_id,
        run_id=value.run_id,
        node_run_id=value.node_run_id,
        access=cast(Any, value.access),
        allowed=value.allowed,
        reason=value.reason,
        id=value.id,
    )


def _utc_iso(value: datetime) -> str:
    return (value if value.tzinfo is not None else value.replace(tzinfo=UTC)).isoformat()


def _asset_bible_owner_ref(value: object) -> OwnerReference:
    if not isinstance(value, dict):
        raise ValueError("asset bible owner reference row is invalid")
    return OwnerReference(
        owner_id=str(value["owner_id"]),
        revision=int(value["revision"]),
        content_hash=str(value["content_hash"]),
        purpose=str(value["purpose"]),
    )


def _asset_bible_version_from_model(value: AssetBibleEntryVersionModel) -> AssetBibleVersion:
    return AssetBibleVersion(
        entry_id=value.entry_id,
        project_id=value.project_id,
        entry_type=cast(Any, value.entry_type),
        payload=dict(value.payload),
        version_number=value.version_number,
        actor_uuid=value.actor_uuid,
        reference_asset_version_refs=tuple(
            _asset_bible_owner_ref(item) for item in value.reference_asset_version_refs
        ),
        generation_spec_refs=tuple(
            _asset_bible_owner_ref(item) for item in value.generation_spec_refs
        ),
        revision=value.revision,
        id=value.id,
        content_hash=value.content_hash,
        schema_version=value.schema_version,
    )


def _continuity_assignment_from_model(value: ContinuityAssignmentModel) -> ContinuityAssignment:
    return ContinuityAssignment(
        project_id=value.project_id,
        level=value.level,
        target_id=value.target_id,
        entry_id=value.entry_id,
        version_id=value.version_id,
        version_revision=value.version_revision,
        content_hash=value.content_hash,
        revision=value.revision,
        schema_version=value.schema_version,
        id=value.id,
        scope_revision=value.scope_revision,
    )


def _assignment_from_json(value: object) -> ContinuityAssignment:
    if not isinstance(value, dict):
        raise ValueError("continuity assignment document is invalid")
    return ContinuityAssignment(**cast(dict[str, Any], value))


def _impact_target_from_json(value: object) -> ContinuityImpactTarget:
    if not isinstance(value, dict):
        raise ValueError("continuity impact target document is invalid")
    return ContinuityImpactTarget(**cast(dict[str, Any], value))


def _handoff_ref_from_json(value: object) -> tuple[str, str, int, str]:
    if not isinstance(value, list) or len(value) != 4:
        raise ValueError("asset bible handoff reference is invalid")
    return str(value[0]), str(value[1]), int(value[2]), str(value[3])


def _revision_ref_from_json(value: object) -> tuple[str, int]:
    if not isinstance(value, list) or len(value) != 2 or not isinstance(value[1], int):
        raise ValueError("continuity source revision is invalid")
    return str(value[0]), value[1]


def _episode_from_model(value: EpisodeModel) -> Episode:
    return Episode(
        id=value.id,
        project_id=value.project_id,
        title=value.title,
        number=value.display_number,
        status=value.status,
        schema_version=value.schema_version,
        revision=value.revision,
        script_spec_ref=value.script_spec_ref,
        script_spec_history=value.script_spec_history,
    )


def _owner_ref_from_json(value: object) -> ImmutableOwnerRef | None:
    if not isinstance(value, dict):
        return None
    return ImmutableOwnerRef(
        id=str(value["id"]),
        revision=int(value["revision"]),
        content_hash=str(value["content_hash"]),
    )


def _spec_version_from_json(value: object) -> SpecVersion:
    if not isinstance(value, dict):
        raise ValueError("scene spec version document is invalid")
    return SpecVersion(
        owner_id=str(value["owner_id"]),
        project_id=str(value["project_id"]),
        episode_id=str(value["episode_id"]),
        kind=cast(Any, value["kind"]),
        payload=cast(dict[str, object], value["payload"]),
        revision=int(value["revision"]),
        id=str(value["id"]),
        content_hash=str(value["content_hash"]),
    )


def _eligibility_from_json(value: object) -> AcceptedMediaEligibility | None:
    if not isinstance(value, dict):
        return None
    return AcceptedMediaEligibility(**cast(dict[str, Any], value))


def _scene_from_models(value: SceneModel, shot_models: Sequence[ShotModel]) -> Scene:
    scene = Scene(
        project_id=value.project_id,
        episode_id=value.episode_id,
        display_number=value.display_number,
        title=value.title,
        id=value.id,
        revision=value.revision,
        schema_version=value.schema_version,
        status=value.status,
        spec_ref=_owner_ref_from_json(value.spec_ref),
        spec_versions=[_spec_version_from_json(item) for item in value.spec_versions],
    )
    for row in sorted(shot_models, key=lambda item: (item.display_number, item.id)):
        shot = Shot(
            scene_id=row.scene_id,
            project_id=row.project_id,
            episode_id=row.episode_id,
            display_number=row.display_number,
            id=row.id,
            revision=row.revision,
            schema_version=row.schema_version,
            status=row.status,
            spec_ref=_owner_ref_from_json(row.spec_ref),
            continuity_snapshot=_owner_ref_from_json(row.continuity_snapshot),
            continuity_task_refs=[
                ref
                for item in row.continuity_task_refs
                if (ref := _owner_ref_from_json(item)) is not None
            ],
            current_image=_eligibility_from_json(row.current_image),
            current_video=_eligibility_from_json(row.current_video),
            spec_versions=[_spec_version_from_json(item) for item in row.spec_versions],
        )
        scene.shots.append(shot)
    return scene


def _decode_brief(value: object) -> CreativeBriefVersion | None:
    if not isinstance(value, dict):
        return None
    return CreativeBriefVersion(
        creative_brief_id=str(value["creative_brief_id"]),
        project_id=str(value["project_id"]),
        subject=str(value["subject"]),
        genre=str(value["genre"]),
        audience=str(value["audience"]),
        character_premise=str(value["character_premise"]),
        style=str(value["style"]),
        episode_duration_seconds=int(value["episode_duration_seconds"]),
        episode_count=int(value["episode_count"]),
        scenes_per_episode=int(value["scenes_per_episode"]),
        shots_per_scene=int(value["shots_per_scene"]),
        revision=int(value["revision"]),
        schema_version=str(value.get("schema_version", "1.0.0")),
        id=str(value["id"]),
        payload_hash=str(value["payload_hash"]),
    )


def _decode_settings(value: object) -> ProjectCreativeSettingsVersion | None:
    if not isinstance(value, dict):
        return None
    return ProjectCreativeSettingsVersion(
        project_id=str(value["project_id"]),
        text_cost_confirmation_threshold=cast(
            dict[str, str] | None, value.get("text_cost_confirmation_threshold")
        ),
        revision=int(value["revision"]),
        schema_version=str(value.get("schema_version", "1.0.0")),
        id=str(value["id"]),
        payload_hash=str(value["payload_hash"]),
    )


def _decode_binding(value: object) -> CreativeBriefSourceBindingSnapshot | None:
    if not isinstance(value, dict):
        return None
    return CreativeBriefSourceBindingSnapshot(
        project_id=str(value["project_id"]),
        source_material_id=str(value["source_material_id"]),
        source_material_revision=int(value["source_material_revision"]),
        source_content_hash=str(value["source_content_hash"]),
        creative_brief_id=str(value["creative_brief_id"]),
        creative_brief_revision=int(value["creative_brief_revision"]),
        creative_brief_payload_hash=str(value["creative_brief_payload_hash"]),
        parse_status=str(value["parse_status"]),
        validation_status=str(value["validation_status"]),
        binding_status=str(value["binding_status"]),
        binding_version=str(value["binding_version"]),
        schema_version=str(value.get("schema_version", "1.0.0")),
        id=str(value["id"]),
    )


class SQLAlchemyProjectRepository(ProjectRepository):
    def __init__(self, session: AsyncSession) -> None:
        self.session = session

    async def get(self, project_id: str) -> Project | None:
        model = await self.session.get(ProjectModel, project_id)
        return _project_from_model(model) if model else None

    async def list(self) -> Sequence[Project]:
        result = await self.session.execute(select(ProjectModel).order_by(ProjectModel.id))
        return [_project_from_model(item) for item in result.scalars()]

    async def add(self, project: Project) -> None:
        self.session.add(
            ProjectModel(
                id=project.id,
                name=project.name,
                status=project.status,
                schema_version=project.schema_version,
                revision=project.revision,
                creation_mode=getattr(project, "creation_mode", None),
                creative_brief_current=_json_value(
                    getattr(project, "creative_brief_current", None)
                ),
                creative_brief_history=_json_value(getattr(project, "creative_brief_history", [])),
                creative_settings_current=_json_value(
                    getattr(project, "creative_settings_current", None)
                ),
                creative_settings_history=_json_value(
                    getattr(project, "creative_settings_history", [])
                ),
                source_binding_current=_json_value(
                    getattr(project, "source_binding_current", None)
                ),
                source_binding_history=_json_value(getattr(project, "source_binding_history", [])),
                story_spec_ref=getattr(project, "story_spec_ref", None),
                story_spec_history=_json_value(getattr(project, "story_spec_history", [])),
                source_materials=_json_value(getattr(project, "source_materials", [])),
            )
        )

    async def save(self, project: Project) -> None:
        result = cast(
            CursorResult[Any],
            await self.session.execute(
                update(ProjectModel)
                .where(
                    ProjectModel.id == project.id,
                    ProjectModel.revision == project.revision - 1,
                )
                .values(
                    name=project.name,
                    status=project.status,
                    schema_version=project.schema_version,
                    revision=project.revision,
                    creation_mode=getattr(project, "creation_mode", None),
                    creative_brief_current=_json_value(
                        getattr(project, "creative_brief_current", None)
                    ),
                    creative_brief_history=_json_value(
                        getattr(project, "creative_brief_history", [])
                    ),
                    creative_settings_current=_json_value(
                        getattr(project, "creative_settings_current", None)
                    ),
                    creative_settings_history=_json_value(
                        getattr(project, "creative_settings_history", [])
                    ),
                    source_binding_current=_json_value(
                        getattr(project, "source_binding_current", None)
                    ),
                    source_binding_history=_json_value(
                        getattr(project, "source_binding_history", [])
                    ),
                    story_spec_ref=getattr(project, "story_spec_ref", None),
                    story_spec_history=_json_value(getattr(project, "story_spec_history", [])),
                    source_materials=_json_value(getattr(project, "source_materials", [])),
                )
            ),
        )
        if result.rowcount != 1:
            current_revision = await self.session.scalar(
                select(ProjectModel.revision).where(ProjectModel.id == project.id)
            )
            if current_revision is None:
                raise KeyError(project.id)
            raise RevisionConflictError(project.id, project.revision - 1, current_revision)


class SQLAlchemyEpisodeRepository(EpisodeRepository):
    def __init__(self, session: AsyncSession) -> None:
        self.session = session

    async def get(self, episode_id: str) -> Episode | None:
        model = await self.session.get(EpisodeModel, episode_id)
        return _episode_from_model(model) if model else None

    async def list_by_project(self, project_id: str) -> Sequence[Episode]:
        result = await self.session.execute(
            select(EpisodeModel)
            .where(EpisodeModel.project_id == project_id)
            .order_by(EpisodeModel.display_number, EpisodeModel.id)
        )
        return [_episode_from_model(item) for item in result.scalars()]

    async def add(self, episode: Episode) -> None:
        self.session.add(
            EpisodeModel(
                id=episode.id,
                project_id=episode.project_id,
                display_number=episode.number,
                title=episode.title,
                status=episode.status,
                schema_version=episode.schema_version,
                revision=episode.revision,
                script_spec_ref=getattr(episode, "script_spec_ref", None),
                script_spec_history=_json_value(getattr(episode, "script_spec_history", [])),
            )
        )

    async def save(self, episode: Episode) -> None:
        result = cast(
            CursorResult[Any],
            await self.session.execute(
                update(EpisodeModel)
                .where(
                    EpisodeModel.id == episode.id,
                    EpisodeModel.revision == episode.revision - 1,
                )
                .values(
                    display_number=episode.number,
                    title=episode.title,
                    status=episode.status,
                    schema_version=episode.schema_version,
                    revision=episode.revision,
                    script_spec_ref=getattr(episode, "script_spec_ref", None),
                    script_spec_history=_json_value(getattr(episode, "script_spec_history", [])),
                )
            ),
        )
        if result.rowcount != 1:
            current_revision = await self.session.scalar(
                select(EpisodeModel.revision).where(EpisodeModel.id == episode.id)
            )
            if current_revision is None:
                raise KeyError(episode.id)
            raise RevisionConflictError(episode.id, episode.revision - 1, current_revision)


class SQLAlchemyAssetRepository(AssetRepository):
    def __init__(self, session: AsyncSession) -> None:
        self.session = session

    async def get(self, asset_id: str) -> Asset | None:
        model = await self.session.get(AssetModel, asset_id)
        return _asset_from_model(model) if model else None

    async def list_by_project(self, project_id: str) -> Sequence[Asset]:
        result = await self.session.execute(
            select(AssetModel).where(AssetModel.project_id == project_id).order_by(AssetModel.id)
        )
        return [_asset_from_model(item) for item in result.scalars()]

    async def add(self, asset: Asset) -> None:
        self.session.add(
            AssetModel(
                id=asset.id,
                project_id=asset.project_id,
                kind=asset.kind,
                name=asset.name,
                status=asset.status,
                schema_version=asset.schema_version,
                revision=asset.revision,
                source_type=asset.source_type,
                catalog_role=asset.catalog_role,
                tags=list(asset.tags),
                authorization_status=asset.authorization_status,
                copyright_owner=asset.copyright_owner,
                license_label=asset.license_label,
                license_reference=asset.license_reference,
            )
        )

    async def save(self, asset: Asset) -> None:
        previous_revision = asset.revision - 1
        result = cast(
            CursorResult[Any],
            await self.session.execute(
                update(AssetModel)
                .where(AssetModel.id == asset.id, AssetModel.revision == previous_revision)
                .values(
                    revision=asset.revision,
                    source_type=asset.source_type,
                    catalog_role=asset.catalog_role,
                    tags=list(asset.tags),
                    authorization_status=asset.authorization_status,
                    copyright_owner=asset.copyright_owner,
                    license_label=asset.license_label,
                    license_reference=asset.license_reference,
                    updated_at=datetime.fromisoformat(asset.updated_at),
                )
            ),
        )
        if result.rowcount != 1:
            current = await self.session.scalar(
                select(AssetModel.revision).where(AssetModel.id == asset.id)
            )
            raise AssetVersionConflictError(asset.id, int(current or 0))


class SQLAlchemyAssetVersionRepository(AssetVersionRepository):
    def __init__(self, session: AsyncSession) -> None:
        self.session = session

    async def get(self, version_id: str) -> AssetVersion | None:
        model = await self.session.get(AssetVersionModel, version_id)
        return _version_from_model(model) if model else None

    async def list_by_asset(self, asset_id: str) -> Sequence[AssetVersion]:
        result = await self.session.execute(
            select(AssetVersionModel)
            .where(AssetVersionModel.asset_id == asset_id)
            .order_by(AssetVersionModel.version_number, AssetVersionModel.id)
        )
        return [_version_from_model(item) for item in result.scalars()]

    async def next_version_number(self, asset_id: str) -> int:
        current = await self.session.scalar(
            select(func.max(AssetVersionModel.version_number)).where(
                AssetVersionModel.asset_id == asset_id
            )
        )
        return int(current or 0) + 1

    async def add(self, version: AssetVersion) -> None:
        obj = version.storage_object
        self.session.add(
            AssetVersionModel(
                id=version.id,
                asset_id=version.asset_id,
                project_id=version.project_id,
                version_number=version.version_number,
                revision=version.revision,
                status=version.status,
                schema_version=version.schema_version,
                storage_ref=obj.object_key,
                checksum=obj.checksum,
                content_hash=version.content_hash,
                storage_provider=obj.storage_provider,
                bucket=obj.bucket,
                region=obj.region,
                object_key=obj.object_key,
                e_tag=obj.e_tag,
                mime_type=obj.mime_type,
                size_bytes=obj.size_bytes,
                media_metadata=dict(obj.media) if obj.media else None,
                metadata_json={"media": dict(obj.media)} if obj.media else {},
            )
        )


class SQLAlchemyUnitOfWork:
    """一个 command 一个 session；adapter 不向 application 暴露 AsyncSession。"""

    def __init__(self, session_factory: async_sessionmaker[AsyncSession]) -> None:
        self._session_factory = session_factory
        self.session: AsyncSession | None = None
        self.projects: SQLAlchemyProjectRepository
        self.episodes: SQLAlchemyEpisodeRepository
        self.assets: SQLAlchemyAssetRepository
        self.asset_versions: SQLAlchemyAssetVersionRepository
        self.outbox_events: list[dict[str, object]]
        self.asset_reservations: dict[str, AssetVersionReservation]
        self.scenes: dict[str, Scene]
        self.shots: dict[str, Shot]
        self.scenes_by_episode: dict[str, list[Scene]]
        self.scene_order_revisions: dict[str, int]
        self.scene_handoff_acks: dict[str, SceneShotOwnerAck]
        self.creative_brief_current: dict[str, CreativeBriefVersion]
        self.source_bindings_current: dict[str, CreativeBriefSourceBindingSnapshot]
        self._phase_one_collections: dict[str, object] = {}
        self._loaded_phase_one_revisions: dict[str, int] = {}
        self._loaded_phase_one_payloads: dict[str, str] = {}
        self._loaded_scene_revisions: dict[str, int] = {}
        self._loaded_shot_revisions: dict[str, int] = {}
        self._loaded_scene_order_revisions: dict[str, int] = {}
        self._loaded_scene_handoff_ids: set[str] = set()
        self._loaded_asset_bible_revisions: dict[str, int] = {}
        self._loaded_asset_bible_entry_revisions: dict[str, int] = {}
        self._loaded_asset_bible_task_revisions: dict[str, int] = {}
        self._loaded_asset_bible_version_ids: set[str] = set()
        self._loaded_asset_bible_relationship_ids: set[str] = set()
        self._loaded_asset_bible_assignment_ids: set[str] = set()
        self._loaded_asset_bible_snapshot_ids: set[str] = set()
        self._loaded_asset_bible_impact_ids: set[str] = set()
        self._loaded_asset_bible_decision_ids: set[str] = set()
        self._loaded_asset_bible_handoff_ids: set[str] = set()
        self._loaded_workflow_source_ids: set[str] = set()
        self._loaded_workflow_binding_revisions: dict[str, int] = {}
        self._loaded_workflow_run_revisions: dict[str, int] = {}
        self._loaded_workflow_node_revisions: dict[str, int] = {}
        self._loaded_workflow_snapshot_ids: set[str] = set()
        self._loaded_workflow_event_ids: set[str] = set()
        self._loaded_workflow_idempotency_ids: set[str] = set()
        self._loaded_workflow_idempotency_keys: set[tuple[str, str]] = set()
        self._loaded_workflow_temporal_revisions: dict[str, int] = {}
        self._loaded_workflow_budget_revisions: dict[str, int] = {}
        self._loaded_workflow_outbox_event_ids: set[str] = set()
        self._loaded_catalog_provider_revisions: dict[str, int] = {}
        self._loaded_catalog_profile_revisions: dict[str, int] = {}
        self._loaded_catalog_model_revisions: dict[str, int] = {}
        self._loaded_catalog_policy_rows: dict[
            tuple[str, str], tuple[str, int, dict[str, object]]
        ] = {}
        self.credential_envelopes: dict[str, CredentialEnvelope] = {}
        self._loaded_catalog_quota_ids: set[str] = set()
        self._loaded_catalog_snapshot_ids: set[str] = set()
        self._loaded_catalog_skill_ids: set[str] = set()
        self._loaded_catalog_call_ids: set[str] = set()
        self._loaded_catalog_call_revisions: dict[str, int] = {}
        self._loaded_catalog_confirmation_ids: set[str] = set()
        self._loaded_catalog_sync_ids: set[str] = set()
        self._loaded_catalog_sync_revisions: dict[str, int] = {}
        self._loaded_catalog_audit_ids: set[str] = set()
        self._loaded_catalog_credential_ids: set[str] = set()
        self._loaded_catalog_credential_by_profile: dict[str, str] = {}
        self._loaded_video_operation_revisions: dict[str, int] = {}
        self._loaded_video_candidate_revisions: dict[str, int] = {}
        self._loaded_edit_revisions: dict[str, int] = {}
        self._loaded_timeline_cut_revisions: dict[str, int] = {}
        self._loaded_timeline_version_ids: set[str] = set()
        self._loaded_export_batch_revisions: dict[str, int] = {}
        self._loaded_export_job_revisions: dict[str, int] = {}
        self._loaded_export_artifact_ids: set[str] = set()
        self._loaded_export_dispatch_revisions: dict[str, int] = {}
        self._loaded_media_inspection_ids: set[str] = set()
        self._loaded_media_derivative_ids: set[str] = set()
        self._loaded_preview_artifact_ids: set[str] = set()
        self._loaded_asset_reservation_revisions: dict[str, int] = {}

    def __call__(self) -> SQLAlchemyUnitOfWork:
        return type(self)(self._session_factory)

    async def __aenter__(self) -> SQLAlchemyUnitOfWork:
        self.session = self._session_factory()
        self.projects = SQLAlchemyProjectRepository(self.session)
        self.episodes = SQLAlchemyEpisodeRepository(self.session)
        self.assets = SQLAlchemyAssetRepository(self.session)
        self.asset_versions = SQLAlchemyAssetVersionRepository(self.session)
        self.asset_reservations = {}
        try:
            await self._load_phase_one_collections()
            await self._load_asset_reservations()
            await self._load_timeline_collections()
            await self._load_media_collections()
            await self._load_export_collections()
            await self._load_catalog_collections()
            await self._load_video_collections()
            await self._load_edit_collections()
            await self._load_storage_profiles()
            await self._load_workflow_collections()
            await self._load_asset_bible_collections()
            await self._load_scene_collections()
        except (OSError, SQLAlchemyError) as error:
            await self.session.close()
            raise DatabaseUnavailableError("business database is unavailable") from error
        return self

    async def _load_asset_reservations(self) -> None:
        """Load normalized Assets owner facts; legacy documents are migration input only."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        rows = list((await self.session.execute(select(AssetVersionReservationModel))).scalars())
        if not rows:
            return
        self.asset_reservations = {
            row.id: AssetVersionReservation(
                project_id=row.project_id,
                asset_id=row.asset_id,
                operation_key=row.operation_key,
                fingerprint=row.fingerprint,
                status=cast(Any, row.status),
                id=row.id,
                revision=row.revision,
                registered_version_id=row.registered_version_id,
                expected_asset_revision=row.expected_asset_revision,
                declared_kind=row.declared_kind,
                declared_mime_type=row.declared_mime_type,
                declared_size_bytes=row.declared_size_bytes,
                declared_checksum=row.declared_checksum,
                storage_profile_id=row.storage_profile_id,
                storage_profile_revision=row.storage_profile_revision,
                storage_profile_snapshot_hash=row.storage_profile_snapshot_hash,
                upload_key=row.upload_key,
                diagnostic=row.diagnostic,
                schema_version=row.schema_version,
            )
            for row in rows
        }
        self._loaded_asset_reservation_revisions = {row.id: row.revision for row in rows}

    async def _load_timeline_collections(self) -> None:
        """Load canonical timeline facts; legacy documents are one-time input only."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        cut_rows = list((await self.session.execute(select(TimelineCurrentCutModel))).scalars())
        version_rows = list((await self.session.execute(select(TimelineVersionModel))).scalars())
        if cut_rows:
            self.timeline_cuts = {}
            for cut_row in cut_rows:
                encoded = (
                    cut_row.payload.get("encoded") if isinstance(cut_row.payload, dict) else None
                )
                if encoded is not None:
                    value = _decode_phase_one(encoded)
                    if isinstance(value, TimelineCut):
                        self.timeline_cuts[cut_row.episode_id] = value
            self._loaded_timeline_cut_revisions = {row.id: row.revision for row in cut_rows}
        if version_rows:
            self.timeline_versions = {}
            for version_row in version_rows:
                encoded = (
                    version_row.snapshot.get("encoded")
                    if isinstance(version_row.snapshot, dict)
                    else None
                )
                if encoded is not None:
                    value = _decode_phase_one(encoded)
                    if isinstance(value, TimelineVersion):
                        self.timeline_versions[version_row.id] = value
            self._loaded_timeline_version_ids = {row.id for row in version_rows}

    async def _load_export_collections(self) -> None:
        """Load export aggregates from normalized batch rows for restart recovery."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        batch_rows = list((await self.session.execute(select(EpisodeExportBatchModel))).scalars())
        job_rows = list((await self.session.execute(select(ExportJobModel))).scalars())
        artifact_rows = list((await self.session.execute(select(ExportArtifactModel))).scalars())
        dispatch_rows = list(
            (await self.session.execute(select(ExportDispatchOutboxModel))).scalars()
        )
        if batch_rows:
            self.export_batches = {}
            self.export_jobs = {}
            for batch_row in batch_rows:
                encoded = (
                    batch_row.payload.get("encoded")
                    if isinstance(batch_row.payload, dict)
                    else None
                )
                if encoded is None:
                    continue
                batch = _decode_phase_one(encoded)
                from video_agent_api.domain.exports import EpisodeExportBatch

                if isinstance(batch, EpisodeExportBatch):
                    self.export_batches[batch.id] = batch
                    self.export_jobs.update({job.id: job for job in batch.jobs})
            self._loaded_export_batch_revisions = {row.id: row.revision for row in batch_rows}
            self._loaded_export_job_revisions = {row.id: row.revision for row in job_rows}
            self._loaded_export_artifact_ids = {row.id for row in artifact_rows}
        self.export_dispatch_outbox = {}
        from video_agent_api.domain.exports import ExportDispatchOutbox

        for dispatch_row in dispatch_rows:
            encoded = (
                dispatch_row.payload.get("encoded")
                if isinstance(dispatch_row.payload, dict)
                else None
            )
            if encoded is None:
                continue
            value = _decode_phase_one(encoded)
            if isinstance(value, ExportDispatchOutbox):
                self.export_dispatch_outbox[value.id] = value
        self._loaded_export_dispatch_revisions = {row.id: row.revision for row in dispatch_rows}

    async def _load_media_collections(self) -> None:
        """Load Media Worker-owned facts independently from Timeline and Provider state."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        inspection_rows = list((await self.session.execute(select(MediaInspectionModel))).scalars())
        derivative_rows = list((await self.session.execute(select(MediaDerivativeModel))).scalars())
        preview_rows = list(
            (await self.session.execute(select(TimelinePreviewArtifactModel))).scalars()
        )
        self.media_inspections: dict[str, MediaInspection] = {}
        self.media_derivatives: dict[str, MediaDerivative] = {}
        self.preview_artifacts: dict[str, PreviewArtifact] = {}
        for inspection_row in inspection_rows:
            encoded = (
                inspection_row.payload.get("encoded")
                if isinstance(inspection_row.payload, dict)
                else None
            )
            if encoded is not None:
                value = _decode_phase_one(encoded)
                if isinstance(value, MediaInspection):
                    self.media_inspections[inspection_row.id] = value
        for derivative_row in derivative_rows:
            encoded = (
                derivative_row.payload.get("encoded")
                if isinstance(derivative_row.payload, dict)
                else None
            )
            if encoded is not None:
                value = _decode_phase_one(encoded)
                if isinstance(value, MediaDerivative):
                    self.media_derivatives[derivative_row.id] = value
        for preview_row in preview_rows:
            encoded = (
                preview_row.payload.get("encoded")
                if isinstance(preview_row.payload, dict)
                else None
            )
            if encoded is not None:
                value = _decode_phase_one(encoded)
                if isinstance(value, PreviewArtifact):
                    self.preview_artifacts[preview_row.id] = value
        self._loaded_media_inspection_ids = {row.id for row in inspection_rows}
        self._loaded_media_derivative_ids = {row.id for row in derivative_rows}
        self._loaded_preview_artifact_ids = {row.id for row in preview_rows}

    async def _load_storage_profiles(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        rows = list((await self.session.execute(select(StorageProfileModel))).scalars())
        if rows:
            self.storage_profiles = {
                row.id: StorageProfile(
                    row.id,
                    row.project_id,
                    row.endpoint,
                    row.bucket,
                    row.region,
                    row.credential_status,
                    row.enabled,
                    row.revision,
                    row.name,
                    row.adapter_key,
                    row.private_bucket,
                    row.bucket_binding_id,
                    row.credential_ref,
                    row.connect_timeout_ms,
                    row.read_timeout_ms,
                    row.write_timeout_ms,
                    row.presign_max_ttl_seconds,
                    tuple(row.project_scope),
                    row.masked_credential_summary,
                )
                for row in rows
            }

    async def _load_video_collections(self) -> None:
        """Load Agnes state from its normalized owner tables, never from phase documents."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        operation_rows = list((await self.session.execute(select(VideoOperationModel))).scalars())
        candidate_rows = list(
            (await self.session.execute(select(VideoTakeCandidateModel))).scalars()
        )
        self.video_operations = {
            (row.run_id, row.logical_operation): _video_operation_from_model(row)
            for row in operation_rows
        }
        self.video_take_candidates = {
            row.id: _video_candidate_from_model(row) for row in candidate_rows
        }
        self._loaded_video_operation_revisions = {row.id: row.revision for row in operation_rows}
        self._loaded_video_candidate_revisions = {row.id: row.revision for row in candidate_rows}

    async def _load_edit_collections(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        legacy_conversations = getattr(self, "conversations", {})
        self.asset_edit_sessions = {}
        self.asset_edit_plans = {}
        self.asset_edit_executions = {}
        self.asset_edit_candidates = {}
        self.accept_decisions = {}
        self.edit_impacts = {}
        self.conversations = {}
        for row in cast(Any, (await self.session.execute(select(AssetEditSessionModel))).scalars()):
            encoded = row.payload.get("encoded") if isinstance(row.payload, dict) else None
            if encoded is not None:
                self.asset_edit_sessions[row.id] = _decode_phase_one(encoded)
            self._loaded_edit_revisions[row.id] = row.revision
        for row in cast(Any, (await self.session.execute(select(AssetEditPlanModel))).scalars()):
            encoded = row.payload.get("encoded") if isinstance(row.payload, dict) else None
            if encoded is not None:
                self.asset_edit_plans[row.id] = _decode_phase_one(encoded)
            self._loaded_edit_revisions[row.id] = row.revision
        for row in cast(
            Any, (await self.session.execute(select(AssetEditExecutionModel))).scalars()
        ):
            encoded = row.payload.get("encoded") if isinstance(row.payload, dict) else None
            if encoded is not None:
                self.asset_edit_executions[row.id] = _decode_phase_one(encoded)
            self._loaded_edit_revisions[row.id] = row.revision
        for row in cast(
            Any, (await self.session.execute(select(AssetEditCandidateModel))).scalars()
        ):
            encoded = row.payload.get("encoded") if isinstance(row.payload, dict) else None
            if encoded is not None:
                self.asset_edit_candidates[row.id] = _decode_phase_one(encoded)
            self._loaded_edit_revisions[row.id] = row.revision
        for row in cast(Any, (await self.session.execute(select(AcceptDecisionModel))).scalars()):
            encoded = row.payload.get("encoded") if isinstance(row.payload, dict) else None
            if encoded is not None:
                self.accept_decisions[row.candidate_id] = _decode_phase_one(encoded)
        for row in cast(Any, (await self.session.execute(select(EditImpactModel))).scalars()):
            encoded = row.payload.get("encoded") if isinstance(row.payload, dict) else None
            if encoded is not None:
                self.edit_impacts[row.id] = _decode_phase_one(encoded)
        conversation_rows = list(
            (await self.session.execute(select(AssetEditConversationModel))).scalars()
        )
        message_rows = list((await self.session.execute(select(AssetEditMessageModel))).scalars())
        turn_rows = list((await self.session.execute(select(AssetEditTurnModel))).scalars())
        messages_by_session: dict[str, list[ConversationMessage]] = {}
        for row in message_rows:
            messages_by_session.setdefault(row.session_id, []).append(
                ConversationMessage(
                    row.session_id,
                    row.sequence,
                    cast(Literal["user", "agent"], row.role),
                    row.content_hash,
                    cast(Literal["complete", "pending", "failed"], row.status),
                    row.correlation_id,
                    row.id,
                )
            )
        turns_by_session: dict[str, list[ConversationTurn]] = {}
        for row in turn_rows:
            turns_by_session.setdefault(row.session_id, []).append(
                ConversationTurn(
                    row.session_id,
                    row.sequence,
                    row.user_message_id,
                    cast(Literal["pending", "complete", "failed", "cancelled"], row.status),
                    row.agent_message_id,
                    row.id,
                    row.revision,
                )
            )
        for row in conversation_rows:
            self.conversations[row.id] = AgentConversation(
                row.project_id,
                row.episode_id,
                row.revision,
                row.id,
                sorted(messages_by_session.get(row.id, []), key=lambda item: item.sequence),
                sorted(turns_by_session.get(row.id, []), key=lambda item: item.sequence),
            )
        if not conversation_rows and legacy_conversations:
            # Legacy phase-one documents are migration input only; the next commit
            # writes the same facts into normalized conversation/message/turn owners.
            self.conversations.update(legacy_conversations)

    async def _load_catalog_collections(self) -> None:
        """Load normalized catalog facts, falling back to legacy documents once."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")

        provider_rows = list((await self.session.execute(select(ProviderModel))).scalars())
        profile_rows = list((await self.session.execute(select(ProviderProfileModel))).scalars())
        model_rows = list((await self.session.execute(select(CatalogModel))).scalars())
        if provider_rows:
            self.providers = {row.id: _provider_from_model(row) for row in provider_rows}
        self._loaded_catalog_provider_revisions = {
            row.id: int(getattr(row, "revision", 1)) for row in provider_rows
        }
        if profile_rows:
            self.profiles = {row.id: _profile_from_model(row) for row in profile_rows}
        self._loaded_catalog_profile_revisions = {
            row.id: int(getattr(row, "revision", 1)) for row in profile_rows
        }
        if model_rows:
            self.models = {row.id: _model_from_model(row) for row in model_rows}
        self._loaded_catalog_model_revisions = {
            row.id: int(getattr(row, "revision", 1)) for row in model_rows
        }

        snapshot_rows = list(
            (await self.session.execute(select(CapabilitySnapshotModel))).scalars()
        )
        if snapshot_rows:
            for row in snapshot_rows:
                profile = self.profiles.get(row.profile_id)
                if profile is not None:
                    current = profile.capability_snapshots.get(row.operation)
                    if current is None or (current.revision, current.id) <= (row.revision, row.id):
                        profile.capability_snapshots[row.operation] = _capability_from_model(row)
            self._loaded_catalog_snapshot_ids = {row.id for row in snapshot_rows}

        policy_rows = list(
            (await self.session.execute(select(ProviderOperationPolicyModel))).scalars()
        )
        for policy_row in policy_rows:
            policy_profile = self.profiles.get(policy_row.profile_id)
            if policy_profile is None:
                continue
            values: dict[str, object] = {
                "maxConcurrency": policy_row.max_concurrency,
                "rateLimit": policy_row.rate_limit,
                "rateWindowSeconds": policy_row.rate_window_seconds,
            }
            policy_profile.operation_policies[policy_row.operation] = values
            self._loaded_catalog_policy_rows[(policy_row.profile_id, policy_row.operation)] = (
                policy_row.id,
                policy_row.revision,
                values,
            )

        quota_rows = list(
            (await self.session.execute(select(ProviderQuotaSnapshotModel))).scalars()
        )
        for quota_row in quota_rows:
            profile = self.profiles.get(quota_row.profile_id)
            if profile is None:
                continue
            quota_current = profile.quota_snapshots.get(quota_row.operation)
            snapshot = ProviderQuotaSnapshot(
                provider_id=quota_row.provider_id,
                profile_id=quota_row.profile_id,
                operation=quota_row.operation,
                status=cast(Any, quota_row.status),
                remaining=quota_row.remaining,
                reset_at=quota_row.reset_at,
                source=quota_row.source,
                captured_at=quota_row.captured_at,
                revision=quota_row.revision,
                id=quota_row.id,
            )
            if quota_current is None or (
                quota_current.revision,
                quota_current.id,
            ) <= (snapshot.revision, snapshot.id):
                profile.quota_snapshots[quota_row.operation] = snapshot
        self._loaded_catalog_quota_ids = {row.id for row in quota_rows}

        skill_rows = list((await self.session.execute(select(SkillRevisionModel))).scalars())
        if skill_rows:
            self.skills = [_skill_from_model(row) for row in skill_rows]
        self._loaded_catalog_skill_ids = {row.id for row in skill_rows}

        call_rows = list((await self.session.execute(select(ProviderCallModel))).scalars())
        if call_rows:
            self.provider_calls = {row.id: _provider_call_from_model(row) for row in call_rows}
            self.provider_call_keys = {
                (row.run_id, row.logical_operation): row.id for row in call_rows
            }
        self._loaded_catalog_call_ids = {row.id for row in call_rows}
        self._loaded_catalog_call_revisions = {row.id: row.revision for row in call_rows}

        confirmation_rows = list(
            (await self.session.execute(select(CostConfirmationModel))).scalars()
        )
        if confirmation_rows:
            self.cost_confirmations = {
                (row.run_id, row.logical_operation): _cost_confirmation_from_model(row)
                for row in confirmation_rows
            }
        self._loaded_catalog_confirmation_ids = {row.id for row in confirmation_rows}

        sync_rows = list((await self.session.execute(select(ModelSyncCandidateModel))).scalars())
        if sync_rows:
            self.model_sync_candidates = {
                row.id: _sync_candidate_from_model(row) for row in sync_rows
            }
        self._loaded_catalog_sync_ids = {row.id for row in sync_rows}

        audit_rows = list((await self.session.execute(select(SkillAccessAuditModel))).scalars())
        if audit_rows:
            self.skill_access_audits = [_skill_access_from_model(row) for row in audit_rows]
        self._loaded_catalog_audit_ids = {row.id for row in audit_rows}

        credential_rows = list((await self.session.execute(select(CredentialMetadata))).scalars())
        if credential_rows:
            for credential_row in credential_rows:
                profile_id = credential_row.profile_id
                if not profile_id:
                    continue
                self.credential_envelopes[profile_id] = CredentialEnvelope(
                    algorithm=credential_row.algorithm or "AES-256-GCM",
                    ciphertext=credential_row.ciphertext,
                    nonce=credential_row.nonce,
                    auth_tag=credential_row.tag,
                    key_version=credential_row.key_version,
                    aad_version=credential_row.aad_version or "v1",
                    profile_id=profile_id,
                    credential_id=credential_row.credential_id or "unbound",
                    masked_prefix=credential_row.masked_prefix or "",
                    last4=credential_row.last4 or "",
                )
                credential_profile = self.profiles.get(profile_id)
                if credential_profile is not None:
                    credential_profile.credential_status = "configured"
                self._loaded_catalog_credential_by_profile[profile_id] = credential_row.id
        self._loaded_catalog_credential_ids = {row.id for row in credential_rows}
        self._loaded_catalog_sync_revisions = {row.id: row.revision for row in sync_rows}

    async def _load_phase_one_collections(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        result = await self.session.execute(
            select(PhaseOneDocument).where(PhaseOneDocument.owner == "phase-one")
        )
        for row in result.scalars():
            self._loaded_phase_one_revisions[row.collection] = row.revision
            payload = row.document.get("payload") if isinstance(row.document, dict) else None
            if payload is None:
                continue
            try:
                value = _decode_phase_one(payload)
            except Exception:
                continue
            self._phase_one_collections[row.collection] = value
            self._loaded_phase_one_payloads[row.collection] = _encode_phase_one(value)
        defaults: dict[str, object] = {
            "creative_modes": {},
            "creative_brief_current": {},
            "creative_briefs": {},
            "creative_settings_current": {},
            "creative_settings": {},
            "source_bindings_current": {},
            "source_bindings": {},
            "scenes": {},
            "shots": {},
            "scenes_by_episode": {},
            "scene_order_revisions": {},
            "scene_handoff_acks": {},
            "audit_events": [],
            "outbox_events": [],
            "asset_bible_entries": {},
            "asset_bibles_by_project": {},
            "asset_bible_by_project": {},
            "asset_bible_assignments": [],
            "asset_bible_relationships": [],
            "asset_bible_snapshots": {},
            "asset_bible_tasks": {},
            "asset_bible_impacts": {},
            "asset_bible_impact_payloads": {},
            "asset_bible_decisions": {},
            "asset_bible_handoff_acks": {},
            "workflow_by_project": {},
            "workflow_bindings": {},
            "workflow_runs": {},
            "workflow_run_keys": {},
            "workflow_run_key_fingerprints": {},
            "workflow_signal_keys": {},
            "run_events": {},
            "run_input_snapshots": {},
            "budget_gates": {},
            "temporal_starts": {},
            "providers": {},
            "profiles": {},
            "models": {},
            "skills": [],
            "asset_reservations": {},
            "timeline_cuts": {},
            "timeline_versions": {},
            "media_inspections": {},
            "media_derivatives": {},
            "preview_artifacts": {},
            "asset_edit_plans": {},
            "asset_edit_candidates": {},
            "asset_edit_sessions": {},
            "asset_edit_executions": {},
            "accept_decisions": {},
            "edit_impacts": {},
            "storage_profiles": {},
            "text_review_batches": {},
            "text_candidates": {},
            "text_handoffs": {},
            "text_handoff_acks": {},
            "skill_route_decisions": {},
            "skill_route_selections": {},
            "source_materials": {},
            "provider_calls": {},
            "provider_call_keys": {},
            "cost_confirmations": {},
            "credential_envelopes": {},
            "model_sync_candidates": {},
            "skill_access_audits": [],
            "usage_audits": [],
            "catalog_overrides": {},
            "export_jobs": {},
            "export_batches": {},
            "export_dispatch_outbox": {},
            "conversations": {},
            "image_generation_candidates": {},
        }
        for key, default in defaults.items():
            setattr(self, key, self._phase_one_collections.get(key, default))
        # Relationship indexes are projections of the canonical scene/shot maps;
        # rebuilding them avoids stale object copies after a fresh UoW reload.
        scenes = getattr(self, "scenes", {})
        if isinstance(scenes, dict):
            rebuilt_scenes_by_episode: dict[str, list[Scene]] = {}
            rebuilt_shots: dict[str, Shot] = {}
            for raw_scene in scenes.values():
                if not isinstance(raw_scene, Scene):
                    continue
                scene = raw_scene
                episode_id = getattr(scene, "episode_id", None)
                if episode_id is not None:
                    rebuilt_scenes_by_episode.setdefault(episode_id, []).append(scene)
                for shot in scene.shots:
                    rebuilt_shots[shot.id] = shot
            self.scenes_by_episode = rebuilt_scenes_by_episode
            self.shots = rebuilt_shots

    async def _load_workflow_collections(self) -> None:
        """Load canonical workflow/run facts; document rows are migration input only."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        source_rows = list(
            (await self.session.execute(select(PublishedWorkflowVersionModel))).scalars()
        )
        if not source_rows:
            return

        self.workflow_by_project = {}
        for source_row in source_rows:
            source = WorkflowVersion(
                project_id=source_row.project_id,
                template_key=source_row.template_key,
                scope_type=source_row.scope_type,
                scope_ids=tuple(source_row.scope_ids),
                definition=dict(source_row.definition),
                revision=source_row.revision,
                content_hash=source_row.content_hash,
                id=source_row.id,
                status=source_row.status,
                version_number=source_row.version_number,
                schema_version=source_row.schema_version,
            )
            self.workflow_by_project[source.project_id] = source
            self._loaded_workflow_source_ids.add(source.id)

        self.workflow_bindings = {}
        binding_rows = list(
            (await self.session.execute(select(ProjectDefaultWorkflowBindingModel))).scalars()
        )
        for binding_row in binding_rows:
            binding = ProjectDefaultWorkflowBinding(
                project_id=binding_row.project_id,
                workflow_version_id=binding_row.workflow_version_id,
                workflow_content_hash=binding_row.workflow_content_hash,
                template_key=binding_row.template_key,
                revision=binding_row.revision,
                id=binding_row.id,
                schema_version=binding_row.schema_version,
                created_at=_utc_iso(binding_row.created_at),
            )
            self.workflow_bindings[binding.project_id] = binding
            self._loaded_workflow_binding_revisions[binding.id] = binding.revision

        node_rows = list((await self.session.execute(select(WorkflowNodeRunModel))).scalars())
        nodes_by_run: dict[str, list[NodeRun]] = {}
        for node_row in node_rows:
            node = NodeRun(
                run_id=node_row.run_id,
                node_key=node_row.node_key,
                status=cast(Any, node_row.status),
                revision=node_row.revision,
                id=node_row.id,
                logical_operation=node_row.logical_operation,
                scope_refs=tuple(node_row.scope_refs),
                output_evidence=(
                    dict(node_row.output_evidence) if node_row.output_evidence else None
                ),
                failure=(dict(node_row.failure) if node_row.failure else None),
                submission_state=cast(Any, node_row.submission_state),
            )
            nodes_by_run.setdefault(node.run_id, []).append(node)
            self._loaded_workflow_node_revisions[node.id] = node.revision

        self.workflow_runs = {}
        run_rows = list((await self.session.execute(select(WorkflowRunModel))).scalars())
        for run_row in run_rows:
            if run_row.workflow_version_id is None:
                continue
            run = WorkflowRun(
                project_id=run_row.project_id,
                workflow_version_id=run_row.workflow_version_id,
                status=cast(Any, run_row.status),
                revision=run_row.revision,
                id=run_row.id,
                rerun_of_run_id=run_row.rerun_of_run_id,
                predecessor_run_id=run_row.predecessor_run_id,
                nodes=sorted(nodes_by_run.get(run_row.id, []), key=lambda item: item.id),
                input_snapshot=(dict(run_row.input_snapshot) if run_row.input_snapshot else None),
                selection_snapshot=dict(run_row.selection_snapshot),
                source_snapshot=dict(run_row.source_snapshot),
                created_at=_utc_iso(run_row.created_at),
                updated_at=_utc_iso(run_row.updated_at),
            )
            self.workflow_runs[run.id] = run
            self._loaded_workflow_run_revisions[run.id] = run.revision

        self.run_input_snapshots = {}
        snapshot_rows = list(
            (await self.session.execute(select(WorkflowRunInputSnapshotModel))).scalars()
        )
        for snapshot_row in snapshot_rows:
            snapshot = RunInputSnapshot(
                run_id=snapshot_row.run_id,
                project_id=snapshot_row.project_id,
                workflow_version_id=snapshot_row.workflow_version_id,
                workflow_content_hash=snapshot_row.workflow_content_hash,
                scope_refs=tuple(snapshot_row.scope_refs),
                owner_refs=tuple(snapshot_row.owner_refs),
                selection_snapshot=dict(snapshot_row.selection_snapshot),
                source_snapshot=dict(snapshot_row.source_snapshot),
                node_inputs=tuple(snapshot_row.node_inputs),
                runnable=snapshot_row.runnable,
                diagnostic=snapshot_row.diagnostic,
                revision=snapshot_row.revision,
                id=snapshot_row.id,
                schema_version=snapshot_row.schema_version,
                created_at=_utc_iso(snapshot_row.created_at),
            )
            self.run_input_snapshots[snapshot.id] = snapshot
            self._loaded_workflow_snapshot_ids.add(snapshot.id)

        self.run_events: dict[str, list[RunEvent]] = {}
        event_rows = list(
            (
                await self.session.execute(
                    select(WorkflowRunEventModel).order_by(
                        WorkflowRunEventModel.run_id, WorkflowRunEventModel.sequence
                    )
                )
            ).scalars()
        )
        for event_row in event_rows:
            event = RunEvent(
                run_id=event_row.run_id,
                sequence=event_row.sequence,
                event_type=event_row.event_type,
                correlation_id=event_row.correlation_id,
                payload=dict(event_row.payload),
                node_run_id=event_row.node_run_id,
                id=event_row.id,
                revision=event_row.revision,
                created_at=_utc_iso(event_row.created_at),
                retention_policy=event_row.retention_policy,
                retention_version=event_row.retention_version,
                hold=event_row.hold,
            )
            self.run_events.setdefault(event.run_id, []).append(event)
            self._loaded_workflow_event_ids.add(event.id)

        self.workflow_run_keys = {}
        self.workflow_run_key_fingerprints = {}
        self.workflow_signal_keys = {}
        idempotency_rows = list(
            (await self.session.execute(select(WorkflowIdempotencyKeyModel))).scalars()
        )
        for idempotency_row in idempotency_rows:
            if idempotency_row.key_kind == "start":
                self.workflow_run_keys[idempotency_row.idempotency_key] = idempotency_row.run_id
                self.workflow_run_key_fingerprints[idempotency_row.idempotency_key] = (
                    idempotency_row.request_fingerprint
                )
            else:
                self.workflow_signal_keys[idempotency_row.idempotency_key] = (
                    idempotency_row.run_id,
                    idempotency_row.request_fingerprint,
                )
            self._loaded_workflow_idempotency_ids.add(idempotency_row.id)
            self._loaded_workflow_idempotency_keys.add(
                (idempotency_row.key_kind, idempotency_row.idempotency_key)
            )

        self.temporal_starts = {}
        temporal_rows = list(
            (await self.session.execute(select(WorkflowTemporalStartModel))).scalars()
        )
        for temporal_row in temporal_rows:
            start = TemporalStart(
                run_id=temporal_row.run_id,
                node_run_id=temporal_row.node_run_id,
                logical_operation=temporal_row.logical_operation,
                workflow_id=temporal_row.workflow_id,
                request_fingerprint=temporal_row.request_fingerprint,
                status=cast(Any, temporal_row.status),
                id=temporal_row.id,
                revision=temporal_row.revision,
                schema_version=temporal_row.schema_version,
                created_at=_utc_iso(temporal_row.created_at),
            )
            self.temporal_starts[start.workflow_id] = start
            self._loaded_workflow_temporal_revisions[start.id] = start.revision

        self.budget_gates = {}
        budget_rows = list((await self.session.execute(select(WorkflowBudgetGateModel))).scalars())
        for budget_row in budget_rows:
            gate = BudgetGate(
                project_id=budget_row.project_id,
                run_id=budget_row.run_id,
                node_run_id=budget_row.node_run_id,
                logical_operation=budget_row.logical_operation,
                request_fingerprint=budget_row.request_fingerprint,
                operation_kind=budget_row.operation_kind,
                batch_size=budget_row.batch_size,
                cost_status=cast(Any, budget_row.cost_status),
                estimated_cost=budget_row.estimated_cost,
                currency=budget_row.currency,
                threshold_snapshot_id=budget_row.threshold_snapshot_id,
                threshold_revision=budget_row.threshold_revision,
                status=cast(Any, budget_row.status),
                confirmation_id=budget_row.confirmation_id,
                user_uuid=budget_row.user_uuid,
                id=budget_row.id,
                revision=budget_row.revision,
                retention_policy=budget_row.retention_policy,
                retention_version=budget_row.retention_version,
                hold=budget_row.hold,
            )
            self.budget_gates[f"{gate.run_id}:{gate.logical_operation}"] = gate
            self._loaded_workflow_budget_revisions[gate.id] = gate.revision

        outbox_rows = list((await self.session.execute(select(WorkflowOutboxEventModel))).scalars())
        existing_event_ids = {
            str(item.get("eventId"))
            for item in self.outbox_events
            if isinstance(item, dict) and item.get("eventId")
        }
        for outbox_row in outbox_rows:
            self._loaded_workflow_outbox_event_ids.add(outbox_row.run_event_id)
            if outbox_row.run_event_id not in existing_event_ids:
                self.outbox_events.append(
                    {
                        "type": outbox_row.event_type,
                        "runId": outbox_row.run_id,
                        "eventId": outbox_row.run_event_id,
                        **dict(outbox_row.payload),
                    }
                )

    async def _load_asset_bible_collections(self) -> None:
        """Load relational owner facts, retaining old document rows only as migration input."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        bible_rows = list((await self.session.execute(select(AssetBibleModel))).scalars())
        if not bible_rows:
            # A pre-0014 phase_one_documents snapshot is migrated on the next successful commit.
            return

        version_rows = list(
            (
                await self.session.execute(
                    select(AssetBibleEntryVersionModel).order_by(
                        AssetBibleEntryVersionModel.entry_id,
                        AssetBibleEntryVersionModel.version_number,
                    )
                )
            ).scalars()
        )
        versions_by_entry: dict[str, list[AssetBibleVersion]] = {}
        versions_by_id: dict[str, AssetBibleVersion] = {}
        for version_row in version_rows:
            version = _asset_bible_version_from_model(version_row)
            versions_by_entry.setdefault(version.entry_id, []).append(version)
            versions_by_id[version.id] = version

        entry_rows = list((await self.session.execute(select(AssetBibleEntryModel))).scalars())
        entries: dict[str, AssetBibleEntry] = {}
        by_project: dict[str, list[AssetBibleEntry]] = {}
        for entry_row in entry_rows:
            versions = versions_by_entry.get(entry_row.id, [])
            current = versions_by_id.get(entry_row.current_version_id or "")
            entry = AssetBibleEntry(
                project_id=entry_row.project_id,
                asset_bible_id=entry_row.asset_bible_id,
                entry_type=cast(Any, entry_row.entry_type),
                id=entry_row.id,
                revision=entry_row.revision,
                current=current,
                versions=versions,
                disabled=entry_row.disabled,
                schema_version=entry_row.schema_version,
            )
            entries[entry.id] = entry
            by_project.setdefault(entry.project_id, []).append(entry)
        for values in by_project.values():
            values.sort(key=lambda item: (item.entry_type, item.id))

        self.asset_bibles_by_project = {
            bible_row.project_id: AssetBible(
                project_id=bible_row.project_id,
                id=bible_row.id,
                revision=bible_row.revision,
                schema_version=bible_row.schema_version,
                current_version_map=dict(bible_row.current_version_map),
            )
            for bible_row in bible_rows
        }
        self.asset_bible_entries = entries
        self.asset_bible_by_project = by_project

        relationship_rows = list(
            (await self.session.execute(select(AssetBibleRelationshipModel))).scalars()
        )
        self.asset_bible_relationships = [
            AssetBibleRelationship(
                relationship_row.project_id,
                relationship_row.source_entry_id,
                relationship_row.target_entry_id,
                cast(Any, relationship_row.kind),
                id=relationship_row.id,
                schema_version=relationship_row.schema_version,
            )
            for relationship_row in relationship_rows
        ]
        assignment_rows = list(
            (await self.session.execute(select(ContinuityAssignmentModel))).scalars()
        )
        self.asset_bible_assignments = [
            _continuity_assignment_from_model(assignment_row) for assignment_row in assignment_rows
        ]

        snapshot_rows = list(
            (await self.session.execute(select(ResolvedContinuitySnapshotModel))).scalars()
        )
        self.asset_bible_snapshots = {
            snapshot_row.id: ResolvedContinuitySnapshot(
                project_id=snapshot_row.project_id,
                target_id=snapshot_row.target_id,
                refs=tuple(_assignment_from_json(item) for item in snapshot_row.refs),
                revision_chain=tuple(
                    _revision_ref_from_json(item) for item in snapshot_row.revision_chain
                ),
                override_chain=tuple(
                    _assignment_from_json(item) for item in snapshot_row.override_chain
                ),
                status=cast(Any, snapshot_row.status),
                revision=snapshot_row.revision,
                id=snapshot_row.id,
                content_hash=snapshot_row.content_hash,
                target_type=cast(Any, snapshot_row.target_type),
                target_revision=snapshot_row.target_revision,
                schema_version=snapshot_row.schema_version,
            )
            for snapshot_row in snapshot_rows
        }

        impact_rows = list(
            (await self.session.execute(select(ContinuityImpactAnalysisModel))).scalars()
        )
        self.asset_bible_impacts = {
            impact_row.id: ContinuityImpactAnalysis(
                project_id=impact_row.project_id,
                entry_id=impact_row.entry_id,
                base_version_id=impact_row.base_version_id,
                candidate_payload_hash=impact_row.candidate_payload_hash,
                target_refs=tuple(
                    _impact_target_from_json(item) for item in impact_row.target_refs
                ),
                status=cast(Any, impact_row.status),
                diagnostic=impact_row.diagnostic,
                revision=impact_row.revision,
                id=impact_row.id,
                target_set_hash=impact_row.target_set_hash,
                candidate_payload=dict(impact_row.candidate_payload),
                reference_asset_version_refs=tuple(
                    _asset_bible_owner_ref(item) for item in impact_row.reference_asset_version_refs
                ),
                generation_spec_refs=tuple(
                    _asset_bible_owner_ref(item) for item in impact_row.generation_spec_refs
                ),
                schema_version=impact_row.schema_version,
            )
            for impact_row in impact_rows
        }

        task_rows = list(
            (await self.session.execute(select(ContinuityRevisionTaskModel))).scalars()
        )
        self.asset_bible_tasks = {
            task_row.id: ContinuityRevisionTask(
                project_id=task_row.project_id,
                target_id=task_row.target_id,
                entry_id=task_row.entry_id,
                status=cast(Any, task_row.status),
                id=task_row.id,
                revision=task_row.revision,
                target_revision=task_row.target_revision,
                old_version_id=task_row.old_version_id,
                new_version_id=task_row.new_version_id,
                snapshot_id=task_row.snapshot_id,
                snapshot_hash=task_row.snapshot_hash,
                reason=task_row.reason,
                correlation_id=task_row.correlation_id,
                target_type=cast(Any, task_row.target_type),
                schema_version=task_row.schema_version,
            )
            for task_row in task_rows
        }

        decision_rows = list(
            (await self.session.execute(select(AssetBibleAcceptDecisionModel))).scalars()
        )
        decisions: dict[str, object] = {}
        for decision_row in decision_rows:
            decision = AssetBibleAcceptDecision(
                decision_row.project_id,
                decision_row.entry_id,
                decision_row.analysis_id,
                decision_row.old_version_id,
                decision_row.new_version_id,
                decision_row.target_set_hash,
                decision_row.actor_uuid,
                decision_row.correlation_id,
                decision_row.fingerprint,
                id=decision_row.id,
                schema_version=decision_row.schema_version,
            )
            successor = versions_by_id.get(decision_row.new_version_id)
            if successor is None:
                raise ValueError("asset bible decision successor is missing")
            decisions[decision_row.fingerprint] = (
                decision,
                successor,
                tuple(decision_row.task_ids),
            )
        self.asset_bible_decisions = decisions

        handoff_rows = list(
            (await self.session.execute(select(AssetBibleHandoffAckModel))).scalars()
        )
        self.asset_bible_handoff_acks = {
            handoff_row.handoff_id: (
                handoff_row.fingerprint,
                AssetBibleHandoffAck(
                    handoff_id=handoff_row.handoff_id,
                    project_id=handoff_row.project_id,
                    payload_hash=handoff_row.payload_hash,
                    entry_version_refs=tuple(
                        _handoff_ref_from_json(item) for item in handoff_row.entry_version_refs
                    ),
                    correlation_id=handoff_row.correlation_id,
                    id=handoff_row.id,
                    schema_version=handoff_row.schema_version,
                ),
            )
            for handoff_row in handoff_rows
        }

        self._loaded_asset_bible_revisions = {row.id: row.revision for row in bible_rows}
        self._loaded_asset_bible_entry_revisions = {row.id: row.revision for row in entry_rows}
        self._loaded_asset_bible_task_revisions = {row.id: row.revision for row in task_rows}
        self._loaded_asset_bible_version_ids = {row.id for row in version_rows}
        self._loaded_asset_bible_relationship_ids = {row.id for row in relationship_rows}
        self._loaded_asset_bible_assignment_ids = {row.id for row in assignment_rows}
        self._loaded_asset_bible_snapshot_ids = {row.id for row in snapshot_rows}
        self._loaded_asset_bible_impact_ids = {row.id for row in impact_rows}
        self._loaded_asset_bible_decision_ids = {row.id for row in decision_rows}
        self._loaded_asset_bible_handoff_ids = {row.handoff_id for row in handoff_rows}

    async def _load_scene_collections(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        scene_rows = list((await self.session.execute(select(SceneModel))).scalars())
        shot_rows = list((await self.session.execute(select(ShotModel))).scalars())
        shots_by_scene: dict[str, list[ShotModel]] = {}
        for row in shot_rows:
            shots_by_scene.setdefault(row.scene_id, []).append(row)
        if scene_rows:
            scenes = {
                row.id: _scene_from_models(row, shots_by_scene.get(row.id, []))
                for row in scene_rows
            }
            self.scenes = scenes
            self.shots = {shot.id: shot for scene in scenes.values() for shot in scene.shots}
            self.scenes_by_episode = {}
            for scene in scenes.values():
                self.scenes_by_episode.setdefault(scene.episode_id, []).append(scene)
            for values in self.scenes_by_episode.values():
                values.sort(key=lambda item: (item.display_number, item.id))
        self._loaded_scene_revisions = {row.id: row.revision for row in scene_rows}
        self._loaded_shot_revisions = {row.id: row.revision for row in shot_rows}

        order_rows = list((await self.session.execute(select(SceneOrderState))).scalars())
        if order_rows:
            self.scene_order_revisions = {row.episode_id: row.revision for row in order_rows}
        self._loaded_scene_order_revisions = {row.episode_id: row.revision for row in order_rows}

        ack_rows = list((await self.session.execute(select(SceneShotHandoffAckModel))).scalars())
        if ack_rows:
            self.scene_handoff_acks = {
                row.handoff_id: SceneShotOwnerAck(
                    handoff_id=row.handoff_id,
                    project_id=row.project_id,
                    episode_id=row.episode_id,
                    scene_ids=tuple(row.scene_ids),
                    shot_ids=tuple(row.shot_ids),
                    payload_hash=row.payload_hash,
                    correlation_id=row.correlation_id,
                    id=row.id,
                )
                for row in ack_rows
            }
        self._loaded_scene_handoff_ids = {row.handoff_id for row in ack_rows}

    async def _persist_scene_collections(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        for scene in self.scenes.values():
            values = {
                "project_id": scene.project_id,
                "episode_id": scene.episode_id,
                "display_number": scene.display_number,
                "title": scene.title,
                "revision": scene.revision,
                "schema_version": scene.schema_version,
                "status": scene.status,
                "spec_ref": _json_value(scene.spec_ref),
                "spec_versions": _json_value(scene.spec_versions),
            }
            original = self._loaded_scene_revisions.get(scene.id)
            if original is None:
                self.session.add(SceneModel(id=scene.id, **values))
            elif scene.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(SceneModel)
                        .where(SceneModel.id == scene.id, SceneModel.revision == original)
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(SceneModel.revision).where(SceneModel.id == scene.id)
                    )
                    raise RevisionConflictError(scene.id, original, int(current or 0))

            for shot in scene.shots:
                shot_values = {
                    "scene_id": shot.scene_id,
                    "project_id": shot.project_id,
                    "episode_id": shot.episode_id,
                    "display_number": shot.display_number,
                    "revision": shot.revision,
                    "schema_version": shot.schema_version,
                    "status": shot.status,
                    "spec_ref": _json_value(shot.spec_ref),
                    "spec_versions": _json_value(shot.spec_versions),
                    "continuity_snapshot": _json_value(shot.continuity_snapshot),
                    "continuity_task_refs": _json_value(shot.continuity_task_refs),
                    "current_image": _json_value(shot.current_image),
                    "current_video": _json_value(shot.current_video),
                }
                shot_original = self._loaded_shot_revisions.get(shot.id)
                if shot_original is None:
                    self.session.add(ShotModel(id=shot.id, **shot_values))
                elif shot.revision != shot_original:
                    result = cast(
                        CursorResult[Any],
                        await self.session.execute(
                            update(ShotModel)
                            .where(ShotModel.id == shot.id, ShotModel.revision == shot_original)
                            .values(**shot_values)
                        ),
                    )
                    if result.rowcount != 1:
                        current = await self.session.scalar(
                            select(ShotModel.revision).where(ShotModel.id == shot.id)
                        )
                        raise RevisionConflictError(shot.id, shot_original, int(current or 0))

        for episode_id, revision in self.scene_order_revisions.items():
            original = self._loaded_scene_order_revisions.get(episode_id)
            if original is None:
                self.session.add(SceneOrderState(episode_id=episode_id, revision=revision))
            elif revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(SceneOrderState)
                        .where(
                            SceneOrderState.episode_id == episode_id,
                            SceneOrderState.revision == original,
                        )
                        .values(revision=revision)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(SceneOrderState.revision).where(
                            SceneOrderState.episode_id == episode_id
                        )
                    )
                    raise RevisionConflictError(episode_id, original, int(current or 0))

        for handoff_id, ack in self.scene_handoff_acks.items():
            if handoff_id in self._loaded_scene_handoff_ids:
                continue
            self.session.add(
                SceneShotHandoffAckModel(
                    id=ack.id,
                    handoff_id=ack.handoff_id,
                    project_id=ack.project_id,
                    episode_id=ack.episode_id,
                    payload_hash=ack.payload_hash,
                    correlation_id=ack.correlation_id,
                    scene_ids=list(ack.scene_ids),
                    shot_ids=list(ack.shot_ids),
                )
            )

    async def _persist_asset_bible_collections(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")

        for bible in self.asset_bibles_by_project.values():
            values = {
                "project_id": bible.project_id,
                "revision": bible.revision,
                "schema_version": bible.schema_version,
                "current_version_map": dict(bible.current_version_map),
            }
            original = self._loaded_asset_bible_revisions.get(bible.id)
            if original is None:
                self.session.add(AssetBibleModel(id=bible.id, **values))
            elif bible.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(AssetBibleModel)
                        .where(AssetBibleModel.id == bible.id, AssetBibleModel.revision == original)
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(AssetBibleModel.revision).where(AssetBibleModel.id == bible.id)
                    )
                    raise RevisionConflictError(bible.id, original, int(current or 0))

        # AssetBibleEntry has a database FK to its bible, while the owner
        # objects are intentionally persisted without ORM relationships. Flush
        # newly-created bibles before the later delete/query triggers autoflush.
        await self.session.flush()

        for entry in self.asset_bible_entries.values():
            values = {
                "asset_bible_id": entry.asset_bible_id,
                "project_id": entry.project_id,
                "entry_type": entry.entry_type,
                "revision": entry.revision,
                "schema_version": entry.schema_version,
                "disabled": entry.disabled,
                "current_version_id": entry.current.id if entry.current is not None else None,
            }
            original = self._loaded_asset_bible_entry_revisions.get(entry.id)
            if original is None:
                self.session.add(AssetBibleEntryModel(id=entry.id, **values))
            elif entry.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(AssetBibleEntryModel)
                        .where(
                            AssetBibleEntryModel.id == entry.id,
                            AssetBibleEntryModel.revision == original,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(AssetBibleEntryModel.revision).where(
                            AssetBibleEntryModel.id == entry.id
                        )
                    )
                    raise RevisionConflictError(entry.id, original, int(current or 0))
            for version in entry.versions:
                if version.id in self._loaded_asset_bible_version_ids:
                    continue
                self.session.add(
                    AssetBibleEntryVersionModel(
                        id=version.id,
                        entry_id=version.entry_id,
                        project_id=version.project_id,
                        entry_type=version.entry_type,
                        payload=dict(version.payload),
                        version_number=version.version_number,
                        actor_uuid=version.actor_uuid,
                        reference_asset_version_refs=cast(
                            Any, _json_value(list(version.reference_asset_version_refs))
                        ),
                        generation_spec_refs=cast(
                            Any, _json_value(list(version.generation_spec_refs))
                        ),
                        revision=version.revision,
                        content_hash=version.content_hash,
                        schema_version=version.schema_version,
                    )
                )

        for relationship in self.asset_bible_relationships:
            if relationship.id in self._loaded_asset_bible_relationship_ids:
                continue
            self.session.add(
                AssetBibleRelationshipModel(
                    id=relationship.id,
                    project_id=relationship.project_id,
                    source_entry_id=relationship.source_entry_id,
                    target_entry_id=relationship.target_entry_id,
                    kind=relationship.kind,
                    schema_version=relationship.schema_version,
                )
            )

        for assignment in self.asset_bible_assignments:
            if assignment.id in self._loaded_asset_bible_assignment_ids:
                continue
            self.session.add(
                ContinuityAssignmentModel(
                    id=assignment.id,
                    project_id=assignment.project_id,
                    level=assignment.level,
                    target_id=assignment.target_id,
                    entry_id=assignment.entry_id,
                    version_id=assignment.version_id,
                    version_revision=assignment.version_revision,
                    content_hash=assignment.content_hash,
                    revision=assignment.revision,
                    schema_version=assignment.schema_version,
                    scope_revision=assignment.scope_revision,
                )
            )

        for snapshot in self.asset_bible_snapshots.values():
            if snapshot.id in self._loaded_asset_bible_snapshot_ids:
                continue
            self.session.add(
                ResolvedContinuitySnapshotModel(
                    id=snapshot.id,
                    project_id=snapshot.project_id,
                    target_type=snapshot.target_type,
                    target_id=snapshot.target_id,
                    target_revision=snapshot.target_revision,
                    refs=cast(Any, _json_value(list(snapshot.refs))),
                    revision_chain=cast(Any, _json_value(list(snapshot.revision_chain))),
                    override_chain=cast(Any, _json_value(list(snapshot.override_chain))),
                    status=snapshot.status,
                    revision=snapshot.revision,
                    content_hash=snapshot.content_hash,
                    schema_version=snapshot.schema_version,
                )
            )

        for analysis in self.asset_bible_impacts.values():
            if analysis.id in self._loaded_asset_bible_impact_ids:
                continue
            self.session.add(
                ContinuityImpactAnalysisModel(
                    id=analysis.id,
                    project_id=analysis.project_id,
                    entry_id=analysis.entry_id,
                    base_version_id=analysis.base_version_id,
                    candidate_payload_hash=analysis.candidate_payload_hash,
                    target_set_hash=analysis.target_set_hash,
                    target_refs=cast(Any, _json_value(list(analysis.target_refs))),
                    candidate_payload=dict(analysis.candidate_payload),
                    reference_asset_version_refs=cast(
                        Any, _json_value(list(analysis.reference_asset_version_refs))
                    ),
                    generation_spec_refs=cast(
                        Any, _json_value(list(analysis.generation_spec_refs))
                    ),
                    status=analysis.status,
                    diagnostic=analysis.diagnostic,
                    revision=analysis.revision,
                    schema_version=analysis.schema_version,
                )
            )

        for task in self.asset_bible_tasks.values():
            values = {
                "project_id": task.project_id,
                "target_type": task.target_type,
                "target_id": task.target_id,
                "target_revision": task.target_revision,
                "entry_id": task.entry_id,
                "old_version_id": task.old_version_id,
                "new_version_id": task.new_version_id,
                "snapshot_id": task.snapshot_id,
                "snapshot_hash": task.snapshot_hash,
                "reason": task.reason,
                "correlation_id": task.correlation_id,
                "status": task.status,
                "revision": task.revision,
                "schema_version": task.schema_version,
            }
            original = self._loaded_asset_bible_task_revisions.get(task.id)
            if original is None:
                self.session.add(ContinuityRevisionTaskModel(id=task.id, **values))
            elif task.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(ContinuityRevisionTaskModel)
                        .where(
                            ContinuityRevisionTaskModel.id == task.id,
                            ContinuityRevisionTaskModel.revision == original,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(ContinuityRevisionTaskModel.revision).where(
                            ContinuityRevisionTaskModel.id == task.id
                        )
                    )
                    raise RevisionConflictError(task.id, original, int(current or 0))

        for value in self.asset_bible_decisions.values():
            decision, _version, task_ids = cast(
                tuple[AssetBibleAcceptDecision, AssetBibleVersion, tuple[str, ...]], value
            )
            if decision.id in self._loaded_asset_bible_decision_ids:
                continue
            self.session.add(
                AssetBibleAcceptDecisionModel(
                    id=decision.id,
                    project_id=decision.project_id,
                    entry_id=decision.entry_id,
                    analysis_id=decision.analysis_id,
                    old_version_id=decision.old_version_id,
                    new_version_id=decision.new_version_id,
                    target_set_hash=decision.target_set_hash,
                    actor_uuid=decision.actor_uuid,
                    correlation_id=decision.correlation_id,
                    fingerprint=decision.fingerprint,
                    task_ids=list(task_ids),
                    schema_version=decision.schema_version,
                )
            )

        for handoff_id, value in self.asset_bible_handoff_acks.items():
            if handoff_id in self._loaded_asset_bible_handoff_ids:
                continue
            fingerprint, ack = value
            self.session.add(
                AssetBibleHandoffAckModel(
                    id=ack.id,
                    handoff_id=ack.handoff_id,
                    project_id=ack.project_id,
                    payload_hash=ack.payload_hash,
                    fingerprint=fingerprint,
                    entry_version_refs=cast(Any, _json_value(list(ack.entry_version_refs))),
                    correlation_id=ack.correlation_id,
                    schema_version=ack.schema_version,
                )
            )
        await self.session.execute(
            delete(PhaseOneDocument).where(
                PhaseOneDocument.owner == "phase-one",
                PhaseOneDocument.collection.in_(_ASSET_BIBLE_DOCUMENT_COLLECTIONS),
            )
        )

    async def _persist_workflow_collections(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")

        for source in self.workflow_by_project.values():
            if source.id in self._loaded_workflow_source_ids:
                continue
            self.session.add(
                PublishedWorkflowVersionModel(
                    id=source.id,
                    revision=source.revision,
                    schema_version=source.schema_version,
                    project_id=source.project_id,
                    template_key=source.template_key,
                    version_number=source.version_number,
                    status=source.status,
                    scope_type=source.scope_type,
                    scope_ids=list(source.scope_ids),
                    definition=cast(Any, _json_value(source.definition)),
                    content_hash=source.content_hash,
                )
            )

        for binding in self.workflow_bindings.values():
            original = self._loaded_workflow_binding_revisions.get(binding.id)
            values = {
                "revision": binding.revision,
                "schema_version": binding.schema_version,
                "project_id": binding.project_id,
                "workflow_version_id": binding.workflow_version_id,
                "workflow_content_hash": binding.workflow_content_hash,
                "template_key": binding.template_key,
            }
            if original is None:
                self.session.add(ProjectDefaultWorkflowBindingModel(id=binding.id, **values))
            elif binding.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(ProjectDefaultWorkflowBindingModel)
                        .where(
                            ProjectDefaultWorkflowBindingModel.id == binding.id,
                            ProjectDefaultWorkflowBindingModel.revision == original,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(ProjectDefaultWorkflowBindingModel.revision).where(
                            ProjectDefaultWorkflowBindingModel.id == binding.id
                        )
                    )
                    raise RevisionConflictError(binding.id, original, int(current or 0))

        for run in self.workflow_runs.values():
            original = self._loaded_workflow_run_revisions.get(run.id)
            run_values = {
                "revision": run.revision,
                "schema_version": "1.0.0",
                "project_id": run.project_id,
                "workflow_version_id": run.workflow_version_id,
                "status": run.status,
                "document": {},
                "rerun_of_run_id": run.rerun_of_run_id,
                "predecessor_run_id": run.predecessor_run_id,
                "input_snapshot": cast(Any, _json_value(run.input_snapshot)),
                "selection_snapshot": cast(Any, _json_value(run.selection_snapshot)),
                "source_snapshot": cast(Any, _json_value(run.source_snapshot)),
            }
            if original is None:
                self.session.add(
                    WorkflowRunModel(
                        id=run.id,
                        created_at=datetime.fromisoformat(run.created_at),
                        updated_at=datetime.fromisoformat(run.updated_at),
                        **run_values,
                    )
                )
            elif run.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(WorkflowRunModel)
                        .where(WorkflowRunModel.id == run.id, WorkflowRunModel.revision == original)
                        .values(**run_values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(WorkflowRunModel.revision).where(WorkflowRunModel.id == run.id)
                    )
                    raise RevisionConflictError(run.id, original, int(current or 0))

            for node in run.nodes:
                node_original = self._loaded_workflow_node_revisions.get(node.id)
                node_values = {
                    "revision": node.revision,
                    "schema_version": "1.0.0",
                    "run_id": node.run_id,
                    "node_key": node.node_key,
                    "status": node.status,
                    "logical_operation": node.logical_operation,
                    "scope_refs": cast(Any, _json_value(list(node.scope_refs))),
                    "output_evidence": cast(Any, _json_value(node.output_evidence)),
                    "failure": cast(Any, _json_value(node.failure)),
                    "submission_state": node.submission_state,
                }
                if node_original is None:
                    self.session.add(WorkflowNodeRunModel(id=node.id, **node_values))
                elif node.revision != node_original:
                    result = cast(
                        CursorResult[Any],
                        await self.session.execute(
                            update(WorkflowNodeRunModel)
                            .where(
                                WorkflowNodeRunModel.id == node.id,
                                WorkflowNodeRunModel.revision == node_original,
                            )
                            .values(**node_values)
                        ),
                    )
                    if result.rowcount != 1:
                        current = await self.session.scalar(
                            select(WorkflowNodeRunModel.revision).where(
                                WorkflowNodeRunModel.id == node.id
                            )
                        )
                        raise RevisionConflictError(node.id, node_original, int(current or 0))

        for snapshot in self.run_input_snapshots.values():
            if snapshot.id in self._loaded_workflow_snapshot_ids:
                continue
            self.session.add(
                WorkflowRunInputSnapshotModel(
                    id=snapshot.id,
                    revision=snapshot.revision,
                    schema_version=snapshot.schema_version,
                    created_at=datetime.fromisoformat(snapshot.created_at),
                    run_id=snapshot.run_id,
                    project_id=snapshot.project_id,
                    workflow_version_id=snapshot.workflow_version_id,
                    workflow_content_hash=snapshot.workflow_content_hash,
                    scope_refs=cast(Any, _json_value(list(snapshot.scope_refs))),
                    owner_refs=cast(Any, _json_value(list(snapshot.owner_refs))),
                    selection_snapshot=cast(Any, _json_value(snapshot.selection_snapshot)),
                    source_snapshot=cast(Any, _json_value(snapshot.source_snapshot)),
                    node_inputs=cast(Any, _json_value(list(snapshot.node_inputs))),
                    runnable=snapshot.runnable,
                    diagnostic=snapshot.diagnostic,
                )
            )

        new_events: list[RunEvent] = []
        for events in self.run_events.values():
            for event in events:
                if event.id in self._loaded_workflow_event_ids:
                    continue
                new_events.append(event)
                self.session.add(
                    WorkflowRunEventModel(
                        id=event.id,
                        revision=event.revision,
                        schema_version="1.0.0",
                        created_at=datetime.fromisoformat(event.created_at),
                        run_id=event.run_id,
                        node_run_id=event.node_run_id,
                        sequence=event.sequence,
                        event_type=event.event_type,
                        correlation_id=event.correlation_id,
                        payload=cast(Any, _json_value(event.payload)),
                        retention_policy=event.retention_policy,
                        retention_version=event.retention_version,
                        hold=event.hold,
                    )
                )

        for key, run_id in self.workflow_run_keys.items():
            pair = ("start", key)
            if pair in self._loaded_workflow_idempotency_keys:
                continue
            self.session.add(
                WorkflowIdempotencyKeyModel(
                    key_kind="start",
                    idempotency_key=key,
                    run_id=run_id,
                    request_fingerprint=self.workflow_run_key_fingerprints[key],
                )
            )
        for key, (run_id, fingerprint) in self.workflow_signal_keys.items():
            pair = ("signal", key)
            if pair in self._loaded_workflow_idempotency_keys:
                continue
            self.session.add(
                WorkflowIdempotencyKeyModel(
                    key_kind="signal",
                    idempotency_key=key,
                    run_id=run_id,
                    request_fingerprint=fingerprint,
                )
            )

        for start in self.temporal_starts.values():
            original = self._loaded_workflow_temporal_revisions.get(start.id)
            values = {
                "revision": start.revision,
                "schema_version": start.schema_version,
                "run_id": start.run_id,
                "node_run_id": start.node_run_id,
                "logical_operation": start.logical_operation,
                "workflow_id": start.workflow_id,
                "request_fingerprint": start.request_fingerprint,
                "status": start.status,
            }
            if original is None:
                self.session.add(
                    WorkflowTemporalStartModel(
                        id=start.id,
                        created_at=datetime.fromisoformat(start.created_at),
                        **values,
                    )
                )
            elif start.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(WorkflowTemporalStartModel)
                        .where(
                            WorkflowTemporalStartModel.id == start.id,
                            WorkflowTemporalStartModel.revision == original,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(WorkflowTemporalStartModel.revision).where(
                            WorkflowTemporalStartModel.id == start.id
                        )
                    )
                    raise RevisionConflictError(start.id, original, int(current or 0))

        for gate in self.budget_gates.values():
            original = self._loaded_workflow_budget_revisions.get(gate.id)
            values = {
                "revision": gate.revision,
                "schema_version": "1.0.0",
                "project_id": gate.project_id,
                "run_id": gate.run_id,
                "node_run_id": gate.node_run_id,
                "logical_operation": gate.logical_operation,
                "request_fingerprint": gate.request_fingerprint,
                "operation_kind": gate.operation_kind,
                "batch_size": gate.batch_size,
                "cost_status": gate.cost_status,
                "estimated_cost": gate.estimated_cost,
                "currency": gate.currency,
                "threshold_snapshot_id": gate.threshold_snapshot_id,
                "threshold_revision": gate.threshold_revision,
                "status": gate.status,
                "confirmation_id": gate.confirmation_id,
                "user_uuid": gate.user_uuid,
                "retention_policy": gate.retention_policy,
                "retention_version": gate.retention_version,
                "hold": gate.hold,
            }
            if original is None:
                self.session.add(WorkflowBudgetGateModel(id=gate.id, **values))
            elif gate.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(WorkflowBudgetGateModel)
                        .where(
                            WorkflowBudgetGateModel.id == gate.id,
                            WorkflowBudgetGateModel.revision == original,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(WorkflowBudgetGateModel.revision).where(
                            WorkflowBudgetGateModel.id == gate.id
                        )
                    )
                    raise RevisionConflictError(gate.id, original, int(current or 0))

        outbox_by_event = {
            str(item.get("eventId")): item
            for item in self.outbox_events
            if isinstance(item, dict) and item.get("eventId")
        }
        for event in new_events:
            if event.id in self._loaded_workflow_outbox_event_ids:
                continue
            item = outbox_by_event.get(event.id, {})
            payload = {
                str(key): value
                for key, value in item.items()
                if key not in {"type", "runId", "eventId"}
            }
            self.session.add(
                WorkflowOutboxEventModel(
                    run_id=event.run_id,
                    run_event_id=event.id,
                    event_type=event.event_type,
                    payload=cast(Any, _json_value(payload)),
                    status="pending",
                )
            )

        await self.session.execute(
            delete(PhaseOneDocument).where(
                PhaseOneDocument.owner == "phase-one",
                PhaseOneDocument.collection.in_(_WORKFLOW_DOCUMENT_COLLECTIONS),
            )
        )

    async def _persist_catalog_collections(self) -> None:
        """Persist catalog owner facts in their normalized tables only."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")

        removed_models = set(self._loaded_catalog_model_revisions) - set(self.models)
        if removed_models:
            await self.session.execute(
                delete(CatalogModel).where(CatalogModel.id.in_(removed_models))
            )

        for provider in self.providers.values():
            values = {
                "name": provider.name,
                "adapter_key": provider.adapter_key,
                "enabled": provider.enabled,
                "revision": provider.revision,
                "schema_version": "1.0.0",
                "approval": provider.approval,
                "feature_gate": provider.feature_gate,
                "adapter_installed": provider.adapter_installed,
            }
            original = self._loaded_catalog_provider_revisions.get(provider.id)
            if original is None:
                self.session.add(ProviderModel(id=provider.id, **values))
            elif provider.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(ProviderModel)
                        .where(ProviderModel.id == provider.id, ProviderModel.revision == original)
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(ProviderModel.revision).where(ProviderModel.id == provider.id)
                    )
                    raise RevisionConflictError(provider.id, original, int(current or 0))

        for profile in self.profiles.values():
            values = {
                "provider_id": profile.provider_id,
                "name": profile.name,
                "enabled": profile.enabled,
                "revision": profile.revision,
                "schema_version": "1.0.0",
                "adapter_identity": profile.adapter_identity,
                "explicit_live_opt_in": profile.explicit_live_opt_in,
                "credential_status": profile.credential_status,
            }
            original = self._loaded_catalog_profile_revisions.get(profile.id)
            if original is None:
                self.session.add(ProviderProfileModel(id=profile.id, **values))
            elif profile.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(ProviderProfileModel)
                        .where(
                            ProviderProfileModel.id == profile.id,
                            ProviderProfileModel.revision == original,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(ProviderProfileModel.revision).where(
                            ProviderProfileModel.id == profile.id
                        )
                    )
                    raise RevisionConflictError(profile.id, original, int(current or 0))

            for operation, policy in profile.operation_policies.items():
                normalized = {
                    "max_concurrency": int(str(policy.get("maxConcurrency", 1))),
                    "rate_limit": int(str(policy.get("rateLimit", 60))),
                    "rate_window_seconds": int(str(policy.get("rateWindowSeconds", 60))),
                }
                key = (profile.id, operation)
                loaded = self._loaded_catalog_policy_rows.get(key)
                if loaded is None:
                    self.session.add(
                        ProviderOperationPolicyModel(
                            profile_id=profile.id,
                            operation=operation,
                            revision=1,
                            schema_version="1.0.0",
                            **normalized,
                        )
                    )
                elif loaded[2] != {
                    "maxConcurrency": normalized["max_concurrency"],
                    "rateLimit": normalized["rate_limit"],
                    "rateWindowSeconds": normalized["rate_window_seconds"],
                }:
                    policy_id, original_revision, _old = loaded
                    result = cast(
                        CursorResult[Any],
                        await self.session.execute(
                            update(ProviderOperationPolicyModel)
                            .where(
                                ProviderOperationPolicyModel.id == policy_id,
                                ProviderOperationPolicyModel.revision == original_revision,
                            )
                            .values(revision=original_revision + 1, **normalized)
                        ),
                    )
                    if result.rowcount != 1:
                        current = await self.session.scalar(
                            select(ProviderOperationPolicyModel.revision).where(
                                ProviderOperationPolicyModel.id == policy_id
                            )
                        )
                        raise RevisionConflictError(policy_id, original_revision, int(current or 0))

            for snapshot in profile.capability_snapshots.values():
                if snapshot.id in self._loaded_catalog_snapshot_ids:
                    continue
                self.session.add(
                    CapabilitySnapshotModel(
                        id=snapshot.id,
                        revision=snapshot.revision,
                        schema_version="1.0.0",
                        provider_id=snapshot.provider_id,
                        profile_id=snapshot.profile_id,
                        model_id=snapshot.model_id,
                        operation=snapshot.operation,
                        runnable=snapshot.runnable,
                        capabilities=list(snapshot.capabilities),
                        captured_at=snapshot.captured_at,
                        retention_policy=snapshot.retention_policy,
                        retention_version=snapshot.retention_version,
                        hold=snapshot.hold,
                    )
                )
            for quota in profile.quota_snapshots.values():
                if quota.id in self._loaded_catalog_quota_ids:
                    continue
                self.session.add(
                    ProviderQuotaSnapshotModel(
                        id=quota.id,
                        revision=quota.revision,
                        schema_version="1.0.0",
                        provider_id=quota.provider_id,
                        profile_id=quota.profile_id,
                        operation=quota.operation,
                        status=quota.status,
                        remaining=quota.remaining,
                        reset_at=quota.reset_at,
                        source=quota.source,
                        captured_at=quota.captured_at,
                    )
                )

        for model in self.models.values():
            values = {
                "profile_id": model.profile_id,
                "model_key": model.model_key,
                "enabled": model.enabled,
                "revision": model.revision,
                "schema_version": "1.0.0",
            }
            original = self._loaded_catalog_model_revisions.get(model.id)
            if original is None:
                self.session.add(CatalogModel(id=model.id, **values))
            elif model.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(CatalogModel)
                        .where(CatalogModel.id == model.id, CatalogModel.revision == original)
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(CatalogModel.revision).where(CatalogModel.id == model.id)
                    )
                    raise RevisionConflictError(model.id, original, int(current or 0))

        for skill in self.skills:
            if skill.id in self._loaded_catalog_skill_ids:
                continue
            self.session.add(
                SkillRevisionModel(
                    id=skill.id,
                    revision=skill.revision,
                    schema_version=skill.schema_version,
                    name=skill.name,
                    version=skill.version,
                    provenance=skill.provenance,
                    approval=skill.approval,
                    enabled=skill.enabled,
                    source_identity=skill.source_identity,
                    digest=skill.digest,
                    source_type=skill.source_type,
                    license_status=skill.license_status,
                    capabilities=list(skill.capabilities),
                )
            )

        for call in self.provider_calls.values():
            if call.id in self._loaded_catalog_call_ids:
                original_call_revision = self._loaded_catalog_call_revisions.get(call.id)
                if original_call_revision is None or call.revision == original_call_revision:
                    continue
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(ProviderCallModel)
                        .where(
                            ProviderCallModel.id == call.id,
                            ProviderCallModel.revision == original_call_revision,
                        )
                        .values(
                            revision=call.revision,
                            status=call.status,
                            provider_request_id=call.provider_request_id,
                            native_usage=cast(Any, _json_value(call.native_usage)),
                            failure_code=call.failure_code,
                        )
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(ProviderCallModel.revision).where(ProviderCallModel.id == call.id)
                    )
                    raise RevisionConflictError(call.id, original_call_revision, int(current or 0))
                continue
            self.session.add(
                ProviderCallModel(
                    id=call.id,
                    revision=call.revision,
                    schema_version="1.0.0",
                    project_id=call.project_id,
                    run_id=call.run_id,
                    node_run_id=call.node_run_id,
                    logical_operation=call.logical_operation,
                    operation=call.operation,
                    provider_id=call.provider_id,
                    profile_id=call.profile_id,
                    model_id=call.model_id,
                    capability_snapshot_id=call.capability_snapshot_id,
                    request_fingerprint=call.request_fingerprint,
                    status=call.status,
                    cost_status=call.cost_status,
                    cost_value=call.cost_value,
                    cost_currency=call.cost_currency,
                    cost_source=call.cost_source,
                    provider_request_id=call.provider_request_id,
                    native_usage=cast(Any, _json_value(call.native_usage)),
                    failure_code=call.failure_code,
                    retention_policy=call.retention_policy,
                    retention_version=call.retention_version,
                    hold=call.hold,
                )
            )

        for confirmation in self.cost_confirmations.values():
            if confirmation.id in self._loaded_catalog_confirmation_ids:
                continue
            self.session.add(
                CostConfirmationModel(
                    id=confirmation.id,
                    revision=1,
                    schema_version="1.0.0",
                    project_id=confirmation.project_id,
                    run_id=confirmation.run_id,
                    logical_operation=confirmation.logical_operation,
                    request_fingerprint=confirmation.request_fingerprint,
                    user_uuid=confirmation.user_uuid,
                    threshold_snapshot_id=confirmation.threshold_snapshot_id,
                    threshold_revision=confirmation.threshold_revision,
                    estimated_cost=confirmation.estimated_cost,
                    cost_status=confirmation.cost_status,
                    operation_kind=confirmation.operation_kind,
                    batch_size=confirmation.batch_size,
                    retention_policy=confirmation.retention_policy,
                    retention_version=confirmation.retention_version,
                    hold=confirmation.hold,
                )
            )

        for candidate in self.model_sync_candidates.values():
            if candidate.id not in self._loaded_catalog_sync_ids:
                self.session.add(
                    ModelSyncCandidateModel(
                        id=candidate.id,
                        revision=candidate.revision,
                        schema_version="1.0.0",
                        profile_id=candidate.profile_id,
                        remote_models=list(candidate.remote_models),
                        added=list(candidate.added),
                        removed=list(candidate.removed),
                        changed=list(candidate.changed),
                        status=candidate.status,
                    )
                )
            else:
                original = self._loaded_catalog_sync_revisions.get(candidate.id)
                if original is not None and candidate.revision != original:
                    result = cast(
                        CursorResult[Any],
                        await self.session.execute(
                            update(ModelSyncCandidateModel)
                            .where(
                                ModelSyncCandidateModel.id == candidate.id,
                                ModelSyncCandidateModel.revision == original,
                            )
                            .values(revision=candidate.revision, status=candidate.status)
                        ),
                    )
                    if result.rowcount != 1:
                        current = await self.session.scalar(
                            select(ModelSyncCandidateModel.revision).where(
                                ModelSyncCandidateModel.id == candidate.id
                            )
                        )
                        raise RevisionConflictError(candidate.id, original, int(current or 0))

        for audit in self.skill_access_audits:
            if audit.id in self._loaded_catalog_audit_ids:
                continue
            self.session.add(
                SkillAccessAuditModel(
                    id=audit.id,
                    revision=1,
                    schema_version="1.0.0",
                    skill_revision_id=audit.skill_revision_id,
                    run_id=audit.run_id,
                    node_run_id=audit.node_run_id,
                    access=audit.access,
                    allowed=audit.allowed,
                    reason=audit.reason,
                )
            )

        credential_profiles = {
            profile_id: envelope for profile_id, envelope in self.credential_envelopes.items()
        }
        for profile_id, envelope in credential_profiles.items():
            credential_profile = self.profiles.get(profile_id)
            if credential_profile is None:
                continue
            credential_id = envelope.credential_id
            values = {
                "provider_id": credential_profile.provider_id,
                "profile_id": profile_id,
                "credential_id": credential_id,
                "algorithm": envelope.algorithm,
                "key_version": envelope.key_version,
                "masked_prefix": envelope.masked_prefix,
                "last4": envelope.last4,
                "ciphertext": envelope.ciphertext,
                "nonce": envelope.nonce,
                "tag": envelope.auth_tag,
                "aad_version": envelope.aad_version,
            }
            existing_id = self._loaded_catalog_credential_by_profile.get(profile_id)
            if existing_id is None:
                self.session.add(CredentialMetadata(id=new_id(), **values))
            else:
                await self.session.execute(
                    update(CredentialMetadata)
                    .where(CredentialMetadata.id == existing_id)
                    .values(**values)
                )

        await self.session.execute(
            delete(PhaseOneDocument).where(
                PhaseOneDocument.owner == "phase-one",
                PhaseOneDocument.collection.in_(
                    {
                        "providers",
                        "profiles",
                        "models",
                        "skills",
                        "provider_calls",
                        "provider_call_keys",
                        "cost_confirmations",
                        "credential_envelopes",
                        "model_sync_candidates",
                        "skill_access_audits",
                        "catalog_overrides",
                        "usage_audits",
                    }
                ),
            )
        )

    async def _persist_phase_one_collections(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        for collection in (
            "audit_events",
            "outbox_events",
            "text_review_batches",
            "text_candidates",
            "text_handoffs",
            "text_handoff_acks",
            "skill_route_decisions",
            "skill_route_selections",
            "source_materials",
            "export_jobs",
            "export_batches",
            "image_generation_candidates",
        ):
            value = getattr(self, collection)
            blob = _encode_phase_one(value)
            existing = await self.session.scalar(
                select(PhaseOneDocument).where(
                    PhaseOneDocument.owner == "phase-one",
                    PhaseOneDocument.collection == collection,
                )
            )
            loaded_revision = self._loaded_phase_one_revisions.get(collection)
            loaded_blob = self._loaded_phase_one_payloads.get(collection)
            if existing is None:
                if loaded_revision is not None:
                    raise RevisionConflictError(collection, loaded_revision, 0)
                if not value:
                    continue
                self.session.add(
                    PhaseOneDocument(
                        owner="phase-one",
                        collection=collection,
                        revision=0,
                        document={"payload": blob},
                    )
                )
                continue
            if loaded_revision is None:
                if not value:
                    continue
                raise RevisionConflictError(collection, 0, existing.revision)
            if loaded_blob == blob:
                continue
            result = cast(
                CursorResult[Any],
                await self.session.execute(
                    update(PhaseOneDocument)
                    .where(
                        PhaseOneDocument.owner == "phase-one",
                        PhaseOneDocument.collection == collection,
                        PhaseOneDocument.revision == loaded_revision,
                    )
                    .values(revision=loaded_revision + 1, document={"payload": blob})
                ),
            )
            if result.rowcount != 1:
                current = await self.session.scalar(
                    select(PhaseOneDocument.revision).where(
                        PhaseOneDocument.owner == "phase-one",
                        PhaseOneDocument.collection == collection,
                    )
                )
                raise RevisionConflictError(collection, loaded_revision, int(current or 0))
        await self.session.execute(
            delete(PhaseOneDocument).where(
                PhaseOneDocument.owner == "phase-one",
                PhaseOneDocument.collection.in_(
                    {
                        "conversations",
                        "storage_profiles",
                        "asset_reservations",
                        "timeline_cuts",
                        "timeline_versions",
                        "export_jobs",
                        "export_batches",
                    }
                ),
            )
        )

    async def _persist_asset_reservations(self) -> None:
        """Persist recoverable reservation state with exact revision CAS."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        for reservation in self.asset_reservations.values():
            values = {
                "project_id": reservation.project_id,
                "asset_id": reservation.asset_id,
                "operation_key": reservation.operation_key,
                "fingerprint": reservation.fingerprint,
                "status": reservation.status,
                "revision": reservation.revision,
                "registered_version_id": reservation.registered_version_id,
                "expected_asset_revision": reservation.expected_asset_revision,
                "declared_kind": reservation.declared_kind,
                "declared_mime_type": reservation.declared_mime_type,
                "declared_size_bytes": reservation.declared_size_bytes,
                "declared_checksum": reservation.declared_checksum,
                "storage_profile_id": reservation.storage_profile_id,
                "storage_profile_revision": reservation.storage_profile_revision,
                "storage_profile_snapshot_hash": reservation.storage_profile_snapshot_hash,
                "upload_key": reservation.upload_key,
                "diagnostic": reservation.diagnostic,
                "schema_version": reservation.schema_version,
            }
            original = self._loaded_asset_reservation_revisions.get(reservation.id)
            if original is None:
                self.session.add(AssetVersionReservationModel(id=reservation.id, **values))
            elif reservation.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(AssetVersionReservationModel)
                        .where(
                            AssetVersionReservationModel.id == reservation.id,
                            AssetVersionReservationModel.revision == original,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(AssetVersionReservationModel.revision).where(
                            AssetVersionReservationModel.id == reservation.id
                        )
                    )
                    raise AssetVersionConflictError(reservation.id, int(current or 0))
        await self.session.execute(
            delete(PhaseOneDocument).where(
                PhaseOneDocument.owner == "phase-one",
                PhaseOneDocument.collection == "asset_reservations",
            )
        )

    async def _persist_timeline_collections(self) -> None:
        """Persist one current Cut per Episode plus immutable named versions."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        for cut in self.timeline_cuts.values():
            values = {
                "project_id": cut.project_id,
                "episode_id": cut.episode_id,
                "revision": cut.revision,
                "schema_version": cut.schema_version,
                "payload": {"encoded": _encode_phase_one(cut)},
            }
            original = self._loaded_timeline_cut_revisions.get(cut.id)
            if original is None:
                self.session.add(TimelineCurrentCutModel(id=cut.id, **values))
            elif original != cut.revision:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(TimelineCurrentCutModel)
                        .where(
                            TimelineCurrentCutModel.id == cut.id,
                            TimelineCurrentCutModel.revision == original,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(TimelineCurrentCutModel.revision).where(
                            TimelineCurrentCutModel.id == cut.id
                        )
                    )
                    raise RevisionConflictError(cut.id, original, int(current or 0))
            if original == cut.revision:
                continue
            await self.session.execute(
                delete(TimelineClipModel).where(TimelineClipModel.cut_id == cut.id)
            )
            await self.session.execute(
                delete(TimelineSoundCueModel).where(TimelineSoundCueModel.cut_id == cut.id)
            )
            await self.session.execute(
                delete(TimelineCaptionModel).where(TimelineCaptionModel.cut_id == cut.id)
            )
            for position, clip in enumerate(cut.clips):
                self.session.add(
                    TimelineClipModel(
                        id=str(clip["id"]),
                        cut_id=cut.id,
                        position=position,
                        asset_version_id=str(clip["assetVersionId"]),
                        asset_version_revision=int(cast(int, clip["assetVersionRevision"])),
                        asset_version_hash=str(clip["assetVersionHash"]),
                        derivative_fingerprint=str(clip["derivativeFingerprint"]),
                        source_in_frame=int(cast(int, clip["inFrame"])),
                        duration_frames=int(cast(int, clip["durationFrames"])),
                        timeline_start_frame=int(cast(int, clip["timelineStart"])),
                        payload=dict(clip),
                    )
                )
            for position, cue in enumerate(cut.cues):
                self.session.add(
                    TimelineSoundCueModel(
                        id=cue.id,
                        cut_id=cut.id,
                        position=position,
                        track=cue.track,
                        asset_version_id=cue.asset_version_id,
                        start_frame=cue.start_frame,
                        duration_frames=cue.duration_frames,
                        priority=cue.priority,
                        payload={"encoded": _encode_phase_one(cue)},
                    )
                )
            for position, caption in enumerate(cut.captions):
                self.session.add(
                    TimelineCaptionModel(
                        id=str(caption["id"]),
                        cut_id=cut.id,
                        position=position,
                        start_frame=int(cast(int, caption["startFrame"])),
                        end_frame=int(cast(int, caption["endFrame"])),
                        text=str(caption["text"]),
                    )
                )

        for version in self.timeline_versions.values():
            if version.id in self._loaded_timeline_version_ids:
                continue
            source_cut_id = str(version.cut_snapshot.get("cutId", ""))
            self.session.add(
                TimelineVersionModel(
                    id=version.id,
                    project_id=version.project_id,
                    episode_id=version.episode_id,
                    source_cut_id=source_cut_id,
                    source_cut_revision=version.source_cut_revision,
                    revision=version.revision,
                    schema_version=version.schema_version,
                    name=version.name,
                    timeline_fingerprint=str(version.cut_snapshot.get("timelineFingerprint", "")),
                    snapshot={"encoded": _encode_phase_one(version)},
                )
            )

    async def _persist_export_collections(self) -> None:
        """Persist batch aggregates and normalized member/job/artifact projections."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        for batch in self.export_batches.values():
            values = {
                "project_id": batch.project_id,
                "revision": batch.revision,
                "schema_version": batch.schema_version,
                "export_profile": batch.export_profile,
                "idempotency_key": batch.idempotency_key,
                "status": batch.status,
                "payload": {"encoded": _encode_phase_one(batch)},
            }
            original_batch_revision = self._loaded_export_batch_revisions.get(batch.id)
            if original_batch_revision is None:
                self.session.add(EpisodeExportBatchModel(id=batch.id, **values))
                for position, selection in enumerate(batch.selections):
                    self.session.add(
                        EpisodeExportMemberModel(
                            batch_id=batch.id,
                            position=position,
                            episode_id=selection.episode_id,
                            timeline_version_id=selection.timeline_version_id,
                            timeline_version_revision=selection.timeline_version_revision,
                            output_base_name=selection.output_base_name,
                        )
                    )

            else:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(EpisodeExportBatchModel)
                        .where(
                            EpisodeExportBatchModel.id == batch.id,
                            EpisodeExportBatchModel.revision == original_batch_revision,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(EpisodeExportBatchModel.revision).where(
                            EpisodeExportBatchModel.id == batch.id
                        )
                    )
                    raise RevisionConflictError(
                        batch.id, original_batch_revision, int(current or 0)
                    )

            for job in batch.jobs:
                job_values = {
                    "batch_id": batch.id,
                    "project_id": job.project_id,
                    "episode_id": job.episode_id,
                    "timeline_version_id": job.timeline_version_id,
                    "revision": job.revision,
                    "status": job.status,
                    "packaging_phase": job.packaging_phase,
                    "logical_operation": job.logical_operation,
                    "render_plan_hash": job.render_plan_hash,
                    "renderer_diagnostic": job.renderer_diagnostic,
                    "execution_snapshot": cast(
                        Any, _json_value(getattr(job, "execution_snapshot", None) or {})
                    ),
                    "payload": {"encoded": _encode_phase_one(job)},
                }
                original_job_revision = self._loaded_export_job_revisions.get(job.id)
                if original_job_revision is None:
                    self.session.add(ExportJobModel(id=job.id, **job_values))
                else:
                    result = cast(
                        CursorResult[Any],
                        await self.session.execute(
                            update(ExportJobModel)
                            .where(
                                ExportJobModel.id == job.id,
                                ExportJobModel.revision == original_job_revision,
                            )
                            .values(**job_values)
                        ),
                    )
                    if result.rowcount != 1:
                        current = await self.session.scalar(
                            select(ExportJobModel.revision).where(ExportJobModel.id == job.id)
                        )
                        raise RevisionConflictError(
                            job.id, original_job_revision, int(current or 0)
                        )

                for artifact in job.artifacts:
                    artifact_values = {
                        "export_job_id": job.id,
                        "artifact_type": artifact.artifact_type,
                        "status": artifact.status,
                        "size_bytes": artifact.size_bytes,
                        "checksum": artifact.checksum,
                        "mime_type": artifact.mime_type,
                        "payload": {"encoded": _encode_phase_one(artifact)},
                    }
                    if artifact.id not in self._loaded_export_artifact_ids:
                        self.session.add(ExportArtifactModel(id=artifact.id, **artifact_values))
                    else:
                        await self.session.execute(
                            update(ExportArtifactModel)
                            .where(ExportArtifactModel.id == artifact.id)
                            .values(**artifact_values)
                        )
                await self.session.execute(
                    delete(ExportDiagnosticTargetModel).where(
                        ExportDiagnosticTargetModel.export_job_id == job.id
                    )
                )
                for diagnostic in job.diagnostics:
                    self.session.add(
                        ExportDiagnosticTargetModel(
                            id=diagnostic.id,
                            export_job_id=job.id,
                            target_type=diagnostic.target_type,
                            owner_id=diagnostic.owner_id,
                            owner_revision=diagnostic.owner_revision,
                            field_path=diagnostic.field_path,
                            route_token=diagnostic.route_token,
                            code=diagnostic.code,
                            payload={"encoded": _encode_phase_one(diagnostic)},
                        )
                    )

        for event in self.export_dispatch_outbox.values():
            values = {
                "project_id": event.project_id,
                "batch_id": event.batch_id,
                "job_id": event.job_id,
                "logical_operation": event.logical_operation,
                "workflow_id": event.workflow_id,
                "status": event.status,
                "attempts": event.attempts,
                "last_error": event.last_error,
                "dispatched_at": (
                    datetime.fromisoformat(event.dispatched_at)
                    if event.dispatched_at is not None
                    else None
                ),
                "revision": event.revision,
                "schema_version": event.schema_version,
                "payload": {"encoded": _encode_phase_one(event)},
            }
            original = self._loaded_export_dispatch_revisions.get(event.id)
            if original is None:
                self.session.add(ExportDispatchOutboxModel(id=event.id, **values))
                continue
            result = cast(
                CursorResult[Any],
                await self.session.execute(
                    update(ExportDispatchOutboxModel)
                    .where(
                        ExportDispatchOutboxModel.id == event.id,
                        ExportDispatchOutboxModel.revision == original,
                    )
                    .values(**values)
                ),
            )
            if result.rowcount != 1:
                current = await self.session.scalar(
                    select(ExportDispatchOutboxModel.revision).where(
                        ExportDispatchOutboxModel.id == event.id
                    )
                )
                raise RevisionConflictError(event.id, original, int(current or 0))

    async def _persist_media_collections(self) -> None:
        """Append verified inspection/derivative/preview records to their owner tables."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        for inspection in self.media_inspections.values():
            if inspection.id in self._loaded_media_inspection_ids:
                continue
            self.session.add(
                MediaInspectionModel(
                    id=inspection.id,
                    project_id=inspection.project_id,
                    asset_version_id=inspection.asset_version_id,
                    asset_version_revision=inspection.asset_version_revision,
                    source_hash=inspection.source_hash,
                    status=inspection.status,
                    revision=inspection.revision,
                    payload={"encoded": _encode_phase_one(inspection)},
                )
            )
        for derivative in self.media_derivatives.values():
            if derivative.id in self._loaded_media_derivative_ids:
                continue
            self.session.add(
                MediaDerivativeModel(
                    id=derivative.id,
                    inspection_id=derivative.inspection_id,
                    kind=derivative.kind,
                    status=derivative.status,
                    fingerprint=derivative.source_fingerprint,
                    payload={"encoded": _encode_phase_one(derivative)},
                )
            )
        for preview in self.preview_artifacts.values():
            if preview.id in self._loaded_preview_artifact_ids:
                continue
            self.session.add(
                TimelinePreviewArtifactModel(
                    id=preview.id,
                    cut_id=preview.cut_id,
                    cut_revision=preview.cut_revision,
                    timeline_fingerprint=preview.timeline_fingerprint,
                    render_plan_hash=preview.render_plan_hash,
                    status=preview.status,
                    payload={"encoded": _encode_phase_one(preview)},
                )
            )

    async def _persist_video_collections(self) -> None:
        """Persist operation CAS and append-only candidate facts independently of legacy JSON."""
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        for operation in self.video_operations.values():
            values = {
                "revision": operation.revision,
                "schema_version": "1.0.0",
                "project_id": operation.project_id,
                "episode_id": operation.episode_id,
                "target_id": operation.target_id,
                "asset_id": operation.asset_id,
                "run_id": operation.run_id,
                "logical_operation": operation.logical_operation,
                "provider_id": operation.provider_id,
                "profile_id": operation.profile_id,
                "model_id": operation.model_id,
                "capability_snapshot_id": operation.capability_snapshot_id,
                "source_asset_version_id": operation.source_asset_version_id,
                "source_asset_version_revision": operation.source_asset_version_revision,
                "source_asset_version_hash": operation.source_asset_version_hash,
                "source_candidate_id": operation.source_candidate_id,
                "source_provenance": operation.source_provenance,
                "shot_spec_id": operation.shot_spec_id,
                "shot_spec_revision": operation.shot_spec_revision,
                "shot_spec_hash": operation.shot_spec_hash,
                "duration_seconds": operation.duration_seconds,
                "aspect_ratio": operation.aspect_ratio,
                "status": operation.status,
                "provider_request_id": operation.provider_request_id,
                "cancel_requested": operation.cancel_requested,
                "observation_fingerprints": list(operation.observation_fingerprints),
                "retention_policy": "long-term-audit",
                "retention_version": "1",
                "hold": False,
            }
            original = self._loaded_video_operation_revisions.get(operation.id)
            if original is None:
                self.session.add(VideoOperationModel(id=operation.id, **values))
            elif operation.revision != original or operation.provider_request_id:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(VideoOperationModel)
                        .where(
                            VideoOperationModel.id == operation.id,
                            VideoOperationModel.revision == original,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(VideoOperationModel.revision).where(
                            VideoOperationModel.id == operation.id
                        )
                    )
                    raise RevisionConflictError(operation.id, original, int(current or 0))

        for candidate in self.video_take_candidates.values():
            values = {
                "revision": candidate.revision,
                "schema_version": "1.0.0",
                "project_id": candidate.project_id,
                "episode_id": candidate.episode_id,
                "target_id": candidate.target_id,
                "run_id": candidate.run_id,
                "logical_operation": candidate.logical_operation,
                "source_asset_version_id": candidate.source_asset_version_id,
                "source_asset_version_revision": candidate.source_asset_version_revision,
                "source_asset_version_hash": candidate.source_asset_version_hash,
                "source_candidate_id": candidate.source_candidate_id,
                "source_provenance": candidate.source_provenance,
                "shot_spec_id": candidate.shot_spec_id,
                "shot_spec_revision": candidate.shot_spec_revision,
                "shot_spec_hash": candidate.shot_spec_hash,
                "duration_seconds": candidate.duration_seconds,
                "aspect_ratio": candidate.aspect_ratio,
                "asset_version_id": candidate.asset_version_id,
                "asset_version_revision": candidate.asset_version_revision,
                "asset_version_hash": candidate.asset_version_hash,
                "provider_request_id": candidate.provider_request_id,
                "status": candidate.status,
                "retention_policy": "long-term-audit",
                "retention_version": "1",
                "hold": False,
            }
            original = self._loaded_video_candidate_revisions.get(candidate.id)
            if original is None:
                self.session.add(VideoTakeCandidateModel(id=candidate.id, **values))
            elif candidate.revision != original:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(VideoTakeCandidateModel)
                        .where(
                            VideoTakeCandidateModel.id == candidate.id,
                            VideoTakeCandidateModel.revision == original,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    current = await self.session.scalar(
                        select(VideoTakeCandidateModel.revision).where(
                            VideoTakeCandidateModel.id == candidate.id
                        )
                    )
                    raise RevisionConflictError(candidate.id, original, int(current or 0))

    async def _persist_edit_collections(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        session = self.session

        async def save_model(model_type: Any, value: Any, project_id: str, payload: object) -> None:
            encoded = _encode_phase_one(payload)
            values = {
                "revision": getattr(value, "revision", 1),
                "schema_version": getattr(value, "schema_version", "1.0.0"),
                "payload": {"encoded": encoded},
            }
            if model_type in {
                AssetEditSessionModel,
                AssetEditPlanModel,
                AssetEditCandidateModel,
            }:
                values["project_id"] = project_id
            if model_type is AssetEditSessionModel:
                values.update({"episode_id": value.episode_id, "status": value.status})
            elif model_type is AssetEditPlanModel:
                values.update({"episode_id": value.episode_id, "status": value.status})
            elif model_type is AssetEditCandidateModel:
                values.update({"plan_id": value.plan_id, "status": value.status})
            elif model_type is AssetEditExecutionModel:
                values.update(
                    {
                        "plan_id": value.plan_id,
                        "run_id": value.run_id,
                        "node_run_id": value.node_run_id,
                        "logical_operation": value.logical_operation,
                        "status": value.status,
                    }
                )
            elif model_type is AcceptDecisionModel:
                values = {
                    "schema_version": "1.0.0",
                    "candidate_id": value.candidate_id,
                    "action": value.action,
                    "payload": {"encoded": encoded},
                }
            elif model_type is EditImpactModel:
                values = {
                    "schema_version": "1.0.0",
                    "plan_id": value.plan_id,
                    "status": value.status,
                    "payload": {"encoded": encoded},
                }
            existing = await session.get(model_type, value.id)
            if existing is None:
                session.add(model_type(id=value.id, **values))
            else:
                original = self._loaded_edit_revisions.get(value.id)
                if original is not None and getattr(value, "revision", original) != original:
                    result = cast(
                        CursorResult[Any],
                        await session.execute(
                            update(model_type)
                            .where(model_type.id == value.id, model_type.revision == original)
                            .values(**values)
                        ),
                    )
                    if result.rowcount != 1:
                        current = await session.scalar(
                            select(model_type.revision).where(model_type.id == value.id)
                        )
                        raise RevisionConflictError(value.id, original, int(current or 0))
                else:
                    existing.payload = {"encoded": encoded}
                    if hasattr(existing, "revision"):
                        existing.revision = getattr(value, "revision", existing.revision)
                    if hasattr(existing, "status") and hasattr(value, "status"):
                        existing.status = value.status

        for value in cast(Any, self.asset_edit_sessions.values()):
            await save_model(AssetEditSessionModel, value, value.project_id, value)
        for value in cast(Any, self.asset_edit_plans.values()):
            await save_model(AssetEditPlanModel, value, value.project_id, value)
        for value in cast(Any, self.asset_edit_executions.values()):
            plan = self.asset_edit_plans.get(value.plan_id)
            await save_model(AssetEditExecutionModel, value, getattr(plan, "project_id", ""), value)
        for value in cast(Any, self.asset_edit_candidates.values()):
            await save_model(AssetEditCandidateModel, value, value.project_id, value)
        for value in cast(Any, self.accept_decisions.values()):
            await save_model(AcceptDecisionModel, value, "", value)
        for value in cast(Any, self.edit_impacts.values()):
            await save_model(EditImpactModel, value, "", value)
        for conversation in cast(Any, self.conversations.values()):
            existing = await session.get(AssetEditConversationModel, conversation.id)
            values = {
                "project_id": conversation.project_id,
                "episode_id": conversation.episode_id,
                "revision": conversation.revision,
                "schema_version": "1.0.0",
            }
            if existing is None:
                session.add(AssetEditConversationModel(id=conversation.id, **values))
            else:
                existing.revision = conversation.revision
            for message in conversation.messages:
                if await session.get(AssetEditMessageModel, message.id) is None:
                    session.add(
                        AssetEditMessageModel(
                            id=message.id,
                            session_id=message.session_id,
                            sequence=message.sequence,
                            role=message.role,
                            content_hash=message.content_hash,
                            status=message.status,
                            correlation_id=message.correlation_id,
                        )
                    )
            for turn in conversation.turns:
                existing_turn = await session.get(AssetEditTurnModel, turn.id)
                values = {
                    "session_id": turn.session_id,
                    "sequence": turn.sequence,
                    "user_message_id": turn.user_message_id,
                    "agent_message_id": turn.agent_message_id,
                    "status": turn.status,
                    "revision": turn.revision,
                }
                if existing_turn is None:
                    session.add(AssetEditTurnModel(id=turn.id, **values))
                else:
                    existing_turn.agent_message_id = turn.agent_message_id
                    existing_turn.status = turn.status
                    existing_turn.revision = turn.revision

    async def _persist_storage_profiles(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        for profile in cast(Any, self.storage_profiles.values()):
            values = {
                "project_id": profile.project_id,
                "revision": profile.revision,
                "schema_version": "1.0.0",
                "name": profile.name,
                "adapter_key": profile.adapter_key,
                "endpoint": profile.endpoint,
                "bucket": profile.bucket,
                "region": profile.region,
                "private_bucket": profile.private_bucket,
                "enabled": profile.enabled,
                "bucket_binding_id": profile.bucket_binding_id,
                "credential_status": profile.credential_status,
                "credential_ref": profile.credential_ref,
                "connect_timeout_ms": profile.connect_timeout_ms,
                "read_timeout_ms": profile.read_timeout_ms,
                "write_timeout_ms": profile.write_timeout_ms,
                "presign_max_ttl_seconds": profile.presign_max_ttl_seconds,
                "project_scope": list(profile.project_scope),
                "masked_credential_summary": profile.masked_credential_summary,
            }
            existing = await self.session.get(StorageProfileModel, profile.id)
            if existing is None:
                self.session.add(StorageProfileModel(id=profile.id, **values))
            elif profile.revision == existing.revision:
                continue
            elif profile.revision != existing.revision + 1:
                raise RevisionConflictError(profile.id, existing.revision, profile.revision)
            else:
                result = cast(
                    CursorResult[Any],
                    await self.session.execute(
                        update(StorageProfileModel)
                        .where(
                            StorageProfileModel.id == profile.id,
                            StorageProfileModel.revision == existing.revision,
                        )
                        .values(**values)
                    ),
                )
                if result.rowcount != 1:
                    raise RevisionConflictError(profile.id, existing.revision, profile.revision)

    async def __aexit__(
        self,
        exc_type: type[BaseException] | None,
        exc: BaseException | None,
        tb: TracebackType | None,
    ) -> None:
        if self.session is None:
            return
        database_error = isinstance(exc, (OSError, SQLAlchemyError))
        try:
            if exc_type is not None:
                await self.rollback()
        finally:
            await self.session.close()
        if database_error and exc is not None:
            raise DatabaseUnavailableError("business database is unavailable") from exc

    async def commit(self) -> None:
        if self.session is None:
            raise RuntimeError("unit of work is not active")
        try:
            await self._persist_scene_collections()
            await self._persist_asset_bible_collections()
            await self._persist_workflow_collections()
            await self._persist_catalog_collections()
            await self._persist_video_collections()
            await self._persist_edit_collections()
            await self._persist_storage_profiles()
            await self._persist_timeline_collections()
            await self._persist_media_collections()
            await self._persist_export_collections()
            await self._persist_asset_reservations()
            await self._persist_phase_one_collections()
            await self.session.commit()
        except IntegrityError as error:
            await self.session.rollback()
            # The unique constraint is the final concurrency guard.
            # Unknown DB errors remain visible.
            if "episode" in str(error).lower() and "number" in str(error).lower():
                raise EpisodeNumberConflictError("unknown", 0) from error
            if "asset_version" in str(error).lower() or "asset version" in str(error).lower():
                raise AssetVersionConflictError("unknown", 0) from error
            raise

    async def rollback(self) -> None:
        if self.session is not None:
            await self.session.rollback()


SqlAlchemyUnitOfWork = SQLAlchemyUnitOfWork


def make_sqlalchemy_uow_factory(
    session_factory: async_sessionmaker[AsyncSession],
) -> Callable[[], SQLAlchemyUnitOfWork]:
    return lambda: SQLAlchemyUnitOfWork(session_factory)
