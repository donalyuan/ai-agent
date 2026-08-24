from __future__ import annotations

import asyncio
import hashlib
import json
from dataclasses import replace
from types import SimpleNamespace

import pytest
from fastapi.testclient import TestClient

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.app import create_app
from video_agent_api.application.catalog import CatalogService
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.runs import (
    BudgetGateCommand,
    EnsureWorkflowCommand,
    HistoricalRerunCommand,
    MediaCandidateCommand,
    MediaInspectCommand,
    ProviderObservationCommand,
    ReviewSignalCommand,
    RunsService,
    SuccessorRunCommand,
)
from video_agent_api.application.scenes import ScenesService
from video_agent_api.domain.assets import Asset, AssetVersion, StorageObject
from video_agent_api.domain.errors import (
    ValidationDomainError,
    WorkflowRunConflictError,
    WorkflowSourceConflictError,
    WorkflowUnconfiguredError,
    WorkflowVersionUnavailableError,
)
from video_agent_api.domain.scenes import (
    AcceptedMediaEligibility,
    ImmutableOwnerRef,
    Scene,
    Shot,
)
from video_agent_api.domain.text_review import (
    StructuredTextCandidate,
    TextOwnerHandoff,
    TextOwnerHandoffAck,
    TextReviewBatch,
)
from video_agent_api.skills.router import RankedSkill, RouteDecision

ACTOR_UUID = "11111111-1111-4111-8111-111111111111"


def _seed_current_route(uow: InMemoryUnitOfWork, project_id: str) -> RouteDecision:
    skill = next(item for item in uow.skills if item.name == "novel-writing")
    ranked = RankedSkill(skill.name, skill.version, 10, skill.digest)
    decision = RouteDecision(
        (ranked,),
        ranked,
        False,
        None,
        ("deterministic_filter", "policy_decide"),
        f"default-current-route:{project_id}",
        1,
        f"default-route-fingerprint:{project_id}",
        project_id,
        "text.generate",
        f"default-launch:{project_id}",
    )
    uow.skill_route_decisions[decision.id] = decision
    return decision


async def _service() -> tuple[InMemoryUnitOfWork, RunsService, object, object]:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("P")
    await CatalogService(lambda: uow).bootstrap()
    _seed_current_route(uow, project.id)
    service = RunsService(lambda: uow)
    workflow = await service.ensure_workflow(
        EnsureWorkflowCommand(project.id, scope_ids=(project.id,))
    )
    return uow, service, project, workflow


def _hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


async def _media_gate_refs(
    uow: InMemoryUnitOfWork,
    project: object,
) -> tuple[tuple[dict[str, object], ...], tuple[dict[str, object], ...], Shot]:
    projects = ProjectsEpisodesService(lambda: uow)
    episode = await projects.create_episode((project.id, "Episode", 1))
    scene = Scene(project.id, episode.id, 1)
    shot = Shot(scene.id, project.id, episode.id, 1)
    scene.shots.append(shot)
    uow.scenes[scene.id] = scene
    uow.scenes_by_episode[episode.id] = [scene]
    uow.shots[shot.id] = shot
    spec = scene.append_shot_spec(
        shot,
        {"framing": "medium", "durationMs": 3000, "aspectRatio": "9:16"},
    )
    snapshot_hash = "c" * 64
    snapshot = SimpleNamespace(
        id="continuity-snapshot",
        project_id=project.id,
        target_id=shot.id,
        status="accepted",
        revision=1,
        content_hash=snapshot_hash,
    )
    uow.asset_bible_snapshots[snapshot.id] = snapshot
    shot.continuity_snapshot = ImmutableOwnerRef(snapshot.id, 1, snapshot_hash)

    image_asset = Asset(project.id, "image", "storyboard")
    image_version = AssetVersion(
        image_asset.id,
        project.id,
        1,
        StorageObject(
            "local",
            "workspace",
            "projects/storyboard.png",
            "image/png",
            32,
            "d" * 64,
            media={"width": 1080, "height": 1920},
        ),
        revision=1,
    )
    await uow.assets.add(image_asset)
    await uow.asset_versions.add(image_version)
    shot.current_image = AcceptedMediaEligibility(
        "accepted-storyboard",
        1,
        project.id,
        episode.id,
        shot.id,
        image_version.id,
        image_version.revision,
        str(image_version.content_hash),
        "text_review",
        "image",
        derivative_status="ready",
    )

    candidates: list[StructuredTextCandidate] = []
    for kind, count in (
        ("story_spec", 1),
        ("script_spec", 1),
        ("episode", 1),
        ("scene", 1),
        ("shot", 1),
        ("shot_spec", 1),
        ("asset_bible_spec", 6),
    ):
        for index in range(count):
            scope_id = f"{kind}-{index}"
            candidates.append(
                StructuredTextCandidate(
                    project.id,
                    kind,  # type: ignore[arg-type]
                    scope_id,
                    {"kind": kind, "schema_version": "1.0.0", "scopeId": scope_id},
                    status="accepted",
                    run_id="text-run",
                )
            )
    batch = TextReviewBatch(
        project.id,
        "text-run",
        1,
        tuple(candidates),
        {
            "creativeBrief": {
                "episodeCount": 1,
                "scenesPerEpisode": 1,
                "shotsPerScene": 1,
            }
        },
        status="accepted",
        revision=2,
    )
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
        for item in candidates
    )
    handoff = TextOwnerHandoff(
        batch.id,
        batch.revision,
        project.id,
        batch.run_id,
        refs,
        _hash(list(refs)),
        batch.run_id,
        ("projects", "episodes", "scenes", "asset_bible"),
    )
    uow.text_review_batches[batch.id] = batch
    uow.text_handoffs[handoff.id] = handoff
    for owner in handoff.required_owners:
        ack = TextOwnerHandoffAck(
            handoff.id, owner, 1, f"{owner}-fingerprint", handoff.correlation_id
        )
        uow.text_handoff_acks[f"{handoff.id}:{owner}"] = ack

    scope_refs = (
        {
            "type": "shot",
            "projectId": project.id,
            "episodeId": episode.id,
            "sceneId": scene.id,
            "shotId": shot.id,
            "revision": shot.revision,
            "shotSpecRevision": spec.revision,
            "shotSpecHash": spec.content_hash,
            "snapshotId": snapshot.id,
            "snapshotRevision": snapshot.revision,
            "contentHash": snapshot.content_hash,
            "candidateId": shot.current_image.candidate_id,
            "assetVersionId": shot.current_image.asset_version_id,
            "assetVersionRevision": shot.current_image.asset_version_revision,
            "assetVersionHash": shot.current_image.asset_version_hash,
        },
    )
    owner_refs = (
        {
            "type": "textReviewHandoff",
            "projectId": project.id,
            "handoffId": handoff.id,
            "textReviewBatchId": batch.id,
            "revision": batch.revision,
            "textReviewBatchRevision": batch.revision,
            "payloadHash": handoff.payload_hash,
        },
    )
    return scope_refs, owner_refs, shot


