from __future__ import annotations

import pytest
from fastapi.testclient import TestClient

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.app import create_app
from video_agent_api.application.assets import AssetsService
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.domain.agent_edit import (
    AssetEditCandidate,
    AssetEditPlan,
    AssetEditSelection,
    AssetEditSession,
    AssetVersionRef,
    ContinuitySnapshotRef,
)
from video_agent_api.domain.asset_bible import (
    ContinuityAssignment,
    ContinuityRevisionTask,
    ResolvedContinuitySnapshot,
)
from video_agent_api.domain.assets import Asset, AssetVersion, StorageObject
from video_agent_api.domain.conversation import AgentConversation
from video_agent_api.domain.scenes import AcceptedMediaEligibility, Shot


def _seed() -> tuple[TestClient, InMemoryUnitOfWork, dict[str, object]]:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    app = create_app(
        readiness_probe=lambda: True,
        projects_episodes_service=projects,
        assets_service=AssetsService(lambda: uow),
    )
    project = "project-review"
    episode = "episode-review"
    shot = "shot-review"
    asset = Asset(project, "image", "Base")
    version = AssetVersion(
        asset.id,
        project,
        1,
        StorageObject("local", "workspace", "review/base.png", "image/png", 4, "a" * 64),
    )
    uow.state.assets[asset.id] = asset
    uow.state.asset_versions[version.id] = version
    assignment = ContinuityAssignment(project, "shot", shot, "entry", "entry-version", 1, "b" * 64)
    snapshot = ResolvedContinuitySnapshot(
        project,
        shot,
        (assignment,),
        ((shot, 1),),
        target_revision=1,
    )
    uow.state.asset_bible_snapshots[snapshot.id] = snapshot
    ref = AssetVersionRef(
        version.id,
        version.revision,
        version.content_hash or "",
        "image",
        project,
        "image/png",
    )
    continuity = ContinuitySnapshotRef(snapshot.id, snapshot.revision, snapshot.content_hash, shot)
    session = AssetEditSession(
        project,
        episode,
        AssetEditSelection(project, episode, shot, ref),
        continuity,
    )
    conversation = AgentConversation(project, episode, id=session.id)
    turn = conversation.append_user_message("c" * 64, "correlation-user")
    conversation.append_agent_reply(turn.id, "d" * 64, "correlation-agent", 1)
    plan = AssetEditPlan(
        project,
        episode,
        ref,
        (),
        "提升人物轮廓清晰度",
        turn.id,
        continuity=continuity,
        target_id=shot,
    )
    candidate = AssetEditCandidate(
        plan.id,
        ref,
        provenance={
            "providerStatus": "succeeded",
            "derivativeStatus": "ready",
            "adapterIdentity": "local_workspace",
        },
        project_id=project,
        episode_id=episode,
        target_id=shot,
    )
    uow.state.asset_edit_sessions[session.id] = session
    uow.state.conversations[session.id] = conversation
    uow.state.asset_edit_plans[plan.id] = plan
    uow.state.asset_edit_candidates[candidate.id] = candidate
    client = TestClient(app)
    client.headers["X-Project-Scope"] = project
    return (
        client,
        uow,
        {
            "project": project,
            "episode": episode,
            "shot": shot,
            "session": session,
            "plan": plan,
            "candidate": candidate,
            "snapshot": snapshot,
        },
    )


