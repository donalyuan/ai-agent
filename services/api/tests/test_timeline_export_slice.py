from __future__ import annotations

import asyncio
from copy import deepcopy
from dataclasses import replace
from datetime import UTC, datetime, timedelta
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

from video_agent_api.adapters.ffmpeg import (
    MockFfmpegRenderAdapter,
    SubprocessFfmpegRenderAdapter,
)
from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.app import create_app
from video_agent_api.application.assets import (
    AppendAssetVersionCommand,
    AssetsService,
    CreateAssetCommand,
)
from video_agent_api.application.exports import ExportService
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.scenes import CreateSceneCommand, CreateShotCommand, ScenesService
from video_agent_api.application.timeline import TimelineService, timeline_cut_projection
from video_agent_api.domain.assets import AssetVersion, StorageObject
from video_agent_api.domain.catalog import default_skill_revisions
from video_agent_api.domain.errors import RevisionConflictError, ValidationDomainError
from video_agent_api.domain.exports import ExportArtifact, ExportDiagnosticTarget, ExportJob
from video_agent_api.domain.media import MediaDerivative, source_fingerprint
from video_agent_api.domain.provider_ops import ProviderCall
from video_agent_api.domain.runs import WorkflowRun
from video_agent_api.domain.scenes import AcceptedMediaEligibility
from video_agent_api.domain.timeline import (
    AssetSelection,
    Ducking,
    SoundCue,
    TimelineCut,
    TimelineVersion,
)
from video_agent_api.domain.video_generation import VideoOperation, VideoTakeCandidate
from video_agent_api.ports.contracts import StorageAuthorizationError
from video_agent_api.ports.storage import (
    LocalOpaqueReadGrantIssuer,
    LocalWorkspaceAdapter,
)


def _clip(
    *,
    clip_id: str = "clip-1",
    start: int = 0,
    in_frame: int = 0,
    out_frame: int = 60,
) -> dict[str, object]:
    return {
        "id": clip_id,
        "projectId": "project",
        "episodeId": "episode",
        "shotId": "shot",
        "assetVersionId": "version-1",
        "assetVersionRevision": 0,
        "assetVersionHash": "a" * 64,
        "derivativeFingerprint": "b" * 64,
        "derivativeStatus": "ready",
        "acceptedCurrent": True,
        "sourceFrames": 120,
        "inFrame": in_frame,
        "outFrame": out_frame,
        "durationFrames": out_frame - in_frame,
        "timelineStart": start,
        "transform": {"position": {"x": 0, "y": 0}, "scale": 1.0, "opacity": 1.0},
        "transition": {"type": "cut", "durationFrames": 0},
    }


async def _generated_export_asset(
    uow: InMemoryUnitOfWork, project_id: str, episode_id: str, label: str
) -> AssetVersion:
    assets = AssetsService(lambda: uow)
    asset = await assets.create_asset(CreateAssetCommand(project_id, label, "video"))
    asset.authorization_status = "verified"
    asset.license = "owned"
    asset.source_type = "provider_generated"
    version = await assets.append_version(
        AppendAssetVersionCommand(
            asset.id,
            StorageObject(
                "local",
                "workspace",
                f"projects/{project_id}/{label}.mp4",
                "video/mp4",
                6,
                "a" * 64,
            ),
            "b" * 64,
        )
    )
    if not uow.skills:
        uow.skills.extend(default_skill_revisions())
    selected_skills = [
        item for item in uow.skills if item.name in {"novel-writing", "drama-skills"}
    ]
    run = WorkflowRun(
        project_id,
        "workflow-version",
        selection_snapshot={
            "skillRevisionIds": [f"{item.name}@{item.version}" for item in selected_skills],
            "skillDigests": [item.digest for item in selected_skills],
        },
    )
    logical_operation = f"video.submit:{label}"
    operation = VideoOperation(
        project_id,
        run.id,
        logical_operation,
        "provider",
        "profile",
        "model",
        "capability",
        "source-version",
        0,
        "c" * 64,
        "shot-spec",
        1,
        "d" * 64,
        1.0,
        "9:16",
        status="succeeded",
    )
    provider_call = ProviderCall(
        project_id,
        run.id,
        None,
        logical_operation,
        "video.submit",
        operation.provider_id,
        operation.profile_id,
        operation.model_id,
        operation.capability_snapshot_id,
        "e" * 64,
        "succeeded",
        cost_status="known",
        cost_value="1.0",
        cost_currency="CNY",
        cost_source="provider-billing",
        native_usage={"seconds": 1},
    )
    candidate = VideoTakeCandidate(
        project_id,
        episode_id,
        "shot",
        run.id,
        logical_operation,
        "source-version",
        0,
        "c" * 64,
        "shot-spec",
        1,
        "d" * 64,
        1.0,
        "9:16",
        version.id,
        version.revision,
        str(version.content_hash),
        "provider-request",
        status="accepted",
    )
    uow.workflow_runs[run.id] = run
    uow.video_operations[(run.id, logical_operation)] = operation
    uow.provider_calls[provider_call.id] = provider_call
    uow.provider_call_keys[(run.id, logical_operation)] = provider_call.id
    uow.video_take_candidates[candidate.id] = candidate
    return version


