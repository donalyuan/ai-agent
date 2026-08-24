"""AgentScope/TextModelPort boundary with deterministic Local test path."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, replace
from typing import Any, Protocol, cast
from uuid import uuid4

from video_agent_api.application.asset_bible import (
    ApplyInitialHandoffCommand,
    AssetBibleService,
    InitialEntrySpec,
)
from video_agent_api.domain.creative import (
    CreativeBriefSourceBindingSnapshot,
    CreativeBriefVersion,
)
from video_agent_api.domain.errors import ProjectAccessForbiddenError, ValidationDomainError
from video_agent_api.domain.text_review import (
    CANONICAL_TEXT_KINDS,
    TEXT_KIND_ALLOWLIST,
    StructuredTextCandidate,
    TextOwnerHandoff,
    TextOwnerHandoffAck,
    TextReviewBatch,
)
from video_agent_api.ports.contracts import AdapterNotConfiguredError, ModelSelection, TextModelPort


@dataclass(frozen=True, slots=True)
class GenerateTextBatchCommand:
    project_id: str
    run_id: str
    brief_revision: int
    selection: ModelSelection
    brief_snapshot: CreativeBriefVersion
    source_binding_snapshot: CreativeBriefSourceBindingSnapshot | None = None
    requested_kinds: tuple[str, ...] = CANONICAL_TEXT_KINDS
    scope_ids: tuple[str, ...] = ("project",)
    correlation_id: str = "local-text"


@dataclass(frozen=True, slots=True)
class RegenerateTextCandidateCommand:
    batch_id: str
    candidate_id: str
    expected_batch_revision: int
    expected_candidate_revision: int
    payload: dict[str, object]
    source_candidate_ids: tuple[str, ...]
    source_hashes: tuple[str, ...]


class TextProviderCallRecorder(Protocol):
    """Narrow catalog boundary used by Text without importing the catalog application slice."""

    async def begin_text_provider_call(
        self,
        *,
        project_id: str,
        run_id: str,
        node_run_id: str,
        logical_operation: str,
        provider_id: str,
        profile_id: str,
        model_id: str,
        capability_snapshot_id: str | None,
        request_fingerprint: str,
    ) -> object: ...

    async def claim_text_provider_call(
        self, run_id: str, logical_operation: str
    ) -> tuple[object, bool]: ...

    async def finalize_provider_call(
        self,
        run_id: str,
        logical_operation: str,
        *,
        status: str,
        failure_code: str | None = None,
        provider_request_id: str | None = None,
        native_usage: dict[str, object] | None = None,
    ) -> object: ...


def _payload_hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def _safe_native_usage(value: object) -> dict[str, object] | None:
    if not isinstance(value, dict):
        return None
    return {
        key: item
        for key, item in value.items()
        if key
        in {
            "inputTokens",
            "outputTokens",
            "totalTokens",
            "input_tokens",
            "output_tokens",
            "total_tokens",
            "characters",
            "durationMs",
            "duration_ms",
            "frames",
            "count",
            "unit",
        }
        and isinstance(item, (str, int, float, bool))
    }


def _batch_provider_request_id(batch: TextReviewBatch) -> str:
    request_ids = {
        value
        for candidate in batch.candidates
        if isinstance((value := candidate.payload.get("providerRequestId")), str) and value
    }
    if len(request_ids) != 1:
        raise ValidationDomainError("text batch ProviderCall evidence is incomplete")
    return next(iter(request_ids))


def _validate_run_selection(
    uow: Any, snapshot: dict[str, object], selection: ModelSelection
) -> None:
    expected = (
        snapshot.get("providerId"),
        snapshot.get("profileId"),
        snapshot.get("modelId"),
        snapshot.get("adapterKey"),
    )
    actual = (
        selection.provider_id,
        selection.profile_id,
        selection.model_id,
        selection.adapter_key,
    )
    if expected != actual:
        raise ValidationDomainError("text model selection does not match the Run snapshot")
    provider = uow.providers.get(selection.provider_id)
    profile = uow.profiles.get(selection.profile_id)
    model = uow.models.get(selection.model_id)
    capability = profile.capability_snapshots.get("text.generate") if profile is not None else None
    frozen_capabilities = snapshot.get("capabilitySnapshots")
    frozen_capability = (
        frozen_capabilities.get("text.generate") if isinstance(frozen_capabilities, dict) else None
    )
    if (
        provider is None
        or profile is None
        or model is None
        or profile.provider_id != provider.id
        or model.profile_id != profile.id
        or not provider.enabled
        or not provider.adapter_installed
        or provider.approval != "approved"
        or not profile.enabled
        or not model.enabled
        or provider.adapter_key != selection.adapter_key
        or profile.adapter_identity != snapshot.get("adapterIdentity")
        or profile.revision != snapshot.get("profileRevision")
        or capability is None
        or not capability.runnable
        or not isinstance(frozen_capability, dict)
        or capability.id != frozen_capability.get("id")
        or capability.revision != frozen_capability.get("revision")
        or capability.operation != "text.generate"
        or (capability.model_id is not None and capability.model_id != model.id)
    ):
        raise ValidationDomainError("text model capability snapshot is stale or unconfigured")


class TextGenerationService:
    def __init__(
        self,
        uow_factory: Any,
        provider: TextModelPort | None = None,
        catalog: TextProviderCallRecorder | None = None,
    ) -> None:
        self._uow_factory = uow_factory
        self._provider = provider
        self._catalog = catalog

    async def generate(self, command: GenerateTextBatchCommand) -> TextReviewBatch:
        if not command.requested_kinds or any(
            kind not in TEXT_KIND_ALLOWLIST for kind in command.requested_kinds
        ):
            raise ValidationDomainError("unsupported_output_type")
        if command.requested_kinds != CANONICAL_TEXT_KINDS:
            raise ValidationDomainError("text generation requires the complete canonical graph")
        if command.scope_ids != (command.project_id,):
            raise ValidationDomainError("text generation scope is invalid")
        if not command.project_id or not command.run_id or command.brief_revision < 1:
            raise ValidationDomainError("text generation owner snapshot is invalid")
        capability_snapshot_id: str | None = None
        text_node_id = ""
        text_logical_operation = ""
        async with self._uow_factory() as uow:
            project = await uow.projects.get(command.project_id)
            run = uow.workflow_runs.get(command.run_id)
            if project is None or run is None or run.project_id != command.project_id:
                raise ValidationDomainError("text generation Project/Run scope is invalid")
            if run.status != "running":
                raise ValidationDomainError("text generation requires a running Run")
            text_nodes = [node for node in run.nodes if node.node_key == "text.generate"]
            if len(text_nodes) != 1 or text_nodes[0].status != "running":
                raise ValidationDomainError(
                    "text generation requires the running text.generate NodeRun"
                )
            _validate_run_selection(uow, run.selection_snapshot, command.selection)
            text_node_id = text_nodes[0].id
            text_logical_operation = text_nodes[0].logical_operation
            raw_capability_id = run.selection_snapshot.get("capabilitySnapshotId")
            capability_snapshot_id = (
                raw_capability_id if isinstance(raw_capability_id, str) else None
            )
            current_brief = getattr(project, "creative_brief_current", None) or (
                uow.creative_brief_current.get(command.project_id)
            )
            brief = command.brief_snapshot
            if (
                current_brief is None
                or brief.project_id != command.project_id
                or brief.revision != command.brief_revision
                or brief.creative_brief_id != current_brief.creative_brief_id
                or brief.revision != current_brief.revision
                or brief.payload_hash != current_brief.payload_hash
            ):
                raise ValidationDomainError("text generation CreativeBrief snapshot is stale")
            creation_mode = getattr(project, "creation_mode", None) or uow.creative_modes.get(
                command.project_id
            )
            current_source = getattr(project, "source_binding_current", None) or (
                uow.source_bindings_current.get(command.project_id)
            )
            if creation_mode == "original":
                if command.source_binding_snapshot is not None or current_source is not None:
                    raise ValidationDomainError(
                        "original text generation must not bind SourceMaterial"
                    )
            elif creation_mode == "adaptation":
                source = command.source_binding_snapshot
                if (
                    source is None
                    or current_source is None
                    or source != current_source
                    or source.project_id != command.project_id
                    or source.creative_brief_id != brief.creative_brief_id
                    or source.creative_brief_revision != brief.revision
                    or source.creative_brief_payload_hash != brief.payload_hash
                    or source.parse_status != "parsed"
                    or source.validation_status != "valid"
                    or source.binding_status != "bound"
                ):
                    raise ValidationDomainError("adaptation SourceMaterial binding is unavailable")
            else:
                raise ValidationDomainError("text generation creationMode is unavailable")
            input_snapshot = _input_snapshot(brief, command.source_binding_snapshot)
            existing_batch = next(
                (
                    item
                    for item in uow.text_review_batches.values()
                    if item.project_id == command.project_id
                    and item.run_id == command.run_id
                    and item.brief_revision == command.brief_revision
                    and item.input_snapshot == input_snapshot
                ),
                None,
            )
        if self._provider is None and existing_batch is None:
            raise AdapterNotConfiguredError("agentscope_text_runtime_unconfigured")
        provider_prompt = {
            "projectId": command.project_id,
            "runId": command.run_id,
            "inputSnapshot": input_snapshot,
            "outputContract": {
                "type": "complete_candidate_graph",
                "schemaVersion": "1.0.0",
            },
        }
        prompt = json.dumps(provider_prompt, sort_keys=True)
        request_fingerprint = hashlib.sha256(prompt.encode()).hexdigest()
        node_run_id = text_node_id
        if self._catalog is not None:
            await self._catalog.begin_text_provider_call(
                project_id=command.project_id,
                run_id=command.run_id,
                node_run_id=node_run_id,
                logical_operation=text_logical_operation,
                provider_id=command.selection.provider_id,
                profile_id=command.selection.profile_id,
                model_id=command.selection.model_id,
                request_fingerprint=request_fingerprint,
                capability_snapshot_id=capability_snapshot_id,
            )
            if existing_batch is not None:
                await self._catalog.finalize_provider_call(
                    command.run_id,
                    text_logical_operation,
                    status="succeeded",
                    provider_request_id=_batch_provider_request_id(existing_batch),
                )
                return existing_batch
            _call, acquired = await self._catalog.claim_text_provider_call(
                command.run_id, text_logical_operation
            )
            if not acquired:
                raise ValidationDomainError("text provider operation requires reconciliation")
        elif existing_batch is not None:
            return existing_batch
        assert self._provider is not None
        result = None
        try:
            result = self._provider.generate_text(prompt, command.selection, command.correlation_id)
            candidates = _candidate_graph_from_provider(
                command, result.request_id, result.payload.get("payload")
            )
        except Exception as error:
            if self._catalog is not None:
                await self._catalog.finalize_provider_call(
                    command.run_id,
                    text_logical_operation,
                    status="failed",
                    failure_code=type(error).__name__,
                    provider_request_id=result.request_id if result is not None else None,
                )
            raise
        safe_usage = _safe_native_usage(result.payload.get("usage"))
        if self._catalog is not None:
            await self._catalog.finalize_provider_call(
                command.run_id,
                text_logical_operation,
                status="unknown",
                provider_request_id=result.request_id,
                native_usage=safe_usage,
            )
        batch = TextReviewBatch(
            command.project_id,
            command.run_id,
            command.brief_revision,
            candidates,
            input_snapshot,
        )
        async with self._uow_factory() as uow:
            existing = next(
                (
                    item
                    for item in uow.text_review_batches.values()
                    if item.fingerprint == batch.fingerprint
                ),
                None,
            )
            if existing is not None:
                return existing
            uow.text_review_batches[batch.id] = batch
            for candidate in candidates:
                uow.text_candidates[candidate.id] = candidate
            uow.audit_events.append({"type": "text.batch.generated", "batchId": batch.id})
            uow.outbox_events.append({"type": "text.batch.generated", "batchId": batch.id})
            await uow.commit()
        if self._catalog is not None:
            await self._catalog.finalize_provider_call(
                command.run_id,
                text_logical_operation,
                status="succeeded",
                provider_request_id=result.request_id,
            )
        return batch

    async def decide(
        self,
        batch_id: str,
        expected_revision: int,
        action: str,
        *,
        project_scope: str | None = None,
    ) -> TextReviewBatch:
        async with self._uow_factory() as uow:
            batch = uow.text_review_batches.get(batch_id)
            if batch is None:
                raise ValidationDomainError("text review batch not found")
            if project_scope is not None and batch.project_id != project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            decided = batch.decide(expected_revision, action)
            decided_candidates = tuple(
                replace(
                    item,
                    status="accepted" if action == "accept" else "rejected",
                )
                for item in decided.candidates
            )
            decided = replace(decided, candidates=decided_candidates)
            uow.text_review_batches[batch_id] = decided
            for candidate in decided_candidates:
                uow.text_candidates[candidate.id] = candidate
            if action == "accept":
                refs = tuple(
                    {
                        "candidateId": item.id,
                        "candidateRevision": item.revision,
                        "kind": item.kind,
                        "scopeId": item.scope_id,
                        "payloadHash": item.payload_hash,
                        "sourceCandidateIds": list(item.source_candidate_ids),
                        "sourceHashes": list(item.source_hashes),
                    }
                    for item in decided_candidates
                )
                handoff = TextOwnerHandoff(
                    decided.id,
                    decided.revision,
                    decided.project_id,
                    decided.run_id,
                    refs,
                    _payload_hash(list(refs)),
                    decided.run_id,
                    ("projects", "episodes", "scenes", "asset_bible"),
                )
                uow.text_handoffs[handoff.id] = handoff
            uow.audit_events.append({"type": f"text.batch.{decided.status}", "batchId": decided.id})
            uow.outbox_events.append(
                {"type": f"text.batch.{decided.status}", "batchId": decided.id}
            )
            await uow.commit()
            return decided

    async def get_batch(
        self, batch_id: str, *, project_scope: str | None = None
    ) -> TextReviewBatch:
        async with self._uow_factory() as uow:
            batch = uow.text_review_batches.get(batch_id)
            if batch is None:
                raise ValidationDomainError("text review batch not found")
            if project_scope is not None and batch.project_id != project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            return batch

    async def list_batches(self, project_id: str) -> list[TextReviewBatch]:
        async with self._uow_factory() as uow:
            return [
                item for item in uow.text_review_batches.values() if item.project_id == project_id
            ]

    async def ack_handoff(
        self,
        handoff_id: str,
        owner: str,
        owner_revision: int,
        fingerprint: str,
        correlation_id: str,
        *,
        project_scope: str | None = None,
    ) -> TextOwnerHandoffAck:
        async with self._uow_factory() as uow:
            handoff = uow.text_handoffs.get(handoff_id)
            if handoff is None or owner not in handoff.required_owners:
                raise ValidationDomainError("text owner handoff scope is invalid")
            if project_scope is not None and handoff.project_id != project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            if correlation_id != handoff.correlation_id:
                raise ValidationDomainError("text owner ack correlation conflict")
            key = f"{handoff.id}:{owner}"
            existing = uow.text_handoff_acks.get(key)
            if existing is not None:
                if existing.fingerprint != fingerprint:
                    raise ValidationDomainError("text owner ack fingerprint conflict")
                return existing
            ack = TextOwnerHandoffAck(
                handoff.id, owner, owner_revision, fingerprint, correlation_id
            )
            uow.text_handoff_acks[key] = ack
            uow.audit_events.append(
                {"type": "text.handoff.acknowledged", "handoffId": handoff.id, "owner": owner}
            )
            uow.outbox_events.append(
                {"type": "text.handoff.acknowledged", "handoffId": handoff.id, "owner": owner}
            )
            await uow.commit()
            return ack

    async def media_gate(
        self, handoff_id: str, *, project_scope: str | None = None
    ) -> dict[str, object]:
        async with self._uow_factory() as uow:
            handoff = uow.text_handoffs.get(handoff_id)
            if handoff is None:
                raise ValidationDomainError("text owner handoff not found")
            if project_scope is not None and handoff.project_id != project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            acknowledged = {
                item.owner
                for item in uow.text_handoff_acks.values()
                if item.handoff_id == handoff.id
            }
            missing = sorted(set(handoff.required_owners) - acknowledged)
            return {
                "status": "ready" if not missing else "blocked",
                "handoffId": handoff.id,
                "missingOwners": missing,
            }

    async def handoff_for_batch(
        self, batch_id: str, *, project_scope: str | None = None
    ) -> TextOwnerHandoff | None:
        async with self._uow_factory() as uow:
            handoff = next(
                (item for item in uow.text_handoffs.values() if item.batch_id == batch_id),
                None,
            )
            if (
                handoff is not None
                and project_scope is not None
                and handoff.project_id != project_scope
            ):
                raise ProjectAccessForbiddenError(project_scope)
            return handoff

    async def apply_asset_bible_handoff(
        self,
        handoff_id: str,
        asset_bible_service: AssetBibleService,
        actor_uuid: str,
    ) -> TextOwnerHandoffAck:
        """Apply only the accepted batch's referenced initial AssetBible specs, then ack."""
        async with self._uow_factory() as uow:
            handoff = uow.text_handoffs.get(handoff_id)
            if handoff is None or "asset_bible" not in handoff.required_owners:
                raise ValidationDomainError("text owner handoff scope is invalid")
            candidates = [
                uow.text_candidates.get(str(item["candidateId"]))
                for item in handoff.candidate_refs
                if item.get("kind") == "asset_bible_spec"
            ]
            if any(item is None or item.status != "accepted" for item in candidates):
                raise ValidationDomainError("asset bible handoff candidate set is stale")
            specs = tuple(
                InitialEntrySpec(
                    entry_id=str(item.payload["stableId"]),
                    entry_type=str(item.payload["entryType"]),
                    payload=dict(item.payload["attributes"]),
                    payload_hash=_payload_hash(dict(item.payload["attributes"])),
                )
                for item in candidates
            )
        ack = await asset_bible_service.apply_initial_handoff(
            ApplyInitialHandoffCommand(
                handoff.id,
                handoff.project_id,
                handoff.payload_hash,
                specs,
                actor_uuid,
                handoff.correlation_id,
            )
        )
        return await self.ack_handoff(
            handoff.id,
            "asset_bible",
            len(ack.entry_version_refs),
            _payload_hash(list(ack.entry_version_refs)),
            handoff.correlation_id,
        )

    async def regenerate(
        self,
        command: RegenerateTextCandidateCommand,
        *,
        project_scope: str | None = None,
    ) -> TextReviewBatch:
        async with self._uow_factory() as uow:
            batch = uow.text_review_batches.get(command.batch_id)
            if (
                batch is not None
                and project_scope is not None
                and batch.project_id != project_scope
            ):
                raise ProjectAccessForbiddenError(project_scope)
            if (
                batch is None
                or batch.status != "pending_review"
                or batch.revision != command.expected_batch_revision
            ):
                raise ValidationDomainError("text review batch is stale")
            target = next(
                (item for item in batch.candidates if item.id == command.candidate_id), None
            )
            if target is None or target.revision != command.expected_candidate_revision:
                raise ValidationDomainError("text candidate is stale or foreign")
            if len(command.source_candidate_ids) != len(command.source_hashes):
                raise ValidationDomainError("regeneration source IDs/hashes are not aligned")
            if command.source_candidate_ids != target.source_candidate_ids:
                raise ValidationDomainError("regeneration source closure is incomplete")
            supplied = dict(zip(command.source_candidate_ids, command.source_hashes, strict=True))
            by_id = {item.id: item for item in batch.candidates}
            if any(
                source_id not in by_id or by_id[source_id].payload_hash != source_hash
                for source_id, source_hash in supplied.items()
            ):
                raise ValidationDomainError("regeneration source closure is stale")

            affected = {target.id}
            changed = True
            while changed:
                changed = False
                for item in batch.candidates:
                    if item.id not in affected and affected.intersection(item.source_candidate_ids):
                        affected.add(item.id)
                        changed = True

            replacements: dict[str, StructuredTextCandidate] = {}
            successor_items: list[StructuredTextCandidate] = []
            for item in batch.candidates:
                if item.id not in affected:
                    successor_items.append(item)
                    continue
                sources = tuple(
                    replacements.get(source_id, by_id[source_id])
                    for source_id in item.source_candidate_ids
                )
                successor = replace(
                    item,
                    payload=(dict(command.payload) if item.id == target.id else dict(item.payload)),
                    source_candidate_ids=tuple(source.id for source in sources),
                    source_hashes=tuple(source.payload_hash for source in sources),
                    revision=item.revision + 1,
                    id=str(uuid4()),
                    payload_hash="",
                    status="provisional",
                    supersedes_id=item.id,
                )
                replacements[item.id] = successor
                successor_items.append(successor)
            stale = replace(batch, status="stale", revision=batch.revision + 1)
            successor_batch = TextReviewBatch(
                batch.project_id,
                batch.run_id,
                batch.brief_revision,
                tuple(successor_items),
                batch.input_snapshot,
                supersedes_batch_id=batch.id,
            )
            uow.text_review_batches[batch.id] = stale
            uow.text_review_batches[successor_batch.id] = successor_batch
            for item in replacements.values():
                uow.text_candidates[item.id] = item
            uow.audit_events.append(
                {
                    "type": "text.batch.successor_created",
                    "batchId": successor_batch.id,
                    "supersedesBatchId": batch.id,
                }
            )
            uow.outbox_events.append(
                {
                    "type": "text.batch.successor_created",
                    "batchId": successor_batch.id,
                }
            )
            await uow.commit()
            return successor_batch


