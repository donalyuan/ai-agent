from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest
from fastapi.testclient import TestClient
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
from video_agent_api.adapters.sqlalchemy_models import Base
from video_agent_api.app import create_app
from video_agent_api.application.assets import (
    AppendAssetVersionCommand,
    AssetsService,
    CreateAssetCommand,
)
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.scenes import (
    AttachContinuityCommand,
    CreateSceneCommand,
    CreateShotCommand,
    ReorderScenesCommand,
    ReviewMediaCommand,
    ScenesService,
)
from video_agent_api.domain.asset_bible import ContinuityAssignment, ResolvedContinuitySnapshot
from video_agent_api.domain.assets import StorageObject
from video_agent_api.domain.errors import RevisionConflictError, ValidationDomainError
from video_agent_api.domain.scenes import SceneShotBatchHandoff


@pytest.fixture
def services() -> tuple[ProjectsEpisodesService, ScenesService]:
    uow = InMemoryUnitOfWork()
    return ProjectsEpisodesService(lambda: uow), ScenesService(lambda: uow)


async def test_scene_shot_scope_and_reorder(services):
    projects, scenes = services
    project = await projects.create_project("P")
    episode = await projects.create_episode((project.id, "E", 1))
    first = await scenes.create_scene(CreateSceneCommand(project.id, episode.id))
    second = await scenes.create_scene(CreateSceneCommand(project.id, episode.id))
    shot = await scenes.create_shot(CreateShotCommand(project.id, episode.id, first.id))
    assert shot.display_number == 1
    await scenes.reorder_scenes(
        ReorderScenesCommand(project.id, episode.id, [second.id, first.id], 1)
    )
    view = await scenes.list_episode(project.id, episode.id)
    assert [item["id"] for item in view] == [second.id, first.id]


async def test_reorder_rejects_incomplete_and_review_approve(services):
    projects, scenes = services
    project = await projects.create_project("P")
    episode = await projects.create_episode((project.id, "E", 1))
    scene = await scenes.create_scene(CreateSceneCommand(project.id, episode.id))
    with pytest.raises(ValidationDomainError):
        await scenes.reorder_scenes(ReorderScenesCommand(project.id, episode.id, [], 1))
    with pytest.raises(ValidationDomainError):
        await scenes.review_video(
            (await scenes.create_shot(CreateShotCommand(project.id, episode.id, scene.id))).id,
            "approve",
        )


def _payload_hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    ).hexdigest()


async def test_accepted_text_handoff_is_atomic_idempotent_and_audited() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    service = ScenesService(lambda: uow)
    project = await projects.create_project("P")
    episode = await projects.create_episode((project.id, "E", 1))
    payload = [
        {
            "candidateId": "scene-candidate",
            "sourceHash": "a" * 64,
            "expectedRevision": 0,
            "payload": {"summary": "arrival"},
            "shots": [
                {
                    "candidateId": "shot-candidate",
                    "sourceHash": "b" * 64,
                    "expectedRevision": 0,
                    "payload": {"durationFrames": 90},
                }
            ],
        }
    ]
    handoff = SceneShotBatchHandoff(
        "handoff",
        project.id,
        episode.id,
        1,
        "correlation",
        _payload_hash(payload),
        True,
        tuple(payload),
    )
    ack = await service.apply_text_handoff(handoff)
    retry = await service.apply_text_handoff(handoff)
    assert retry.id == ack.id
    assert len(uow.scenes) == 1 and len(uow.shots) == 1
    assert uow.audit_events[-1]["type"] == "scene-shot.handoff.applied"
    before = (len(uow.scenes), len(uow.shots), len(uow.audit_events))
    with pytest.raises(ValidationDomainError, match="payload hash"):
        SceneShotBatchHandoff(
            "invalid", project.id, episode.id, 1, "correlation", "c" * 64, True, tuple(payload)
        )
    assert (len(uow.scenes), len(uow.shots), len(uow.audit_events)) == before


