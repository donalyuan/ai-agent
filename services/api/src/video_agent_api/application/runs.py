from __future__ import annotations

import json
from copy import deepcopy
from dataclasses import asdict, dataclass, replace
from datetime import UTC, datetime
from hashlib import sha256
from re import fullmatch
from typing import Any, cast
from uuid import UUID, uuid4

from video_agent_api.application.ports import MediaCurrentOwner
from video_agent_api.domain.catalog import CapabilitySnapshot, Model, Provider, ProviderProfile
from video_agent_api.domain.errors import (
    ProjectAccessForbiddenError,
    RevisionConflictError,
    UnsupportedFeatureError,
    ValidationDomainError,
    WorkflowRunConflictError,
    WorkflowRunNotFoundError,
    WorkflowSourceConflictError,
    WorkflowUnconfiguredError,
    WorkflowVersionUnavailableError,
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
    temporal_workflow_id,
)

SCHEMA_VERSION = "1.0.0"
DEFAULT_TEMPLATE_KEY = "drama-mvp-a-default"
DEFAULT_NODE_KEYS = (
    "text.generate",
    "text.review",
    "media.generate.image",
    "media.review.image",
    "media.generate.video",
    "media.review.video",
    "media.inspect",
    "timeline.handoff",
)
SAFE_SUMMARY_KEYS = frozenset(
    {
        "id",
        "ownerId",
        "revision",
        "contentHash",
        "payloadHash",
        "status",
        "count",
        "kind",
        "type",
        "candidateId",
        "assetId",
        "assetVersionId",
        "snapshotId",
        "schemaVersion",
        "code",
        "message",
        "retryable",
    }
)
SELECTION_SNAPSHOT_KEYS = frozenset(
    {
        "selectionSnapshotId",
        "provider",
        "providerId",
        "profile",
        "profileId",
        "modelId",
        "adapterKey",
        "adapterIdentity",
        "profileRevision",
        "capabilitySnapshotId",
        "capabilityRevision",
        "capabilityOperation",
        "capabilitySnapshots",
        "skills",
        "skillRevisionIds",
        "skillDigests",
        "decision",
        "decisionRevision",
        "routeStatus",
        "source",
        "routeDecisionId",
        "routeSelectionId",
    }
)
REFERENCE_ID_KEYS = frozenset(
    {
        "id",
        "ownerId",
        "projectId",
        "episodeId",
        "sceneId",
        "shotId",
        "candidateId",
        "assetId",
        "assetVersionId",
        "snapshotId",
        "workflowVersionId",
        "runId",
        "nodeRunId",
        "textReviewBatchId",
        "handoffId",
    }
)
REFERENCE_REVISION_KEYS = frozenset(
    {
        "revision",
        "ownerRevision",
        "projectRevision",
        "episodeRevision",
        "sceneRevision",
        "shotRevision",
        "shotSpecRevision",
        "candidateRevision",
        "assetRevision",
        "assetVersionRevision",
        "snapshotRevision",
        "workflowRevision",
        "runRevision",
        "nodeRevision",
        "textReviewBatchRevision",
    }
)
REFERENCE_HASH_KEYS = frozenset(
    {
        "assetVersionHash",
        "contentHash",
        "payloadHash",
        "shotSpecHash",
        "sourceHash",
        "workflowContentHash",
        "fingerprintHash",
    }
)
REFERENCE_METADATA_KEYS = frozenset({"type", "kind", "status", "schemaVersion"})
REFERENCE_KEYS = (
    REFERENCE_ID_KEYS | REFERENCE_REVISION_KEYS | REFERENCE_HASH_KEYS | REFERENCE_METADATA_KEYS
)
IMMEDIATE_TEMPORAL_NODE_KEYS = frozenset(
    {"text.generate", "media.generate.image", "media.generate.video"}
)
MEDIA_NODE_KEYS = frozenset(key for key in DEFAULT_NODE_KEYS if key.startswith("media.")) | {
    "timeline.handoff"
}
MEDIA_CANDIDATE_KEYS = frozenset(
    {
        "candidateId",
        "candidateRevision",
        "projectId",
        "episodeId",
        "targetId",
        "assetVersionId",
        "assetVersionRevision",
        "assetVersionHash",
        "provenance",
        "mediaKind",
        "shotSpecRevision",
        "shotSpecHash",
        "durationMs",
        "aspectRatio",
        "derivativeStatus",
        "storageStatus",
        "providerStatus",
        "expectedShotRevision",
    }
)


@dataclass(frozen=True, slots=True)
class EnsureWorkflowCommand:
    project_id: str
    scope_type: str = "project"
    scope_ids: tuple[str, ...] = ()


@dataclass(frozen=True, slots=True)
class HistoricalRerunCommand:
    project_id: str
    snapshot_id: str
    expected_snapshot_revision: int


@dataclass(frozen=True, slots=True)
class SuccessorRunCommand:
    project_id: str
    predecessor_run_id: str
    expected_predecessor_revision: int
    reuse_node_ids: tuple[str, ...] = ()
    selection_snapshot: dict[str, object] | None = None


@dataclass(frozen=True, slots=True)
class StartRunCommand:
    project_id: str
    workflow_version_id: str
    node_keys: tuple[str, ...]
    scope_refs: tuple[dict[str, object], ...]
    owner_refs: tuple[dict[str, object], ...]
    selection_snapshot: dict[str, object] | None
    idempotency_key: str
    route_decision_id: str | None = None


@dataclass(frozen=True, slots=True)
class ReviewSignalCommand:
    run_id: str
    node_run_id: str
    expected_node_revision: int
    decision: str
    correlation_id: str
    actor_uuid: str


@dataclass(frozen=True, slots=True)
class BudgetGateCommand:
    run_id: str
    node_run_id: str
    logical_operation: str
    request_fingerprint: str
    operation_kind: str
    batch_size: int
    cost_status: str
    estimated_cost: str | None
    currency: str | None
    threshold_snapshot_id: str | None
    threshold_revision: int | None
    expected_node_revision: int | None = None


@dataclass(frozen=True, slots=True)
class ProviderObservationCommand:
    run_id: str
    node_run_id: str
    correlation_id: str
    observation: str
    payload: dict[str, object]


@dataclass(frozen=True, slots=True)
class MediaCandidateCommand:
    run_id: str
    generation_node_run_id: str
    review_node_run_id: str
    expected_generation_revision: int
    expected_review_revision: int
    correlation_id: str
    candidate: dict[str, object]


@dataclass(frozen=True, slots=True)
class MediaInspectCommand:
    run_id: str
    node_run_id: str
    expected_node_revision: int
    shot_id: str
    candidate_id: str
    derivative_status: str
    correlation_id: str


def _canonical_hash(value: object) -> str:
    return sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def _stable_uuid(value: str, label: str) -> None:
    try:
        UUID(value)
    except (ValueError, AttributeError) as error:
        raise ValidationDomainError(f"{label} must be a stable UUID") from error


def _is_sha256(value: object) -> bool:
    return isinstance(value, str) and fullmatch(r"[0-9a-fA-F]{64}", value) is not None


def _validate_owner_references(
    project_id: str,
    refs: tuple[dict[str, object], ...],
    label: str,
) -> tuple[dict[str, object], ...]:
    for ref in refs:
        if not isinstance(ref, dict) or not ref or set(ref) - REFERENCE_KEYS:
            raise ValidationDomainError(f"{label} must contain only stable owner references")
        identifiers = [ref[key] for key in REFERENCE_ID_KEYS if key in ref]
        if not identifiers or any(not isinstance(value, str) or not value for value in identifiers):
            raise ValidationDomainError(f"{label} requires a stable owner ID")
        if "projectId" in ref and ref["projectId"] != project_id:
            raise ValidationDomainError(f"{label} contains a foreign project reference")
        revisions = [ref[key] for key in REFERENCE_REVISION_KEYS if key in ref]
        if not revisions or any(
            isinstance(value, bool) or not isinstance(value, int) or value < 1
            for value in revisions
        ):
            raise ValidationDomainError(f"{label} requires a positive owner revision")
        if any(not _is_sha256(ref[key]) for key in REFERENCE_HASH_KEYS if key in ref):
            raise ValidationDomainError(f"{label} contains an invalid owner hash")
        if any(
            not isinstance(ref[key], str) or not ref[key]
            for key in REFERENCE_METADATA_KEYS
            if key in ref
        ):
            raise ValidationDomainError(f"{label} contains invalid reference metadata")
    return refs


def _logical_operation(node_key: str, qualifier: str = "") -> str:
    if node_key.startswith("media.generate."):
        media_kind = node_key.rsplit(".", 1)[-1]
        parts = ["media.generate", media_kind]
    else:
        parts = [node_key]
    if qualifier:
        parts.append(qualifier)
    parts.append(str(uuid4()))
    return ":".join(parts)


