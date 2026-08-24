"""AssetBible application commands and owner-safe projections."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Protocol

from video_agent_api.application.ports import (
    AssetBibleUnitOfWork,
    AssetBibleUnitOfWorkFactory,
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
    canonical_hash,
    resolve_assignments,
    validate_reference_payload,
    validate_relationship,
)
from video_agent_api.domain.errors import (
    ProjectNotFoundError,
    RevisionConflictError,
    ValidationDomainError,
)

SYSTEM_ACTOR_UUID = "00000000-0000-4000-8000-000000000001"


@dataclass(frozen=True, slots=True)
class CreateEntryCommand:
    project_id: str
    entry_type: str


@dataclass(frozen=True, slots=True)
class UpdateEntryCommand:
    project_id: str
    entry_id: str
    payload: dict[str, object]
    expected_revision: int
    actor_uuid: str = SYSTEM_ACTOR_UUID
    reference_asset_version_refs: tuple[OwnerReference, ...] = ()
    generation_spec_refs: tuple[OwnerReference, ...] = ()


@dataclass(frozen=True, slots=True)
class DisableEntryCommand:
    project_id: str
    entry_id: str
    expected_revision: int


@dataclass(frozen=True, slots=True)
class AssignContinuityCommand:
    project_id: str
    level: str
    target_id: str
    entry_id: str
    version_id: str
    version_revision: int
    content_hash: str
    scope_revision: int = 1


@dataclass(frozen=True, slots=True)
class CreateRelationshipCommand:
    project_id: str
    source_entry_id: str
    target_entry_id: str
    kind: str


@dataclass(frozen=True, slots=True)
class PreviewImpactCommand:
    project_id: str
    entry_id: str
    expected_revision: int
    payload: dict[str, object]
    actor_uuid: str = SYSTEM_ACTOR_UUID
    reference_asset_version_refs: tuple[OwnerReference, ...] = ()
    generation_spec_refs: tuple[OwnerReference, ...] = ()
    owner_projection_complete: bool = True
    diagnostic: str | None = None


@dataclass(frozen=True, slots=True)
class AcceptImpactCommand:
    project_id: str
    entry_id: str
    analysis_id: str
    expected_analysis_revision: int
    expected_entry_revision: int
    expected_asset_bible_revision: int
    candidate_payload_hash: str
    target_refs: tuple[ContinuityImpactTarget, ...]
    target_set_hash: str
    actor_uuid: str
    correlation_id: str


@dataclass(frozen=True, slots=True)
class InitialEntrySpec:
    entry_id: str
    entry_type: str
    payload: dict[str, object]
    payload_hash: str


@dataclass(frozen=True, slots=True)
class ApplyInitialHandoffCommand:
    handoff_id: str
    project_id: str
    payload_hash: str
    specs: tuple[InitialEntrySpec, ...]
    actor_uuid: str
    correlation_id: str


@dataclass(frozen=True, slots=True)
class OwnerProjectionResult:
    targets: tuple[ContinuityImpactTarget, ...]
    complete: bool
    diagnostic: str | None = None


class ContinuityOwnerQueryPort(Protocol):
    async def find_references(
        self, project_id: str, entry_id: str, version_id: str
    ) -> OwnerProjectionResult: ...


def _entry_for_project(
    uow: AssetBibleUnitOfWork, project_id: str, entry_id: str
) -> AssetBibleEntry:
    entry = uow.asset_bible_entries.get(entry_id)
    if entry is None or entry.project_id != project_id:
        raise ValidationDomainError("asset bible entry scope is invalid")
    return entry


def _bible_for_project(uow: AssetBibleUnitOfWork, project_id: str) -> AssetBible:
    bible = uow.asset_bibles_by_project.get(project_id)
    if bible is None:
        raise ValidationDomainError("asset bible is unavailable")
    return bible


def _target_revision(
    uow: AssetBibleUnitOfWork, level: str, target_id: str, project_id: str
) -> int | None:
    if level == "project":
        return None
    if level == "episode":
        # Episode repository is async, so episode validation is performed by the caller.
        return 1
    if level == "scene":
        scene = uow.scenes.get(target_id)
        return scene.revision if scene is not None and scene.project_id == project_id else None
    if level == "shot":
        shot = uow.shots.get(target_id)
        return shot.revision if shot is not None and shot.project_id == project_id else None
    return None


class AssetBibleService:
    def __init__(
        self,
        uow_factory: AssetBibleUnitOfWorkFactory,
        owner_query: ContinuityOwnerQueryPort | None = None,
    ) -> None:
        self._uow_factory = uow_factory
        self._owner_query = owner_query

    async def create_entry(self, command: CreateEntryCommand) -> AssetBibleEntry:
        async with self._uow_factory() as uow:
            if await uow.projects.get(command.project_id) is None:
                raise ProjectNotFoundError(command.project_id)
            bible = uow.asset_bibles_by_project.get(command.project_id)
            if bible is None:
                bible = AssetBible(command.project_id)
                uow.asset_bibles_by_project[command.project_id] = bible
            item = AssetBibleEntry(
                command.project_id,
                bible.id,
                command.entry_type,  # type: ignore[arg-type]
            )
            uow.asset_bible_entries[item.id] = item
            uow.asset_bible_by_project.setdefault(command.project_id, []).append(item)
            uow.audit_events.append({"type": "asset-bible.entry.created", "entryId": item.id})
            uow.outbox_events.append({"type": "asset-bible.entry.created", "entryId": item.id})
            await uow.commit()
            return item

    async def update_entry(self, command: UpdateEntryCommand) -> AssetBibleVersion:
        async with self._uow_factory() as uow:
            entry = _entry_for_project(uow, command.project_id, command.entry_id)
            bible = _bible_for_project(uow, command.project_id)
            self._validate_typed_payload(uow, entry, command.payload)
            version = entry.successor(
                command.payload,
                command.expected_revision,
                command.actor_uuid,
                command.reference_asset_version_refs,
                command.generation_spec_refs,
            )
            bible.set_current(entry.id, version.id, bible.revision)
            uow.audit_events.append(
                {"type": "asset-bible.version.appended", "versionId": version.id}
            )
            uow.outbox_events.append(
                {"type": "asset-bible.version.appended", "versionId": version.id}
            )
            await uow.commit()
            return version

    async def disable_entry(self, command: DisableEntryCommand) -> AssetBibleEntry:
        async with self._uow_factory() as uow:
            entry = _entry_for_project(uow, command.project_id, command.entry_id)
            entry.disable(command.expected_revision)
            uow.audit_events.append({"type": "asset-bible.entry.disabled", "entryId": entry.id})
            uow.outbox_events.append({"type": "asset-bible.entry.disabled", "entryId": entry.id})
            await uow.commit()
            return entry

    @staticmethod
    def _validate_typed_payload(
        uow: AssetBibleUnitOfWork, entry: AssetBibleEntry, payload: dict[str, object]
    ) -> None:
        validate_reference_payload(payload)
        required_ref = {"look": "characterRef", "scene_visual": "locationRef"}.get(entry.entry_type)
        if required_ref is None:
            return
        reference = payload.get(required_ref)
        if not isinstance(reference, dict):
            raise ValidationDomainError(f"{entry.entry_type} requires {required_ref}")
        target = uow.asset_bible_entries.get(str(reference.get("entryId", "")))
        expected_type = "character" if entry.entry_type == "look" else "location"
        if (
            target is None
            or target.project_id != entry.project_id
            or target.entry_type != expected_type
            or target.current is None
            or target.disabled
            or reference.get("versionId") != target.current.id
            or reference.get("revision") != target.current.revision
            or reference.get("hash") != target.current.content_hash
        ):
            raise ValidationDomainError("typed asset bible reference is stale or foreign")

    async def assign(self, command: AssignContinuityCommand) -> ContinuityAssignment:
        async with self._uow_factory() as uow:
            entry = _entry_for_project(uow, command.project_id, command.entry_id)
            if entry.disabled:
                raise ValidationDomainError("continuity version is disabled")
            version = next((item for item in entry.versions if item.id == command.version_id), None)
            if (
                version is None
                or version.revision != command.version_revision
                or version.content_hash != command.content_hash
            ):
                raise ValidationDomainError("continuity version is stale or foreign")
            actual_scope_revision = await self._scope_revision(
                uow, command.project_id, command.level, command.target_id
            )
            if actual_scope_revision != command.scope_revision:
                raise RevisionConflictError(
                    command.target_id, command.scope_revision, actual_scope_revision or 0
                )
            duplicate = next(
                (
                    item
                    for item in uow.asset_bible_assignments
                    if item.project_id == command.project_id
                    and item.level == command.level
                    and item.target_id == command.target_id
                    and item.entry_id == command.entry_id
                ),
                None,
            )
            if duplicate is not None:
                if (
                    duplicate.version_id == command.version_id
                    and duplicate.scope_revision == command.scope_revision
                ):
                    return duplicate
                raise ValidationDomainError("continuity assignment is ambiguous")
            assignment = ContinuityAssignment(
                command.project_id,
                command.level,
                command.target_id,
                command.entry_id,
                command.version_id,
                command.version_revision,
                command.content_hash,
                scope_revision=command.scope_revision,
            )
            uow.asset_bible_assignments.append(assignment)
            uow.audit_events.append(
                {"type": "asset-bible.assignment.created", "assignmentId": assignment.id}
            )
            uow.outbox_events.append(
                {"type": "asset-bible.assignment.created", "assignmentId": assignment.id}
            )
            await uow.commit()
            return assignment

    @staticmethod
    async def _scope_revision(
        uow: AssetBibleUnitOfWork, project_id: str, level: str, target_id: str
    ) -> int | None:
        if level == "project":
            project = await uow.projects.get(target_id)
            return project.revision if project is not None and project.id == project_id else None
        if level == "episode":
            episode = await uow.episodes.get(target_id)
            return (
                episode.revision
                if episode is not None and episode.project_id == project_id
                else None
            )
        return _target_revision(uow, level, target_id, project_id)

    async def resolve(
        self,
        project_id: str,
        target_id: str,
        scope_ids: dict[str, str] | None = None,
        *,
        persist: bool = True,
    ) -> ResolvedContinuitySnapshot:
        async with self._uow_factory() as uow:
            scopes = scope_ids or {"shot": target_id}
            if set(scopes) - {"project", "episode", "scene", "shot"}:
                raise ValidationDomainError("continuity resolution scope is invalid")
            target_type = next(
                (
                    level
                    for level in reversed(("project", "episode", "scene", "shot"))
                    if level in scopes
                ),
                "shot",
            )
            target_revision = await self._scope_revision(uow, project_id, target_type, target_id)
            if target_revision is None:
                raise ValidationDomainError("continuity resolution target is stale or foreign")
            allowed_targets = set(scopes.values())
            assignments = [
                item
                for item in uow.asset_bible_assignments
                if item.project_id == project_id and item.target_id in allowed_targets
            ]
            for assignment in assignments:
                entry = _entry_for_project(uow, project_id, assignment.entry_id)
                version = next(
                    (item for item in entry.versions if item.id == assignment.version_id), None
                )
                if (
                    entry.disabled
                    or version is None
                    or version.revision != assignment.version_revision
                    or version.content_hash != assignment.content_hash
                ):
                    raise ValidationDomainError(
                        "continuity assignment version is stale or disabled"
                    )
                current_scope_revision = await self._scope_revision(
                    uow, project_id, assignment.level, assignment.target_id
                )
                if current_scope_revision != assignment.scope_revision:
                    raise ValidationDomainError("continuity assignment scope revision drift")
            snapshot = resolve_assignments(
                project_id,
                target_id,
                assignments,
                target_type=target_type,  # type: ignore[arg-type]
                target_revision=target_revision,
            )
            if persist:
                uow.asset_bible_snapshots[snapshot.id] = snapshot
                await uow.commit()
            return snapshot

    async def create_relationship(
        self, command: CreateRelationshipCommand
    ) -> AssetBibleRelationship:
        async with self._uow_factory() as uow:
            source = _entry_for_project(uow, command.project_id, command.source_entry_id)
            target = _entry_for_project(uow, command.project_id, command.target_entry_id)
            relationship = validate_relationship(
                source, target, command.kind, uow.asset_bible_relationships
            )
            duplicate = next(
                (
                    item
                    for item in uow.asset_bible_relationships
                    if item.source_entry_id == relationship.source_entry_id
                    and item.target_entry_id == relationship.target_entry_id
                    and item.kind == relationship.kind
                ),
                None,
            )
            if duplicate is not None:
                return duplicate
            uow.asset_bible_relationships.append(relationship)
            uow.audit_events.append(
                {"type": "asset-bible.relationship.created", "relationshipId": relationship.id}
            )
            uow.outbox_events.append(
                {"type": "asset-bible.relationship.created", "relationshipId": relationship.id}
            )
            await uow.commit()
            return relationship

    async def _projection(
        self, uow: AssetBibleUnitOfWork, project_id: str, entry: AssetBibleEntry
    ) -> OwnerProjectionResult:
        current = entry.current
        if current is None:
            raise ValidationDomainError("asset bible entry current version is unavailable")
        if self._owner_query is not None:
            return await self._owner_query.find_references(project_id, entry.id, current.id)
        targets: list[ContinuityImpactTarget] = []
        snapshot_targets: set[str] = set()
        for snapshot in uow.asset_bible_snapshots.values():
            if snapshot.project_id != project_id or snapshot.target_type == "project":
                continue
            if not any(
                item.entry_id == entry.id and item.version_id == current.id
                for item in snapshot.refs
            ):
                continue
            snapshot_targets.add(snapshot.target_id)
            current_target_revision = await self._scope_revision(
                uow, project_id, snapshot.target_type, snapshot.target_id
            )
            if current_target_revision is None:
                return OwnerProjectionResult((), False, "owner_projection_revision_drift")
            targets.append(
                ContinuityImpactTarget(
                    target_type=snapshot.target_type,
                    target_id=snapshot.target_id,
                    target_revision=current_target_revision,
                    reason="resolved_version_reference",
                    snapshot_id=snapshot.id,
                    snapshot_hash=snapshot.content_hash,
                )
            )
        direct_targets = {
            item.target_id
            for item in uow.asset_bible_assignments
            if item.project_id == project_id
            and item.entry_id == entry.id
            and item.version_id == current.id
            and item.level != "project"
        }
        missing_snapshots = direct_targets - snapshot_targets
        return OwnerProjectionResult(
            tuple(sorted(targets, key=lambda item: (item.target_type, item.target_id))),
            not missing_snapshots,
            "owner_projection_incomplete" if missing_snapshots else None,
        )

    async def preview_impact(self, command: PreviewImpactCommand) -> ContinuityImpactAnalysis:
        async with self._uow_factory() as uow:
            entry = _entry_for_project(uow, command.project_id, command.entry_id)
            if entry.current is None:
                raise ValidationDomainError("asset bible entry current version is unavailable")
            if entry.revision != command.expected_revision:
                raise RevisionConflictError(entry.id, command.expected_revision, entry.revision)
            self._validate_typed_payload(uow, entry, command.payload)
            candidate = AssetBibleVersion(
                entry_id=entry.id,
                project_id=entry.project_id,
                entry_type=entry.entry_type,
                payload=command.payload,
                version_number=len(entry.versions) + 1,
                actor_uuid=command.actor_uuid,
                reference_asset_version_refs=command.reference_asset_version_refs,
                generation_spec_refs=command.generation_spec_refs,
            )
            projection = await self._projection(uow, command.project_id, entry)
            complete = command.owner_projection_complete and projection.complete
            diagnostic = command.diagnostic or projection.diagnostic
            analysis = ContinuityImpactAnalysis(
                project_id=command.project_id,
                entry_id=command.entry_id,
                base_version_id=entry.current.id,
                candidate_payload_hash=candidate.content_hash,
                target_refs=projection.targets,
                status="complete" if complete else "incomplete",
                diagnostic=diagnostic,
                candidate_payload=dict(command.payload),
                reference_asset_version_refs=command.reference_asset_version_refs,
                generation_spec_refs=command.generation_spec_refs,
            )
            uow.asset_bible_impacts[analysis.id] = analysis
            await uow.commit()
            return analysis

    async def accept_impact(
        self, command: AcceptImpactCommand
    ) -> tuple[AssetBibleAcceptDecision, AssetBibleVersion, list[ContinuityRevisionTask]]:
        fingerprint = canonical_hash(
            {
                "projectId": command.project_id,
                "entryId": command.entry_id,
                "analysisId": command.analysis_id,
                "expectedAnalysisRevision": command.expected_analysis_revision,
                "expectedEntryRevision": command.expected_entry_revision,
                "expectedAssetBibleRevision": command.expected_asset_bible_revision,
                "candidatePayloadHash": command.candidate_payload_hash,
                "targetRefs": [item.canonical_value() for item in command.target_refs],
                "targetSetHash": command.target_set_hash,
                "actorUuid": command.actor_uuid,
                "correlationId": command.correlation_id,
            }
        )
        async with self._uow_factory() as uow:
            previous = uow.asset_bible_decisions.get(fingerprint)
            if previous is not None:
                decision, version, task_ids = previous
                return decision, version, [uow.asset_bible_tasks[item] for item in task_ids]
            entry = _entry_for_project(uow, command.project_id, command.entry_id)
            bible = _bible_for_project(uow, command.project_id)
            analysis = uow.asset_bible_impacts.get(command.analysis_id)
            if entry.current is None:
                raise ValidationDomainError("asset bible entry current version is unavailable")
            if entry.revision != command.expected_entry_revision:
                raise RevisionConflictError(
                    entry.id, command.expected_entry_revision, entry.revision
                )
            if bible.revision != command.expected_asset_bible_revision:
                raise RevisionConflictError(
                    bible.id, command.expected_asset_bible_revision, bible.revision
                )
            if (
                analysis is None
                or analysis.project_id != command.project_id
                or analysis.entry_id != command.entry_id
                or analysis.revision != command.expected_analysis_revision
                or analysis.status != "complete"
                or analysis.base_version_id != entry.current.id
                or analysis.candidate_payload_hash != command.candidate_payload_hash
                or analysis.target_refs != command.target_refs
                or analysis.target_set_hash != command.target_set_hash
            ):
                raise ValidationDomainError("continuity impact analysis is stale or incomplete")
            current_projection = await self._projection(uow, command.project_id, entry)
            if (
                not current_projection.complete
                or current_projection.targets != command.target_refs
                or canonical_hash([item.canonical_value() for item in current_projection.targets])
                != command.target_set_hash
            ):
                raise ValidationDomainError("continuity impact target set is stale")
            old_version = entry.current
            version = entry.successor(
                analysis.candidate_payload,
                command.expected_entry_revision,
                command.actor_uuid,
                analysis.reference_asset_version_refs,
                analysis.generation_spec_refs,
            )
            bible.set_current(entry.id, version.id, command.expected_asset_bible_revision)
            tasks: list[ContinuityRevisionTask] = []
            for target in command.target_refs:
                task = ContinuityRevisionTask(
                    project_id=command.project_id,
                    target_id=target.target_id,
                    entry_id=entry.id,
                    target_revision=target.target_revision,
                    old_version_id=old_version.id,
                    new_version_id=version.id,
                    snapshot_id=target.snapshot_id,
                    snapshot_hash=target.snapshot_hash,
                    reason=target.reason,
                    correlation_id=command.correlation_id,
                    target_type=target.target_type,
                )
                dedupe_key = (task.target_type, task.target_id, task.entry_id, task.new_version_id)
                existing = next(
                    (
                        item
                        for item in uow.asset_bible_tasks.values()
                        if (item.target_type, item.target_id, item.entry_id, item.new_version_id)
                        == dedupe_key
                    ),
                    None,
                )
                if existing is None:
                    uow.asset_bible_tasks[task.id] = task
                    tasks.append(task)
                else:
                    tasks.append(existing)
            decision = AssetBibleAcceptDecision(
                command.project_id,
                entry.id,
                analysis.id,
                old_version.id,
                version.id,
                command.target_set_hash,
                command.actor_uuid,
                command.correlation_id,
                fingerprint,
            )
            uow.asset_bible_decisions[fingerprint] = (
                decision,
                version,
                tuple(item.id for item in tasks),
            )
            uow.audit_events.append(
                {"type": "asset-bible.revision.accepted", "decisionId": decision.id}
            )
            uow.outbox_events.append(
                {"type": "asset-bible.revision.accepted", "decisionId": decision.id}
            )
            await uow.commit()
            return decision, version, tasks

    async def transition_task(
        self, project_id: str, task_id: str, target: str, expected_revision: int
    ) -> ContinuityRevisionTask:
        async with self._uow_factory() as uow:
            task = uow.asset_bible_tasks.get(task_id)
            if task is None or task.project_id != project_id:
                raise ValidationDomainError("continuity task scope is invalid")
            task.transition(target, expected_revision)
            uow.audit_events.append({"type": "continuity.task.transitioned", "taskId": task.id})
            uow.outbox_events.append({"type": "continuity.task.transitioned", "taskId": task.id})
            await uow.commit()
            return task

    async def apply_initial_handoff(
        self, command: ApplyInitialHandoffCommand
    ) -> AssetBibleHandoffAck:
        fingerprint = canonical_hash(
            {
                "handoffId": command.handoff_id,
                "projectId": command.project_id,
                "payloadHash": command.payload_hash,
                "specs": command.specs,
            }
        )
        async with self._uow_factory() as uow:
            existing = uow.asset_bible_handoff_acks.get(command.handoff_id)
            if existing is not None:
                if existing[0] != fingerprint:
                    raise ValidationDomainError("asset bible handoff fingerprint conflict")
                return existing[1]
            if await uow.projects.get(command.project_id) is None:
                raise ProjectNotFoundError(command.project_id)
            bible = uow.asset_bibles_by_project.get(command.project_id)
            if bible is None:
                bible = AssetBible(command.project_id)
                uow.asset_bibles_by_project[command.project_id] = bible
            refs: list[tuple[str, str, int, str]] = []
            for spec in command.specs:
                if canonical_hash(spec.payload) != spec.payload_hash:
                    raise ValidationDomainError("asset bible initial spec hash mismatch")
                entry = uow.asset_bible_entries.get(spec.entry_id)
                if entry is None:
                    entry = AssetBibleEntry(
                        command.project_id,
                        bible.id,
                        spec.entry_type,  # type: ignore[arg-type]
                        id=spec.entry_id,
                    )
                    uow.asset_bible_entries[entry.id] = entry
                    uow.asset_bible_by_project.setdefault(command.project_id, []).append(entry)
                elif entry.project_id != command.project_id or entry.entry_type != spec.entry_type:
                    raise ValidationDomainError("asset bible initial spec identity conflict")
            for spec in command.specs:
                entry = uow.asset_bible_entries[spec.entry_id]
                payload = dict(spec.payload)
                relationship_key = {
                    "look": ("characterEntryId", "characterRef"),
                    "scene_visual": ("locationEntryId", "locationRef"),
                }.get(entry.entry_type)
                if relationship_key is not None:
                    source_key, target_key = relationship_key
                    target = uow.asset_bible_entries.get(str(payload.pop(source_key, "")))
                    if target is None or target.current is None:
                        raise ValidationDomainError(
                            "asset bible initial relationship is incomplete"
                        )
                    payload[target_key] = {
                        "entryId": target.id,
                        "versionId": target.current.id,
                        "revision": target.current.revision,
                        "hash": target.current.content_hash,
                    }
                self._validate_typed_payload(uow, entry, payload)
                if entry.current is None:
                    version = entry.successor(payload, entry.revision, command.actor_uuid)
                    bible.set_current(entry.id, version.id, bible.revision)
                elif (
                    entry.current.content_hash
                    != AssetBibleVersion(
                        entry_id=entry.id,
                        project_id=entry.project_id,
                        entry_type=entry.entry_type,
                        payload=payload,
                        version_number=entry.current.version_number,
                        actor_uuid=command.actor_uuid,
                    ).content_hash
                ):
                    raise ValidationDomainError("asset bible initial spec current conflict")
                current = entry.current
                if current is None:
                    raise ValidationDomainError("asset bible initial current is unavailable")
                refs.append((entry.id, current.id, current.revision, current.content_hash))
            ack = AssetBibleHandoffAck(
                command.handoff_id,
                command.project_id,
                command.payload_hash,
                tuple(sorted(refs)),
                command.correlation_id,
            )
            uow.asset_bible_handoff_acks[command.handoff_id] = (fingerprint, ack)
            uow.audit_events.append(
                {"type": "asset-bible.handoff.acknowledged", "handoffId": command.handoff_id}
            )
            uow.outbox_events.append(
                {"type": "asset-bible.handoff.acknowledged", "handoffId": command.handoff_id}
            )
            await uow.commit()
            return ack

    async def impact(self, project_id: str, entry_id: str) -> dict[str, object]:
        async with self._uow_factory() as uow:
            entry = _entry_for_project(uow, project_id, entry_id)
            if entry.current is None:
                return {"projectId": project_id, "entryId": entry_id, "targets": []}
            projection = await self._projection(uow, project_id, entry)
            return {
                "projectId": project_id,
                "entryId": entry_id,
                "targets": [item.canonical_value() for item in projection.targets],
                "complete": projection.complete,
                "diagnostic": projection.diagnostic,
                "targetSetHash": canonical_hash(
                    [item.canonical_value() for item in projection.targets]
                ),
            }

    async def list_entries(self, project_id: str) -> list[AssetBibleEntry]:
        async with self._uow_factory() as uow:
            return sorted(
                uow.asset_bible_by_project.get(project_id, ()),
                key=lambda item: (item.entry_type, item.id),
            )

    async def get_entry(self, project_id: str, entry_id: str) -> AssetBibleEntry:
        async with self._uow_factory() as uow:
            return _entry_for_project(uow, project_id, entry_id)

    async def list_tasks(self, project_id: str) -> list[ContinuityRevisionTask]:
        async with self._uow_factory() as uow:
            return sorted(
                (item for item in uow.asset_bible_tasks.values() if item.project_id == project_id),
                key=lambda item: (item.target_type, item.target_id, item.id),
            )

    async def get_snapshot(self, project_id: str, snapshot_id: str) -> ResolvedContinuitySnapshot:
        async with self._uow_factory() as uow:
            snapshot = uow.asset_bible_snapshots.get(snapshot_id)
            if snapshot is None or snapshot.project_id != project_id:
                raise ValidationDomainError("continuity snapshot scope is invalid")
            return snapshot

    async def get_handoff_ack(self, project_id: str, handoff_id: str) -> AssetBibleHandoffAck:
        async with self._uow_factory() as uow:
            value = uow.asset_bible_handoff_acks.get(handoff_id)
            if value is None or value[1].project_id != project_id:
                raise ValidationDomainError("asset bible handoff ack scope is invalid")
            return value[1]

    async def consumer_projection(self, project_id: str, snapshot_id: str) -> dict[str, object]:
        """Expose frozen IDs/hashes only; consumers never receive mutable AssetBible payloads."""
        async with self._uow_factory() as uow:
            snapshot = uow.asset_bible_snapshots.get(snapshot_id)
            if snapshot is None or snapshot.project_id != project_id:
                raise ValidationDomainError("continuity snapshot scope is invalid")
            asset_refs: list[OwnerReference] = []
            generation_refs: list[OwnerReference] = []
            for assignment in snapshot.refs:
                entry = _entry_for_project(uow, project_id, assignment.entry_id)
                version = next(
                    (item for item in entry.versions if item.id == assignment.version_id), None
                )
                if version is None:
                    raise ValidationDomainError("continuity snapshot version is unavailable")
                asset_refs.extend(version.reference_asset_version_refs)
                generation_refs.extend(version.generation_spec_refs)
            return {
                "snapshotRef": {
                    "id": snapshot.id,
                    "revision": snapshot.revision,
                    "hash": snapshot.content_hash,
                },
                "assetVersionRefs": asset_refs,
                "generationSpecRefs": generation_refs,
            }
