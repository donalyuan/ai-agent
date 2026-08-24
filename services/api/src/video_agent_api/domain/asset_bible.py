"""AssetBible owner：稳定身份、不可变版本与确定性连续性解析。"""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from typing import Literal
from uuid import UUID, uuid4

from .errors import RevisionConflictError, ValidationDomainError

EntryType = Literal["character", "look", "location", "scene_visual", "prop", "visual_style"]
ScopeType = Literal["project", "episode", "scene", "shot"]
TargetType = Literal["episode", "scene", "shot"]
ENTRY_TYPES = ("character", "look", "location", "scene_visual", "prop", "visual_style")
LEVELS = ("project", "episode", "scene", "shot")
_LEVEL_RANK = {value: index for index, value in enumerate(LEVELS)}
_FORBIDDEN_PAYLOAD_KEYS = {
    "base64",
    "binary",
    "blob",
    "bytes",
    "content",
    "data",
    "downloadUrl",
    "metadata",
    "objectKey",
    "object_key",
    "prompt",
    "promptText",
    "url",
}


def canonical_hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, default=str, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


def _validate_hash(value: str, label: str) -> None:
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ValidationDomainError(f"{label} must be a lowercase sha256 hash")


def _validate_uuid(value: str, label: str) -> None:
    try:
        UUID(value)
    except (ValueError, TypeError) as error:
        raise ValidationDomainError(f"{label} must be a UUID") from error


def _validate_reference_only(value: object) -> None:
    if isinstance(value, dict):
        forbidden = _FORBIDDEN_PAYLOAD_KEYS.intersection(value)
        if forbidden:
            raise ValidationDomainError(
                f"asset bible payload copies owner data: {sorted(forbidden)[0]}"
            )
        for nested in value.values():
            _validate_reference_only(nested)
    elif isinstance(value, list):
        for nested in value:
            _validate_reference_only(nested)


def validate_reference_payload(value: object) -> None:
    """Validate that AssetBible attributes contain references, never owner payload copies."""
    _validate_reference_only(value)


@dataclass(slots=True)
class AssetBible:
    project_id: str
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    schema_version: str = "1.0.0"
    current_version_map: dict[str, str] = field(default_factory=dict)

    def set_current(self, entry_id: str, version_id: str, expected_revision: int) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        self.current_version_map[entry_id] = version_id
        self.revision += 1


@dataclass(frozen=True, slots=True)
class OwnerReference:
    owner_id: str
    revision: int
    content_hash: str
    purpose: str

    def __post_init__(self) -> None:
        if self.revision < 0 or not self.purpose.strip():
            raise ValidationDomainError("asset bible owner reference is invalid")
        _validate_uuid(self.owner_id, "owner reference id")
        _validate_hash(self.content_hash, "owner reference hash")


@dataclass(frozen=True, slots=True)
class AssetBibleVersion:
    entry_id: str
    project_id: str
    entry_type: EntryType
    payload: dict[str, object]
    version_number: int
    actor_uuid: str
    reference_asset_version_refs: tuple[OwnerReference, ...] = ()
    generation_spec_refs: tuple[OwnerReference, ...] = ()
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    content_hash: str = ""
    schema_version: str = "1.0.0"

    def __post_init__(self) -> None:
        if self.entry_type not in ENTRY_TYPES or not self.payload or self.version_number < 1:
            raise ValidationDomainError("asset bible version type/payload is invalid")
        if self.revision != 1:
            raise ValidationDomainError("asset bible entry version is immutable")
        _validate_uuid(self.actor_uuid, "actor UUID")
        _validate_reference_only(self.payload)
        expected_hash = canonical_hash(
            {
                "entryType": self.entry_type,
                "attributes": self.payload,
                "referenceAssetVersionRefs": self.reference_asset_version_refs,
                "generationSpecRefs": self.generation_spec_refs,
            }
        )
        if self.content_hash:
            _validate_hash(self.content_hash, "asset bible version hash")
            if self.content_hash != expected_hash:
                raise ValidationDomainError("asset bible version hash mismatch")
        else:
            object.__setattr__(self, "content_hash", expected_hash)