def _validate_media_candidate(candidate: dict[str, object]) -> None:
    required = {
        "candidateId",
        "candidateRevision",
        "projectId",
        "episodeId",
        "targetId",
        "assetVersionId",
        "assetVersionRevision",
        "assetVersionHash",
        "provenance",
        "mediaKind",
        "storageStatus",
        "providerStatus",
        "expectedShotRevision",
    }
    if required - set(candidate) or set(candidate) - MEDIA_CANDIDATE_KEYS:
        raise ValidationDomainError("media candidate owner contract is incomplete")
    if candidate.get("mediaKind") not in {"image", "video"}:
        raise ValidationDomainError("media candidate kind is invalid")
    if (
        candidate.get("storageStatus") != "verified"
        or candidate.get("providerStatus") != "succeeded"
        or candidate.get("provenance") != "media_review"
        or candidate.get("derivativeStatus", "pending") != "pending"
    ):
        raise ValidationDomainError("media candidate terminal storage result is not verified")
    for key in ("candidateId", "projectId", "episodeId", "targetId", "assetVersionId"):
        if not isinstance(candidate[key], str) or not candidate[key]:
            raise ValidationDomainError("media candidate contains an invalid stable owner ID")
    for key in (
        "candidateRevision",
        "assetVersionRevision",
        "expectedShotRevision",
    ):
        value = candidate[key]
        if isinstance(value, bool) or not isinstance(value, int) or value < 1:
            raise ValidationDomainError("media candidate contains an invalid owner revision")
    if not _is_sha256(candidate["assetVersionHash"]):
        raise ValidationDomainError("media candidate AssetVersion hash is invalid")
    if candidate["mediaKind"] == "video":
        if (
            isinstance(candidate.get("shotSpecRevision"), bool)
            or not isinstance(candidate.get("shotSpecRevision"), int)
            or cast(int, candidate["shotSpecRevision"]) < 1
            or not _is_sha256(candidate.get("shotSpecHash"))
            or isinstance(candidate.get("durationMs"), bool)
            or not isinstance(candidate.get("durationMs"), int)
            or cast(int, candidate["durationMs"]) < 1
            or candidate.get("aspectRatio") not in {"9:16", "16:9", "1:1"}
        ):
            raise ValidationDomainError("video candidate owner snapshot is incomplete")


def _fact_value(value: object, key: str) -> object:
    if isinstance(value, dict):
        return value.get(key)
    return getattr(value, key, None)


def _candidate_ref(candidate: object) -> dict[str, object]:
    value = cast(Any, candidate)
    return {
        "candidateId": str(value.id),
        "candidateRevision": int(value.revision),
        "kind": str(value.kind),
        "scopeId": str(value.scope_id),
        "payloadHash": str(value.payload_hash),
        "sourceCandidateIds": list(value.source_candidate_ids),
        "sourceHashes": list(value.source_hashes),
    }


def _validate_text_media_gate(
    uow: Any,
    project_id: str,
    owner_refs: tuple[dict[str, object], ...],
) -> None:
    handoff_refs = [item for item in owner_refs if item.get("type") == "textReviewHandoff"]
    if len(handoff_refs) != 1:
        raise WorkflowRunConflictError("accepted text handoff owner reference is required")
    ref = handoff_refs[0]
    handoff = uow.text_handoffs.get(ref.get("handoffId"))
    batch = uow.text_review_batches.get(ref.get("textReviewBatchId"))
    if (
        handoff is None
        or batch is None
        or handoff.batch_id != batch.id
        or handoff.batch_revision != batch.revision
        or handoff.project_id != project_id
        or batch.project_id != project_id
        or batch.status != "accepted"
        or ref.get("revision") != batch.revision
        or ref.get("textReviewBatchRevision") != batch.revision
        or ref.get("payloadHash") != handoff.payload_hash
    ):
        raise WorkflowRunConflictError("accepted text successor closure is stale or foreign")
    successor = next(
        (
            item
            for item in uow.text_review_batches.values()
            if getattr(item, "supersedes_batch_id", None) == batch.id
            and getattr(item, "status", None) != "rejected"
        ),
        None,
    )
    if successor is not None:
        raise WorkflowRunConflictError("accepted text successor closure has been superseded")
    candidates = tuple(batch.candidates)
    by_id = {item.id: item for item in candidates}
    if not candidates or any(item.status != "accepted" for item in candidates):
        raise WorkflowRunConflictError("accepted text candidate set is partial or stale")
    for candidate in candidates:
        if candidate.project_id != project_id or any(
            source_id not in by_id or by_id[source_id].payload_hash != source_hash
            for source_id, source_hash in zip(
                candidate.source_candidate_ids,
                candidate.source_hashes,
                strict=True,
            )
        ):
            raise WorkflowRunConflictError("accepted text candidate source closure is stale")
    expected_refs = tuple(_candidate_ref(item) for item in candidates)
    if handoff.candidate_refs != expected_refs or handoff.payload_hash != _canonical_hash(
        list(expected_refs)
    ):
        raise WorkflowRunConflictError("accepted text handoff hash or candidate set is stale")
    acknowledgements = [
        item
        for item in uow.text_handoff_acks.values()
        if _fact_value(item, "handoff_id") == handoff.id
    ]
    acknowledged = {
        str(_fact_value(item, "owner"))
        for item in acknowledgements
        if _fact_value(item, "correlation_id") == handoff.correlation_id
        and isinstance(_fact_value(item, "owner_revision"), int)
        and cast(int, _fact_value(item, "owner_revision")) >= 1
    }
    if acknowledged != set(handoff.required_owners):
        raise WorkflowRunConflictError("text handoff owner acknowledgements are incomplete")


async def _validate_storyboard_media_gate(
    uow: Any,
    project_id: str,
    node_keys: list[str],
    scope_refs: tuple[dict[str, object], ...],
) -> None:
    shot_refs = [item for item in scope_refs if item.get("type") == "shot"]
    if not shot_refs or len({item.get("shotId") for item in shot_refs}) != len(shot_refs):
        raise WorkflowRunConflictError("exact storyboard Shot scope is required")
    direct_video = "media.generate.video" in node_keys and "media.generate.image" not in node_keys
    direct_inspect = "media.inspect" in node_keys and "media.review.video" not in node_keys
    direct_timeline = "timeline.handoff" in node_keys and "media.inspect" not in node_keys
    for ref in shot_refs:
        shot = uow.shots.get(ref.get("shotId"))
        scene = uow.scenes.get(ref.get("sceneId"))
        if (
            shot is None
            or scene is None
            or shot.project_id != project_id
            or scene.project_id != project_id
            or shot.scene_id != scene.id
            or shot.episode_id != ref.get("episodeId")
            or ref.get("projectId") != project_id
            or ref.get("revision") != shot.revision
            or shot.spec_ref is None
            or ref.get("shotSpecRevision") != shot.spec_ref.revision
            or ref.get("shotSpecHash") != shot.spec_ref.content_hash
        ):
            raise WorkflowRunConflictError("storyboard Shot/ShotSpec owner snapshot is stale")
        snapshot = uow.asset_bible_snapshots.get(ref.get("snapshotId"))
        pending_tasks = [
            task
            for task in uow.asset_bible_tasks.values()
            if getattr(task, "project_id", None) == project_id
            and getattr(task, "target_id", None) == shot.id
            and getattr(task, "status", None) in {"pending", "acknowledged"}
        ]
        if (
            shot.continuity_snapshot is None
            or shot.continuity_task_refs
            or pending_tasks
            or snapshot is None
            or getattr(snapshot, "project_id", None) != project_id
            or getattr(snapshot, "target_id", None) != shot.id
            or getattr(snapshot, "status", None) != "accepted"
            or ref.get("snapshotRevision") != shot.continuity_snapshot.revision
            or ref.get("contentHash") != shot.continuity_snapshot.content_hash
            or getattr(snapshot, "id", None) != shot.continuity_snapshot.id
            or getattr(snapshot, "revision", None) != shot.continuity_snapshot.revision
            or getattr(snapshot, "content_hash", None) != shot.continuity_snapshot.content_hash
        ):
            raise WorkflowRunConflictError("accepted continuity snapshot is stale or incomplete")
        if direct_video:
            await _validate_current_media_ref(uow, shot.current_image, ref, "image")
        if direct_inspect or direct_timeline:
            await _validate_current_media_ref(
                uow,
                shot.current_video,
                ref,
                "video",
                require_ready=direct_timeline,
            )


async def _validate_current_media_ref(
    uow: Any,
    current: object,
    ref: dict[str, object],
    media_kind: str,
    *,
    require_ready: bool = False,
) -> None:
    if (
        current is None
        or getattr(current, "media_kind", None) != media_kind
        or not getattr(current, "accepted", False)
        or ref.get("candidateId") != getattr(current, "candidate_id", None)
        or ref.get("assetVersionId") != getattr(current, "asset_version_id", None)
        or ref.get("assetVersionRevision") != getattr(current, "asset_version_revision", None)
        or ref.get("assetVersionHash") != getattr(current, "asset_version_hash", None)
        or (require_ready and getattr(current, "derivative_status", None) != "ready")
    ):
        raise WorkflowRunConflictError(f"accepted current {media_kind} eligibility is stale")
    current_value = cast(Any, current)
    version = await uow.asset_versions.get(str(current_value.asset_version_id))
    if (
        version is None
        or version.project_id != current_value.project_id
        or version.revision != current_value.asset_version_revision
        or version.content_hash != current_value.asset_version_hash
    ):
        raise WorkflowRunConflictError(f"accepted current {media_kind} AssetVersion is stale")


def _event(
    uow: Any,
    run: WorkflowRun,
    event_type: str,
    correlation_id: str,
    payload: dict[str, object],
    node_run_id: str | None = None,
) -> RunEvent:
    events = uow.run_events.setdefault(run.id, [])
    event = RunEvent(
        run.id,
        len(events) + 1,
        event_type,
        correlation_id,
        payload,
        node_run_id,
    )
    events.append(event)
    uow.audit_events.append(
        {"type": event_type, "runId": run.id, "eventId": event.id, "sequence": event.sequence}
    )
    uow.outbox_events.append(
        {"type": event_type, "runId": run.id, "eventId": event.id, "sequence": event.sequence}
    )
    return event