def test_current_cut_edits_are_integer_only_atomic_and_revisioned() -> None:
    cut = TimelineCut("episode", "project")
    cut.edit(1, "add_clip", clip=_clip())
    assert cut.revision == 2 and cut.clips[0]["durationFrames"] == 60
    before = deepcopy(cut.clips)
    with pytest.raises(ValidationDomainError, match="integer frame"):
        cut.edit(2, "trim_clip", clip_id="clip-1", in_frame=0, out_frame=59.5)
    assert cut.revision == 2 and cut.clips == before
    cut.edit(2, "trim_clip", clip_id="clip-1", in_frame=10, out_frame=50)
    assert cut.revision == 3 and cut.clips[0]["durationFrames"] == 40
    with pytest.raises(RevisionConflictError):
        cut.edit(2, "delete_clip", clip_id="clip-1")
    assert cut.revision == 3 and len(cut.clips) == 1


def test_static_transform_split_reorder_delete_and_keyframes() -> None:
    cut = TimelineCut("episode", "project")
    cut.edit(1, "add_clip", clip=_clip())
    cut.edit(
        2,
        "set_clip_transform",
        clip_id="clip-1",
        transform={"position": {"x": 12, "y": -4}, "scale": 1.2, "opacity": 0.8},
    )
    with pytest.raises(ValidationDomainError, match="static transform"):
        cut.edit(
            3,
            "set_clip_transform",
            clip_id="clip-1",
            transform={"position": {"x": 0, "y": 0}, "scale": 1, "opacity": 1, "keyframes": []},
        )
    cut.edit(3, "split_clip", clip_id="clip-1", split_frame=30)
    assert len(cut.clips) == 2 and [item["durationFrames"] for item in cut.clips] == [30, 30]
    ids = [str(item["id"]) for item in reversed(cut.clips)]
    cut.edit(4, "reorder_clips", clip_ids=ids)
    assert [item["timelineStart"] for item in cut.clips] == [0, 30]
    cut.edit(5, "delete_clip", clip_id=ids[1])
    assert cut.revision == 6 and [item["id"] for item in cut.clips] == [ids[0]]


def test_crossfade_is_bounded_and_requires_exact_adjacency() -> None:
    cut = TimelineCut("episode", "project")
    cut.edit(1, "add_clip", clip=_clip(out_frame=30))
    second = _clip(clip_id="clip-2", start=25, out_frame=40)
    second["transition"] = {"type": "crossfade", "durationFrames": 5}
    cut.edit(2, "add_clip", clip=second)
    assert cut.clips[1]["timelineStart"] == 25
    invalid = _clip(clip_id="clip-3", start=60, out_frame=20)
    with pytest.raises(ValidationDomainError, match="adjacent"):
        cut.edit(3, "add_clip", clip=invalid)
    assert cut.revision == 3 and len(cut.clips) == 2


def test_replace_clip_source_is_exact_and_preserves_edit_attributes() -> None:
    cut = TimelineCut("episode", "project")
    cut.edit(1, "add_clip", clip=_clip())
    old = {
        "assetVersionId": "version-1",
        "assetVersionRevision": 0,
        "assetVersionHash": "a" * 64,
        "derivativeFingerprint": "b" * 64,
    }
    new = AssetSelection(
        "project",
        "episode",
        "version-2",
        1,
        "c" * 64,
        "d" * 64,
        available_frames=120,
        shot_id="shot",
    )
    before = deepcopy(cut.clips[0])
    with pytest.raises(ValidationDomainError, match="old source"):
        cut.edit(
            2,
            "replace_clip_source",
            clip_id="clip-1",
            old_source=dict(old, assetVersionId="stale"),
            new_source=new,
        )
    assert cut.revision == 2 and cut.clips[0] == before
    cut.edit(2, "replace_clip_source", clip_id="clip-1", old_source=old, new_source=new)
    assert cut.clips[0]["assetVersionId"] == "version-2"
    for field in ("id", "timelineStart", "durationFrames", "transform", "transition"):
        assert cut.clips[0][field] == before[field]