async def _candidate(
    uow: InMemoryUnitOfWork,
    shot: Shot,
    media_kind: str,
) -> dict[str, object]:
    asset = Asset(shot.project_id, media_kind, f"generated-{media_kind}")
    mime = "image/png" if media_kind == "image" else "video/mp4"
    media = (
        {"width": 1080, "height": 1920}
        if media_kind == "image"
        else {"duration_ms": 3000, "width": 1080, "height": 1920}
    )
    version = AssetVersion(
        asset.id,
        shot.project_id,
        1,
        StorageObject(
            "local",
            "workspace",
            f"projects/generated-{media_kind}",
            mime,
            64,
            ("e" if media_kind == "image" else "f") * 64,
            media=media,
        ),
        revision=1,
    )
    await uow.assets.add(asset)
    await uow.asset_versions.add(version)
    value: dict[str, object] = {
        "candidateId": f"{media_kind}-candidate",
        "candidateRevision": 1,
        "projectId": shot.project_id,
        "episodeId": shot.episode_id,
        "targetId": shot.id,
        "assetVersionId": version.id,
        "assetVersionRevision": version.revision,
        "assetVersionHash": version.content_hash,
        "provenance": "media_review",
        "mediaKind": media_kind,
        "storageStatus": "verified",
        "providerStatus": "succeeded",
        "expectedShotRevision": shot.revision,
    }
    if media_kind == "video":
        value.update(
            {
                "shotSpecRevision": shot.spec_ref.revision,
                "shotSpecHash": shot.spec_ref.content_hash,
                "durationMs": 3000,
                "aspectRatio": "9:16",
            }
        )
    return value


async def test_default_workflow_is_fixed_published_and_ensure_is_idempotent() -> None:
    uow, service, project, workflow = await _service()
    retry = await service.ensure_workflow(
        EnsureWorkflowCommand(project.id, scope_ids=(project.id,))
    )
    assert retry.id == workflow.id
    assert workflow.status == "published" and workflow.content_hash
    assert [item["key"] for item in workflow.definition["nodes"]] == [
        "text.generate",
        "text.review",
        "media.generate.image",
        "media.review.image",
        "media.generate.video",
        "media.review.video",
        "media.inspect",
        "timeline.handoff",
    ]
    assert workflow.definition["skills"] == ["novel-writing", "drama-skills"]
    assert len(uow.workflow_by_project) == 1

    projection = await service.get_default_workflow_projection(project.id)
    assert projection["id"] == workflow.id
    assert projection["bindingId"] == uow.workflow_bindings[project.id].id
    assert projection["bindingRevision"] == uow.workflow_bindings[project.id].revision
    assert projection["contentHash"] == workflow.content_hash


@pytest.mark.parametrize(
    ("case", "error_type"),
    [
        ("missing-binding", WorkflowUnconfiguredError),
        ("missing-source", WorkflowVersionUnavailableError),
        ("non-published", WorkflowVersionUnavailableError),
        ("cross-project", WorkflowSourceConflictError),
        ("scope-mismatch", WorkflowSourceConflictError),
        ("hash-mismatch", WorkflowSourceConflictError),
    ],
)
async def test_workflow_source_rejections_precede_run_side_effects(
    case: str, error_type: type[Exception]
) -> None:
    uow, service, project, workflow = await _service()
    binding = uow.workflow_bindings[project.id]
    if case == "missing-binding":
        uow.workflow_bindings.pop(project.id)
    elif case == "missing-source":
        uow.workflow_by_project.pop(project.id)
    elif case == "non-published":
        object.__setattr__(workflow, "status", "draft")
    elif case == "cross-project":
        object.__setattr__(workflow, "project_id", "foreign-project")
    elif case == "scope-mismatch":
        object.__setattr__(workflow, "scope_ids", ("foreign-project",))
    elif case == "hash-mismatch":
        uow.workflow_bindings[project.id] = replace(
            binding, workflow_content_hash="f" * 64, revision=binding.revision + 1
        )

    before = (
        len(uow.workflow_runs),
        len(uow.run_events),
        len(uow.run_input_snapshots),
        len(uow.temporal_starts),
        len(uow.audit_events),
        len(uow.outbox_events),
    )
    with pytest.raises(error_type):
        await service.start_run(
            project.id,
            workflow.id,
            ["text.generate"],
            idempotency_key=f"source-rejection-{case}",
        )
    assert before == (
        len(uow.workflow_runs),
        len(uow.run_events),
        len(uow.run_input_snapshots),
        len(uow.temporal_starts),
        len(uow.audit_events),
        len(uow.outbox_events),
    )


async def test_run_source_snapshot_is_deeply_frozen_from_later_binding_changes() -> None:
    uow, service, project, workflow = await _service()
    run = await service.start_run(project.id, workflow.id, ["text.generate"])
    snapshot_id = str(run.input_snapshot["snapshotId"])
    original_source = json.loads(json.dumps(run.source_snapshot))
    binding = uow.workflow_bindings[project.id]
    uow.workflow_bindings[project.id] = replace(binding, revision=binding.revision + 1)
    workflow.definition["nodes"] = []

    assert run.source_snapshot == original_source
    assert uow.run_input_snapshots[snapshot_id].source_snapshot == original_source
    assert run.source_snapshot["bindingRevision"] == 1