def _safe_summary(value: object) -> object:
    if isinstance(value, dict):
        return {
            str(key): _safe_summary(item)
            for key, item in value.items()
            if str(key) in SAFE_SUMMARY_KEYS
        }
    if isinstance(value, (list, tuple)):
        return [_safe_summary(item) for item in value]
    if isinstance(value, (str, int, float, bool)) or value is None:
        return value
    return str(type(value).__name__)


def _validate_frozen_selection(selection: dict[str, object]) -> dict[str, object]:
    required = {
        "selectionSnapshotId",
        "provider",
        "providerId",
        "profile",
        "profileId",
        "modelId",
        "adapterKey",
        "adapterIdentity",
        "profileRevision",
        "capabilitySnapshotId",
        "capabilityRevision",
        "capabilityOperation",
        "capabilitySnapshots",
        "skills",
        "skillRevisionIds",
        "skillDigests",
        "decision",
        "decisionRevision",
        "routeStatus",
        "source",
    }
    if required - set(selection) or set(selection) - SELECTION_SNAPSHOT_KEYS:
        raise ValidationDomainError("workflow selection is unresolved")
    if (
        selection.get("routeStatus") != "selected"
        or selection.get("decision") not in {"fixed", "selected", "manual"}
        or selection.get("source") == "settings"
        or selection.get("adapterIdentity") != "local_workspace"
        or selection.get("provider") != "mock"
        or selection.get("profile") != "local-test-offline"
        or selection.get("adapterKey") != "mock"
    ):
        raise ValidationDomainError("workflow selection is pending, stale, disabled or unapproved")
    skills = selection.get("skills")
    revisions = selection.get("skillRevisionIds")
    digests = selection.get("skillDigests")
    capability_snapshots = selection.get("capabilitySnapshots")
    if (
        not isinstance(selection.get("selectionSnapshotId"), str)
        or not selection["selectionSnapshotId"]
        or any(
            not isinstance(selection.get(key), str) or not selection.get(key)
            for key in (
                "providerId",
                "profileId",
                "modelId",
                "capabilitySnapshotId",
                "capabilityOperation",
            )
        )
        or isinstance(selection.get("profileRevision"), bool)
        or not isinstance(selection.get("profileRevision"), int)
        or cast(int, selection["profileRevision"]) < 1
        or isinstance(selection.get("capabilityRevision"), bool)
        or not isinstance(selection.get("capabilityRevision"), int)
        or cast(int, selection["capabilityRevision"]) < 1
        or not isinstance(capability_snapshots, dict)
        or not capability_snapshots
        or any(
            not isinstance(operation, str)
            or not isinstance(value, dict)
            or set(value) != {"id", "revision"}
            or not isinstance(value.get("id"), str)
            or not value.get("id")
            or isinstance(value.get("revision"), bool)
            or not isinstance(value.get("revision"), int)
            or cast(int, value["revision"]) < 1
            for operation, value in capability_snapshots.items()
        )
        or isinstance(selection.get("decisionRevision"), bool)
        or not isinstance(selection.get("decisionRevision"), int)
        or cast(int, selection["decisionRevision"]) < 1
        or not isinstance(skills, list)
        or not skills
        or any(not isinstance(item, str) or not item for item in skills)
        or not isinstance(revisions, list)
        or len(revisions) != len(skills)
        or any(not isinstance(item, str) or not item for item in revisions)
        or not isinstance(digests, list)
        or len(digests) != len(skills)
        or any(not isinstance(item, str) or not item for item in digests)
        or (
            "routeDecisionId" in selection
            and (
                not isinstance(selection["routeDecisionId"], str)
                or not selection["routeDecisionId"]
            )
        )
    ):
        raise ValidationDomainError("workflow selection skill revisions are unresolved")
    return dict(selection)


def _selection_operation(node_keys: list[str]) -> str:
    operations = _selection_operations(node_keys)
    if operations:
        return operations[0]
    raise ValidationDomainError("workflow selection has no runnable provider operation")


def _selection_operations(node_keys: list[str]) -> list[str]:
    operations: list[str] = []
    if "text.generate" in node_keys or "text.review" in node_keys:
        operations.append("text.generate")
    if "media.generate.image" in node_keys or "media.review.image" in node_keys:
        operations.append("image.generate")
    if "media.generate.video" in node_keys or "media.review.video" in node_keys:
        operations.append("video.submit")
    return operations


def _catalog_provider_selection(uow: Any, node_keys: list[str]) -> dict[str, object]:
    operations = _selection_operations(node_keys)
    operation = _selection_operation(node_keys)
    candidates: list[tuple[Provider, ProviderProfile, Model, dict[str, CapabilitySnapshot]]] = []
    for profile in uow.profiles.values():
        provider = uow.providers.get(profile.provider_id)
        if (
            provider is None
            or provider.adapter_key != "mock"
            or provider.approval != "approved"
            or not provider.enabled
            or not provider.adapter_installed
            or not profile.enabled
            or profile.adapter_identity != "local_workspace"
        ):
            continue
        capability_values = {
            selected_operation: profile.capability_snapshots.get(selected_operation)
            for selected_operation in operations
        }
        if any(item is None or not item.runnable for item in capability_values.values()):
            continue
        capabilities = cast(dict[str, CapabilitySnapshot], capability_values)
        models = [
            model
            for model in uow.models.values()
            if model.profile_id == profile.id
            and model.enabled
            and all(
                capability.model_id is None or capability.model_id == model.id
                for capability in capabilities.values()
            )
        ]
        candidates.extend((provider, profile, model, capabilities) for model in models)
    if len(candidates) != 1:
        raise ValidationDomainError("workflow provider selection is missing or ambiguous")
    provider, profile, model, capabilities = candidates[0]
    capability = capabilities[operation]
    return {
        "provider": provider.adapter_key,
        "providerId": provider.id,
        "profile": "local-test-offline",
        "profileId": profile.id,
        "modelId": model.id,
        "adapterKey": provider.adapter_key,
        "adapterIdentity": profile.adapter_identity,
        "profileRevision": profile.revision,
        "capabilitySnapshotId": capability.id,
        "capabilityRevision": capability.revision,
        "capabilityOperation": operation,
        "capabilitySnapshots": {
            selected_operation: {
                "id": selected_capability.id,
                "revision": selected_capability.revision,
            }
            for selected_operation, selected_capability in capabilities.items()
        },
    }


def _source_snapshot(
    workflow: WorkflowVersion, binding: ProjectDefaultWorkflowBinding
) -> dict[str, object]:
    return {
        "workflowVersionId": workflow.id,
        "versionNumber": workflow.version_number,
        "contentHash": workflow.content_hash,
        "definition": deepcopy(workflow.definition),
        "scopeType": workflow.scope_type,
        "scopeIds": list(workflow.scope_ids),
        "bindingId": binding.id,
        "bindingRevision": binding.revision,
        "templateKey": workflow.template_key,
        "schemaVersion": workflow.schema_version,
    }


def _make_temporal_start(run: WorkflowRun, node: NodeRun) -> TemporalStart:
    request_fingerprint = _canonical_hash(
        {
            "runId": run.id,
            "nodeRunId": node.id,
            "logicalOperation": node.logical_operation,
            "source": run.source_snapshot,
            "selection": run.selection_snapshot,
        }
    )
    return TemporalStart(
        run.id,
        node.id,
        node.logical_operation,
        temporal_workflow_id(
            run.project_id,
            run.workflow_version_id,
            run.id,
            node.logical_operation,
        ),
        request_fingerprint,
    )