def test_sound_cue_ducking_caption_and_unsupported_automation_are_atomic() -> None:
    cut = TimelineCut("episode", "project")
    cue = SoundCue(
        "dialogue",
        "audio",
        0,
        90,
        asset_version_hash="a" * 64,
        continuity_refs=({"ownerType": "shot", "id": "shot", "revision": 1, "hash": "b" * 64},),
        fade_in_frames=5,
        fade_out_frames=5,
    )
    cut.edit(1, "add_sound_cue", cue=cue)
    ducking = Ducking(True, ((30, 60), (0, 40), (90, 100)), 9.0, 3, 6, ("music", "ambience"))
    cut.edit(2, "set_ducking", ducking=ducking)
    assert cut.ducking is not None and cut.ducking.dialogue_intervals == ((0, 60), (90, 100))
    cut.edit(3, "upsert_caption", caption={"text": "Hello", "startFrame": 0, "endFrame": 30})
    before = (deepcopy(cut.cues), cut.revision)
    with pytest.raises(ValidationDomainError, match="automation"):
        cut.edit(4, "set_sound_cue_mix", cue_id=cue.id, automation=[])
    assert (cut.cues, cut.revision) == before
    with pytest.raises(ValidationDomainError, match="style"):
        cut.edit(
            4,
            "upsert_caption",
            caption={"text": "x", "startFrame": 0, "endFrame": 1, "font": "x"},
        )


async def _timeline_owners() -> tuple[InMemoryUnitOfWork, TimelineService, dict[str, object]]:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    scenes = ScenesService(lambda: uow)
    assets = AssetsService(lambda: uow)
    project = await projects.create_project("P")
    episode = await projects.create_episode((project.id, "E", 1))
    scene = await scenes.create_scene(CreateSceneCommand(project.id, episode.id))
    shot = await scenes.create_shot(CreateShotCommand(project.id, episode.id, scene.id))
    asset = await assets.create_asset(CreateAssetCommand(project.id, "take", "video"))
    asset.authorization_status, asset.license = "verified", "owned"
    version = await assets.append_version(
        AppendAssetVersionCommand(
            asset.id,
            StorageObject("local", "workspace", "projects/p/take.mp4", "video/mp4", 4, "d" * 64),
            "a" * 64,
        )
    )
    shot.current_video = AcceptedMediaEligibility(
        "candidate",
        1,
        project.id,
        episode.id,
        shot.id,
        version.id,
        version.revision,
        version.content_hash or "",
        "media_review",
        "video",
        1,
        "c" * 64,
        4000,
        "9:16",
        "ready",
    )
    raw = _clip()
    raw.update(
        {
            "projectId": project.id,
            "episodeId": episode.id,
            "shotId": shot.id,
            "assetVersionId": version.id,
            "assetVersionRevision": version.revision,
            "assetVersionHash": version.content_hash,
        }
    )
    fingerprint = source_fingerprint(version.id, version.revision, version.content_hash or "")
    raw["derivativeFingerprint"] = fingerprint
    derivative = MediaDerivative(
        project.id,
        "inspection",
        version.id,
        version.revision,
        version.content_hash or "",
        fingerprint,
        "proxy",
        "ready",
        {"maxWidth": 540},
        "ffmpeg",
        "7.1",
        "derive:timeline-test",
        object_ref={
            "profileId": "local",
            "objectKey": f"projects/{project.id}/proxy.mp4",
            "operationKey": "derive:timeline-test",
        },
        checksum="e" * 64,
        size_bytes=4,
    )
    uow.media_derivatives[derivative.id] = derivative
    return (
        uow,
        TimelineService(lambda: uow),
        {
            "project": project,
            "episode": episode,
            "scene": scene,
            "shot": shot,
            "clip": raw,
            "videoVersion": version,
        },
    )


async def _audio_cue(
    uow: InMemoryUnitOfWork, project: object, episode: object, **changes: object
) -> dict[str, object]:
    assets = AssetsService(lambda: uow)
    asset = await assets.create_asset(CreateAssetCommand(project.id, "music", "audio"))
    asset.authorization_status, asset.license = "verified", "owned"
    version = await assets.append_version(
        AppendAssetVersionCommand(
            asset.id,
            StorageObject(
                "local",
                "workspace",
                f"projects/{project.id}/music.wav",
                "audio/wav",
                16,
                "9" * 64,
            ),
            "8" * 64,
        )
    )
    value: dict[str, object] = {
        "id": "00000000-0000-4000-8000-000000000601",
        "projectId": project.id,
        "episodeId": episode.id,
        "assetVersionId": version.id,
        "assetVersionRevision": version.revision,
        "assetVersionHash": version.content_hash,
        "explicitSelection": True,
        "storageVerified": True,
        "authorizationStatus": "authorized",
        "licenseStatus": "approved",
        "track": "music",
        "startFrame": 0,
        "durationFrames": 30,
        "trigger": "manual",
        "triggerRef": None,
        "priority": 10,
        "continuityRefs": [],
        "gainDb": -2.0,
        "mute": False,
        "solo": False,
        "fadeInFrames": 3,
        "fadeOutFrames": 3,
    }
    value.update(changes)
    return value