async def test_run_events_review_signal_and_sse_cursor_are_monotonic() -> None:
    uow, service, project, workflow = await _service()
    run = await service.start_run(
        project.id,
        workflow.id,
        ["text.generate"],
        idempotency_key="start-once",
    )
    retry = await service.start_run(
        project.id,
        workflow.id,
        ["text.generate"],
        idempotency_key="start-once",
    )
    assert retry.id == run.id
    node = run.nodes[0]
    await service.transition_node(run.id, node.id, "running")
    await service.transition_node(run.id, node.id, "waiting_review")
    before = len(uow.run_events[run.id])
    with pytest.raises(ValidationDomainError, match="accept, reject or retake"):
        await service.signal_review(
            ReviewSignalCommand(run.id, node.id, node.revision, "approve", "legacy", "user")
        )
    assert len(uow.run_events[run.id]) == before
    await service.signal_review(
        ReviewSignalCommand(run.id, node.id, node.revision, "accept", "review", ACTOR_UUID)
    )
    events = await service.events(run.id)
    assert [item.sequence for item in events] == list(range(1, len(events) + 1))
    assert [item.sequence for item in await service.events(run.id, 2)] == list(
        range(3, len(events) + 1)
    )
    cleanup = await service.cleanup_events(run.id)
    assert cleanup == {
        "status": "skipped",
        "diagnostic": "run_event_long_term_no_gc",
        "retained": len(events),
    }


async def test_budget_gate_confirmation_is_exact_and_recovery_safe() -> None:
    uow, service, project, workflow = await _service()
    scope_refs, owner_refs, _shot = await _media_gate_refs(uow, project)
    run = await service.start_run(
        project.id,
        workflow.id,
        ["media.generate.image"],
        scope_refs=scope_refs,
        owner_refs=owner_refs,
    )
    node = run.nodes[0]
    await service.transition_node(run.id, node.id, "running")
    gate = await service.create_budget_gate(
        BudgetGateCommand(
            run.id,
            node.id,
            node.logical_operation,
            "fingerprint",
            "image.generate",
            4,
            "unknown",
            None,
            None,
            "threshold",
            1,
        )
    )
    assert gate.status == "pending_confirmation" and run.status == "waiting_review"
    with pytest.raises(WorkflowRunConflictError, match="mismatched"):
        await service.confirm_budget(
            run.id, node.logical_operation, "foreign", "confirmation", ACTOR_UUID
        )
    confirmed = await service.confirm_budget(
        run.id, node.logical_operation, "fingerprint", "confirmation", ACTOR_UUID
    )
    assert confirmed.status == "confirmed" and confirmed.user_uuid == ACTOR_UUID
    retry = await service.confirm_budget(
        run.id, node.logical_operation, "fingerprint", "confirmation", ACTOR_UUID
    )
    assert retry.id == confirmed.id
    with pytest.raises(WorkflowRunConflictError, match="stale or mismatched"):
        await service.confirm_budget(
            run.id, node.logical_operation, "fingerprint", "duplicate", ACTOR_UUID
        )
    assert len(uow.budget_gates) == 1


async def test_budget_gate_binds_exact_operation_and_text_threshold() -> None:
    uow, service, project, workflow = await _service()
    run = await service.start_run(project.id, workflow.id, ["text.generate"])
    node = run.nodes[0]
    await service.transition_node(run.id, node.id, "running")
    before = (len(uow.budget_gates), len(uow.run_events[run.id]), len(uow.outbox_events))
    with pytest.raises(ValidationDomainError, match="threshold snapshot"):
        await service.create_budget_gate(
            BudgetGateCommand(
                run.id,
                node.id,
                node.logical_operation,
                "text-fingerprint",
                "text.generate",
                1,
                "known",
                "0.50",
                "CNY",
                None,
                None,
            )
        )
    with pytest.raises(WorkflowRunConflictError, match="run scope"):
        await service.create_budget_gate(
            BudgetGateCommand(
                run.id,
                node.id,
                "foreign-operation",
                "text-fingerprint",
                "text.generate",
                1,
                "known",
                "0.50",
                "CNY",
                "threshold-1",
                1,
            )
        )
    assert before == (
        len(uow.budget_gates),
        len(uow.run_events[run.id]),
        len(uow.outbox_events),
    )
    run = uow.workflow_runs[run.id]
    node = run.nodes[0]
    gate = await service.create_budget_gate(
        BudgetGateCommand(
            run.id,
            node.id,
            node.logical_operation,
            "text-fingerprint",
            "text.generate",
            1,
            "known",
            "0.50",
            "CNY",
            "threshold-1",
            1,
        )
    )
    current = uow.workflow_runs[run.id]
    assert gate.threshold_snapshot_id == "threshold-1" and current.status == "waiting_review"


async def test_cancel_late_result_successor_and_historical_rerun_preserve_history() -> None:
    uow, service, project, workflow = await _service()
    scope_refs, owner_refs, _shot = await _media_gate_refs(uow, project)
    cancelled = await service.start_run(
        project.id,
        workflow.id,
        ["media.generate.video"],
        scope_refs=scope_refs,
        owner_refs=owner_refs,
    )
    cancel_node = cancelled.nodes[0]
    await service.transition_node(cancelled.id, cancel_node.id, "running")
    await service.cancel(cancelled.id)
    assert cancelled.status == "cancel_requested"
    with pytest.raises(WorkflowRunConflictError, match="late node result"):
        await service.transition_node(cancelled.id, cancel_node.id, "succeeded")
    cancelled = await service.acknowledge_cancel(cancelled.id)
    assert cancelled.status == "cancelled"

    failed = await service.start_run(
        project.id, workflow.id, ["text.generate"], idempotency_key="failed"
    )
    failed_node = failed.nodes[0]
    await service.transition_node(failed.id, failed_node.id, "running")
    await service.transition_node(failed.id, failed_node.id, "failed")
    successor = await service.create_successor_from_failure(
        SuccessorRunCommand(project.id, failed.id, failed.revision)
    )
    assert failed.status == "failed" and successor.predecessor_run_id == failed.id
    assert successor.nodes[0].logical_operation != failed.nodes[0].logical_operation

    source_snapshot_id = str(failed.input_snapshot["snapshotId"])
    rerun = await service.create_run_from_historical_snapshot(
        HistoricalRerunCommand(project.id, source_snapshot_id, 1)
    )
    assert rerun.rerun_of_run_id == failed.id
    assert rerun.nodes[0].status == "pending"
    assert rerun.nodes[0].logical_operation != successor.nodes[0].logical_operation
    assert any(
        start.run_id == failed.id and start.logical_operation == failed_node.logical_operation
        for start in uow.temporal_starts.values()
    )