def _input_snapshot(
    brief: CreativeBriefVersion,
    source: CreativeBriefSourceBindingSnapshot | None,
) -> dict[str, object]:
    creative_brief: dict[str, object] = {
        "projectId": brief.project_id,
        "creativeBriefId": brief.creative_brief_id,
        "subject": brief.subject,
        "genre": brief.genre,
        "audience": brief.audience,
        "characterPremise": brief.character_premise,
        "style": brief.style,
        "episodeDurationSeconds": brief.episode_duration_seconds,
        "episodeCount": brief.episode_count,
        "scenesPerEpisode": brief.scenes_per_episode,
        "shotsPerScene": brief.shots_per_scene,
        "revision": brief.revision,
        "schema_version": brief.schema_version,
        "payloadHash": brief.payload_hash,
    }
    source_snapshot: dict[str, object] | None = None
    if source is not None:
        source_snapshot = {
            "projectId": source.project_id,
            "sourceMaterialId": source.source_material_id,
            "sourceMaterialRevision": source.source_material_revision,
            "sourceContentHash": source.source_content_hash,
            "creativeBriefId": source.creative_brief_id,
            "creativeBriefRevision": source.creative_brief_revision,
            "creativeBriefPayloadHash": source.creative_brief_payload_hash,
            "parseStatus": source.parse_status,
            "validationStatus": source.validation_status,
            "bindingStatus": source.binding_status,
            "bindingVersion": source.binding_version,
            "schema_version": source.schema_version,
        }
    return {"creativeBrief": creative_brief, "sourceBinding": source_snapshot}


