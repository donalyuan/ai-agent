from __future__ import annotations

from typing import Any, Literal, cast

from video_agent_api.domain.agent_edit import (
    AcceptDecision,
    AssetEditCandidate,
    AssetEditExecution,
    AssetEditPlan,
    AssetEditSelection,
    AssetEditSession,
    AssetVersionRef,
    ContinuitySnapshotRef,
    EditImpact,
    reject_unsupported_asset_edit,
)
from video_agent_api.domain.errors import (
    AssetEditConflictError,
    AssetEditContinuityStaleError,
    AssetEditNotFoundError,
    ProjectAccessForbiddenError,
    RevisionConflictError,
    ValidationDomainError,
)


class AgentEditService:
    def __init__(self, uow_factory: Any, scenes_service: Any | None = None) -> None:
        self._uow_factory = uow_factory
        self._scenes_service = scenes_service

    @staticmethod
    def _require_project_scope(project_id: str, project_scope: str | None) -> None:
        if project_scope is not None and project_id != project_scope:
            raise ProjectAccessForbiddenError(project_scope)

    async def create_plan(
        self,
        project_id: str,
        episode_id: str,
        kind: str,
        base: AssetVersionRef,
        refs: tuple[AssetVersionRef, ...],
        instruction: str,
        turn_id: str,
        **payload: object,
    ) -> AssetEditPlan:
        reject_unsupported_asset_edit(kind, dict(payload))
        requested_schema_version = payload.pop("schema_version", "1.0.0")
        if requested_schema_version != "1.0.0":
            raise ValidationDomainError("unsupported schemaVersion")
        continuity = payload.pop("continuity", None)
        target_id = str(payload.pop("target_id", ""))
        session_id = str(payload.pop("session_id", ""))
        run_id = str(payload.pop("run_id", ""))
        node_run_id = str(payload.pop("node_run_id", ""))
        logical_operation = str(payload.pop("logical_operation", ""))
        correlation_id = str(payload.pop("correlation_id", ""))
        if continuity is not None and not isinstance(continuity, ContinuitySnapshotRef):
            raise ValidationDomainError("continuity reference is invalid")
        plan = AssetEditPlan(
            project_id,
            episode_id,
            base,
            refs,
            instruction,
            turn_id,
            continuity=continuity,
            target_id=target_id,
            session_id=session_id,
            run_id=run_id,
            node_run_id=node_run_id,
            logical_operation=logical_operation,
            correlation_id=correlation_id,
        )
        async with self._uow_factory() as uow:
            await self._validate_versions(uow, project_id, (base, *refs))
            self._validate_continuity(uow, project_id, target_id, continuity)
            uow.asset_edit_plans[plan.id] = plan
            uow.audit_events.append({"type": "asset_edit.plan.created", "planId": plan.id})
            await uow.commit()
        return plan

    @staticmethod
    async def _validate_versions(
        uow: Any, project_id: str, refs: tuple[AssetVersionRef, ...]
    ) -> None:
        for reference in refs:
            version = await uow.asset_versions.get(reference.id)
            if (
                version is None
                or version.project_id != project_id
                or (reference.project_id and reference.project_id != project_id)
                or version.revision != reference.revision
                or version.content_hash != reference.content_hash
                or version.storage_object.mime_type.split("/", 1)[0] != reference.kind
                or (reference.mime_type and version.storage_object.mime_type != reference.mime_type)
            ):
                raise AssetEditConflictError("asset version is stale or foreign")

    @staticmethod
    def _validate_continuity(
        uow: Any, project_id: str, target_id: str, continuity: ContinuitySnapshotRef | None
    ) -> None:
        if continuity is None:
            return
        snapshot = uow.asset_bible_snapshots.get(continuity.id)
        if (
            snapshot is None
            or snapshot.project_id != project_id
            or snapshot.target_id != target_id
            or snapshot.status != "accepted"
            or snapshot.revision != continuity.revision
            or snapshot.content_hash != continuity.content_hash
        ):
            raise AssetEditContinuityStaleError("continuity_stale")
        pending = [
            task
            for task in uow.asset_bible_tasks.values()
            if task.project_id == project_id
            and task.target_id == target_id
            and task.status in {"pending", "acknowledged"}
        ]
        if pending:
            raise AssetEditContinuityStaleError("continuity_stale")

    async def create_session(
        self,
        project_id: str,
        episode_id: str,
        selection: AssetEditSelection,
        continuity: ContinuitySnapshotRef,
    ) -> AssetEditSession:
        if selection.project_id != project_id or selection.episode_id != episode_id:
            raise ValidationDomainError("asset edit selection scope is invalid")
        async with self._uow_factory() as uow:
            await self._validate_versions(
                uow, project_id, (selection.primary, *selection.references)
            )
            self._validate_continuity(uow, project_id, selection.target_id, continuity)
            session = AssetEditSession(project_id, episode_id, selection, continuity)
            uow.asset_edit_sessions[session.id] = session
            await uow.commit()
            return session

    async def append_user_message(
        self,
        session_id: str,
        content_hash: str,
        correlation_id: str,
        expected_revision: int | None = None,
        *,
        project_scope: str | None = None,
    ) -> Any:
        async with self._uow_factory() as uow:
            session = uow.asset_edit_sessions.get(session_id)
            if session is None or session.status != "active":
                raise AssetEditNotFoundError("asset edit session not found")
            self._require_project_scope(session.project_id, project_scope)
            conversation = uow.conversations.get(session_id)
            if conversation is None:
                from video_agent_api.domain.conversation import AgentConversation

                conversation = AgentConversation(
                    session.project_id, session.episode_id, id=session_id
                )
                uow.conversations[session_id] = conversation
            if expected_revision is not None and conversation.revision != expected_revision:
                raise RevisionConflictError(session_id, expected_revision, conversation.revision)
            duplicate = next(
                (
                    message
                    for message in conversation.messages
                    if message.role == "user" and message.correlation_id == correlation_id
                ),
                None,
            )
            if duplicate is not None:
                if duplicate.content_hash != content_hash:
                    raise ValidationDomainError("conversation correlation id conflict")
                return next(
                    turn for turn in conversation.turns if turn.user_message_id == duplicate.id
                )
            turn = conversation.append_user_message(content_hash, correlation_id)
            await uow.commit()
            return turn

    async def append_agent_reply(
        self,
        session_id: str,
        turn_id: str,
        content_hash: str,
        correlation_id: str,
        expected_turn_revision: int,
        status: str = "complete",
        *,
        project_scope: str | None = None,
    ) -> Any:
        async with self._uow_factory() as uow:
            conversation = uow.conversations.get(session_id)
            if conversation is None:
                raise AssetEditNotFoundError("asset edit conversation not found")
            self._require_project_scope(conversation.project_id, project_scope)
            message = conversation.append_agent_reply(
                turn_id, content_hash, correlation_id, expected_turn_revision, cast(Any, status)
            )
            await uow.commit()
            return message

    async def generate_plan_from_turn(
        self,
        session_id: str,
        turn_id: str,
        base: AssetVersionRef,
        refs: tuple[AssetVersionRef, ...],
        instruction: str,
        *,
        kind: str,
        target_id: str,
        run_id: str,
        node_run_id: str,
        logical_operation: str,
        correlation_id: str,
        project_scope: str | None = None,
    ) -> AssetEditPlan:
        async with self._uow_factory() as uow:
            session = uow.asset_edit_sessions.get(session_id)
            conversation = uow.conversations.get(session_id)
            if session is None or conversation is None:
                raise AssetEditNotFoundError("asset edit session or conversation not found")
            self._require_project_scope(session.project_id, project_scope)
            turn = next((item for item in conversation.turns if item.id == turn_id), None)
            if turn is None or turn.status != "complete":
                raise ValidationDomainError("completed agent turn is required")
            if turn.session_id != session_id:
                raise ValidationDomainError("conversation turn scope is invalid")
            await self._validate_versions(uow, session.project_id, (base, *refs))
            await self._validate_plan_continuity(
                uow,
                AssetEditPlan(
                    session.project_id,
                    session.episode_id,
                    base,
                    refs,
                    instruction,
                    turn_id,
                    continuity=session.continuity,
                    target_id=target_id,
                    session_id=session_id,
                    run_id=run_id,
                    node_run_id=node_run_id,
                    logical_operation=logical_operation,
                    correlation_id=correlation_id,
                ),
            )
        return await self.create_plan(
            session.project_id,
            session.episode_id,
            kind,
            base,
            refs,
            instruction,
            turn_id,
            continuity=session.continuity,
            target_id=target_id,
            session_id=session_id,
            run_id=run_id,
            node_run_id=node_run_id,
            logical_operation=logical_operation,
            correlation_id=correlation_id,
        )

    async def register_candidate(
        self,
        plan_id: str,
        version: AssetVersionRef,
        provenance: dict[str, object] | None = None,
        *,
        execution_id: str | None = None,
    ) -> AssetEditCandidate:
        candidate = AssetEditCandidate(plan_id, version, provenance=dict(provenance or {}))
        async with self._uow_factory() as uow:
            plan = uow.asset_edit_plans.get(plan_id)
            if plan is None:
                raise AssetEditNotFoundError("asset edit plan not found")
            if plan.status in {"stale"}:
                raise AssetEditConflictError("asset edit plan is stale")
            await self._validate_versions(uow, plan.project_id, (version,))
            existing = next(
                (
                    item
                    for item in uow.asset_edit_candidates.values()
                    if item.plan_id == plan_id
                    and item.asset_version.id == version.id
                    and item.asset_version.revision == version.revision
                    and item.asset_version.content_hash == version.content_hash
                ),
                None,
            )
            if existing is not None:
                if execution_id is not None and existing.provenance.get("executionId") not in {
                    None,
                    execution_id,
                }:
                    raise ValidationDomainError("candidate provenance conflict")
                return cast(AssetEditCandidate, existing)
            candidate.project_id = plan.project_id
            candidate.episode_id = plan.episode_id
            candidate.target_id = plan.target_id
            if execution_id is not None:
                candidate.provenance["executionId"] = execution_id
            uow.asset_edit_candidates[candidate.id] = candidate
            await uow.commit()
        return candidate

    async def decide(
        self,
        candidate_id: str,
        action: str,
        expected_revision: int,
        *,
        expected_base_version_id: str | None = None,
        scope: tuple[str, ...] = (),
        candidate_facts: dict[str, object] | None = None,
        logical_operation: str | None = None,
        project_scope: str | None = None,
    ) -> AssetEditCandidate:
        async with self._uow_factory() as uow:
            candidate = uow.asset_edit_candidates.get(candidate_id)
            if candidate is None:
                raise AssetEditNotFoundError("asset edit candidate not found")
            plan = uow.asset_edit_plans.get(candidate.plan_id)
            if plan is None:
                raise AssetEditNotFoundError("asset edit plan not found")
            self._require_project_scope(plan.project_id, project_scope)
            if candidate.asset_version.kind not in {"image", "video"}:
                raise ValidationDomainError("asset edit candidate kind is invalid")
            if candidate.status != "pending_review":
                raise ValidationDomainError("candidate is terminal")
            if action == "approve":
                raise ValidationDomainError("review decision must be accept, reject or retake")
            if action == "retake" and not logical_operation:
                raise ValidationDomainError("retake logicalOperation is required")
            if action == "accept" and (not scope or any(not value for value in scope)):
                raise ValidationDomainError("explicit acceptance scope is required")
            if expected_base_version_id is not None and expected_base_version_id != plan.base.id:
                raise AssetEditConflictError("base version is stale")
            if candidate_facts is not None:
                self._validate_candidate_facts(candidate, plan, candidate_facts)
            if action == "accept":
                await self._validate_plan_continuity(uow, plan)
                if plan.target_id and plan.target_id not in scope:
                    raise ValidationDomainError("acceptance scope is outside plan impact")
                if self._scenes_service is None:
                    raise ValidationDomainError("scene eligibility owner is not configured")
                await self._scenes_service.accept_current_media_in_transaction(
                    uow,
                    project_id=plan.project_id,
                    episode_id=plan.episode_id,
                    shot_id=plan.target_id,
                    candidate={
                        "candidateId": candidate.id,
                        "candidateRevision": candidate.revision,
                        "projectId": candidate.project_id,
                        "episodeId": candidate.episode_id,
                        "targetId": candidate.target_id,
                        "assetVersionId": candidate.asset_version.id,
                        "assetVersionRevision": candidate.asset_version.revision,
                        "assetVersionHash": candidate.asset_version.content_hash,
                        "provenance": "asset_edit",
                        "mediaKind": candidate.asset_version.kind,
                    },
                    expected_shot_revision=int(
                        str((candidate_facts or {}).get("expectedTargetRevision", 1))
                    ),
                )
            candidate.decide(action, expected_revision)
            if action == "retake":
                uow.outbox_events.append(
                    {
                        "type": "asset-edit.retake.requested",
                        "candidateId": candidate.id,
                        "planId": plan.id,
                        "logicalOperation": logical_operation,
                    }
                )
            uow.accept_decisions[candidate.id] = AcceptDecision(
                candidate.id,
                action,  # type: ignore[arg-type]
                expected_revision,
                (candidate.target_id,),
            )
            await uow.commit()
            return cast(AssetEditCandidate, candidate)

    @staticmethod
    def _validate_candidate_facts(
        candidate: AssetEditCandidate, plan: AssetEditPlan, facts: dict[str, object]
    ) -> None:
        expected = {
            "candidateId": candidate.id,
            "projectId": plan.project_id,
            "episodeId": plan.episode_id,
            "targetId": plan.target_id,
            "assetVersionId": candidate.asset_version.id,
            "assetVersionRevision": candidate.asset_version.revision,
            "assetVersionHash": candidate.asset_version.content_hash,
        }
        for key, value in expected.items():
            if key in facts and facts[key] != value:
                raise AssetEditConflictError("candidate provenance is stale or foreign")

    async def _validate_plan_continuity(self, uow: Any, plan: AssetEditPlan) -> None:
        if plan.continuity is not None:
            self._validate_continuity(uow, plan.project_id, plan.target_id, plan.continuity)

    async def get_plan(self, plan_id: str, *, project_scope: str | None = None) -> AssetEditPlan:
        async with self._uow_factory() as uow:
            plan = uow.asset_edit_plans.get(plan_id)
            if plan is None:
                raise AssetEditNotFoundError("asset edit plan not found")
            self._require_project_scope(plan.project_id, project_scope)
            return cast(AssetEditPlan, plan)

    async def list_sessions(
        self, project_id: str, episode_id: str | None = None
    ) -> list[AssetEditSession]:
        """读取 project-scoped session index；不会恢复其他 episode 的 session。"""
        async with self._uow_factory() as uow:
            return [
                cast(AssetEditSession, session)
                for session in uow.asset_edit_sessions.values()
                if session.project_id == project_id
                and (episode_id is None or session.episode_id == episode_id)
            ]

    async def get_session_projection(self, project_id: str, session_id: str) -> dict[str, object]:
        """返回 Review 恢复所需 owner projection，不复制 owner facts 到 UI 状态。"""
        async with self._uow_factory() as uow:
            session = uow.asset_edit_sessions.get(session_id)
            if session is None or session.project_id != project_id:
                raise AssetEditNotFoundError("asset edit session not found")
            conversation = uow.conversations.get(session_id)
            plans = [
                cast(AssetEditPlan, plan)
                for plan in uow.asset_edit_plans.values()
                if plan.project_id == project_id
                and plan.episode_id == session.episode_id
                and plan.turn_id
                in {turn.id for turn in (conversation.turns if conversation is not None else [])}
            ]
            return self._session_projection(uow, session, conversation, plans)

    async def get_plan_projection(self, project_id: str, plan_id: str) -> dict[str, object]:
        async with self._uow_factory() as uow:
            plan = uow.asset_edit_plans.get(plan_id)
            if plan is None or plan.project_id != project_id:
                raise AssetEditNotFoundError("asset edit plan not found")
            candidates = [
                cast(AssetEditCandidate, candidate)
                for candidate in uow.asset_edit_candidates.values()
                if candidate.plan_id == plan.id
            ]
            impact = next(
                (
                    cast(EditImpact, value)
                    for value in uow.edit_impacts.values()
                    if value.plan_id == plan.id
                ),
                None,
            )
            return self._plan_projection(uow, plan, candidates, impact)

    @classmethod
    def _session_projection(
        cls, uow: Any, session: AssetEditSession, conversation: Any, plans: list[AssetEditPlan]
    ) -> dict[str, object]:
        continuity = cls._continuity_projection(uow, session.project_id, session.continuity)
        return {
            "id": session.id,
            "schemaVersion": "1.0.0",
            "revision": session.revision,
            "status": session.status,
            "projectId": session.project_id,
            "episodeId": session.episode_id,
            "targetId": session.selection.target_id,
            "selection": {
                "projectId": session.selection.project_id,
                "episodeId": session.selection.episode_id,
                "targetId": session.selection.target_id,
                "primary": cls._version_ref_projection(session.selection.primary),
                "references": [
                    cls._version_ref_projection(item) for item in session.selection.references
                ],
            },
            "continuity": continuity,
            "conversation": cls._conversation_projection(conversation),
            "plans": [
                cls._plan_projection(
                    uow,
                    plan,
                    [
                        cast(AssetEditCandidate, candidate)
                        for candidate in uow.asset_edit_candidates.values()
                        if candidate.plan_id == plan.id
                    ],
                    next(
                        (
                            cast(EditImpact, value)
                            for value in uow.edit_impacts.values()
                            if value.plan_id == plan.id
                        ),
                        None,
                    ),
                )
                for plan in plans
            ],
        }

    @classmethod
    def _conversation_projection(cls, conversation: Any) -> dict[str, object]:
        if conversation is None:
            return {"id": "", "schemaVersion": "1.0.0", "revision": 0, "messages": [], "turns": []}
        return {
            "id": conversation.id,
            "schemaVersion": "1.0.0",
            "projectId": conversation.project_id,
            "episodeId": conversation.episode_id,
            "revision": conversation.revision,
            "messages": [
                {
                    "id": item.id,
                    "sessionId": item.session_id,
                    "sequence": item.sequence,
                    "role": item.role,
                    "contentHash": item.content_hash,
                    "status": item.status,
                    "correlationId": item.correlation_id,
                }
                for item in sorted(conversation.messages, key=lambda value: value.sequence)
            ],
            "turns": [
                {
                    "id": item.id,
                    "sessionId": item.session_id,
                    "sequence": item.sequence,
                    "userMessageId": item.user_message_id,
                    "agentMessageId": item.agent_message_id,
                    "status": item.status,
                    "revision": item.revision,
                }
                for item in sorted(conversation.turns, key=lambda value: value.sequence)
            ],
        }

    @staticmethod
    def _version_ref_projection(reference: AssetVersionRef) -> dict[str, object]:
        return {
            "assetVersionId": reference.id,
            "revision": reference.revision,
            "contentHash": reference.content_hash,
            "kind": reference.kind,
            "projectId": reference.project_id,
            "mimeType": reference.mime_type,
        }

    @staticmethod
    def _candidate_provenance_projection(
        uow: Any, candidate: AssetEditCandidate, plan: AssetEditPlan
    ) -> dict[str, object]:
        projection = dict(candidate.provenance)
        shot = uow.shots.get(plan.target_id)
        current = None
        if shot is not None:
            current = (
                shot.current_image
                if candidate.asset_version.kind == "image"
                else shot.current_video
            )
        accepted_current = bool(
            current is not None
            and current.candidate_id == candidate.id
            and current.candidate_revision == candidate.revision - 1
            and current.asset_version_id == candidate.asset_version.id
            and current.asset_version_revision == candidate.asset_version.revision
            and current.asset_version_hash == candidate.asset_version.content_hash
        )
        projection["acceptedCurrent"] = accepted_current
        if accepted_current and current is not None:
            projection["derivativeStatus"] = current.derivative_status
        return projection

    @classmethod
    def _plan_projection(
        cls, uow: Any, plan: AssetEditPlan, candidates: list[AssetEditCandidate], impact: Any
    ) -> dict[str, object]:
        continuity = cls._continuity_projection(uow, plan.project_id, plan.continuity)
        status = impact.status if impact is not None else "clear"
        if continuity["status"] != "accepted_current":
            status = "continuity_stale"
        return {
            "id": plan.id,
            "schemaVersion": plan.schema_version,
            "revision": plan.revision,
            "projectId": plan.project_id,
            "episodeId": plan.episode_id,
            "targetId": plan.target_id,
            "turnId": plan.turn_id,
            "sessionId": plan.session_id,
            "runId": plan.run_id,
            "nodeRunId": plan.node_run_id,
            "logicalOperation": plan.logical_operation,
            "correlationId": plan.correlation_id,
            "status": plan.status,
            "instruction": plan.instruction,
            "base": cls._version_ref_projection(plan.base),
            "references": [cls._version_ref_projection(item) for item in plan.references],
            "cost": {
                "status": "unknown",
                "source": "owner_unavailable",
                "currency": None,
                "estimated": None,
            },
            "impact": {
                "id": impact.id if impact is not None else None,
                "status": status,
                "reasons": list(impact.reasons) if impact is not None else [],
                "staleTargets": [],
            },
            "continuity": continuity,
            "candidates": [
                {
                    "id": candidate.id,
                    "schemaVersion": "1.0.0",
                    "revision": candidate.revision,
                    "status": candidate.status,
                    "projectId": candidate.project_id or plan.project_id,
                    "episodeId": candidate.episode_id or plan.episode_id,
                    "targetId": candidate.target_id or plan.target_id,
                    "assetVersion": cls._version_ref_projection(candidate.asset_version),
                    "provenance": cls._candidate_provenance_projection(uow, candidate, plan),
                }
                for candidate in candidates
            ],
        }

    @staticmethod
    def _continuity_projection(
        uow: Any, project_id: str, reference: ContinuitySnapshotRef | None
    ) -> dict[str, object]:
        if reference is None:
            return {"status": "not_bound", "snapshot": None, "chain": [], "tasks": []}
        snapshot = uow.asset_bible_snapshots.get(reference.id)
        tasks = [
            task
            for task in uow.asset_bible_tasks.values()
            if task.project_id == project_id and task.target_id == reference.target_id
        ]
        snapshot_valid = bool(
            snapshot is not None
            and snapshot.project_id == project_id
            and snapshot.status == "accepted"
            and snapshot.revision == reference.revision
            and snapshot.content_hash == reference.content_hash
        )
        pending = any(task.status in {"pending", "acknowledged"} for task in tasks)
        return {
            "status": "accepted_current" if snapshot_valid and not pending else "continuity_stale",
            "snapshot": {
                "id": reference.id,
                "revision": reference.revision,
                "contentHash": reference.content_hash,
                "targetId": reference.target_id,
            },
            "chain": [
                {"targetId": item.target_id, "level": item.level, "revision": item.revision}
                for item in (snapshot.refs if snapshot is not None else ())
            ],
            "tasks": [
                {
                    "id": task.id,
                    "targetId": task.target_id,
                    "status": task.status,
                    "revision": task.revision,
                }
                for task in tasks
            ],
        }

    async def list_candidates(
        self, plan_id: str, *, project_scope: str | None = None
    ) -> list[AssetEditCandidate]:
        async with self._uow_factory() as uow:
            plan = uow.asset_edit_plans.get(plan_id)
            if plan is None:
                raise AssetEditNotFoundError("asset edit plan not found")
            self._require_project_scope(plan.project_id, project_scope)
            return [
                cast(AssetEditCandidate, item)
                for item in uow.asset_edit_candidates.values()
                if item.plan_id == plan_id
            ]

    async def compare_candidate(
        self, candidate_id: str, *, project_scope: str | None = None
    ) -> dict[str, object]:
        async with self._uow_factory() as uow:
            candidate = uow.asset_edit_candidates.get(candidate_id)
            if candidate is None:
                raise AssetEditNotFoundError("asset edit candidate not found")
            plan = uow.asset_edit_plans.get(candidate.plan_id)
            if plan is None:
                raise AssetEditNotFoundError("asset edit plan not found")
            self._require_project_scope(plan.project_id, project_scope)
            reasons: list[str] = []
            try:
                await self._validate_versions(uow, plan.project_id, (plan.base,))
            except AssetEditConflictError:
                reasons.append("base_version_conflict")
            try:
                await self._validate_plan_continuity(uow, plan)
            except AssetEditContinuityStaleError:
                reasons.append("continuity_stale")
            status: Literal["clear", "stale", "continuity_stale"] = "stale" if reasons else "clear"
            impact = EditImpact(plan.id, status, tuple(reasons))
            uow.edit_impacts[impact.id] = impact
            await uow.commit()
            return {"id": impact.id, "planId": plan.id, "status": status, "reasons": reasons}

    async def reconcile_execution(
        self,
        execution_id: str,
        status: str,
        *,
        provider_request_id: str | None = None,
        project_scope: str | None = None,
    ) -> AssetEditExecution:
        async with self._uow_factory() as uow:
            execution = uow.asset_edit_executions.get(execution_id)
            if execution is None:
                raise AssetEditNotFoundError("asset edit execution not found")
            plan = uow.asset_edit_plans.get(execution.plan_id)
            if plan is None:
                raise AssetEditNotFoundError("asset edit plan not found")
            self._require_project_scope(plan.project_id, project_scope)
            execution.transition(status)
            if provider_request_id is not None:
                if execution.provider_request_id not in {None, provider_request_id}:
                    raise ValidationDomainError("provider request id conflict")
                execution.provider_request_id = provider_request_id
            await uow.commit()
            return cast(AssetEditExecution, execution)

    async def get_execution(
        self, execution_id: str, *, project_scope: str | None = None
    ) -> AssetEditExecution:
        async with self._uow_factory() as uow:
            execution = uow.asset_edit_executions.get(execution_id)
            if execution is None:
                raise AssetEditNotFoundError("asset edit execution not found")
            plan = uow.asset_edit_plans.get(execution.plan_id)
            if plan is None:
                raise AssetEditNotFoundError("asset edit plan not found")
            self._require_project_scope(plan.project_id, project_scope)
            return cast(AssetEditExecution, execution)

    async def execute(
        self,
        plan_id: str,
        plan_revision: int,
        run_id: str,
        node_run_id: str,
        logical_operation: str,
        correlation_id: str,
        request_fingerprint: str,
        *,
        project_scope: str | None = None,
    ) -> AssetEditExecution:
        async with self._uow_factory() as uow:
            plan = uow.asset_edit_plans.get(plan_id)
            if plan is None:
                raise AssetEditNotFoundError("asset edit plan not found")
            self._require_project_scope(plan.project_id, project_scope)
            if not run_id or not node_run_id or not logical_operation or not correlation_id:
                raise ValidationDomainError(
                    "runId, nodeRunId, logicalOperation and correlationId are required"
                )
            key = (run_id, node_run_id, logical_operation)
            existing = next(
                (
                    item
                    for item in uow.asset_edit_executions.values()
                    if (item.run_id, item.node_run_id, item.logical_operation) == key
                ),
                None,
            )
            if existing is not None:
                if existing.request_fingerprint != request_fingerprint:
                    raise AssetEditConflictError("asset edit execution idempotency conflict")
                return cast(AssetEditExecution, existing)
            if plan.revision != plan_revision or plan.status != "pending_review":
                raise AssetEditConflictError("asset edit plan is stale")
            await self._validate_versions(uow, plan.project_id, (plan.base, *plan.references))
            await self._validate_plan_continuity(uow, plan)
            execution = AssetEditExecution(
                plan_id,
                plan_revision,
                run_id,
                node_run_id,
                logical_operation,
                correlation_id,
                request_fingerprint,
            )
            uow.asset_edit_executions[execution.id] = execution
            plan.status = "executing"
            plan.revision += 1
            uow.outbox_events.append(
                {"type": "asset-edit.execute", "executionId": execution.id, "runId": run_id}
            )
            await uow.commit()
            return execution