async def test_start_source_idempotency_and_selection_rejections_are_zero_write() -> None:
    uow, service, project, workflow = await _service()
    run = await service.start_run(
        project.id, workflow.id, ["text.generate"], idempotency_key="exact-start"
    )
    before = (
        len(uow.workflow_runs),
        len(uow.run_events),
        len(uow.outbox_events),
        len(uow.temporal_starts),
    )
    with pytest.raises(WorkflowRunConflictError, match="fingerprint conflict"):
        await service.start_run(
            project.id, workflow.id, ["text.review"], idempotency_key="exact-start"
        )
    assert before == (
        len(uow.workflow_runs),
        len(uow.run_events),
        len(uow.outbox_events),
        len(uow.temporal_starts),
    )

    selected_digest = next(item.digest for item in uow.skills if item.name == "drama-skills")
    selected_skill = RankedSkill("drama-skills", "1.0.0", 10, selected_digest)
    decision = RouteDecision(
        (selected_skill,),
        selected_skill,
        False,
        None,
        ("deterministic_filter", "policy_decide"),
        "decision-current",
        1,
        "input-fingerprint",
        project.id,
        "text.generate",
        "launch-current",
    )
    uow.skill_route_decisions[decision.id] = decision
    routed = await service.start_run(
        project.id,
        workflow.id,
        ["text.generate"],
        route_decision_id=decision.id,
        idempotency_key="selected-route",
    )
    assert routed.selection_snapshot["routeDecisionId"] == decision.id
    assert routed.selection_snapshot["skillDigests"] == [selected_digest]

    pending_decision = RouteDecision(
        (selected_skill,),
        None,
        True,
        "manual_required",
        ("deterministic_filter", "policy_decide"),
        "decision-pending",
        1,
        "pending-fingerprint",
        project.id,
        "text.generate",
        "launch-pending",
    )
    uow.skill_route_decisions[pending_decision.id] = pending_decision
    pending_counts = (len(uow.workflow_runs), len(uow.temporal_starts), len(uow.outbox_events))
    with pytest.raises(ValidationDomainError, match="explicit human selection"):
        await service.start_run(
            project.id,
            workflow.id,
            ["text.generate"],
            route_decision_id=pending_decision.id,
            idempotency_key="pending-decision",
        )
    assert pending_counts == (
        len(uow.workflow_runs),
        len(uow.temporal_starts),
        len(uow.outbox_events),
    )

    pending = {**run.selection_snapshot, "routeStatus": "pending"}
    pending_snapshot_counts = (
        len(uow.workflow_runs),
        len(uow.run_events),
        len(uow.outbox_events),
        len(uow.temporal_starts),
    )
    with pytest.raises(ValidationDomainError, match="pending, stale, disabled"):
        await service.start_run(
            project.id,
            workflow.id,
            ["text.generate"],
            selection_snapshot=pending,
            idempotency_key="pending-route",
        )
    assert pending_snapshot_counts == (
        len(uow.workflow_runs),
        len(uow.run_events),
        len(uow.outbox_events),
        len(uow.temporal_starts),
    )


async def test_run_creation_requires_catalog_backed_current_skill_route() -> None:
    uow, service, project, workflow = await _service()
    decision = next(iter(uow.skill_route_decisions.values()))
    uow.skill_route_decisions.clear()
    before = (len(uow.workflow_runs), len(uow.temporal_starts), len(uow.outbox_events))
    with pytest.raises(ValidationDomainError, match="unique current skill route"):
        await service.start_run(project.id, workflow.id, ["text.generate"])
    assert before == (len(uow.workflow_runs), len(uow.temporal_starts), len(uow.outbox_events))

    uow.skill_route_decisions[decision.id] = decision
    run = await service.start_run(
        project.id,
        workflow.id,
        ["text.generate"],
        route_decision_id=decision.id,
        idempotency_key="owner-route",
    )
    with pytest.raises(ValidationDomainError, match="client selection does not match"):
        await service.start_run(
            project.id,
            workflow.id,
            ["text.generate"],
            selection_snapshot=run.selection_snapshot,
            route_decision_id=decision.id,
            idempotency_key="client-snapshot-bypass",
        )

    stale = RankedSkill(
        decision.candidates[0].name,
        decision.candidates[0].version,
        decision.candidates[0].score,
        "f" * 64,
    )
    stale_decision = replace(
        decision,
        id="stale-route",
        candidates=(stale,),
        selected=stale,
    )
    uow.skill_route_decisions[stale_decision.id] = stale_decision
    with pytest.raises(ValidationDomainError, match="stale or unapproved"):
        await service.start_run(
            project.id,
            workflow.id,
            ["text.generate"],
            route_decision_id=stale_decision.id,
            idempotency_key="stale-route",
        )


@pytest.mark.parametrize(
    ("unsafe_input", "message"),
    [
        (
            {"scope_refs": ({"projectId": "PROJECT_ID", "revision": 1, "title": "private title"},)},
            "stable owner references",
        ),
        (
            {"owner_refs": ({"ownerId": "owner-1", "revision": 1, "contentHash": "not-a-hash"},)},
            "invalid owner hash",
        ),
        (
            {"scope_refs": ({"projectId": "foreign-project", "revision": 1},)},
            "foreign project reference",
        ),
        ({"selection_snapshot": {"apiKey": "plaintext"}}, "selection is unresolved"),
    ],
)
async def test_run_start_rejects_non_reference_input_before_uow(
    unsafe_input: dict[str, object], message: str
) -> None:
    uow, service, project, workflow = await _service()
    normalized = {
        key: (
            tuple(
                {
                    **item,
                    **({"projectId": project.id} if item.get("projectId") == "PROJECT_ID" else {}),
                }
                for item in value
            )
            if key in {"scope_refs", "owner_refs"}
            else value
        )
        for key, value in unsafe_input.items()
    }
    before = (
        len(uow.workflow_runs),
        len(uow.run_events),
        len(uow.run_input_snapshots),
        len(uow.temporal_starts),
        len(uow.audit_events),
        len(uow.outbox_events),
    )
    with pytest.raises(ValidationDomainError, match=message):
        await service.start_run(
            project.id,
            workflow.id,
            ["text.generate"],
            idempotency_key=f"unsafe-{message}",
            **normalized,
        )
    assert before == (
        len(uow.workflow_runs),
        len(uow.run_events),
        len(uow.run_input_snapshots),
        len(uow.temporal_starts),
        len(uow.audit_events),
        len(uow.outbox_events),
    )