def test_review_projection_restores_owner_facts_without_read_side_effects() -> None:
    client, uow, facts = _seed()
    before = (
        len(uow.state.audit_events),
        len(uow.state.outbox_events),
        len(uow.state.provider_calls),
        len(uow.state.asset_edit_plans),
    )
    project = facts["project"]
    session = facts["session"]
    response = client.get(f"/v1/projects/{project}/asset-edit-sessions/{session.id}")
    assert response.status_code == 200, response.text
    payload = response.json()
    assert payload["schemaVersion"] == "1.0.0"
    assert payload["projectId"] == project
    assert payload["episodeId"] == facts["episode"]
    assert payload["selection"]["primary"]["contentHash"] == "a" * 64
    assert payload["conversation"]["messages"][0]["role"] == "user"
    assert payload["conversation"]["messages"][1]["role"] == "agent"
    assert payload["conversation"]["turns"][0]["status"] == "complete"
    assert payload["plans"][0]["cost"]["status"] == "unknown"
    assert payload["plans"][0]["cost"]["source"] == "owner_unavailable"
    assert payload["plans"][0]["candidates"][0]["provenance"]["adapterIdentity"] == (
        "local_workspace"
    )
    assert payload["continuity"]["status"] == "accepted_current"
    assert payload["continuity"]["tasks"] == []
    assert before == (
        len(uow.state.audit_events),
        len(uow.state.outbox_events),
        len(uow.state.provider_calls),
        len(uow.state.asset_edit_plans),
    )


def test_review_projection_derives_accepted_current_from_scenes_owner() -> None:
    client, uow, facts = _seed()
    candidate = facts["candidate"]
    version = candidate.asset_version
    shot = Shot("scene-review", facts["project"], facts["episode"], 1, id=facts["shot"])
    shot.current_image = AcceptedMediaEligibility(
        candidate.id,
        candidate.revision,
        facts["project"],
        facts["episode"],
        facts["shot"],
        version.id,
        version.revision,
        version.content_hash,
        "asset_edit",
        "image",
        derivative_status="ready",
    )
    uow.state.shots[shot.id] = shot
    candidate.decide("accept", candidate.revision)

    payload = client.get(
        f"/v1/projects/{facts['project']}/asset-edit-sessions/{facts['session'].id}"
    ).json()
    provenance = payload["plans"][0]["candidates"][0]["provenance"]
    assert provenance["acceptedCurrent"] is True
    assert provenance["derivativeStatus"] == "ready"

    shot.current_image = None
    payload = client.get(
        f"/v1/projects/{facts['project']}/asset-edit-sessions/{facts['session'].id}"
    ).json()
    assert payload["plans"][0]["candidates"][0]["provenance"]["acceptedCurrent"] is False


def test_review_projection_lists_by_episode_and_rejects_foreign_scope() -> None:
    client, _, facts = _seed()
    project = facts["project"]
    episode = facts["episode"]
    session = facts["session"]
    listed = client.get(
        f"/v1/projects/{project}/asset-edit-sessions", params={"episodeId": episode}
    )
    assert listed.status_code == 200
    assert [item["id"] for item in listed.json()["items"]] == [session.id]
    foreign = client.get(
        f"/v1/projects/{project}/asset-edit-sessions/{session.id}",
        headers={"X-Project-Scope": "foreign"},
    )
    assert foreign.status_code == 403


def test_review_projection_exposes_pending_continuity_task_and_project_scoped_plan() -> None:
    client, uow, facts = _seed()
    task = ContinuityRevisionTask(
        facts["project"],
        facts["shot"],
        "entry",
        snapshot_id=facts["snapshot"].id,
        snapshot_hash=facts["snapshot"].content_hash,
    )
    uow.state.asset_bible_tasks[task.id] = task
    project = facts["project"]
    session = facts["session"]
    plan = facts["plan"]
    payload = client.get(f"/v1/projects/{project}/asset-edit-sessions/{session.id}").json()
    assert payload["continuity"]["status"] == "continuity_stale"
    assert payload["continuity"]["tasks"][0]["status"] == "pending"

    plan_response = client.get(f"/v1/projects/{project}/asset-edit-plans/{plan.id}")
    assert plan_response.status_code == 200
    assert plan_response.json()["id"] == plan.id
    assert (
        client.get(
            f"/v1/projects/foreign/asset-edit-plans/{plan.id}",
            headers={"X-Project-Scope": "foreign"},
        ).status_code
        == 404
    )


@pytest.mark.parametrize("episode_id", ["", "foreign-episode"])
def test_review_projection_episode_filter_never_falls_back(episode_id: str) -> None:
    client, _, facts = _seed()
    response = client.get(
        f"/v1/projects/{facts['project']}/asset-edit-sessions",
        params={"episodeId": episode_id},
    )
    assert response.status_code == 200
    assert response.json()["items"] == []


