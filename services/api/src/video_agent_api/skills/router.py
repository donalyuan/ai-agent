"""固定四段路由流水线，任何不确定性都保留人工选择。"""

from __future__ import annotations

import builtins
from dataclasses import dataclass
from typing import Protocol

from video_agent_api.skills.registry import SkillRegistry, SkillRevision


@dataclass(frozen=True, slots=True)
class RouteContext:
    project_type: str
    stage: str
    target_model: str
    query: str
    allowed_tools: set[str]
    allowed_licenses: set[str]


@dataclass(frozen=True, slots=True)
class RankedSkill:
    name: str
    version: str
    score: int


@dataclass(frozen=True, slots=True)
class RouteDecision:
    candidates: tuple[RankedSkill, ...]
    selected: RankedSkill | None
    needs_manual_selection: bool
    fallback_reason: str | None
    audit_stages: tuple[str, ...]


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
        ]

    def _rank(self, query: str, records: builtins.list[SkillRevision]) -> tuple[RankedSkill, ...]:
        terms = set(query.lower().split())
        ranked = []
        for record in records:
            haystack = set(
                " ".join((record.name, *record.capabilities)).lower().replace("-", " ").split()
            )
            ranked.append(
                RankedSkill(record.name, record.version, len(terms & haystack) + record.priority)
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
        return RouteDecision(candidates, selected, needs_manual, fallback, tuple(audit))