@pytest.mark.asyncio
async def test_application_persists_owner_facts_and_immutable_snapshot() -> None:
    uow, service, values = await _timeline_owners()
    episode, project = values["episode"], values["project"]
    cut = await service.edit(
        episode.id, 1, "add_clip", {"clip": values["clip"]}, project_id=project.id
    )
    assert cut.revision == 2 and uow.audit_events[-1]["type"] == "timeline.edited"
    version = await service.publish(episode.id, "Director cut", 2, project.id)
    snapshot = deepcopy(version.cut_snapshot)
    await service.edit(episode.id, 2, "delete_clip", {"clip_id": "clip-1"}, project_id=project.id)
    assert version.cut_snapshot == snapshot and version.source_cut_revision == 2
    assert timeline_cut_projection(cut)["schemaVersion"] == cut.schema_version


@pytest.mark.asyncio
async def test_application_rejects_foreign_stale_or_unready_without_writes() -> None:
    uow, service, values = await _timeline_owners()
    episode, project = values["episode"], values["project"]
    for change in (
        {"episodeId": "foreign"},
        {"assetVersionHash": "f" * 64},
        {"derivativeStatus": "pending"},
    ):
        before = (len(uow.timeline_cuts), len(uow.audit_events), len(uow.outbox_events))
        with pytest.raises(ValidationDomainError):
            await service.edit(
                episode.id,
                1,
                "add_clip",
                {"clip": dict(values["clip"], **change)},
                project_id=project.id,
            )
        assert (len(uow.timeline_cuts), len(uow.audit_events), len(uow.outbox_events)) == before


@pytest.mark.asyncio
async def test_audio_owner_selection_trigger_order_and_remove_are_exact() -> None:
    uow, service, values = await _timeline_owners()
    episode, project = values["episode"], values["project"]
    cut = await service.edit(
        episode.id, 1, "add_clip", {"clip": values["clip"]}, project_id=project.id
    )
    cue = await _audio_cue(uow, project, episode)
    cut = await service.edit(
        episode.id, cut.revision, "add_sound_cue", {"cue": cue}, project_id=project.id
    )
    second = await _audio_cue(
        uow,
        project,
        episode,
        id="00000000-0000-4000-8000-000000000602",
        priority=90,
    )
    cut = await service.edit(
        episode.id, cut.revision, "add_sound_cue", {"cue": second}, project_id=project.id
    )
    assert [item.priority for item in cut.cues] == [90, 10]
    audio_version_id = cut.cues[0].asset_version_id
    cut = await service.edit(
        episode.id,
        cut.revision,
        "remove_sound_cue",
        {"cue_id": cut.cues[0].id},
        project_id=project.id,
    )
    assert await uow.asset_versions.get(audio_version_id) is not None


@pytest.mark.asyncio
@pytest.mark.parametrize(
    "changes",
    [
        {"cueType": "music"},
        {"automation": []},
        {"keyframes": []},
        {"priority": 101},
        {"startFrame": 60},
        {"explicitSelection": False},
        {"storageVerified": False},
        {"episodeId": "foreign"},
    ],
)
async def test_audio_invalid_selector_alias_or_frame_is_zero_write(
    changes: dict[str, object],
) -> None:
    uow, service, values = await _timeline_owners()
    episode, project = values["episode"], values["project"]
    cut = await service.edit(
        episode.id, 1, "add_clip", {"clip": values["clip"]}, project_id=project.id
    )
    cue = await _audio_cue(uow, project, episode, **changes)
    before = (cut.revision, len(cut.cues), len(uow.audit_events), len(uow.outbox_events))
    with pytest.raises(ValidationDomainError):
        await service.edit(
            episode.id, cut.revision, "add_sound_cue", {"cue": cue}, project_id=project.id
        )
    current = uow.timeline_cuts[episode.id]
    assert (
        current.revision,
        len(current.cues),
        len(uow.audit_events),
        len(uow.outbox_events),
    ) == before