async def test_waiting_review_aggregate_and_duplicate_signal_are_exact() -> None:
    uow, service, project, workflow = await _service()
    run = await service.start_run(project.id, workflow.id, ["text.generate", "text.review"])
    first, second = run.nodes
    for node in run.nodes:
        await service.transition_node(run.id, node.id, "running")
        await service.transition_node(run.id, node.id, "waiting_review")
    first_revision = first.revision
    accepted = ReviewSignalCommand(
        run.id, first.id, first_revision, "accept", "review-first", ACTOR_UUID
    )
    await service.signal_review(accepted)
    assert first.status == "succeeded" and second.status == "waiting_review"
    assert run.status == "waiting_review"
    event_count = len(uow.run_events[run.id])
    retry = await service.signal_review(accepted)
    assert retry.id == run.id and len(uow.run_events[run.id]) == event_count
    await service.signal_review(
        ReviewSignalCommand(
            run.id, second.id, second.revision, "accept", "review-second", ACTOR_UUID
        )
    )
    assert run.status == "succeeded"


async def test_successor_reuse_requires_exact_evidence_and_reconciliation() -> None:
    uow, service, project, workflow = await _service()
    failed = await service.start_run(project.id, workflow.id, ["text.generate", "text.review"])
    reusable, failed_node = failed.nodes
    await service.transition_node(failed.id, reusable.id, "running")
    reusable.output_evidence = {"contentHash": "a" * 64, "revision": 1}
    await service.transition_node(failed.id, reusable.id, "succeeded")
    await service.transition_node(failed.id, failed_node.id, "running")
    await service.transition_node(failed.id, failed_node.id, "failed")
    successor = await service.create_successor_from_failure(
        SuccessorRunCommand(project.id, failed.id, failed.revision, (reusable.id,))
    )
    reused = successor.nodes[0]
    assert reused.status == "succeeded"
    assert reused.output_evidence["sourceNodeRunId"] == reusable.id
    assert (
        successor.selection_snapshot["selectionSnapshotId"]
        != failed.selection_snapshot["selectionSnapshotId"]
    )

    before = len(uow.workflow_runs)
    with pytest.raises(WorkflowRunConflictError, match="evidence"):
        await service.create_successor_from_failure(
            SuccessorRunCommand(project.id, failed.id, failed.revision, (failed_node.id,))
        )
    assert len(uow.workflow_runs) == before

    scope_refs, owner_refs, _shot = await _media_gate_refs(uow, project)
    unresolved = await service.start_run(
        project.id,
        workflow.id,
        ["media.generate.video"],
        scope_refs=scope_refs,
        owner_refs=owner_refs,
        idempotency_key="unknown",
    )
    unknown_node = unresolved.nodes[0]
    await service.transition_node(unresolved.id, unknown_node.id, "running")
    await service.mark_submission_unknown(unresolved.id, unknown_node.id)
    await service.transition_node(unresolved.id, unknown_node.id, "failed")
    with pytest.raises(WorkflowRunConflictError, match="requires reconciliation"):
        await service.create_successor_from_failure(
            SuccessorRunCommand(project.id, unresolved.id, unresolved.revision)
        )
    assert len(uow.workflow_runs) == before + 1


async def test_historical_snapshot_is_explicit_project_scoped_and_new_operations() -> None:
    uow, service, project, workflow = await _service()
    source = await service.start_run(project.id, workflow.id, ["text.generate"])
    snapshot_id = str(source.input_snapshot["snapshotId"])
    listed = await service.list_input_snapshots(project.id)
    assert [item["id"] for item in listed] == [snapshot_id]
    detail = await service.get_input_snapshot(project.id, snapshot_id)
    assert detail["runId"] == source.id and detail["runnable"] is True
    rerun = await service.create_run_from_historical_snapshot(
        HistoricalRerunCommand(project.id, snapshot_id, 1)
    )
    assert rerun.rerun_of_run_id == source.id
    assert rerun.source_snapshot == source.source_snapshot
    assert rerun.nodes[0].logical_operation != source.nodes[0].logical_operation
    with pytest.raises(ValidationDomainError, match="historical_snapshot_missing"):
        await service.create_run_from_historical_snapshot(
            HistoricalRerunCommand(project.id, "missing", 1)
        )


