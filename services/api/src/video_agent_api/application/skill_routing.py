"""Persisted Skill route decision and human selection boundary."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, cast

from video_agent_api.domain.errors import (
    ProjectAccessForbiddenError,
    RevisionConflictError,
    ValidationDomainError,
)
from video_agent_api.skills.registry import SkillRegistry
from video_agent_api.skills.router import (
    RouteContext,
    RouteDecision,
    SkillRouter,
    SkillRouteSelection,
)


@dataclass(frozen=True, slots=True)
class ResolveSkillRouteCommand:
    project_id: str
    node_key: str
    launch_id: str
    project_type: str
    stage: str
    target_model: str
    query: str
    allowed_tools: frozenset[str]
    allowed_licenses: frozenset[str]
    allowed_skills: frozenset[str]
    required_capabilities: frozenset[str]
    selection_mode: str


@dataclass(frozen=True, slots=True)
class SelectSkillRouteCommand:
    decision_id: str
    skill_name: str
    skill_version: str
    actor_uuid: str
    expected_revision: int


class SkillRoutingService:
    def __init__(self, uow_factory: Any, registry: SkillRegistry) -> None:
        self._uow_factory = uow_factory
        self._registry = registry
        self._router = SkillRouter(registry)

    async def resolve(self, command: ResolveSkillRouteCommand) -> RouteDecision:
        if (
            not command.node_key
            or not command.launch_id
            or command.selection_mode not in {"fixed", "inherit"}
            or not command.allowed_skills
            or not command.required_capabilities
        ):
            raise ValidationDomainError("skill route command is invalid")
        async with self._uow_factory() as uow:
            if await uow.projects.get(command.project_id) is None:
                raise ValidationDomainError("skill route project is unavailable")
            existing = next(
                (
                    item
                    for item in uow.skill_route_decisions.values()
                    if item.project_id == command.project_id
                    and item.node_key == command.node_key
                    and item.launch_id == command.launch_id
                ),
                None,
            )
            decision = self._router.route(
                RouteContext(
                    command.project_type,
                    command.stage,
                    command.target_model,
                    command.query,
                    set(command.allowed_tools),
                    set(command.allowed_licenses),
                    command.project_id,
                    command.node_key,
                    command.launch_id,
                    command.allowed_skills,
                    command.required_capabilities,
                    command.selection_mode,
                )
            )
            if existing is not None:
                if existing.input_fingerprint == decision.input_fingerprint:
                    return cast(RouteDecision, existing)
                raise RevisionConflictError(existing.id, existing.revision, existing.revision)
            uow.skill_route_decisions[decision.id] = decision
            uow.audit_events.append({"type": "skill.route.resolved", "decisionId": decision.id})
            uow.outbox_events.append({"type": "skill.route.resolved", "decisionId": decision.id})
            await uow.commit()
            return decision

    async def select(
        self, command: SelectSkillRouteCommand, *, project_scope: str | None = None
    ) -> SkillRouteSelection:
        async with self._uow_factory() as uow:
            decision = uow.skill_route_decisions.get(command.decision_id)
            if decision is None:
                raise ValidationDomainError("skill route decision is unavailable")
            if project_scope is not None and decision.project_id != project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            existing = uow.skill_route_selections.get(decision.id)
            if existing is not None:
                if (
                    existing.skill_name == command.skill_name
                    and existing.skill_version == command.skill_version
                ):
                    return cast(SkillRouteSelection, existing)
                raise RevisionConflictError(
                    decision.id, command.expected_revision, decision.revision
                )
            try:
                selection = self._router.select(
                    decision,
                    command.skill_name,
                    command.skill_version,
                    command.actor_uuid,
                    command.expected_revision,
                )
            except ValueError as error:
                if "revision conflict" in str(error):
                    raise RevisionConflictError(
                        decision.id, command.expected_revision, decision.revision
                    ) from error
                raise ValidationDomainError(str(error)) from error
            uow.skill_route_selections[decision.id] = selection
            uow.audit_events.append({"type": "skill.route.selected", "decisionId": decision.id})
            uow.outbox_events.append({"type": "skill.route.selected", "decisionId": decision.id})
            await uow.commit()
            return selection

    async def get(self, decision_id: str, *, project_scope: str | None = None) -> RouteDecision:
        async with self._uow_factory() as uow:
            decision = uow.skill_route_decisions.get(decision_id)
            if decision is None:
                raise ValidationDomainError("skill route decision is unavailable")
            if project_scope is not None and decision.project_id != project_scope:
                raise ProjectAccessForbiddenError(project_scope)
            return cast(RouteDecision, decision)

    async def list(self, project_id: str) -> list[RouteDecision]:
        async with self._uow_factory() as uow:
            return [
                item for item in uow.skill_route_decisions.values() if item.project_id == project_id
            ]