@pytest.mark.asyncio
async def test_audio_rejects_non_audio_stale_trigger_and_continuity_refs() -> None:
    uow, service, values = await _timeline_owners()
    episode, project, shot = values["episode"], values["project"], values["shot"]
    cut = await service.edit(
        episode.id, 1, "add_clip", {"clip": values["clip"]}, project_id=project.id
    )
    base = await _audio_cue(uow, project, episode)
    invalid_values = [
        {
            **base,
            "assetVersionId": values["videoVersion"].id,
            "assetVersionRevision": values["videoVersion"].revision,
            "assetVersionHash": values["videoVersion"].content_hash,
        },
        {
            **base,
            "trigger": "shot_start",
            "triggerRef": {
                "ownerType": "shot",
                "id": shot.id,
                "revision": shot.revision,
                "startFrame": 1,
                "offsetFrames": 0,
            },
        },
        {
            **base,
            "continuityRefs": [
                {"ownerType": "shot", "id": shot.id, "revision": shot.revision, "hash": "0" * 64},
                {"ownerType": "shot", "id": shot.id, "revision": shot.revision, "hash": "0" * 64},
            ],
        },
    ]
    for invalid in invalid_values:
        with pytest.raises(ValidationDomainError):
            await service.edit(
                episode.id,
                cut.revision,
                "add_sound_cue",
                {"cue": invalid},
                project_id=project.id,
            )
        cut = uow.timeline_cuts[episode.id]
        assert cut.revision == 2 and not cut.cues


@pytest.mark.asyncio
async def test_export_diagnostic_resolves_exact_owner_facts_without_mutation() -> None:
    uow, _timeline, values = await _timeline_owners()
    project, episode = values["project"], values["episode"]
    version = TimelineVersion(
        episode.id,
        7,
        "Diagnostic",
        {
            "schema_version": "1.0.0",
            "clips": [{"id": "clip-owner"}],
            "captions": [{"id": "caption-owner"}],
            "soundCues": [{"id": "cue-owner"}],
        },
        project.id,
    )
    uow.timeline_versions[version.id] = version
    job = ExportJob(project.id, episode.id, version.id)
    artifact = ExportArtifact(job.id, "mp4", "pending")
    job.append_artifact(artifact)
    uow.export_jobs[job.id] = job
    service = ExportService(lambda: uow, MockFfmpegRenderAdapter())
    snapshot = deepcopy(version.cut_snapshot)
    owner_cases = (
        ("clip", "clip-owner", 7),
        ("caption", "caption-owner", 7),
        ("sound_cue", "cue-owner", 7),
        ("asset_version", values["videoVersion"].id, values["videoVersion"].revision),
        ("artifact", artifact.id, job.revision),
        ("renderer", None, None),
        ("storage", None, None),
    )
    for target_type, owner_id, owner_revision in owner_cases:
        target = ExportDiagnosticTarget(
            target_type,
            project.id,
            episode.id,
            version.id,
            owner_id,
            owner_revision,
            None,
            "safe_route_token_1234",
            "diagnostic",
        )
        assert (await service.diagnostic(target))["owner_id"] == owner_id
    assert version.cut_snapshot == snapshot

    stale = ExportDiagnosticTarget(
        "clip",
        project.id,
        episode.id,
        version.id,
        "clip-owner",
        6,
        None,
        "safe_route_token_1234",
        "stale",
    )
    with pytest.raises(ValidationDomainError, match="stale"):
        await service.diagnostic(stale)
    with pytest.raises(ValidationDomainError, match="array positions"):
        ExportDiagnosticTarget(
            "clip",
            project.id,
            episode.id,
            version.id,
            "clip-owner",
            7,
            "clips[0]",
            "safe_route_token_1234",
            "bad_path",
        )