_OUTPUT_FIELDS = {
    "story_spec": {"logline"},
    "script_spec": {"episodeNumber", "durationSeconds"},
    "episode": {"episodeNumber"},
    "scene": {"episodeId", "sceneNumber"},
    "shot": {"sceneId", "shotNumber"},
    "shot_spec": {"durationFrames", "assetBibleRefs"},
    "asset_bible_spec": {"entryType", "stableId", "attributes"},
}
_SOURCE_KINDS = {
    "story_spec": (),
    "script_spec": ("story_spec",),
    "episode": ("script_spec",),
    "scene": ("episode",),
    "shot": ("scene",),
    "shot_spec": ("shot",),
    "asset_bible_spec": ("story_spec",),
}
_ASSET_BIBLE_TYPES = {
    "character",
    "look",
    "location",
    "scene_visual",
    "prop",
    "visual_style",
}


def _positive_int(value: object) -> bool:
    return not isinstance(value, bool) and isinstance(value, int) and value > 0


def _validate_candidate_fields(
    kind: str,
    scope_id: str,
    fields: dict[str, object],
    sources: tuple[StructuredTextCandidate, ...],
) -> None:
    if set(fields) != _OUTPUT_FIELDS[kind]:
        raise ValidationDomainError("structured model output violates candidate schema")
    if kind == "story_spec":
        valid = isinstance(fields["logline"], str) and bool(fields["logline"])
    elif kind in {"script_spec", "episode"}:
        valid = _positive_int(fields["episodeNumber"])
        if kind == "script_spec":
            valid = valid and _positive_int(fields["durationSeconds"])
        valid = valid and sources[0].scope_id in {scope_id, sources[0].project_id}
    elif kind == "scene":
        valid = (
            isinstance(fields["episodeId"], str)
            and fields["episodeId"] == sources[0].scope_id
            and _positive_int(fields["sceneNumber"])
        )
    elif kind == "shot":
        valid = (
            isinstance(fields["sceneId"], str)
            and fields["sceneId"] == sources[0].scope_id
            and _positive_int(fields["shotNumber"])
        )
    elif kind == "shot_spec":
        refs = fields["assetBibleRefs"]
        valid = (
            scope_id == sources[0].scope_id
            and _positive_int(fields["durationFrames"])
            and isinstance(refs, list)
            and len(refs) == 6
            and all(isinstance(item, str) and item for item in refs)
            and len(set(refs)) == 6
        )
    else:
        valid = (
            scope_id == sources[0].project_id
            and fields["entryType"] in _ASSET_BIBLE_TYPES
            and isinstance(fields["stableId"], str)
            and bool(fields["stableId"])
            and isinstance(fields["attributes"], dict)
            and bool(fields["attributes"])
        )
    if not valid:
        raise ValidationDomainError("structured model output violates candidate schema")