def test_completed_turn_creates_only_a_frozen_pending_review_plan() -> None:
    client, uow, facts = _seed()
    session = facts["session"]
    conversation = uow.state.conversations[session.id]
    turn = conversation.turns[0]
    primary = session.selection.primary
    before = (
        len(uow.state.asset_edit_plans),
        len(uow.state.asset_edit_candidates),
        len(uow.state.asset_versions),
        len(uow.state.outbox_events),
    )
    response = client.post(
        f"/v1/asset-edit-sessions/{session.id}/turns/{turn.id}/asset-edit-plans",
        json={
            "schemaVersion": "1.0.0",
            "sessionId": session.id,
            "conversationId": session.id,
            "turnId": turn.id,
            "episodeId": session.episode_id,
            "targetId": session.selection.target_id,
            "kind": "image",
            "base": {
                "id": primary.id,
                "revision": primary.revision,
                "contentHash": primary.content_hash,
                "kind": primary.kind,
                "projectId": primary.project_id,
                "mimeType": primary.mime_type,
            },
            "references": [],
            "instruction": "提高轮廓清晰度",
            "runId": "run-review",
            "nodeRunId": "node-review",
            "logicalOperation": "review:turn:plan-2",
            "correlationId": "correlation-plan",
        },
    )
    assert response.status_code == 201, response.text
    plan = uow.state.asset_edit_plans[response.json()["id"]]
    assert plan.status == "pending_review"
    assert (plan.session_id, plan.run_id, plan.node_run_id) == (
        session.id,
        "run-review",
        "node-review",
    )
    assert plan.logical_operation == "review:turn:plan-2"
    assert (
        client.get(
            f"/v1/asset-edit-plans/{plan.id}/candidates",
            headers={"X-Project-Scope": "foreign"},
        ).status_code
        == 403
    )
    assert before[0] + 1 == len(uow.state.asset_edit_plans)
    assert before[1:] == (
        len(uow.state.asset_edit_candidates),
        len(uow.state.asset_versions),
        len(uow.state.outbox_events),
    )


def test_pending_or_foreign_turn_plan_request_has_zero_mutation() -> None:
    client, uow, facts = _seed()
    session = facts["session"]
    conversation = uow.state.conversations[session.id]
    pending = conversation.append_user_message("e" * 64, "correlation-pending")
    primary = session.selection.primary
    before = (
        len(uow.state.asset_edit_plans),
        len(uow.state.asset_edit_candidates),
        len(uow.state.asset_versions),
        len(uow.state.outbox_events),
    )
    payload = {
        "schemaVersion": "1.0.0",
        "sessionId": session.id,
        "conversationId": session.id,
        "turnId": pending.id,
        "episodeId": session.episode_id,
        "targetId": session.selection.target_id,
        "kind": "video",
        "base": {
            "id": primary.id,
            "revision": primary.revision,
            "contentHash": primary.content_hash,
            "kind": primary.kind,
            "projectId": primary.project_id,
            "mimeType": primary.mime_type,
        },
        "references": [],
        "instruction": "重拍",
        "runId": "run-review",
        "nodeRunId": "node-review",
        "logicalOperation": "review:pending",
        "correlationId": "correlation-plan",
    }
    response = client.post(
        f"/v1/asset-edit-sessions/{session.id}/turns/{pending.id}/asset-edit-plans",
        json=payload,
    )
    assert response.status_code == 422
    assert before == (
        len(uow.state.asset_edit_plans),
        len(uow.state.asset_edit_candidates),
        len(uow.state.asset_versions),
        len(uow.state.outbox_events),
    )
    foreign = client.post(
        f"/v1/asset-edit-sessions/{session.id}/turns/{pending.id}/asset-edit-plans",
        json={**payload, "conversationId": "foreign"},
    )
    assert foreign.status_code == 422
    assert before == (
        len(uow.state.asset_edit_plans),
        len(uow.state.asset_edit_candidates),
        len(uow.state.asset_versions),
        len(uow.state.outbox_events),
    )