@pytest.mark.asyncio
async def test_multi_episode_batch_preflights_all_members_before_independent_jobs(
    tmp_path: Path,
) -> None:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("P")
    episodes = [
        await projects.create_episode((project.id, f"E{number}", number)) for number in (1, 2)
    ]
    versions: list[TimelineVersion] = []
    for number, episode in enumerate(episodes, start=1):
        clip = _clip(clip_id=f"clip-{number}")
        asset_version = await _generated_export_asset(uow, project.id, episode.id, f"take-{number}")
        clip.update(
            {
                "projectId": project.id,
                "episodeId": episode.id,
                "assetVersionId": asset_version.id,
                "assetVersionRevision": asset_version.revision,
                "assetVersionHash": asset_version.content_hash,
            }
        )
        version = TimelineVersion(
            episode.id,
            1,
            f"Published {number}",
            {
                "schema_version": "1.0.0",
                "clips": [clip],
                "soundCues": [],
                "captions": [],
                "ducking": None,
            },
            project.id,
        )
        uow.timeline_versions[version.id] = version
        versions.append(version)
    service = ExportService(
        lambda: uow,
        MockFfmpegRenderAdapter(),
        storage=LocalWorkspaceAdapter(tmp_path / "storage"),
    )
    selections = [
        {
            "episodeId": episode.id,
            "timelineVersionId": version.id,
            "timelineVersionRevision": version.revision,
            "outputBaseName": f"episode-{number:02d}",
        }
        for number, (episode, version) in enumerate(zip(episodes, versions, strict=True), start=1)
    ]
    batch = await service.create_batch(
        project.id,
        selections,
        "light",
        "multi-episode",
        storage_profile_id="local-test-offline",
        storage_profile_revision=1,
    )
    assert len(batch.jobs) == 2
    assert {job.episode_id for job in batch.jobs} == {item.id for item in episodes}
    assert len({job.id for job in batch.jobs}) == 2
    assert all(len(job.artifacts) == 3 for job in batch.jobs)
    assert len(uow.export_dispatch_outbox) == 2
    assert {(event.job_id, event.status) for event in uow.export_dispatch_outbox.values()} == {
        (job.id, "pending") for job in batch.jobs
    }

    before = (len(uow.export_batches), len(uow.export_jobs), len(uow.outbox_events))
    invalid = deepcopy(selections)
    invalid[1]["timelineVersionRevision"] = 2
    with pytest.raises(ValidationDomainError):
        await service.create_batch(
            project.id,
            invalid,
            "light",
            "invalid-member",
            storage_profile_id="local-test-offline",
            storage_profile_revision=1,
        )
    assert (len(uow.export_batches), len(uow.export_jobs), len(uow.outbox_events)) == before


def test_runtime_has_no_openspec_plan_or_timeline_draft_dependency() -> None:
    from pathlib import Path

    source_root = Path(__file__).parents[1] / "src" / "video_agent_api"
    runtime = "\n".join(path.read_text() for path in source_root.rglob("*.py"))
    assert "plan-phase-one-drama-mvp-a" not in runtime
    assert "class TimelineDraft" not in runtime


def _http_owner_client(
    tmp_path: Path,
) -> tuple[TestClient, InMemoryUnitOfWork, dict[str, object]]:
    uow = InMemoryUnitOfWork()
    projects = ProjectsEpisodesService(lambda: uow)
    app = create_app(readiness_probe=lambda: True, projects_episodes_service=projects)
    storage = LocalWorkspaceAdapter(tmp_path / "storage")
    app.state.export_service = ExportService(
        lambda: uow, MockFfmpegRenderAdapter(), storage=storage
    )
    client = TestClient(app)
    project = client.post("/v1/projects", json={"name": "P"}).json()
    episode = client.post(
        f"/v1/projects/{project['id']}/episodes", json={"number": 1, "title": "E"}
    ).json()
    asset_version = asyncio.run(
        _generated_export_asset(uow, project["id"], episode["id"], "http-take")
    )
    cut = TimelineCut(episode["id"], project["id"])
    cut.clips.append(_clip())
    cut.clips[0].update(
        {
            "projectId": project["id"],
            "episodeId": episode["id"],
            "assetVersionId": asset_version.id,
            "assetVersionRevision": asset_version.revision,
            "assetVersionHash": asset_version.content_hash,
        }
    )
    uow.timeline_cuts[episode["id"]] = cut
    version = TimelineVersion(
        episode["id"],
        cut.revision,
        "Published",
        {
            "schema_version": "1.0.0",
            "clips": [deepcopy(cut.clips[0])],
            "soundCues": [],
            "captions": [],
            "ducking": None,
        },
        project["id"],
    )
    uow.timeline_versions[version.id] = version
    return client, uow, {"project": project, "episode": episode, "version": version}


def _export_settings() -> dict[str, object]:
    return {
        "aspectRatio": "9:16",
        "width": 1080,
        "height": 1920,
        "fps": 30,
        "container": "mp4",
        "videoCodec": "h264",
        "pixelFormat": "yuv420p",
        "audioCodec": "aac",
        "sampleRate": 48000,
        "subtitleEncoding": "UTF-8",
    }