async def test_media_accept_uses_exact_asset_and_shot_spec_cas() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    scenes = ScenesService(lambda: uow)
    assets = AssetsService(lambda: uow)
    project = await projects.create_project("P")
    episode = await projects.create_episode((project.id, "E", 1))
    scene = await scenes.create_scene(CreateSceneCommand(project.id, episode.id))
    shot = await scenes.create_shot(CreateShotCommand(project.id, episode.id, scene.id))
    spec = await scenes.append_spec(
        project.id, episode.id, scene.id, {"durationFrames": 90}, shot.id
    )
    asset = await assets.create_asset(CreateAssetCommand(project.id, "take", "video"))
    version = await assets.append_version(
        AppendAssetVersionCommand(
            asset.id,
            StorageObject(
                "local",
                "workspace",
                "projects/p/take.mp4",
                "video/mp4",
                4,
                "d" * 64,
                media={"duration_ms": 3000, "width": 1080, "height": 1920},
            ),
            "e" * 64,
        )
    )
    candidate: dict[str, object] = {
        "candidateId": "candidate",
        "candidateRevision": 1,
        "projectId": project.id,
        "episodeId": episode.id,
        "targetId": shot.id,
        "assetVersionId": version.id,
        "assetVersionRevision": version.revision,
        "assetVersionHash": version.content_hash,
        "provenance": "media_review",
        "mediaKind": "video",
        "shotSpecRevision": spec.revision,
        "shotSpecHash": spec.content_hash,
        "durationMs": 3000,
        "aspectRatio": "9:16",
        "derivativeStatus": "pending",
    }
    with pytest.raises(ValidationDomainError, match="continuity snapshot"):
        await scenes.review_media(
            ReviewMediaCommand(project.id, episode.id, shot.id, "accept", candidate, shot.revision)
        )
    assignment = ContinuityAssignment(
        project.id,
        "shot",
        shot.id,
        "entry",
        "version",
        1,
        "a" * 64,
    )
    snapshot = ResolvedContinuitySnapshot(
        project.id,
        shot.id,
        (assignment,),
        (("entry", 1),),
    )
    uow.asset_bible_snapshots[snapshot.id] = snapshot
    shot = await scenes.attach_continuity(
        AttachContinuityCommand(
            project.id,
            episode.id,
            shot.id,
            snapshot.id,
            snapshot.revision,
            snapshot.content_hash,
            shot.revision,
        )
    )
    accepted = await scenes.review_media(
        ReviewMediaCommand(project.id, episode.id, shot.id, "accept", candidate, shot.revision)
    )
    assert accepted.current_video is not None
    assert accepted.current_video.timeline_ready is False
    version_count = len(uow.asset_versions._state.asset_versions)
    current = accepted.current_video
    conflicting_retry = dict(candidate, derivativeStatus="ready")
    with pytest.raises(ValidationDomainError, match="idempotency conflict"):
        await scenes.review_media(
            ReviewMediaCommand(
                project.id,
                episode.id,
                shot.id,
                "accept",
                conflicting_retry,
                accepted.revision,
            )
        )
    rejected = await scenes.review_media(
        ReviewMediaCommand(project.id, episode.id, shot.id, "reject", candidate, accepted.revision)
    )
    assert rejected.current_video == current
    retaken = await scenes.review_media(
        ReviewMediaCommand(project.id, episode.id, shot.id, "retake", candidate, accepted.revision)
    )
    assert retaken.current_video == current
    assert len(uow.asset_versions._state.asset_versions) == version_count
    stale = dict(candidate, candidateId="stale", assetVersionHash="f" * 64)
    with pytest.raises(ValidationDomainError, match="stale or foreign"):
        await scenes.review_media(
            ReviewMediaCommand(project.id, episode.id, shot.id, "accept", stale, shot.revision)
        )
    assert uow.shots[shot.id].current_video == current


async def test_image_candidate_requires_exact_accept_provenance_and_asset_version() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    scenes = ScenesService(lambda: uow)
    assets = AssetsService(lambda: uow)
    project = await projects.create_project("P")
    episode = await projects.create_episode((project.id, "E", 1))
    scene = await scenes.create_scene(CreateSceneCommand(project.id, episode.id))
    shot = await scenes.create_shot(CreateShotCommand(project.id, episode.id, scene.id))
    assignment = ContinuityAssignment(project.id, "shot", shot.id, "entry", "version", 1, "a" * 64)
    snapshot = ResolvedContinuitySnapshot(project.id, shot.id, (assignment,), (("entry", 1),))
    uow.asset_bible_snapshots[snapshot.id] = snapshot
    shot = await scenes.attach_continuity(
        AttachContinuityCommand(
            project.id,
            episode.id,
            shot.id,
            snapshot.id,
            snapshot.revision,
            snapshot.content_hash,
            shot.revision,
        )
    )
    asset = await assets.create_asset(CreateAssetCommand(project.id, "frame", "image"))
    version = await assets.append_version(
        AppendAssetVersionCommand(
            asset.id,
            StorageObject(
                "local",
                "workspace",
                "projects/p/frame.png",
                "image/png",
                4,
                "d" * 64,
            ),
            "e" * 64,
        )
    )
    candidate: dict[str, object] = {
        "candidateId": "image-candidate",
        "candidateRevision": 1,
        "projectId": project.id,
        "episodeId": episode.id,
        "targetId": shot.id,
        "assetVersionId": version.id,
        "assetVersionRevision": version.revision,
        "assetVersionHash": version.content_hash,
        "provenance": "media_review",
        "mediaKind": "image",
    }
    accepted = await scenes.review_media(
        ReviewMediaCommand(project.id, episode.id, shot.id, "accept", candidate, shot.revision)
    )
    assert accepted.current_image is not None
    projection = (await scenes.list_episode(project.id, episode.id))[0]["shots"][0]
    assert projection["currentImage"] == {
        "candidateId": "image-candidate",
        "candidateRevision": 1,
        "projectId": project.id,
        "episodeId": episode.id,
        "targetId": shot.id,
        "assetVersionId": version.id,
        "assetVersionRevision": version.revision,
        "assetVersionHash": version.content_hash,
        "provenance": "media_review",
        "mediaKind": "image",
        "shotSpecRevision": None,
        "shotSpecHash": None,
        "durationMs": None,
        "aspectRatio": None,
        "derivativeStatus": "pending",
        "status": "accepted_current",
        "timelineReady": False,
    }
    version_count = len(uow.asset_versions._state.asset_versions)
    cancelled_late = dict(candidate, candidateId="late", provenance="cancelled_late")
    with pytest.raises(ValidationDomainError, match="provenance"):
        await scenes.review_media(
            ReviewMediaCommand(
                project.id,
                episode.id,
                shot.id,
                "accept",
                cancelled_late,
                accepted.revision,
            )
        )
    assert len(uow.asset_versions._state.asset_versions) == version_count