@dataclass(slots=True)
class AssetBibleEntry:
    project_id: str
    asset_bible_id: str
    entry_type: EntryType
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    current: AssetBibleVersion | None = None
    versions: list[AssetBibleVersion] = field(default_factory=list)
    disabled: bool = False
    schema_version: str = "1.0.0"

    def __post_init__(self) -> None:
        if self.entry_type not in ENTRY_TYPES:
            raise ValidationDomainError("unknown asset bible entry type")

    def disable(self, expected_revision: int) -> None:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        self.disabled = True
        self.revision += 1

    def successor(
        self,
        payload: dict[str, object],
        expected_revision: int,
        actor_uuid: str,
        reference_asset_version_refs: tuple[OwnerReference, ...] = (),
        generation_spec_refs: tuple[OwnerReference, ...] = (),
    ) -> AssetBibleVersion:
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        if self.disabled:
            raise ValidationDomainError("disabled asset bible entry is immutable")
        version = AssetBibleVersion(
            entry_id=self.id,
            project_id=self.project_id,
            entry_type=self.entry_type,
            payload=dict(payload),
            version_number=len(self.versions) + 1,
            actor_uuid=actor_uuid,
            reference_asset_version_refs=reference_asset_version_refs,
            generation_spec_refs=generation_spec_refs,
        )
        self.versions.append(version)
        self.current = version
        self.revision += 1
        return version


@dataclass(frozen=True, slots=True)
class ContinuityAssignment:
    project_id: str
    level: str
    target_id: str
    entry_id: str
    version_id: str
    version_revision: int
    content_hash: str
    revision: int = 1
    schema_version: str = "1.0.0"
    id: str = field(default_factory=lambda: str(uuid4()))
    scope_revision: int = 1

    def __post_init__(self) -> None:
        if (
            self.level not in LEVELS
            or not self.target_id
            or not self.entry_id
            or self.revision < 1
            or self.version_revision < 1
            or self.scope_revision < 1
        ):
            raise ValidationDomainError("continuity assignment scope is invalid")
        _validate_hash(self.content_hash, "continuity assignment hash")


@dataclass(frozen=True, slots=True)
class ResolvedContinuitySnapshot:
    project_id: str
    target_id: str
    refs: tuple[ContinuityAssignment, ...]
    revision_chain: tuple[tuple[str, int], ...]
    override_chain: tuple[ContinuityAssignment, ...] = ()
    status: Literal["accepted", "incomplete"] = "accepted"
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    content_hash: str = ""
    target_type: ScopeType = "shot"
    target_revision: int = 1
    schema_version: str = "1.0.0"

    def __post_init__(self) -> None:
        if not self.refs or self.target_type not in LEVELS or self.target_revision < 1:
            raise ValidationDomainError("resolved continuity snapshot must not be empty")
        expected_hash = canonical_hash(
            {
                "targetType": self.target_type,
                "targetId": self.target_id,
                "targetRevision": self.target_revision,
                "resolved": [_assignment_hash_value(item) for item in self.refs],
                "chain": [_assignment_hash_value(item) for item in self.override_chain],
                "sourceRevisions": self.revision_chain,
            }
        )
        if self.content_hash:
            _validate_hash(self.content_hash, "resolved continuity snapshot hash")
            if self.content_hash != expected_hash:
                raise ValidationDomainError("resolved continuity snapshot hash mismatch")
        else:
            object.__setattr__(self, "content_hash", expected_hash)