def test_timeline_http_is_project_scoped_strict_and_returns_authoritative_409(
    tmp_path: Path,
) -> None:
    client, uow, owner = _http_owner_client(tmp_path)
    project = owner["project"]
    episode = owner["episode"]
    path = f"/v1/projects/{project['id']}/episodes/{episode['id']}/timeline"
    scope = {"X-Project-Scope": project["id"]}
    assert client.get(path).status_code == 403
    current = client.get(path, headers=scope)
    assert current.status_code == 200
    assert current.json()["schemaVersion"] == "1.0.0"
    before = deepcopy(uow.timeline_cuts[episode["id"]].clips)
    conflict = client.post(
        f"{path}/commands",
        headers=scope,
        json={
            "expectedRevision": 2,
            "command": "DeleteClip",
            "payload": {"clipId": "clip-1"},
            "schemaVersion": "1.0.0",
        },
    )
    assert conflict.status_code == 409
    assert conflict.json()["authoritative"]["revision"] == 1
    assert uow.timeline_cuts[episode["id"]].clips == before
    alias = client.post(
        f"{path}/commands",
        headers=scope,
        json={
            "expectedRevision": 1,
            "command": "DeleteClip",
            "payload": {"clipId": "clip-1"},
            "schemaVersion": "1.0.0",
            "schema_version": "1.0.0",
        },
    )
    assert alias.status_code == 422 and uow.timeline_cuts[episode["id"]].clips == before
    forbidden = client.get(path, headers={"X-Project-Scope": "foreign"})
    assert forbidden.status_code == 403


def test_timeline_publish_preflight_is_read_only_and_conflicts_authoritatively(
    tmp_path: Path,
) -> None:
    client, uow, owner = _http_owner_client(tmp_path)
    project, episode = owner["project"], owner["episode"]
    scope = {"X-Project-Scope": project["id"]}
    path = f"/v1/projects/{project['id']}/episodes/{episode['id']}/timeline/versions/preflight"
    before = (len(uow.timeline_versions), len(uow.audit_events), len(uow.outbox_events))
    ready = client.post(
        path,
        headers=scope,
        json={"name": "check-only", "expectedRevision": 1, "schemaVersion": "1.0.0"},
    )
    assert ready.status_code == 200
    assert ready.json()["expectedRevision"] == 1
    assert (len(uow.timeline_versions), len(uow.audit_events), len(uow.outbox_events)) == before
    stale = client.post(
        path,
        headers=scope,
        json={"name": "stale", "expectedRevision": 2, "schemaVersion": "1.0.0"},
    )
    assert stale.status_code == 409
    assert stale.json()["authoritative"]["revision"] == 1