def _validate_complete_candidate_graph(
    command: GenerateTextBatchCommand,
    candidates: tuple[StructuredTextCandidate, ...],
) -> None:
    brief = command.brief_snapshot
    expected_counts = {
        "story_spec": 1,
        "script_spec": brief.episode_count,
        "episode": brief.episode_count,
        "scene": brief.episode_count * brief.scenes_per_episode,
        "shot": brief.episode_count * brief.scenes_per_episode * brief.shots_per_scene,
        "shot_spec": brief.episode_count * brief.scenes_per_episode * brief.shots_per_scene,
        "asset_bible_spec": len(_ASSET_BIBLE_TYPES),
    }
    by_kind = {
        kind: tuple(item for item in candidates if item.kind == kind)
        for kind in TEXT_KIND_ALLOWLIST
    }
    if {kind: len(items) for kind, items in by_kind.items()} != expected_counts:
        raise ValidationDomainError("structured model output candidate graph is incomplete")

    story = by_kind["story_spec"][0]
    if story.scope_id != command.project_id:
        raise ValidationDomainError("structured model output candidate scope is invalid")

    asset_bible = by_kind["asset_bible_spec"]
    asset_types = {item.payload["entryType"] for item in asset_bible}
    asset_refs = {item.payload["stableId"] for item in asset_bible}
    if asset_types != _ASSET_BIBLE_TYPES or len(asset_refs) != len(_ASSET_BIBLE_TYPES):
        raise ValidationDomainError("structured model output AssetBible closure is invalid")

    def indexed(
        items: tuple[StructuredTextCandidate, ...], field: str, expected: int
    ) -> dict[int, StructuredTextCandidate]:
        result = {cast(int, item.payload[field]): item for item in items}
        if len(result) != len(items) or set(result) != set(range(1, expected + 1)):
            raise ValidationDomainError(
                "structured model output numbering or cardinality is invalid"
            )
        return result

    scripts = indexed(by_kind["script_spec"], "episodeNumber", brief.episode_count)
    episodes = indexed(by_kind["episode"], "episodeNumber", brief.episode_count)
    for episode_number in range(1, brief.episode_count + 1):
        script = scripts[episode_number]
        episode = episodes[episode_number]
        if (
            script.scope_id != episode.scope_id
            or script.payload["durationSeconds"] != brief.episode_duration_seconds
            or episode.source_candidate_ids != (script.id,)
        ):
            raise ValidationDomainError("structured model output episode closure is invalid")
        scenes = tuple(
            item for item in by_kind["scene"] if item.source_candidate_ids == (episode.id,)
        )
        scenes_by_number = indexed(scenes, "sceneNumber", brief.scenes_per_episode)
        for scene_number in range(1, brief.scenes_per_episode + 1):
            scene = scenes_by_number[scene_number]
            shots = tuple(
                item for item in by_kind["shot"] if item.source_candidate_ids == (scene.id,)
            )
            shots_by_number = indexed(shots, "shotNumber", brief.shots_per_scene)
            for shot_number in range(1, brief.shots_per_scene + 1):
                shot = shots_by_number[shot_number]
                shot_specs = tuple(
                    item for item in by_kind["shot_spec"] if item.source_candidate_ids == (shot.id,)
                )
                if (
                    len(shot_specs) != 1
                    or shot_specs[0].scope_id != shot.scope_id
                    or set(cast(list[str], shot_specs[0].payload["assetBibleRefs"])) != asset_refs
                ):
                    raise ValidationDomainError(
                        "structured model output ShotSpec closure is invalid"
                    )