class RunsService:
    def __init__(
        self,
        uow_factory: Any,
        media_current_owner: MediaCurrentOwner | None = None,
    ) -> None:
        self._uow_factory = uow_factory
        self._media_current_owner = media_current_owner

    async def ensure_workflow(self, command: EnsureWorkflowCommand) -> WorkflowVersion:
        if command.scope_type != "project" or command.scope_ids not in {
            (),
            (command.project_id,),
        }:
            raise WorkflowSourceConflictError("workflow_source_conflict")
        async with self._uow_factory() as uow:
            if await uow.projects.get(command.project_id) is None:
                raise WorkflowUnconfiguredError("workflow_unconfigured")
            existing = cast(WorkflowVersion | None, uow.workflow_by_project.get(command.project_id))
            binding = cast(
                ProjectDefaultWorkflowBinding | None,
                uow.workflow_bindings.get(command.project_id),
            )
            if existing is not None or binding is not None:
                if (
                    not isinstance(existing, WorkflowVersion)
                    or not isinstance(binding, ProjectDefaultWorkflowBinding)
                    or binding.workflow_version_id != existing.id
                    or binding.workflow_content_hash != existing.content_hash
                    or existing.project_id != command.project_id
                    or existing.status != "published"
                ):
                    raise WorkflowSourceConflictError("workflow_source_conflict")
                return existing
            workflow = WorkflowVersion(
                command.project_id,
                scope_type="project",
                scope_ids=(command.project_id,),
                definition={
                    "nodes": [
                        {
                            "key": key,
                            "ports": {
                                "input": f"{key}.input.v1",
                                "output": f"{key}.output.v1",
                            },
                        }
                        for key in DEFAULT_NODE_KEYS
                    ],
                    "compatibilityLogicalOperations": {
                        "media.generate": ["media.generate.image", "media.generate.video"]
                    },
                    "skills": ["novel-writing", "drama-skills"],
                    "schemaVersion": SCHEMA_VERSION,
                },
            )
            binding = ProjectDefaultWorkflowBinding(
                command.project_id,
                workflow.id,
                workflow.content_hash,
            )
            uow.workflow_by_project[command.project_id] = workflow
            uow.workflow_bindings[command.project_id] = binding
            uow.audit_events.append(
                {
                    "type": "workflow.default.ensured",
                    "workflowVersionId": workflow.id,
                    "bindingId": binding.id,
                }
            )
            uow.outbox_events.append(
                {
                    "type": "workflow.default.ensured",
                    "workflowVersionId": workflow.id,
                    "bindingId": binding.id,
                }
            )
            await uow.commit()
            return workflow

    async def get_default_workflow(self, project_id: str) -> WorkflowVersion:
        async with self._uow_factory() as uow:
            workflow = cast(WorkflowVersion | None, uow.workflow_by_project.get(project_id))
            binding = cast(
                ProjectDefaultWorkflowBinding | None, uow.workflow_bindings.get(project_id)
            )
            if workflow is None or binding is None:
                raise WorkflowUnconfiguredError("workflow_unconfigured")
            source, _binding = self._validate_source(project_id, workflow.id, workflow, binding)
            return source

    async def get_default_workflow_projection(self, project_id: str) -> dict[str, object]:
        """Return the published source together with the binding CAS owner facts."""
        async with self._uow_factory() as uow:
            workflow = cast(WorkflowVersion | None, uow.workflow_by_project.get(project_id))
            binding = cast(
                ProjectDefaultWorkflowBinding | None,
                uow.workflow_bindings.get(project_id),
            )
            if workflow is None or binding is None:
                raise WorkflowUnconfiguredError("workflow_unconfigured")
            source, frozen_binding = self._validate_source(
                project_id,
                workflow.id,
                workflow,
                binding,
            )
            return {
                "id": source.id,
                "projectId": source.project_id,
                "templateKey": source.template_key,
                "scopeType": source.scope_type,
                "scopeIds": list(source.scope_ids),
                "definition": deepcopy(source.definition),
                "revision": source.revision,
                "contentHash": source.content_hash,
                "status": source.status,
                "versionNumber": source.version_number,
                "schemaVersion": source.schema_version,
                "bindingId": frozen_binding.id,
                "bindingRevision": frozen_binding.revision,
            }

    @staticmethod
    def _validate_source(
        project_id: str,
        workflow_version_id: str,
        workflow: object,
        binding: object,
    ) -> tuple[WorkflowVersion, ProjectDefaultWorkflowBinding]:
        if not isinstance(binding, ProjectDefaultWorkflowBinding):
            raise WorkflowUnconfiguredError("workflow_unconfigured")
        if (
            not isinstance(workflow, WorkflowVersion)
            or workflow.id != workflow_version_id
            or binding.workflow_version_id != workflow_version_id
            or workflow.status != "published"
        ):
            raise WorkflowVersionUnavailableError("workflow_version_unavailable")
        if (
            workflow.project_id != project_id
            or binding.project_id != project_id
            or workflow.template_key != DEFAULT_TEMPLATE_KEY
            or workflow.scope_type != "project"
            or workflow.scope_ids != (project_id,)
            or binding.workflow_content_hash != workflow.content_hash
            or binding.template_key != workflow.template_key
        ):
            raise WorkflowSourceConflictError("workflow_source_conflict")
        return workflow, binding

    async def start_run(
        self,
        project_id: str,
        workflow_version_id: str,
        node_keys: list[str],
        *,
        selection_snapshot: dict[str, object] | None = None,
        scope_refs: tuple[dict[str, object], ...] = (),
        owner_refs: tuple[dict[str, object], ...] = (),
        idempotency_key: str | None = None,
        route_decision_id: str | None = None,
        expected_binding_revision: int | None = None,
    ) -> WorkflowRun:
        if not node_keys or len(node_keys) != len(set(node_keys)):
            raise ValidationDomainError("workflow node selection is invalid")
        _validate_owner_references(project_id, scope_refs, "scopeRefs")
        _validate_owner_references(project_id, owner_refs, "ownerRefs")
        if selection_snapshot is not None:
            _validate_frozen_selection(selection_snapshot)
        operation_key = idempotency_key or _canonical_hash(
            {"projectId": project_id, "workflowVersionId": workflow_version_id, "nodes": node_keys}
        )
        request_fingerprint = _canonical_hash(
            {
                "projectId": project_id,
                "workflowVersionId": workflow_version_id,
                "nodes": node_keys,
                "scopeRefs": scope_refs,
                "ownerRefs": owner_refs,
                "selection": selection_snapshot,
                "routeDecisionId": route_decision_id,
            }
        )
        async with self._uow_factory() as uow:
            existing_id = uow.workflow_run_keys.get(operation_key)
            if existing_id is not None:
                if uow.workflow_run_key_fingerprints.get(operation_key) != request_fingerprint:
                    raise WorkflowRunConflictError("run idempotency fingerprint conflict")
                existing = cast(WorkflowRun | None, uow.workflow_runs.get(existing_id))
                if existing is None:
                    raise WorkflowRunConflictError("run idempotency owner is unavailable")
                return existing
            workflow, binding = self._validate_source(
                project_id,
                workflow_version_id,
                uow.workflow_by_project.get(project_id),
                uow.workflow_bindings.get(project_id),
            )
            if (
                expected_binding_revision is not None
                and binding.revision != expected_binding_revision
            ):
                raise RevisionConflictError(binding.id, expected_binding_revision, binding.revision)
            definition_nodes = cast(list[dict[str, object]], workflow.definition.get("nodes", []))
            allowed = {str(item["key"]) for item in definition_nodes}
            if set(node_keys) - allowed:
                raise ValidationDomainError("workflow node selection is invalid")
            frozen_selection = self._resolve_selection(
                uow,
                project_id,
                node_keys,
                selection_snapshot,
                route_decision_id,
            )
            if set(node_keys).intersection(MEDIA_NODE_KEYS):
                _validate_text_media_gate(uow, project_id, owner_refs)
                await _validate_storyboard_media_gate(uow, project_id, node_keys, scope_refs)
            run = WorkflowRun(
                project_id,
                workflow.id,
                input_snapshot={
                    "workflowVersionId": workflow.id,
                    "workflowRevision": workflow.revision,
                    "workflowContentHash": workflow.content_hash,
                    "scopeType": workflow.scope_type,
                    "scopeIds": list(workflow.scope_ids),
                    "startFingerprint": request_fingerprint,
                },
                selection_snapshot=frozen_selection,
                source_snapshot=_source_snapshot(workflow, binding),
            )
            run.nodes = [
                NodeRun(
                    run.id,
                    key,
                    logical_operation=_logical_operation(key),
                    scope_refs=scope_refs,
                )
                for key in node_keys
            ]
            run.transition("running")
            snapshot = RunInputSnapshot(
                run.id,
                project_id,
                workflow.id,
                workflow.content_hash,
                scope_refs,
                owner_refs,
                deepcopy(frozen_selection),
                deepcopy(run.source_snapshot),
                tuple(
                    {
                        "nodeRunId": node.id,
                        "nodeKey": node.node_key,
                        "logicalOperation": node.logical_operation,
                        "scopeRefs": list(node.scope_refs),
                    }
                    for node in run.nodes
                ),
            )
            run.input_snapshot = {**(run.input_snapshot or {}), "snapshotId": snapshot.id}
            uow.workflow_runs[run.id] = run
            uow.workflow_run_keys[operation_key] = run.id
            uow.workflow_run_key_fingerprints[operation_key] = request_fingerprint
            uow.run_input_snapshots[snapshot.id] = snapshot
            _event(uow, run, "run.started", run.id, {"status": run.status})
            for node in run.nodes:
                should_start = node.node_key in IMMEDIATE_TEMPORAL_NODE_KEYS
                if node.node_key == "media.generate.video" and "media.generate.image" in node_keys:
                    should_start = False
                if should_start:
                    start = _make_temporal_start(run, node)
                    uow.temporal_starts.setdefault(start.workflow_id, start)
            await uow.commit()
            return run

    @staticmethod
    def _resolve_selection(
        uow: Any,
        project_id: str,
        node_keys: list[str],
        selection_snapshot: dict[str, object] | None,
        route_decision_id: str | None,
    ) -> dict[str, object]:
        if route_decision_id is None:
            decisions = [
                item
                for item in uow.skill_route_decisions.values()
                if item.project_id == project_id and item.node_key == "text.generate"
            ]
            if len(decisions) != 1:
                raise ValidationDomainError("a unique current skill route decision is required")
            decision = decisions[0]
        else:
            decision = uow.skill_route_decisions.get(route_decision_id)
        if (
            decision is None
            or decision.project_id != project_id
            or decision.node_key != "text.generate"
        ):
            raise ValidationDomainError("skill route decision is missing or foreign")
        selected = decision.selected
        manual = uow.skill_route_selections.get(decision.id)
        if selected is None and manual is None:
            raise ValidationDomainError("skill route requires explicit human selection")
        if manual is not None:
            skill_name = manual.skill_name
            skill_version = manual.skill_version
            skill_digest = manual.skill_digest
            mode = "manual"
        else:
            skill_name = selected.name
            skill_version = selected.version
            skill_digest = selected.digest
            mode = "selected"
        candidate = next(
            (
                item
                for item in decision.candidates
                if item.name == skill_name and item.version == skill_version
            ),
            None,
        )
        catalog_revision = next(
            (
                item
                for item in uow.skills
                if item.name == skill_name and item.version == skill_version
            ),
            None,
        )
        if (
            candidate is None
            or candidate.digest != skill_digest
            or catalog_revision is None
            or catalog_revision.digest != skill_digest
            or catalog_revision.provenance != "verified_snapshot"
            or catalog_revision.approval != "approved"
            or not catalog_revision.enabled
            or catalog_revision.license_status != "verified"
            or not _is_sha256(skill_digest)
            or (manual is not None and manual.expected_revision != decision.revision)
        ):
            raise ValidationDomainError("skill route revision is stale or unapproved")
        frozen = _validate_frozen_selection(
            {
                **_catalog_provider_selection(uow, node_keys),
                "selectionSnapshotId": str(uuid4()),
                "skills": [skill_name],
                "skillRevisionIds": [f"{skill_name}@{skill_version}"],
                "skillDigests": [skill_digest],
                "decision": mode,
                "decisionRevision": decision.revision,
                "routeDecisionId": decision.id,
                **({"routeSelectionId": manual.id} if manual is not None else {}),
                "routeStatus": "selected",
                "source": "skill-route-decision",
            }
        )
        if selection_snapshot is not None and selection_snapshot != frozen:
            raise ValidationDomainError("client selection does not match current owner route")
        return frozen

    async def transition_node(self, run_id: str, node_id: str, target: str) -> WorkflowRun:
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(run_id))
            if run is None:
                raise WorkflowRunNotFoundError(run_id)
            if run.status == "cancel_requested" or run.status in {
                "succeeded",
                "failed",
                "cancelled",
            }:
                raise WorkflowRunConflictError(
                    "late node result cannot replace owner terminal state"
                )
            node = next((item for item in run.nodes if item.id == node_id), None)
            if node is None:
                raise ValidationDomainError("node run not found")
            node.transition(cast(Any, target))
            run.recompute_from_nodes()
            _event(
                uow,
                run,
                "node.transitioned",
                run.id,
                {"target": target, "nodeRevision": node.revision},
                node.id,
            )
            await uow.commit()
            return run

    async def enter_text_review(
        self,
        run_id: str,
        node_id: str,
        batch_id: str,
        expected_node_revision: int,
    ) -> WorkflowRun:
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(run_id))
            if run is None:
                raise WorkflowRunNotFoundError(run_id)
            node = next((item for item in run.nodes if item.id == node_id), None)
            batch = uow.text_review_batches.get(batch_id)
            if (
                node is None
                or node.node_key not in {"text.generate", "text.review"}
                or batch is None
                or batch.project_id != run.project_id
                or batch.run_id != run.id
                or batch.status != "pending_review"
                or not batch.candidates
                or any(candidate.status != "provisional" for candidate in batch.candidates)
            ):
                raise WorkflowRunConflictError("text review batch is partial, stale or foreign")
            existing_batch_id = (node.output_evidence or {}).get("textReviewBatchId")
            if node.status == "waiting_review" and existing_batch_id == batch.id:
                return run
            if node.status != "running" or node.revision != expected_node_revision:
                raise WorkflowRunConflictError("text review node revision is stale")
            node.output_evidence = {
                "textReviewBatchId": batch.id,
                "textReviewBatchRevision": batch.revision,
                "batchFingerprint": batch.fingerprint,
                "candidateCount": len(batch.candidates),
            }
            node.transition("waiting_review")
            run.recompute_from_nodes()
            _event(
                uow,
                run,
                "text.review_required",
                batch.id,
                {
                    "batchId": batch.id,
                    "batchRevision": batch.revision,
                    "candidateCount": len(batch.candidates),
                },
                node.id,
            )
            await uow.commit()
            return run

    async def resume_text_review_handoff(
        self,
        run_id: str,
        node_id: str,
        handoff_id: str,
        expected_node_revision: int,
    ) -> WorkflowRun:
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(run_id))
            if run is None:
                raise WorkflowRunNotFoundError(run_id)
            node = next((item for item in run.nodes if item.id == node_id), None)
            handoff = uow.text_handoffs.get(handoff_id)
            batch = uow.text_review_batches.get(handoff.batch_id) if handoff is not None else None
            existing = uow.workflow_signal_keys.get(f"text-handoff:{handoff_id}")
            if existing is not None:
                if existing[0] != run.id:
                    raise WorkflowRunConflictError("text handoff idempotency conflict")
                return run
            if (
                node is None
                or handoff is None
                or batch is None
                or node.status != "waiting_review"
                or node.revision != expected_node_revision
                or run.status != "waiting_review"
                or handoff.run_id != run.id
                or handoff.project_id != run.project_id
                or batch.run_id != run.id
                or batch.project_id != run.project_id
                or batch.status != "accepted"
                or batch.revision != handoff.batch_revision
                or (node.output_evidence or {}).get("textReviewBatchId") != batch.id
            ):
                raise WorkflowRunConflictError("text handoff is stale or foreign")
            refs_by_id = {str(item.get("candidateId")): item for item in handoff.candidate_refs}
            if (
                len(refs_by_id) != len(batch.candidates)
                or any(candidate.status != "accepted" for candidate in batch.candidates)
                or any(
                    candidate.id not in refs_by_id
                    or refs_by_id[candidate.id].get("candidateRevision") != candidate.revision
                    or refs_by_id[candidate.id].get("payloadHash") != candidate.payload_hash
                    for candidate in batch.candidates
                )
                or _canonical_hash(list(handoff.candidate_refs)) != handoff.payload_hash
            ):
                raise WorkflowRunConflictError("text handoff candidate closure is partial or stale")
            acknowledgements = {
                ack.owner: ack
                for ack in uow.text_handoff_acks.values()
                if ack.handoff_id == handoff.id and ack.correlation_id == handoff.correlation_id
            }
            if set(acknowledgements) != set(handoff.required_owners) or any(
                ack.owner_revision < 1 for ack in acknowledgements.values()
            ):
                raise WorkflowRunConflictError("text handoff owner acknowledgements are incomplete")
            node.output_evidence = {
                **(node.output_evidence or {}),
                "handoffId": handoff.id,
                "payloadHash": handoff.payload_hash,
                "ownerAckCount": len(acknowledgements),
                "status": "accepted",
            }
            node.transition("succeeded")
            run.recompute_from_nodes()
            fingerprint = _canonical_hash(
                {
                    "handoffId": handoff.id,
                    "payloadHash": handoff.payload_hash,
                    "owners": sorted(acknowledgements),
                }
            )
            uow.workflow_signal_keys[f"text-handoff:{handoff.id}"] = (run.id, fingerprint)
            _event(
                uow,
                run,
                "text.handoff_consumed",
                handoff.correlation_id,
                {
                    "handoffId": handoff.id,
                    "payloadHash": handoff.payload_hash,
                    "ownerAckCount": len(acknowledgements),
                },
                node.id,
            )
            await uow.commit()
            return run

    async def mark_submission_unknown(self, run_id: str, node_id: str) -> WorkflowRun:
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(run_id))
            if run is None:
                raise WorkflowRunNotFoundError(run_id)
            node = next((item for item in run.nodes if item.id == node_id), None)
            if node is None or node.status not in {"running", "cancel_requested"}:
                raise WorkflowRunConflictError("submission_unknown node state is invalid")
            node.submission_state = "submission_unknown"
            node.revision += 1
            workflow_id = temporal_workflow_id(
                run.project_id,
                run.workflow_version_id,
                run.id,
                node.logical_operation,
            )
            start = uow.temporal_starts.get(workflow_id)
            if isinstance(start, TemporalStart):
                uow.temporal_starts[start.workflow_id] = replace(
                    start, status="submission_unknown", revision=start.revision + 1
                )
            _event(uow, run, "provider.submission_unknown", run.id, {}, node.id)
            await uow.commit()
            return run

    async def reconcile_submission(self, run_id: str, node_id: str) -> WorkflowRun:
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(run_id))
            if run is None:
                raise WorkflowRunNotFoundError(run_id)
            node = next((item for item in run.nodes if item.id == node_id), None)
            if node is None or node.submission_state != "submission_unknown":
                raise WorkflowRunConflictError("submission reconciliation is unavailable")
            node.submission_state = "reconciled"
            node.revision += 1
            workflow_id = temporal_workflow_id(
                run.project_id,
                run.workflow_version_id,
                run.id,
                node.logical_operation,
            )
            start = uow.temporal_starts.get(workflow_id)
            if isinstance(start, TemporalStart):
                uow.temporal_starts[start.workflow_id] = replace(
                    start, status="reconciled", revision=start.revision + 1
                )
            _event(uow, run, "provider.submission_reconciled", run.id, {}, node.id)
            await uow.commit()
            return run

    async def append_provider_observation(self, command: ProviderObservationCommand) -> RunEvent:
        if command.observation not in {"submit", "poll", "cancel", "result"}:
            raise ValidationDomainError("provider observation kind is invalid")
        allowed_payload = {
            "status",
            "attempt",
            "requestId",
            "providerCallId",
            "candidateId",
            "assetVersionId",
            "progress",
            "retryable",
            "failureCode",
        }
        if set(command.payload) - allowed_payload:
            raise ValidationDomainError("provider observation payload contains non-summary data")
        fingerprint = _canonical_hash(asdict(command))
        key = f"provider:{command.observation}:{command.correlation_id}"
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(command.run_id))
            if run is None:
                raise WorkflowRunNotFoundError(command.run_id)
            node = next((item for item in run.nodes if item.id == command.node_run_id), None)
            if node is None:
                raise WorkflowRunConflictError("provider observation node is foreign")
            existing = uow.workflow_signal_keys.get(key)
            if existing is not None:
                if existing != (run.id, fingerprint):
                    raise WorkflowRunConflictError("provider observation idempotency conflict")
                event_type = f"provider.agnes.{command.observation}"
                event = next(
                    (
                        item
                        for item in uow.run_events.get(run.id, [])
                        if item.event_type == event_type
                        and item.correlation_id == command.correlation_id
                    ),
                    None,
                )
                if event is None:
                    raise WorkflowRunConflictError("provider observation event is unavailable")
                return cast(RunEvent, event)
            event = _event(
                uow,
                run,
                f"provider.agnes.{command.observation}",
                command.correlation_id,
                dict(command.payload),
                node.id,
            )
            uow.workflow_signal_keys[key] = (run.id, fingerprint)
            await uow.commit()
            return event

    async def cancel(self, run_id: str, expected_revision: int | None = None) -> WorkflowRun:
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(run_id))
            if run is None:
                raise WorkflowRunNotFoundError(run_id)
            if expected_revision is not None and run.revision != expected_revision:
                raise RevisionConflictError(run.id, expected_revision, run.revision)
            if run.status in {"queued", "running", "waiting_review"}:
                run.transition("cancel_requested")
                for node in run.nodes:
                    if node.status in {"pending", "running", "waiting_review"}:
                        node.transition("cancel_requested")
                _event(uow, run, "run.cancel_requested", run.id, {"status": run.status})
                await uow.commit()
            return run

    async def acknowledge_cancel(
        self, run_id: str, expected_revision: int | None = None
    ) -> WorkflowRun:
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(run_id))
            if run is None:
                raise WorkflowRunNotFoundError(run_id)
            if expected_revision is not None and run.revision != expected_revision:
                raise RevisionConflictError(run.id, expected_revision, run.revision)
            if run.status != "cancel_requested":
                raise WorkflowRunConflictError("cancel request is unavailable")
            for node in run.nodes:
                if node.status == "cancel_requested":
                    node.transition("cancelled")
            run.transition("cancelled")
            _event(uow, run, "run.cancelled", run.id, {"status": run.status})
            await uow.commit()
            return run

    @staticmethod
    def _activate_node(uow: Any, run: WorkflowRun, node_key: str) -> NodeRun | None:
        node = next(
            (item for item in run.nodes if item.node_key == node_key and item.status == "pending"),
            None,
        )
        if node is None:
            return None
        node.transition("running")
        start = _make_temporal_start(run, node)
        uow.temporal_starts.setdefault(start.workflow_id, start)
        return node

    async def record_media_candidate(self, command: MediaCandidateCommand) -> WorkflowRun:
        _validate_media_candidate(command.candidate)
        fingerprint = _canonical_hash(asdict(command))
        idempotency_key = f"media-candidate:{command.candidate['candidateId']}"
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(command.run_id))
            if run is None:
                raise WorkflowRunNotFoundError(command.run_id)
            existing = uow.workflow_signal_keys.get(idempotency_key)
            if existing is not None:
                if existing != (run.id, fingerprint):
                    raise WorkflowRunConflictError("media candidate idempotency conflict")
                return run
            generation = next(
                (item for item in run.nodes if item.id == command.generation_node_run_id),
                None,
            )
            review = next(
                (item for item in run.nodes if item.id == command.review_node_run_id),
                None,
            )
            media_kind = str(command.candidate["mediaKind"])
            if (
                generation is None
                or review is None
                or generation.node_key != f"media.generate.{media_kind}"
                or review.node_key != f"media.review.{media_kind}"
                or generation.status != "running"
                or review.status != "pending"
                or generation.revision != command.expected_generation_revision
                or review.revision != command.expected_review_revision
                or run.status not in {"running", "waiting_review"}
            ):
                raise WorkflowRunConflictError("media candidate stage owner is stale or foreign")
            provider_call = next(
                (
                    item
                    for item in uow.provider_calls.values()
                    if _fact_value(item, "runId") == run.id
                    and _fact_value(item, "nodeRunId") == generation.id
                    and _fact_value(item, "correlationId") == command.correlation_id
                ),
                None,
            )
            if (
                provider_call is None
                or _fact_value(provider_call, "status") != "succeeded"
                or _fact_value(provider_call, "assetVersionId")
                != command.candidate["assetVersionId"]
            ):
                raise WorkflowRunConflictError("verified Provider terminal result is required")
            version = await uow.asset_versions.get(str(command.candidate["assetVersionId"]))
            if (
                version is None
                or version.project_id != run.project_id
                or version.revision != command.candidate["assetVersionRevision"]
                or version.content_hash != command.candidate["assetVersionHash"]
            ):
                raise WorkflowRunConflictError("media candidate AssetVersion is stale or foreign")
            generation.output_evidence = deepcopy(command.candidate)
            generation.transition("succeeded")
            review.output_evidence = deepcopy(command.candidate)
            review.transition("running")
            review.transition("waiting_review")
            run.recompute_from_nodes()
            uow.workflow_signal_keys[idempotency_key] = (run.id, fingerprint)
            _event(
                uow,
                run,
                "media.candidate.recorded",
                command.correlation_id,
                {
                    "candidateId": str(command.candidate["candidateId"]),
                    "assetVersionId": str(command.candidate["assetVersionId"]),
                    "kind": media_kind,
                    "status": "pending_review",
                },
                generation.id,
            )
            await uow.commit()
            return run

    async def complete_media_inspect(self, command: MediaInspectCommand) -> WorkflowRun:
        if command.derivative_status not in {"pending", "ready", "failed", "stale"}:
            raise ValidationDomainError("media derivative status is invalid")
        fingerprint = _canonical_hash(asdict(command))
        idempotency_key = f"media-inspect:{command.correlation_id}"
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(command.run_id))
            if run is None:
                raise WorkflowRunNotFoundError(command.run_id)
            existing = uow.workflow_signal_keys.get(idempotency_key)
            if existing is not None:
                if existing != (run.id, fingerprint):
                    raise WorkflowRunConflictError("media inspect idempotency conflict")
                return run
            node = next((item for item in run.nodes if item.id == command.node_run_id), None)
            if (
                node is None
                or node.node_key != "media.inspect"
                or node.status != "running"
                or node.revision != command.expected_node_revision
                or run.status not in {"running", "waiting_review"}
            ):
                raise WorkflowRunConflictError("media inspect stage owner is stale or foreign")
            if self._media_current_owner is None:
                raise WorkflowRunConflictError("media current owner is unconfigured")
            await self._media_current_owner.update_derivative_in_transaction(
                uow,
                project_id=run.project_id,
                shot_id=command.shot_id,
                candidate_id=command.candidate_id,
                derivative_status=command.derivative_status,
            )
            if command.derivative_status == "ready":
                node.transition("succeeded")
                node.output_evidence = {
                    "candidateId": command.candidate_id,
                    "status": "ready",
                }
                node.failure = None
                self._activate_node(uow, run, "timeline.handoff")
            else:
                node.output_evidence = {
                    "candidateId": command.candidate_id,
                    "status": command.derivative_status,
                }
                node.failure = (
                    {
                        "code": f"derivative_{command.derivative_status}",
                        "message": "accepted media derivative is not ready",
                        "retryable": True,
                    }
                    if command.derivative_status in {"failed", "stale"}
                    else None
                )
                node.revision += 1
            run.recompute_from_nodes()
            uow.workflow_signal_keys[idempotency_key] = (run.id, fingerprint)
            _event(
                uow,
                run,
                "media.inspect.completed",
                command.correlation_id,
                {
                    "candidateId": command.candidate_id,
                    "status": command.derivative_status,
                },
                node.id,
            )
            await uow.commit()
            return run

    async def signal_review(self, command: ReviewSignalCommand) -> WorkflowRun:
        if command.decision not in {"accept", "reject", "retake"}:
            raise ValidationDomainError("review decision must be accept, reject or retake")
        _stable_uuid(command.actor_uuid, "review actor")
        fingerprint = _canonical_hash(asdict(command))
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(command.run_id))
            if run is None:
                raise WorkflowRunNotFoundError(command.run_id)
            existing = uow.workflow_signal_keys.get(command.correlation_id)
            if existing is not None:
                if existing != (run.id, fingerprint):
                    raise WorkflowRunConflictError("review signal idempotency conflict")
                return run
            node = next((item for item in run.nodes if item.id == command.node_run_id), None)
            if (
                node is None
                or node.status != "waiting_review"
                or node.revision != command.expected_node_revision
                or run.status != "waiting_review"
            ):
                raise WorkflowRunConflictError("review signal is stale or foreign")
            if command.decision == "retake" and node.node_key != "media.review.video":
                raise ValidationDomainError("retake is only valid for video review")
            if command.decision == "reject":
                node.transition("failed")
                node.failure = {
                    "code": "review_rejected",
                    "message": "candidate rejected",
                    "retryable": False,
                }
                run.recompute_from_nodes()
            elif command.decision == "retake":
                successor = NodeRun(
                    run.id,
                    "media.generate.video",
                    logical_operation=_logical_operation("media.generate.video", "retake"),
                    scope_refs=node.scope_refs,
                )
                successor_review = NodeRun(
                    run.id,
                    "media.review.video",
                    logical_operation=_logical_operation("media.review.video", "retake"),
                    scope_refs=node.scope_refs,
                )
                node.transition("failed")
                node.failure = {
                    "code": "review_retake",
                    "message": "candidate retained and superseded by retake",
                    "retryable": True,
                    "supersededByNodeRunId": successor.id,
                }
                run.nodes.extend((successor, successor_review))
                successor.transition("running")
                start = _make_temporal_start(run, successor)
                uow.temporal_starts.setdefault(start.workflow_id, start)
            else:
                if node.node_key in {"media.review.image", "media.review.video"}:
                    candidate = deepcopy(node.output_evidence or {})
                    _validate_media_candidate(candidate)
                    if self._media_current_owner is None:
                        raise WorkflowRunConflictError("media current owner is unconfigured")
                    await self._media_current_owner.accept_current_media_in_transaction(
                        uow,
                        project_id=run.project_id,
                        episode_id=str(candidate["episodeId"]),
                        shot_id=str(candidate["targetId"]),
                        candidate=candidate,
                        expected_shot_revision=cast(int, candidate["expectedShotRevision"]),
                    )
                node.transition("succeeded")
                if node.node_key == "media.review.image":
                    self._activate_node(uow, run, "media.generate.video")
                elif node.node_key == "media.review.video":
                    self._activate_node(uow, run, "media.inspect")
            run.recompute_from_nodes()
            uow.workflow_signal_keys[command.correlation_id] = (run.id, fingerprint)
            _event(
                uow,
                run,
                "review.decided",
                command.correlation_id,
                {"decision": command.decision, "actorUuid": command.actor_uuid},
                node.id,
            )
            await uow.commit()
            return run

    async def create_successor_from_failure(self, command: SuccessorRunCommand) -> WorkflowRun:
        if len(set(command.reuse_node_ids)) != len(command.reuse_node_ids):
            raise ValidationDomainError("reuse node IDs must be unique")
        async with self._uow_factory() as uow:
            predecessor = cast(
                WorkflowRun | None, uow.workflow_runs.get(command.predecessor_run_id)
            )
            if predecessor is None:
                raise WorkflowRunNotFoundError(command.predecessor_run_id)
            if predecessor.project_id != command.project_id:
                raise ProjectAccessForbiddenError(command.project_id)
            if predecessor.revision != command.expected_predecessor_revision:
                raise RevisionConflictError(
                    predecessor.id, command.expected_predecessor_revision, predecessor.revision
                )
            if predecessor.status != "failed":
                raise WorkflowRunConflictError("failed predecessor is required")
            if any(node.submission_state == "submission_unknown" for node in predecessor.nodes):
                raise WorkflowRunConflictError("submission_unknown requires reconciliation")
            reuse = set(command.reuse_node_ids)
            selected_reuse = [node for node in predecessor.nodes if node.id in reuse]
            if len(selected_reuse) != len(reuse) or any(
                node.status != "succeeded" or not node.output_evidence for node in selected_reuse
            ):
                raise WorkflowRunConflictError("succeeded-node reuse evidence is stale or missing")
            successor_node_keys = [
                node.node_key for node in predecessor.nodes if node.id not in reuse
            ]
            selection = self._resolve_selection(
                uow,
                command.project_id,
                successor_node_keys,
                command.selection_snapshot,
                str(
                    (command.selection_snapshot or predecessor.selection_snapshot).get(
                        "routeDecisionId", ""
                    )
                )
                or None,
            )
            successor = WorkflowRun(
                predecessor.project_id,
                predecessor.workflow_version_id,
                predecessor_run_id=predecessor.id,
                input_snapshot={
                    **(predecessor.input_snapshot or {}),
                    "predecessorRunId": predecessor.id,
                },
                selection_snapshot=deepcopy(selection),
                source_snapshot=deepcopy(predecessor.source_snapshot),
            )
            for source in predecessor.nodes:
                if source.id in reuse:
                    node = NodeRun(
                        successor.id,
                        source.node_key,
                        "succeeded",
                        logical_operation=f"reuse:{source.id}",
                        scope_refs=source.scope_refs,
                        output_evidence={
                            "sourceNodeRunId": source.id,
                            "sourceNodeRevision": source.revision,
                            "evidenceHash": _canonical_hash(source.output_evidence),
                        },
                    )
                else:
                    node = NodeRun(
                        successor.id,
                        source.node_key,
                        logical_operation=f"{source.node_key}:successor:{uuid4()}",
                        scope_refs=source.scope_refs,
                    )
                successor.nodes.append(node)
            successor.transition("running")
            snapshot = self._snapshot_for_lineage(
                successor,
                predecessor,
                "predecessorRunId",
                predecessor.id,
            )
            successor.input_snapshot = {
                **(successor.input_snapshot or {}),
                "snapshotId": snapshot.id,
            }
            uow.workflow_runs[successor.id] = successor
            uow.run_input_snapshots[snapshot.id] = snapshot
            _event(
                uow,
                successor,
                "run.successor_created",
                successor.id,
                {"predecessorRunId": predecessor.id, "reuseCount": len(reuse)},
            )
            for node in successor.nodes:
                if node.status != "succeeded":
                    start = _make_temporal_start(successor, node)
                    uow.temporal_starts.setdefault(start.workflow_id, start)
            await uow.commit()
            return successor

    async def create_successor(self, run_id: str, reuse_node_ids: list[str]) -> WorkflowRun:
        async with self._uow_factory() as uow:
            predecessor = cast(WorkflowRun | None, uow.workflow_runs.get(run_id))
            if predecessor is None:
                raise WorkflowRunNotFoundError(run_id)
            command = SuccessorRunCommand(
                predecessor.project_id,
                run_id,
                predecessor.revision,
                tuple(reuse_node_ids),
            )
        return await self.create_successor_from_failure(command)

    @staticmethod
    def _snapshot_for_lineage(
        run: WorkflowRun,
        source: WorkflowRun,
        lineage_key: str,
        lineage_value: str,
    ) -> RunInputSnapshot:
        source_snapshot_id = str((source.input_snapshot or {}).get("snapshotId", ""))
        return RunInputSnapshot(
            run.id,
            run.project_id,
            run.workflow_version_id,
            str(run.source_snapshot["contentHash"]),
            tuple(ref for node in run.nodes for ref in node.scope_refs),
            ({"ownerId": source_snapshot_id, "revision": 1, "type": "runInputSnapshot"},),
            deepcopy(run.selection_snapshot),
            deepcopy(run.source_snapshot),
            tuple(
                {
                    "nodeRunId": node.id,
                    "nodeKey": node.node_key,
                    "logicalOperation": node.logical_operation,
                    "scopeRefs": list(node.scope_refs),
                }
                for node in run.nodes
            ),
            diagnostic=f"{lineage_key}:{lineage_value}",
        )

    async def list_input_snapshots(self, project_id: str) -> list[dict[str, object]]:
        async with self._uow_factory() as uow:
            values = [
                item for item in uow.run_input_snapshots.values() if item.project_id == project_id
            ]
            return [
                self._snapshot_projection(item)
                for item in sorted(values, key=lambda x: x.created_at)
            ]

    async def get_input_snapshot(self, project_id: str, snapshot_id: str) -> dict[str, object]:
        async with self._uow_factory() as uow:
            snapshot = uow.run_input_snapshots.get(snapshot_id)
            if snapshot is None:
                raise ValidationDomainError("historical_snapshot_missing")
            if snapshot.project_id != project_id:
                raise ProjectAccessForbiddenError(project_id)
            return self._snapshot_projection(snapshot)

    @staticmethod
    def _snapshot_projection(snapshot: RunInputSnapshot) -> dict[str, object]:
        return {
            "id": snapshot.id,
            "schemaVersion": snapshot.schema_version,
            "revision": snapshot.revision,
            "projectId": snapshot.project_id,
            "runId": snapshot.run_id,
            "workflowVersionId": snapshot.workflow_version_id,
            "workflowContentHash": snapshot.workflow_content_hash,
            "scopeRefs": list(snapshot.scope_refs),
            "ownerRefs": list(snapshot.owner_refs),
            "selectionSnapshot": deepcopy(snapshot.selection_snapshot),
            "sourceSnapshot": deepcopy(snapshot.source_snapshot),
            "nodeInputs": list(snapshot.node_inputs),
            "runnable": snapshot.runnable,
            "diagnostic": snapshot.diagnostic,
            "createdAt": snapshot.created_at,
        }

    async def create_run_from_historical_snapshot(
        self, command: HistoricalRerunCommand
    ) -> WorkflowRun:
        async with self._uow_factory() as uow:
            snapshot = uow.run_input_snapshots.get(command.snapshot_id)
            if snapshot is None:
                raise ValidationDomainError("historical_snapshot_missing")
            if snapshot.project_id != command.project_id:
                raise ProjectAccessForbiddenError(command.project_id)
            if snapshot.revision != command.expected_snapshot_revision:
                raise RevisionConflictError(
                    snapshot.id, command.expected_snapshot_revision, snapshot.revision
                )
            if not snapshot.runnable:
                raise WorkflowRunConflictError(
                    snapshot.diagnostic or "historical_snapshot_unrunnable"
                )
            source = cast(WorkflowRun | None, uow.workflow_runs.get(snapshot.run_id))
            if source is None or source.project_id != command.project_id:
                raise WorkflowRunConflictError("historical snapshot source is unavailable")
            if any(node.submission_state == "submission_unknown" for node in source.nodes):
                raise WorkflowRunConflictError("submission_unknown requires reconciliation")
            node_keys = [str(item["nodeKey"]) for item in snapshot.node_inputs]
            selection = self._resolve_selection(
                uow,
                command.project_id,
                node_keys,
                None,
                str(snapshot.selection_snapshot.get("routeDecisionId", "")) or None,
            )
            rerun = WorkflowRun(
                snapshot.project_id,
                snapshot.workflow_version_id,
                rerun_of_run_id=source.id,
                input_snapshot={
                    "sourceSnapshotId": snapshot.id,
                    "sourceSnapshotRevision": snapshot.revision,
                    "rerunOfRunId": source.id,
                },
                selection_snapshot=deepcopy(selection),
                source_snapshot=deepcopy(snapshot.source_snapshot),
            )
            for index, item in enumerate(snapshot.node_inputs, 1):
                rerun.nodes.append(
                    NodeRun(
                        rerun.id,
                        str(item["nodeKey"]),
                        logical_operation=f"{item['nodeKey']}:historical:{index}:{uuid4()}",
                        scope_refs=tuple(item.get("scopeRefs", [])),
                    )
                )
            if not rerun.nodes:
                raise WorkflowRunConflictError("historical snapshot contains no runnable nodes")
            rerun.transition("running")
            rerun_snapshot = self._snapshot_for_lineage(
                rerun,
                source,
                "rerunOfRunId",
                source.id,
            )
            rerun.input_snapshot = {**(rerun.input_snapshot or {}), "snapshotId": rerun_snapshot.id}
            uow.workflow_runs[rerun.id] = rerun
            uow.run_input_snapshots[rerun_snapshot.id] = rerun_snapshot
            _event(
                uow,
                rerun,
                "run.historical_rerun_created",
                rerun.id,
                {"rerunOfRunId": source.id, "sourceSnapshotId": snapshot.id},
            )
            for node in rerun.nodes:
                start = _make_temporal_start(rerun, node)
                uow.temporal_starts.setdefault(start.workflow_id, start)
            await uow.commit()
            return rerun

    async def create_budget_gate(self, command: BudgetGateCommand) -> BudgetGate:
        if command.cost_status not in {"known", "unknown"}:
            raise ValidationDomainError("budget cost status is invalid")
        if command.cost_status == "known" and (
            command.estimated_cost is None or command.currency is None
        ):
            raise ValidationDomainError("known budget cost requires estimate and currency")
        if command.operation_kind.startswith("text") and (
            command.threshold_snapshot_id is None or command.threshold_revision is None
        ):
            raise ValidationDomainError("text threshold snapshot is required")
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(command.run_id))
            node = (
                next((item for item in run.nodes if item.id == command.node_run_id), None)
                if run is not None
                else None
            )
            if (
                run is None
                or node is None
                or command.logical_operation != node.logical_operation
                or node.status != "running"
            ):
                raise WorkflowRunConflictError("budget gate run scope is invalid")
            if (
                command.expected_node_revision is not None
                and node.revision != command.expected_node_revision
            ):
                raise RevisionConflictError(node.id, command.expected_node_revision, node.revision)
            key = f"{run.id}:{command.logical_operation}"
            existing = cast(BudgetGate | None, uow.budget_gates.get(key))
            if existing is not None:
                if existing.request_fingerprint != command.request_fingerprint:
                    raise WorkflowRunConflictError("budget gate fingerprint conflict")
                return existing
            gate = BudgetGate(
                run.project_id,
                run.id,
                node.id,
                command.logical_operation,
                command.request_fingerprint,
                command.operation_kind,
                command.batch_size,
                command.cost_status,  # type: ignore[arg-type]
                command.estimated_cost,
                command.currency,
                command.threshold_snapshot_id,
                command.threshold_revision,
            )
            uow.budget_gates[key] = gate
            node.transition("waiting_review")
            run.recompute_from_nodes()
            _event(
                uow,
                run,
                "budget.confirmation_required",
                run.id,
                {"logicalOperation": command.logical_operation, "costStatus": command.cost_status},
                node.id,
            )
            await uow.commit()
            return gate

    async def confirm_budget(
        self,
        run_id: str,
        logical_operation: str,
        request_fingerprint: str,
        confirmation_id: str,
        user_uuid: str,
        expected_gate_revision: int | None = None,
    ) -> BudgetGate:
        _stable_uuid(user_uuid, "budget confirmation user")
        async with self._uow_factory() as uow:
            key = f"{run_id}:{logical_operation}"
            gate = cast(BudgetGate | None, uow.budget_gates.get(key))
            run = cast(WorkflowRun | None, uow.workflow_runs.get(run_id))
            if gate is None or run is None or gate.request_fingerprint != request_fingerprint:
                raise WorkflowRunConflictError("budget confirmation is stale or mismatched")
            if expected_gate_revision is not None and gate.revision != expected_gate_revision:
                raise RevisionConflictError(gate.id, expected_gate_revision, gate.revision)
            if gate.status == "confirmed":
                if gate.confirmation_id == confirmation_id and gate.user_uuid == user_uuid:
                    return gate
                raise WorkflowRunConflictError("budget confirmation is stale or mismatched")
            node = next((item for item in run.nodes if item.id == gate.node_run_id), None)
            if (
                node is None
                or node.logical_operation != logical_operation
                or node.status != "waiting_review"
            ):
                raise WorkflowRunConflictError("budget confirmation owner state is stale")
            confirmed = replace(
                gate,
                status="confirmed",
                confirmation_id=confirmation_id,
                user_uuid=user_uuid,
                revision=gate.revision + 1,
            )
            uow.budget_gates[key] = confirmed
            node.transition("running")
            run.recompute_from_nodes()
            _event(
                uow,
                run,
                "budget.confirmed",
                confirmation_id,
                {"logicalOperation": logical_operation, "userUuid": user_uuid},
                gate.node_run_id,
            )
            await uow.commit()
            return confirmed

    async def events(
        self, run_id: str, after: int = 0, project_id: str | None = None
    ) -> list[RunEvent]:
        if after < 0:
            raise ValidationDomainError("event cursor must be a non-negative integer")
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(run_id))
            if run is None:
                raise WorkflowRunNotFoundError(run_id)
            if project_id is not None and run.project_id != project_id:
                raise ProjectAccessForbiddenError(project_id)
            return [item for item in uow.run_events.get(run_id, []) if item.sequence > after]

    async def detail(self, run_id: str, project_id: str | None = None) -> dict[str, object]:
        async with self._uow_factory() as uow:
            run = cast(WorkflowRun | None, uow.workflow_runs.get(run_id))
            if run is None:
                raise WorkflowRunNotFoundError(run_id)
            if project_id is not None and run.project_id != project_id:
                raise ProjectAccessForbiddenError(project_id)
            events = uow.run_events.get(run_id, [])[-20:]
            now = datetime.now(UTC)
            created = datetime.fromisoformat(run.created_at)
            return {
                "id": run.id,
                "projectId": run.project_id,
                "workflowVersionId": run.workflow_version_id,
                "schemaVersion": SCHEMA_VERSION,
                "revision": run.revision,
                "status": run.status,
                "sourceSnapshot": deepcopy(run.source_snapshot),
                "selectionSnapshot": deepcopy(run.selection_snapshot),
                "createdAt": run.created_at,
                "updatedAt": run.updated_at,
                "elapsedSeconds": max(0, int((now - created).total_seconds())),
                "nodes": [
                    {
                        "id": node.id,
                        "nodeKey": node.node_key,
                        "revision": node.revision,
                        "status": node.status,
                        "logicalOperation": node.logical_operation,
                        "scopeRefs": list(node.scope_refs),
                        "inputSummary": _safe_summary(node.scope_refs),
                        "outputSummary": _safe_summary(node.output_evidence or {}),
                        "failure": _safe_summary(node.failure or {}),
                        "submissionState": node.submission_state,
                    }
                    for node in run.nodes
                ],
                "recentEvents": [asdict(item) for item in events],
                "failure": next(
                    (_safe_summary(node.failure) for node in run.nodes if node.failure), None
                ),
                "allowedActions": (
                    ["cancel"]
                    if run.status in {"queued", "running", "waiting_review"}
                    else ["createSuccessor"]
                    if run.status == "failed"
                    else []
                ),
            }

    async def unsupported_mutation(self) -> None:
        raise UnsupportedFeatureError("unsupported")

    async def cleanup_events(self, run_id: str) -> dict[str, object]:
        async with self._uow_factory() as uow:
            if run_id not in uow.workflow_runs:
                raise WorkflowRunNotFoundError(run_id)
            return {
                "status": "skipped",
                "diagnostic": "run_event_long_term_no_gc",
                "retained": len(uow.run_events.get(run_id, [])),
            }