def test_export_http_batch_retry_artifact_and_renderer_boundaries(tmp_path: Path) -> None:
    client, uow, owner = _http_owner_client(tmp_path)
    project, episode, version = owner["project"], owner["episode"], owner["version"]
    path = f"/v1/projects/{project['id']}/export-batches"
    scope = {"X-Project-Scope": project["id"]}
    body = {
        "selections": [
            {
                "episodeId": episode["id"],
                "timelineVersionId": version.id,
                "timelineVersionRevision": 1,
                "outputBaseName": "episode-01",
            }
        ],
        "exportProfile": "light",
        "idempotencyKey": "http-export-1",
        "storageProfileId": "local-test-offline",
        "storageProfileRevision": 1,
        "expectedRevision": 1,
        "settings": _export_settings(),
        "schemaVersion": "1.0.0",
    }
    created = client.post(path, json=body, headers=scope)
    assert created.status_code == 201
    batch = created.json()
    assert batch["schemaVersion"] == "1.0.0"
    assert batch["members"][0]["episodeId"] == episode["id"]
    assert client.post(path, json=body, headers=scope).json()["id"] == batch["id"]
    before = len(uow.export_batches)
    for invalid in (
        {**body, "profile": "light"},
        {**body, "exportProfile": "portable", "idempotencyKey": "portable"},
        {**body, "schemaVersion": "2.0.0", "idempotencyKey": "schema"},
    ):
        assert client.post(path, json=invalid, headers=scope).status_code == 422
        assert len(uow.export_batches) == before
    forbidden = client.post(path, json=body, headers={"X-Project-Scope": "foreign"})
    assert forbidden.status_code == 403
    read = client.get(f"{path}/{batch['id']}", headers=scope)
    assert read.status_code == 200 and read.json() == batch

    domain_batch = uow.export_batches[batch["id"]]
    failed = domain_batch.jobs[0]
    failed.status = "failed"
    retry = client.post(
        f"{path}/{batch['id']}/retries",
        headers=scope,
        json={
            "episodeIds": [episode["id"]],
            "logicalOperation": "retry-1",
            "schemaVersion": "1.0.0",
        },
    )
    assert retry.status_code == 201
    assert retry.json()[0]["logicalOperation"] == "retry-1"
    assert len(domain_batch.jobs) == 2
    assert len(uow.export_dispatch_outbox) == 2
    assert any(
        event.job_id == domain_batch.jobs[-1].id
        and event.logical_operation == "retry-1"
        and event.status == "pending"
        for event in uow.export_dispatch_outbox.values()
    )

    job = domain_batch.jobs[-1]
    job.status = "packaging"
    job.packaging_phase = "registering"
    checksum = "e" * 64
    artifact_body = {
        "artifactType": "mp4",
        "sizeBytes": 8,
        "checksum": checksum,
        "verified": True,
        "expectedRevision": job.revision,
        "schemaVersion": "1.0.0",
        "storageProfileRevision": 1,
        "storedObject": {
            "projectId": project["id"],
            "profileId": "local",
            "bucket": "workspace",
            "objectKey": f"exports/{job.id}/episode.mp4",
            "sizeBytes": 8,
            "checksum": checksum,
            "mimeType": "video/mp4",
            "etag": "etag",
            "operationKey": f"export-upload:{project['id']}:{job.id}:mp4",
            "verified": True,
        },
    }
    registered = client.post(
        f"/v1/projects/{project['id']}/export-jobs/{job.id}/artifacts",
        headers=scope,
        json=artifact_body,
    )
    assert registered.status_code == 201 and registered.json()["status"] == "verified"
    assert "objectKey" not in registered.text and "workspace://" not in registered.text

    issuer = LocalOpaqueReadGrantIssuer()
    client.app.state.export_service = ExportService(lambda: uow, MockFfmpegRenderAdapter(), issuer)
    artifact = next(item for item in job.artifacts if item.artifact_type == "mp4")
    grant_path = (
        f"/v1/projects/{project['id']}/episodes/{episode['id']}/timeline/versions/"
        f"{version.id}/export-jobs/{job.id}/artifacts/{artifact.id}/download-grants"
    )
    grant = client.post(
        grant_path,
        json={"ttlSeconds": 300, "schemaVersion": "1.0.0"},
        headers=scope,
    )
    assert grant.status_code == 200
    assert set(grant.json()) == {
        "schemaVersion",
        "artifactId",
        "expiresAt",
        "action",
        "accessPath",
    }
    assert grant.json()["action"] == "read"
    assert "objectKey" not in grant.text and "workspace://" not in grant.text
    token = grant.json()["accessPath"].rsplit("/", 1)[-1]
    assert issuer.resolve(token).project_id == project["id"]
    with pytest.raises(StorageAuthorizationError, match="expired"):
        issuer.resolve(token, now=grant.json()["expiresAt"])

    rejected_paths = (
        grant_path.replace(f"/projects/{project['id']}/", "/projects/foreign/"),
        grant_path.replace(f"/episodes/{episode['id']}/", "/episodes/foreign/"),
        grant_path.replace(f"/versions/{version.id}/", "/versions/foreign/"),
        grant_path.replace(f"/export-jobs/{job.id}/", "/export-jobs/foreign/"),
        grant_path.replace(f"/artifacts/{artifact.id}/", "/artifacts/foreign/"),
    )
    for rejected_path in rejected_paths:
        response = client.post(
            rejected_path,
            json={"ttlSeconds": 60, "schemaVersion": "1.0.0"},
            headers=scope,
        )
        assert response.status_code in {403, 422}
        assert "objectKey" not in response.text and "workspace://" not in response.text
    unauthorized = client.post(
        grant_path,
        json={"ttlSeconds": 60, "schemaVersion": "1.0.0"},
        headers={"X-Project-Scope": "foreign"},
    )
    assert unauthorized.status_code == 403
    assert (
        client.post(
            grant_path,
            json={"ttlSeconds": 301, "schemaVersion": "1.0.0"},
            headers=scope,
        ).status_code
        == 422
    )

    unavailable_changes = (
        {"hold": True},
        {"status": "held"},
        {"license_status": "denied"},
        {"expires_at": (datetime.now(UTC) - timedelta(seconds=1)).isoformat()},
    )
    for changes in unavailable_changes:
        current_job = uow.export_jobs[job.id]
        current_artifact = next(item for item in current_job.artifacts if item.id == artifact.id)
        artifact_index = current_job.artifacts.index(current_artifact)
        current_job.artifacts[artifact_index] = replace(current_artifact, **changes)
        denied = client.post(
            grant_path,
            json={"ttlSeconds": 60, "schemaVersion": "1.0.0"},
            headers=scope,
        )
        assert denied.status_code == 422
        assert "objectKey" not in denied.text and "workspace://" not in denied.text

    client.app.state.export_service = ExportService(
        lambda: uow, SubprocessFfmpegRenderAdapter(None, None)
    )
    probe = client.get(f"/v1/projects/{project['id']}/renderer/probe", headers=scope)
    assert probe.status_code == 503
    assert probe.json()["detail"]["type"] == "renderer_unconfigured"
    assert "FFMPEG_PATH and FFPROBE_PATH are required" in probe.json()["detail"]["message"]