def _candidate_graph_from_provider(
    command: GenerateTextBatchCommand,
    provider_request_id: str,
    output: object,
) -> tuple[StructuredTextCandidate, ...]:
    if not isinstance(output, dict) or set(output) != {"candidates"}:
        raise ValidationDomainError("structured model output must contain a candidate graph")
    raw_candidates = output["candidates"]
    if not isinstance(raw_candidates, list) or not raw_candidates:
        raise ValidationDomainError("structured model output candidate graph is invalid")
    generated: list[StructuredTextCandidate] = []
    by_key: dict[str, StructuredTextCandidate] = {}
    for raw in raw_candidates:
        if not isinstance(raw, dict) or set(raw) != {
            "key",
            "kind",
            "scopeId",
            "sourceKeys",
            "payload",
        }:
            raise ValidationDomainError("structured model output candidate graph is invalid")
        key = raw["key"]
        kind = raw["kind"]
        scope_id = raw["scopeId"]
        source_keys = raw["sourceKeys"]
        fields = raw["payload"]
        if (
            not isinstance(key, str)
            or not key
            or key in by_key
            or not isinstance(kind, str)
            or kind not in TEXT_KIND_ALLOWLIST
            or not isinstance(scope_id, str)
            or not scope_id
            or not isinstance(source_keys, list)
            or any(not isinstance(item, str) or not item for item in source_keys)
            or len(source_keys) != len(set(source_keys))
            or not isinstance(fields, dict)
            or any(not isinstance(item, str) for item in fields)
        ):
            raise ValidationDomainError("structured model output candidate graph is invalid")
        try:
            sources = tuple(by_key[item] for item in source_keys)
        except KeyError as error:
            raise ValidationDomainError(
                "structured model output source closure is invalid"
            ) from error
        expected_source_kinds = _SOURCE_KINDS[kind]
        if tuple(item.kind for item in sources) != expected_source_kinds:
            raise ValidationDomainError("structured model output source closure is invalid")
        _validate_candidate_fields(kind, scope_id, fields, sources)
        candidate = StructuredTextCandidate(
            command.project_id,
            kind,  # type: ignore[arg-type]
            scope_id,
            {
                "kind": kind,
                "status": "generated",
                "providerRequestId": provider_request_id,
                "modelId": command.selection.model_id,
                "scopeId": scope_id,
                "schema_version": "1.0.0",
                **fields,
            },
            tuple(item.id for item in sources),
            tuple(item.payload_hash for item in sources),
            run_id=command.run_id,
        )
        generated.append(candidate)
        by_key[key] = candidate
    candidates = tuple(generated)
    _validate_complete_candidate_graph(command, candidates)
    return candidates
