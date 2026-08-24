"""固定四段路由流水线，任何不确定性都保留人工选择。"""

from __future__ import annotations

import builtins
from dataclasses import dataclass
from hashlib import sha256
from typing import Protocol
from uuid import NAMESPACE_URL, UUID, uuid4, uuid5

from video_agent_api.skills.registry import SkillRegistry, SkillRevision


@dataclass(frozen=True, slots=True)
class RouteContext:
    project_type: str
    stage: str
    target_model: str
    query: str
    allowed_tools: set[str]
    allowed_licenses: set[str]
    project_id: str = ""
    node_key: str = ""
    launch_id: str = ""
    allowed_skills: frozenset[str] = frozenset()
    required_capabilities: frozenset[str] = frozenset()
    selection_mode: str = "inherit"


@dataclass(frozen=True, slots=True)
class RankedSkill:
    name: str
    version: str
    score: int
    digest: str = ""
    score_source: str = "lexical"


@dataclass(frozen=True, slots=True)
class RouteDecision:
    candidates: tuple[RankedSkill, ...]
    selected: RankedSkill | None
    needs_manual_selection: bool
    fallback_reason: str | None
    audit_stages: tuple[str, ...]
    id: str = ""
    revision: int = 1
    input_fingerprint: str = ""
    project_id: str = ""
    node_key: str = ""
    launch_id: str = ""
    router_policy: str = "deterministic_filter_lexical_optional_semantic"
    router_version: str = "1.0.0"


@dataclass(frozen=True, slots=True)
class SkillRouteSelection:
    decision_id: str
    skill_name: str
    skill_version: str
    expected_revision: int
    actor_uuid: str
    id: str
    fingerprint: str
    skill_digest: str


def _decision_id(fingerprint: str) -> str:
    return str(uuid5(NAMESPACE_URL, f"video-agent:skill-route:{fingerprint}"))


class SemanticAdapter(Protocol):
    def rank(
        self, query: str, candidates: tuple[RankedSkill, ...]
    ) -> tuple[tuple[RankedSkill, ...], float]: ...


class SemanticAdapterUnavailableError(RuntimeError):
    """适配器显式报告其依赖不可用；其他错误必须暴露给调用方。"""


class SkillRouter:
    def __init__(
        self, registry: SkillRegistry, semantic_adapter: SemanticAdapter | None = None
    ) -> None:
        self._registry = registry
        self._semantic_adapter = semantic_adapter

    def _filter(self, context: RouteContext) -> builtins.list[SkillRevision]:
        return [
            candidate
            for candidate in self._registry.routable()
            if context.project_type in candidate.project_types
            and context.stage in candidate.stages
            and context.target_model in candidate.target_models
            and candidate.license in context.allowed_licenses
            and candidate.allowed_tools.issubset(context.allowed_tools)
            and (not context.allowed_skills or candidate.name in context.allowed_skills)
            and context.required_capabilities.issubset(candidate.capabilities)
        ]

    def _rank(self, query: str, records: builtins.list[SkillRevision]) -> tuple[RankedSkill, ...]:
        terms = set(query.lower().split())
        ranked = []
        for record in records:
            haystack = set(
                " ".join((record.name, *record.capabilities)).lower().replace("-", " ").split()
            )
            ranked.append(
                RankedSkill(
                    record.name,
                    record.version,
                    len(terms & haystack) + record.priority,
                    record.upstream_digest or "",
                )
            )
        return tuple(sorted(ranked, key=lambda item: (-item.score, item.name, item.version)))

    def route(self, context: RouteContext) -> RouteDecision:
        candidates = self._rank(context.query, self._filter(context))
        audit = ["deterministic_filter", "lexical_rank"]
        fallback: str | None = None
        if self._semantic_adapter is None:
            fallback = "semantic_adapter_unconfigured"
        else:
            try:
                semantic_candidates, confidence = self._semantic_adapter.rank(
                    context.query, candidates
                )
                if confidence < 0.8:
                    fallback = "semantic_adapter_low_confidence"
                else:
                    candidates = semantic_candidates
                audit.append("optional_semantic_adapter")
            except SemanticAdapterUnavailableError as error:
                fallback = f"semantic_adapter_unavailable:{type(error).__name__}"
                audit.append("optional_semantic_adapter")
        tied = len(candidates) > 1 and candidates[0].score == candidates[1].score
        needs_manual = not candidates or tied or fallback is not None
        selected = None if needs_manual else candidates[0]
        audit.append("policy_decide")
        candidate_names = ",".join(item.name for item in candidates)
        fingerprint = sha256(
            f"{context.project_type}|{context.stage}|{context.target_model}|"
            f"{context.query}|{candidate_names}|{context.project_id}|{context.node_key}|"
            f"{context.launch_id}|{context.selection_mode}".encode()
        ).hexdigest()
        return RouteDecision(
            candidates,
            selected,
            needs_manual,
            fallback,
            tuple(audit),
            _decision_id(fingerprint),
            1,
            fingerprint,
            context.project_id,
            context.node_key,
            context.launch_id,
        )

    def select(
        self,
        decision: RouteDecision,
        skill_name: str,
        skill_version: str,
        actor_uuid: str,
        expected_revision: int = 1,
    ) -> SkillRouteSelection:
        if decision.revision != expected_revision:
            raise ValueError("skill route decision revision conflict")
        candidate = next(
            (
                item
                for item in decision.candidates
                if item.name == skill_name and item.version == skill_version
            ),
            None,
        )
        if candidate is None:
            raise ValueError("skill selection is not a current candidate")
        record = self._registry.resolve(skill_name, skill_version)
        if record not in self._registry.routable() or record.upstream_digest != candidate.digest:
            raise ValueError("skill revision is disabled, unapproved or drifted")
        try:
            UUID(actor_uuid)
        except ValueError as error:
            raise ValueError("skill selection actor must be a stable UUID") from error
        return SkillRouteSelection(
            decision.id,
            record.name,
            record.version,
            expected_revision,
            actor_uuid,
            str(uuid4()),
            sha256(
                f"{decision.id}|{record.name}|{record.version}|{record.upstream_digest}|"
                f"{actor_uuid}".encode()
            ).hexdigest(),
            record.upstream_digest,
        )