@dataclass(frozen=True, slots=True)
class ContinuityImpactTarget:
    target_type: TargetType
    target_id: str
    target_revision: int
    reason: str
    snapshot_id: str
    snapshot_hash: str
    suggested_action: Literal["review", "regenerate", "acknowledge"] = "review"

    def __post_init__(self) -> None:
        if self.target_type not in {"episode", "scene", "shot"} or self.target_revision < 1:
            raise ValidationDomainError("continuity impact target is invalid")
        if not self.reason or not self.snapshot_id:
            raise ValidationDomainError("continuity impact target evidence is incomplete")
        _validate_hash(self.snapshot_hash, "continuity impact snapshot hash")

    def canonical_value(self) -> dict[str, object]:
        return {
            "targetType": self.target_type,
            "targetId": self.target_id,
            "targetRevision": self.target_revision,
            "reason": self.reason,
            "snapshotId": self.snapshot_id,
            "snapshotHash": self.snapshot_hash,
            "suggestedAction": self.suggested_action,
        }


@dataclass(frozen=True, slots=True)
class ContinuityImpactAnalysis:
    project_id: str
    entry_id: str
    base_version_id: str
    candidate_payload_hash: str
    target_refs: tuple[ContinuityImpactTarget, ...]
    status: Literal["complete", "incomplete"] = "complete"
    diagnostic: str | None = None
    revision: int = 1
    id: str = field(default_factory=lambda: str(uuid4()))
    target_set_hash: str = ""
    candidate_payload: dict[str, object] = field(default_factory=dict)
    reference_asset_version_refs: tuple[OwnerReference, ...] = ()
    generation_spec_refs: tuple[OwnerReference, ...] = ()
    schema_version: str = "1.0.0"

    def __post_init__(self) -> None:
        ordered = tuple(
            sorted(
                self.target_refs,
                key=lambda item: (item.target_type, item.target_id, item.target_revision),
            )
        )
        if ordered != self.target_refs or len(
            {(x.target_type, x.target_id) for x in ordered}
        ) != len(ordered):
            raise ValidationDomainError("continuity impact target set is not canonical")
        expected_hash = canonical_hash([item.canonical_value() for item in ordered])
        if self.target_set_hash:
            _validate_hash(self.target_set_hash, "continuity target set hash")
            if self.target_set_hash != expected_hash:
                raise ValidationDomainError("continuity target set hash mismatch")
        else:
            object.__setattr__(self, "target_set_hash", expected_hash)


@dataclass(slots=True)
class ContinuityRevisionTask:
    project_id: str
    target_id: str
    entry_id: str
    status: Literal["pending", "acknowledged", "resolved", "superseded"] = "pending"
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    target_revision: int = 1
    old_version_id: str = ""
    new_version_id: str = ""
    snapshot_id: str = ""
    snapshot_hash: str = ""
    reason: str = "entry_successor"
    correlation_id: str = ""
    target_type: TargetType = "shot"
    schema_version: str = "1.0.0"

    def transition(self, target: str, expected_revision: int) -> None:
        allowed = {
            "pending": {"acknowledged", "superseded"},
            "acknowledged": {"resolved", "superseded"},
            "resolved": set(),
            "superseded": set(),
        }
        if expected_revision != self.revision:
            raise RevisionConflictError(self.id, expected_revision, self.revision)
        if target not in allowed[self.status]:
            raise ValidationDomainError("invalid continuity task transition")
        self.status = target  # type: ignore[assignment]
        self.revision += 1


@dataclass(frozen=True, slots=True)
class AssetBibleRelationship:
    project_id: str
    source_entry_id: str
    target_entry_id: str
    kind: Literal["character_look", "location_scene_visual", "related"]
    id: str = field(default_factory=lambda: str(uuid4()))
    schema_version: str = "1.0.0"


@dataclass(frozen=True, slots=True)
class AssetBibleAcceptDecision:
    project_id: str
    entry_id: str
    analysis_id: str
    old_version_id: str
    new_version_id: str
    target_set_hash: str
    actor_uuid: str
    correlation_id: str
    fingerprint: str
    id: str = field(default_factory=lambda: str(uuid4()))
    schema_version: str = "1.0.0"

    def __post_init__(self) -> None:
        _validate_uuid(self.actor_uuid, "actor UUID")
        _validate_hash(self.target_set_hash, "decision target set hash")
        _validate_hash(self.fingerprint, "decision fingerprint")