def test_scene_http_uses_camel_case_and_two_consistent_views() -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    app = create_app(
        readiness_probe=lambda: True,
        projects_episodes_service=projects,
        assets_service=AssetsService(lambda: uow),
    )
    client = TestClient(app)
    project = client.post("/v1/projects", json={"name": "P"}).json()
    scope_header = {"X-Project-Scope": project["id"]}
    episode = client.post(
        f"/v1/projects/{project['id']}/episodes", json={"title": "E", "number": 1}
    ).json()
    created = client.post(
        f"/v1/projects/{project['id']}/episodes/{episode['id']}/scenes",
        headers=scope_header,
        json={"schemaVersion": "1.0.0"},
    )
    assert created.status_code == 201
    assert created.json()["schemaVersion"] == "1.0.0"
    assert created.json()["number"] == 1
    assert created.json()["title"] == "Scene 1"
    assert "schema_version" not in created.json()
    scene = created.json()
    shot = client.post(
        f"/v1/projects/{project['id']}/episodes/{episode['id']}/scenes/{scene['id']}/shots",
        headers=scope_header,
        json={"schemaVersion": "1.0.0"},
    )
    assert shot.status_code == 201 and shot.json()["projectId"] == project["id"]
    storyboard = client.get(
        f"/v1/projects/{project['id']}/episodes/{episode['id']}/storyboard",
        headers=scope_header,
    ).json()
    scope = client.get(
        f"/v1/projects/{project['id']}/episodes/{episode['id']}/workflow-scope",
        headers=scope_header,
    ).json()
    assert scope["scenes"] == storyboard
    assert uow.workflow_runs == {}
    before = (len(uow.scenes), len(uow.shots), len(uow.audit_events))
    conflict = client.post(
        f"/v1/projects/{project['id']}/episodes/{episode['id']}/scenes",
        headers=scope_header,
        json={"schemaVersion": "1.0.0", "schema_version": "2.0.0"},
    )
    assert conflict.status_code == 422
    assert (len(uow.scenes), len(uow.shots), len(uow.audit_events)) == before
    unsupported = client.post(
        f"/v1/projects/{project['id']}/episodes/{episode['id']}/storyboard/structure",
        headers=scope_header,
        json={"operation": "splitScene"},
    )
    assert unsupported.status_code == 422
    assert (len(uow.scenes), len(uow.shots), len(uow.audit_events)) == before


async def test_sqlalchemy_scene_rows_reload_and_reject_stale_write(tmp_path: Path) -> None:
    engine = create_async_engine(f"sqlite+aiosqlite:///{tmp_path / 'scenes.db'}")
    async with engine.begin() as connection:
        await connection.run_sync(Base.metadata.create_all)
    factory = make_sqlalchemy_uow_factory(async_sessionmaker(engine, expire_on_commit=False))
    projects = ProjectsEpisodesService(factory)
    scenes = ScenesService(factory)
    project = await projects.create_project("P")
    episode = await projects.create_episode((project.id, "E", 1))
    scene = await scenes.create_scene(CreateSceneCommand(project.id, episode.id))
    shot = await scenes.create_shot(CreateShotCommand(project.id, episode.id, scene.id))
    spec = await scenes.append_spec(
        project.id,
        episode.id,
        scene.id,
        {"durationFrames": 90},
        shot.id,
    )

    reloaded = await scenes.list_episode(project.id, episode.id)
    assert reloaded[0]["id"] == scene.id
    assert reloaded[0]["shots"][0]["specRef"] == {
        "ownerId": spec.id,
        "revision": spec.revision,
        "contentHash": spec.content_hash,
        "purpose": "shotSpec",
    }

    first = factory()
    second = factory()
    async with first as first_uow, second as second_uow:
        first_scene = first_uow.scenes[scene.id]
        second_scene = second_uow.scenes[scene.id]
        first_scene.title = "First"
        first_scene.revision += 1
        await first_uow.commit()
        second_scene.title = "Stale"
        second_scene.revision += 1
        with pytest.raises(RevisionConflictError):
            await second_uow.commit()
    assert (await scenes.list_episode(project.id, episode.id))[0]["title"] == "First"
    await engine.dispose()