async def test_text_review_enters_once_and_resumes_only_after_exact_owner_acks() -> None:
    uow, service, project, workflow = await _service()
    run = await service.start_run(project.id, workflow.id, ["text.generate"])
    node = run.nodes[0]
    await service.transition_node(run.id, node.id, "running")
    candidates = [
        SimpleNamespace(
            id="candidate-1",
            revision=1,
            payload_hash="a" * 64,
            status="provisional",
        ),
        SimpleNamespace(
            id="candidate-2",
            revision=1,
            payload_hash="b" * 64,
            status="provisional",
        ),
    ]
    batch = SimpleNamespace(
        id="batch-1",
        revision=1,
        fingerprint="batch-fingerprint",
        project_id=project.id,
        run_id=run.id,
        status="pending_review",
        candidates=candidates,
    )
    uow.text_review_batches[batch.id] = batch
    await service.enter_text_review(run.id, node.id, batch.id, node.revision)
    first_event_count = len(uow.run_events[run.id])
    retry = await service.enter_text_review(run.id, node.id, batch.id, node.revision)
    assert retry.id == run.id and len(uow.run_events[run.id]) == first_event_count

    batch.status = "accepted"
    batch.revision = 2
    for candidate in candidates:
        candidate.status = "accepted"
    refs = tuple(
        {
            "candidateId": candidate.id,
            "candidateRevision": candidate.revision,
            "payloadHash": candidate.payload_hash,
        }
        for candidate in candidates
    )
    payload_hash = hashlib.sha256(
        json.dumps(list(refs), sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()
    handoff = SimpleNamespace(
        id="handoff-1",
        batch_id=batch.id,
        batch_revision=batch.revision,
        project_id=project.id,
        run_id=run.id,
        candidate_refs=refs,
        payload_hash=payload_hash,
        correlation_id=run.id,
        required_owners=("projects", "episodes", "scenes", "asset_bible"),
    )
    uow.text_handoffs[handoff.id] = handoff
    uow.text_handoff_acks["projects"] = SimpleNamespace(
        handoff_id=handoff.id,
        owner="projects",
        owner_revision=1,
        correlation_id=handoff.correlation_id,
    )
    before = len(uow.run_events[run.id])
    with pytest.raises(WorkflowRunConflictError, match="acknowledgements are incomplete"):
        await service.resume_text_review_handoff(run.id, node.id, handoff.id, node.revision)
    run = uow.workflow_runs[run.id]
    node = run.nodes[0]
    assert len(uow.run_events[run.id]) == before and node.status == "waiting_review"
    for owner in handoff.required_owners[1:]:
        uow.text_handoff_acks[owner] = SimpleNamespace(
            handoff_id=handoff.id,
            owner=owner,
            owner_revision=1,
            correlation_id=handoff.correlation_id,
        )
    await service.resume_text_review_handoff(run.id, node.id, handoff.id, node.revision)
    run = uow.workflow_runs[run.id]
    node = run.nodes[0]
    assert node.status == "succeeded" and run.status == "succeeded"
    consumed_count = len(uow.run_events[run.id])
    await service.resume_text_review_handoff(run.id, node.id, handoff.id, node.revision)
    assert len(uow.run_events[run.id]) == consumed_count


@pytest.mark.parametrize(
    "mutation",
    ["stale-batch", "partial-ack", "foreign-shot", "stale-shot", "stale-spec", "stale-snapshot"],
)
async def test_pre_media_gate_rejects_stale_owner_closure_without_side_effects(
    mutation: str,
) -> None:
    uow, service, project, workflow = await _service()
    scope_refs, owner_refs, shot = await _media_gate_refs(uow, project)
    handoff = next(iter(uow.text_handoffs.values()))
    batch = uow.text_review_batches[handoff.batch_id]
    if mutation == "stale-batch":
        object.__setattr__(batch, "status", "stale")
    elif mutation == "partial-ack":
        uow.text_handoff_acks.pop(f"{handoff.id}:scenes")
    elif mutation == "foreign-shot":
        scope_refs[0]["projectId"] = "foreign-project"
    elif mutation == "stale-shot":
        scope_refs[0]["revision"] = shot.revision + 1
    elif mutation == "stale-spec":
        scope_refs[0]["shotSpecHash"] = "0" * 64
    else:
        scope_refs[0]["contentHash"] = "0" * 64
    before = (
        len(uow.workflow_runs),
        len(uow.run_events),
        len(uow.provider_calls),
        len(uow.outbox_events),
        len(uow.temporal_starts),
    )
    with pytest.raises((ValidationDomainError, WorkflowRunConflictError)):
        await service.start_run(
            project.id,
            workflow.id,
            ["media.generate.video"],
            scope_refs=scope_refs,
            owner_refs=owner_refs,
            idempotency_key=f"invalid-media-gate-{mutation}",
        )
    assert before == (
        len(uow.workflow_runs),
        len(uow.run_events),
        len(uow.provider_calls),
        len(uow.outbox_events),
        len(uow.temporal_starts),
    )


async def test_media_stages_require_explicit_review_and_ready_derivative() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("media-stage")
    await CatalogService(lambda: uow).bootstrap()
    _seed_current_route(uow, project.id)
    scenes = ScenesService(lambda: uow)
    service = RunsService(lambda: uow, scenes)
    workflow = await service.ensure_workflow(
        EnsureWorkflowCommand(project.id, scope_ids=(project.id,))
    )
    scope_refs, owner_refs, shot = await _media_gate_refs(uow, project)
    run = await service.start_run(
        project.id,
        workflow.id,
        [
            "media.generate.image",
            "media.review.image",
            "media.generate.video",
            "media.review.video",
            "media.inspect",
            "timeline.handoff",
        ],
        scope_refs=scope_refs,
        owner_refs=owner_refs,
        idempotency_key="media-stages",
    )
    by_key = {node.node_key: node for node in run.nodes}
    assert len(uow.temporal_starts) == 1
    assert next(iter(uow.temporal_starts.values())).node_run_id == by_key["media.generate.image"].id

    image_generate = by_key["media.generate.image"]
    image_review = by_key["media.review.image"]
    image_generate.transition("running")
    image_candidate = await _candidate(uow, shot, "image")
    uow.provider_calls["image-result"] = {
        "runId": run.id,
        "nodeRunId": image_generate.id,
        "correlationId": "image-result",
        "status": "succeeded",
        "assetVersionId": image_candidate["assetVersionId"],
    }
    await service.record_media_candidate(
        MediaCandidateCommand(
            run.id,
            image_generate.id,
            image_review.id,
            image_generate.revision,
            image_review.revision,
            "image-result",
            image_candidate,
        )
    )
    assert image_generate.status == "succeeded"
    assert image_review.status == "waiting_review"
    assert shot.current_image.candidate_id == "accepted-storyboard"
    await service.signal_review(
        ReviewSignalCommand(
            run.id,
            image_review.id,
            image_review.revision,
            "accept",
            "accept-image",
            ACTOR_UUID,
        )
    )
    assert shot.current_image.candidate_id == image_candidate["candidateId"]
    assert by_key["media.generate.video"].status == "running"

    video_generate = by_key["media.generate.video"]
    video_review = by_key["media.review.video"]
    video_candidate = await _candidate(uow, shot, "video")
    uow.provider_calls["video-result"] = {
        "runId": run.id,
        "nodeRunId": video_generate.id,
        "correlationId": "video-result",
        "status": "succeeded",
        "assetVersionId": video_candidate["assetVersionId"],
    }
    await service.record_media_candidate(
        MediaCandidateCommand(
            run.id,
            video_generate.id,
            video_review.id,
            video_generate.revision,
            video_review.revision,
            "video-result",
            video_candidate,
        )
    )
    await service.signal_review(
        ReviewSignalCommand(
            run.id,
            video_review.id,
            video_review.revision,
            "accept",
            "accept-video",
            ACTOR_UUID,
        )
    )
    inspect = by_key["media.inspect"]
    timeline = by_key["timeline.handoff"]
    assert shot.current_video.candidate_id == video_candidate["candidateId"]
    assert inspect.status == "running" and timeline.status == "pending"

    await service.complete_media_inspect(
        MediaInspectCommand(
            run.id,
            inspect.id,
            inspect.revision,
            shot.id,
            str(video_candidate["candidateId"]),
            "pending",
            "inspect-pending",
        )
    )
    assert shot.current_video.derivative_status == "pending"
    assert timeline.status == "pending"
    pending_revision = inspect.revision
    await service.complete_media_inspect(
        MediaInspectCommand(
            run.id,
            inspect.id,
            pending_revision,
            shot.id,
            str(video_candidate["candidateId"]),
            "failed",
            "inspect-failed",
        )
    )
    assert shot.current_video.derivative_status == "failed"
    assert inspect.status == "running" and inspect.failure["retryable"] is True
    assert timeline.status == "pending"
    await service.complete_media_inspect(
        MediaInspectCommand(
            run.id,
            inspect.id,
            inspect.revision,
            shot.id,
            str(video_candidate["candidateId"]),
            "ready",
            "inspect-ready",
        )
    )
    assert shot.current_video.derivative_status == "ready"
    assert inspect.status == "succeeded" and timeline.status == "running"


async def test_video_retake_creates_new_generation_and_review_operations() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("retake")
    await CatalogService(lambda: uow).bootstrap()
    _seed_current_route(uow, project.id)
    scenes = ScenesService(lambda: uow)
    service = RunsService(lambda: uow, scenes)
    workflow = await service.ensure_workflow(
        EnsureWorkflowCommand(project.id, scope_ids=(project.id,))
    )
    scope_refs, owner_refs, shot = await _media_gate_refs(uow, project)
    run = await service.start_run(
        project.id,
        workflow.id,
        ["media.generate.video", "media.review.video"],
        scope_refs=scope_refs,
        owner_refs=owner_refs,
        idempotency_key="video-retake",
    )
    generation, review = run.nodes
    generation.transition("running")
    candidate = await _candidate(uow, shot, "video")
    uow.provider_calls["retake-result"] = {
        "runId": run.id,
        "nodeRunId": generation.id,
        "correlationId": "retake-result",
        "status": "succeeded",
        "assetVersionId": candidate["assetVersionId"],
    }
    await service.record_media_candidate(
        MediaCandidateCommand(
            run.id,
            generation.id,
            review.id,
            generation.revision,
            review.revision,
            "retake-result",
            candidate,
        )
    )
    old_current = shot.current_video
    await service.signal_review(
        ReviewSignalCommand(
            run.id,
            review.id,
            review.revision,
            "retake",
            "retake-video",
            ACTOR_UUID,
        )
    )
    successors = run.nodes[2:]
    assert [node.node_key for node in successors] == [
        "media.generate.video",
        "media.review.video",
    ]
    assert successors[0].logical_operation != generation.logical_operation
    assert successors[1].logical_operation != review.logical_operation
    assert shot.current_video is old_current
    assert not uow.budget_gates


async def test_agnes_observation_normalizes_once_without_provider_event_copy() -> None:
    uow, service, project, workflow = await _service()
    scope_refs, owner_refs, _shot = await _media_gate_refs(uow, project)
    run = await service.start_run(
        project.id,
        workflow.id,
        ["media.generate.video"],
        scope_refs=scope_refs,
        owner_refs=owner_refs,
    )
    node = run.nodes[0]
    uow.provider_calls["provider-call"] = {
        "id": "provider-call",
        "runId": run.id,
        "nodeRunId": node.id,
        "correlationId": "agnes-submit-1",
    }
    before_provider = dict(uow.provider_calls)
    command = ProviderObservationCommand(
        run.id,
        node.id,
        "agnes-submit-1",
        "submit",
        {"status": "submitted", "providerCallId": "provider-call", "attempt": 1},
    )
    event = await service.append_provider_observation(command)
    retry = await service.append_provider_observation(command)
    assert retry.id == event.id and event.event_type == "provider.agnes.submit"
    assert uow.provider_calls == before_provider
    assert (
        len([item for item in uow.run_events[run.id] if item.event_type == "provider.agnes.submit"])
        == 1
    )
    with pytest.raises(ValidationDomainError, match="non-summary"):
        await service.append_provider_observation(
            ProviderObservationCommand(
                run.id,
                node.id,
                "agnes-result-raw",
                "result",
                {"rawPayload": "must-not-persist"},
            )
        )


def test_run_http_detail_sse_and_unsupported_mutation() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    asyncio.run(CatalogService(lambda: uow).bootstrap())
    client = TestClient(
        create_app(readiness_probe=lambda: True, projects_episodes_service=projects)
    )
    project = client.post("/v1/projects", json={"name": "P"}).json()
    route = _seed_current_route(uow, project["id"])
    workflow = client.post(
        f"/v1/projects/{project['id']}/workflow/default/ensure",
        json={"schemaVersion": "1.0.0"},
        headers={"X-Project-Scope": project["id"]},
    ).json()
    response = client.post(
        f"/v1/projects/{project['id']}/runs",
        json={
            "workflowVersionId": workflow["id"],
            "nodeKeys": ["text.generate"],
            "idempotencyKey": "http-run",
            "routeDecisionId": route.id,
            "expectedBindingRevision": 1,
            "schemaVersion": "1.0.0",
        },
        headers={"If-Match": "1", "X-Project-Scope": project["id"]},
    )
    assert response.status_code == 201
    run = response.json()
    assert client.get(f"/v1/runs/{run['id']}").status_code == 403
    assert client.get(f"/v1/runs/{run['id']}/events").status_code == 403
    detail = client.get(f"/v1/runs/{run['id']}", headers={"X-Project-Scope": project["id"]})
    assert detail.status_code == 200
    assert detail.json()["selectionSnapshot"]["adapterIdentity"] == "local_workspace"
    stream = client.get(
        f"/v1/runs/{run['id']}/events",
        headers={"Last-Event-ID": "0", "X-Project-Scope": project["id"]},
    )
    assert stream.status_code == 200
    assert "event: run.started" in stream.text and "id: 1" in stream.text
    before = (len(uow.workflow_runs), len(uow.outbox_events))
    unsupported = client.post(
        f"/v1/projects/{project['id']}/workflow/mutations",
        json={"operation": "publishDraft", "schemaVersion": "1.0.0"},
        headers={"X-Project-Scope": project["id"]},
    )
    assert unsupported.status_code == 422
    assert (len(uow.workflow_runs), len(uow.outbox_events)) == before


def test_run_http_schema_cas_scope_snapshot_and_safe_detail_contracts() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    asyncio.run(CatalogService(lambda: uow).bootstrap())
    client = TestClient(
        create_app(readiness_probe=lambda: True, projects_episodes_service=projects)
    )
    project = client.post("/v1/projects", json={"name": "HTTP contracts"}).json()
    route = _seed_current_route(uow, project["id"])
    other = client.post("/v1/projects", json={"name": "Foreign"}).json()
    unconfigured = client.post("/v1/projects", json={"name": "Unconfigured"}).json()
    unconfigured_before = (len(uow.workflow_by_project), len(uow.outbox_events))
    missing_source = client.get(
        f"/v1/projects/{unconfigured['id']}/workflow/default",
        headers={"X-Project-Scope": unconfigured["id"]},
    )
    assert missing_source.status_code == 422
    assert missing_source.json()["detail"]["type"] == "workflow_unconfigured"
    assert unconfigured_before == (len(uow.workflow_by_project), len(uow.outbox_events))
    project_path = f"/v1/projects/{project['id']}"
    workflow = client.post(
        f"{project_path}/workflow/default/ensure",
        json={"schemaVersion": "1.0.0"},
        headers={"X-Project-Scope": project["id"]},
    ).json()
    read_counts = (len(uow.workflow_by_project), len(uow.outbox_events))
    read_source = client.get(
        f"{project_path}/workflow/default",
        headers={"X-Project-Scope": project["id"]},
    )
    assert read_source.status_code == 200
    assert read_counts == (len(uow.workflow_by_project), len(uow.outbox_events))

    base_body = {
        "workflowVersionId": workflow["id"],
        "nodeKeys": ["text.generate"],
        "idempotencyKey": "strict-http-run",
        "routeDecisionId": route.id,
        "expectedBindingRevision": 1,
    }
    before = (len(uow.workflow_runs), len(uow.run_events), len(uow.temporal_starts))
    scoped_match = {"If-Match": "1", "X-Project-Scope": project["id"]}
    missing_schema = client.post(f"{project_path}/runs", json=base_body, headers=scoped_match)
    dual_schema = client.post(
        f"{project_path}/runs",
        json={**base_body, "schemaVersion": "1.0.0", "schema_version": "1.0.0"},
        headers=scoped_match,
    )
    wrong_match = client.post(
        f"{project_path}/runs",
        json={**base_body, "schemaVersion": "1.0.0"},
        headers={"If-Match": "2", "X-Project-Scope": project["id"]},
    )
    unsafe_ref = client.post(
        f"{project_path}/runs",
        json={
            **base_body,
            "schemaVersion": "1.0.0",
            "idempotencyKey": "unsafe-http-run",
            "scopeRefs": [
                {
                    "projectId": project["id"],
                    "revision": 1,
                    "objectKey": "private/provider/output.bin",
                }
            ],
        },
        headers={"If-Match": "1", "X-Project-Scope": project["id"]},
    )
    assert [
        missing_schema.status_code,
        dual_schema.status_code,
        wrong_match.status_code,
        unsafe_ref.status_code,
    ] == [
        422,
        422,
        409,
        422,
    ]
    assert before == (len(uow.workflow_runs), len(uow.run_events), len(uow.temporal_starts))

    created = client.post(
        f"{project_path}/runs",
        json={**base_body, "schemaVersion": "1.0.0"},
        headers={"If-Match": "1", "X-Project-Scope": project["id"]},
    )
    assert created.status_code == 201
    run = created.json()
    node = uow.workflow_runs[run["id"]].nodes[0]
    node.output_evidence = {"prompt": "private text", "contentHash": "a" * 64}
    detail = client.get(f"/v1/runs/{run['id']}", headers={"X-Project-Scope": project["id"]})
    assert detail.status_code == 200
    output_summary = detail.json()["nodes"][0]["outputSummary"]
    assert output_summary == {"contentHash": "a" * 64}
    assert "private text" not in detail.text

    foreign = client.get(f"/v1/runs/{run['id']}", headers={"X-Project-Scope": other["id"]})
    invalid_cursor = client.get(
        f"/v1/runs/{run['id']}/events",
        headers={
            "Last-Event-ID": "not-a-number",
            "X-Project-Scope": project["id"],
        },
    )
    assert foreign.status_code == 403 and invalid_cursor.status_code == 422

    snapshot_id = run["inputSnapshot"]["snapshotId"]
    snapshots = client.get(
        f"{project_path}/run-input-snapshots",
        headers={"X-Project-Scope": project["id"]},
    )
    snapshot = client.get(
        f"{project_path}/run-input-snapshots/{snapshot_id}",
        headers={"X-Project-Scope": project["id"]},
    )
    assert snapshots.status_code == 200 and snapshots.json()[0]["id"] == snapshot_id
    assert snapshot.status_code == 200 and snapshot.json()["runId"] == run["id"]
    rerun = client.post(
        f"{project_path}/run-input-snapshots/{snapshot_id}/rerun",
        json={"expectedSnapshotRevision": 1, "schemaVersion": "1.0.0"},
        headers={"If-Match": "1", "X-Project-Scope": project["id"]},
    )
    assert rerun.status_code == 201
    assert rerun.json()["rerunOfRunId"] == run["id"]