@dataclass(frozen=True, slots=True)
class AssetBibleHandoffAck:
    handoff_id: str
    project_id: str
    payload_hash: str
    entry_version_refs: tuple[tuple[str, str, int, str], ...]
    correlation_id: str
    id: str = field(default_factory=lambda: str(uuid4()))
    schema_version: str = "1.0.0"

    def __post_init__(self) -> None:
        _validate_hash(self.payload_hash, "asset bible handoff payload hash")


def validate_relationship(
    source: AssetBibleEntry,
    target: AssetBibleEntry,
    kind: str,
    relationships: list[AssetBibleRelationship],
) -> AssetBibleRelationship:
    if source.project_id != target.project_id or source.id == target.id:
        raise ValidationDomainError("asset bible relationship is foreign or cyclic")
    expected = {
        "character_look": ("look", "character"),
        "location_scene_visual": ("scene_visual", "location"),
    }
    if kind not in {"character_look", "location_scene_visual", "related"}:
        raise ValidationDomainError("unknown asset bible relationship kind")
    if kind in expected and (source.entry_type, target.entry_type) != expected[kind]:
        raise ValidationDomainError("asset bible relationship type mismatch")
    graph: dict[str, set[str]] = {}
    for item in relationships:
        graph.setdefault(item.source_entry_id, set()).add(item.target_entry_id)
    graph.setdefault(source.id, set()).add(target.id)
    pending = [target.id]
    seen: set[str] = set()
    while pending:
        current = pending.pop()
        if current == source.id:
            raise ValidationDomainError("asset bible relationship cycle")
        if current not in seen:
            seen.add(current)
            pending.extend(graph.get(current, ()))
    return AssetBibleRelationship(
        source.project_id,
        source.id,
        target.id,
        kind,  # type: ignore[arg-type]
    )


def _assignment_hash_value(assignment: ContinuityAssignment) -> dict[str, object]:
    return {
        "level": assignment.level,
        "targetId": assignment.target_id,
        "scopeRevision": assignment.scope_revision,
        "entryId": assignment.entry_id,
        "versionId": assignment.version_id,
        "versionRevision": assignment.version_revision,
        "contentHash": assignment.content_hash,
        "assignmentRevision": assignment.revision,
    }


def resolve_assignments(
    project_id: str,
    target_id: str,
    assignments: list[ContinuityAssignment],
    *,
    target_type: ScopeType = "shot",
    target_revision: int = 1,
) -> ResolvedContinuitySnapshot:
    selected: dict[str, ContinuityAssignment] = {}
    ordered = sorted(
        assignments,
        key=lambda item: (_LEVEL_RANK[item.level], item.entry_id, item.version_id, item.id),
    )
    seen_scope: dict[tuple[str, str], ContinuityAssignment] = {}
    for assignment in ordered:
        if assignment.project_id != project_id:
            raise ValidationDomainError("continuity assignment is foreign")
        key = (assignment.entry_id, assignment.level)
        previous = seen_scope.get(key)
        if previous is not None and previous.version_id != assignment.version_id:
            raise ValidationDomainError("continuity assignment is ambiguous")
        seen_scope[key] = assignment
        selected[assignment.entry_id] = assignment
    refs = tuple(
        sorted(selected.values(), key=lambda item: (item.entry_id, item.level, item.version_id))
    )
    return ResolvedContinuitySnapshot(
        project_id=project_id,
        target_id=target_id,
        refs=refs,
        revision_chain=tuple((item.id, item.revision) for item in ordered),
        override_chain=tuple(ordered),
        target_type=target_type,
        target_revision=target_revision,
    )
