from __future__ import annotations

import hashlib
import json
import subprocess
from dataclasses import replace
from pathlib import Path

import pytest
from alembic.config import Config
from sqlalchemy.ext.asyncio import async_sessionmaker, create_async_engine

from alembic import command
from video_agent_api.adapters.ffmpeg import (
    MockFfmpegRenderAdapter,
    SubprocessFfmpegRenderAdapter,
)
from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.adapters.media_temporal import (
    MediaActivityDependencies,
    TemporalMediaStarter,
    configure_media_activities,
    media_derivative,
    media_render,
    media_storage_terminal_handoff,
    media_storage_upload,
)
from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
from video_agent_api.application.assets import (
    AppendAssetVersionCommand,
    AssetsService,
    CreateAssetCommand,
)
from video_agent_api.application.export_worker import (
    EpisodeExportWorker,
    ExecuteExportJobCommand,
    _storage_profile_matches_snapshot,
)
from video_agent_api.application.exports import ExportService
from video_agent_api.application.media import (
    MEDIA_ROUTE,
    MEDIA_SCHEMA_VERSION,
    MEDIA_TASK_QUEUE,
    MEDIA_WORKFLOW_TYPE,
    MediaDispatchAdmission,
    MediaDispatchService,
    MediaOwnerService,
    RecordDerivativeCommand,
    RecordInspectionCommand,
)
from video_agent_api.application.projects_episodes import ProjectsEpisodesService
from video_agent_api.application.rendering import (
    compile_ffmpeg_filter_graph,
    compile_preview_manifest,
    compile_render_plan,
    render_srt,
    verify_parity,
)
from video_agent_api.application.scenes import _accept_current_media
from video_agent_api.domain.assets import StorageObject
from video_agent_api.domain.catalog import default_skill_revisions
from video_agent_api.domain.errors import (
    RendererCapabilityUnsupportedError,
    RendererUnconfiguredError,
    RevisionConflictError,
    ValidationDomainError,
)
from video_agent_api.domain.media import PreviewArtifact
from video_agent_api.domain.provider_ops import ProviderCall
from video_agent_api.domain.runs import WorkflowRun
from video_agent_api.domain.scenes import ImmutableOwnerRef, Shot
from video_agent_api.domain.timeline import TimelineCut, TimelineVersion
from video_agent_api.domain.video_generation import VideoOperation, VideoTakeCandidate
from video_agent_api.ports.contracts import (
    StorageCapability,
    StorageRetryableError,
    StorageValidationError,
    StoredObjectRef,
)
from video_agent_api.ports.rendering import (
    LoudnessMeasurement,
    RenderOutputInspection,
    RenderRequest,
    RenderResult,
)
from video_agent_api.ports.storage import LocalWorkspaceAdapter
from video_agent_api.resilience import (
    OperationsResilienceCoordinator,
    RuntimeResourceSnapshot,
    admission_refs,
    capacity_snapshot,
)


def _metadata(checksum: str) -> dict[str, object]:
    return {
        "mimeType": "video/mp4",
        "sizeBytes": 1024,
        "checksum": checksum,
        "durationFrames": 60,
        "timebase": "1/30",
        "fpsNumerator": 30,
        "fpsDenominator": 1,
        "frameCount": 60,
        "width": 1080,
        "height": 1920,
        "videoCodec": "h264",
        "pixelFormat": "yuv420p",
        "audioTracks": 1,
        "sampleRate": 48000,
        "channels": 2,
    }


def test_media_storage_preflight_rejects_bucket_or_profile_snapshot_drift() -> None:
    snapshot = {
        "adapterKey": "local_workspace",
        "profileId": "local-test-offline",
        "projectId": "project-1",
        "bucket": "workspace",
        "bucketBindingId": "local-workspace",
        "endpoint": "workspace://local",
        "region": "local",
        "revision": 1,
    }
    capability = {
        "profileRevision": 1,
        "minPartSizeBytes": 1,
        "maxPartSizeBytes": 64 * 1024 * 1024,
        "maxPartCount": 10_000,
        "maxObjectSizeBytes": 8 * 1024**4,
    }

    assert (
        _storage_profile_matches_snapshot(
            "local_workspace", snapshot, capability, bucket="workspace"
        )
        is True
    )
    assert (
        _storage_profile_matches_snapshot(
            "local_workspace", snapshot, capability, bucket="foreign-bucket"
        )
        is False
    )


async def _asset_owner(
    uow: InMemoryUnitOfWork,
) -> tuple[object, object, object]:
    projects = ProjectsEpisodesService(lambda: uow)
    project = await projects.create_project("P")
    episode = await projects.create_episode((project.id, "E", 1))
    assets = AssetsService(lambda: uow)
    asset = await assets.create_asset(CreateAssetCommand(project.id, "Take", "video"))
    asset.authorization_status, asset.license = "verified", "owned"
    asset.source_type = "provider_generated"
    version = await assets.append_version(
        AppendAssetVersionCommand(
            asset.id,
            StorageObject(
                "local",
                "workspace",
                f"projects/{project.id}/take.mp4",
                "video/mp4",
                len(b"source"),
                hashlib.sha256(b"source").hexdigest(),
            ),
            "b" * 64,
        )
    )
    logical_operation = "video.submit:export-source"
    uow.skills.extend(default_skill_revisions())
    selected_skills = [
        item for item in uow.skills if item.name in {"novel-writing", "drama-skills"}
    ]
    run = WorkflowRun(
        project.id,
        "workflow-version",
        selection_snapshot={
            "skillRevisionIds": [f"{item.name}@{item.version}" for item in selected_skills],
            "skillDigests": [item.digest for item in selected_skills],
        },
    )
    operation = VideoOperation(
        project.id,
        run.id,
        logical_operation,
        "provider",
        "profile",
        "model",
        "capability",
        "source-version",
        0,
        "a" * 64,
        "shot-spec",
        1,
        "b" * 64,
        1.0,
        "9:16",
        status="succeeded",
    )
    provider_call = ProviderCall(
        project.id,
        run.id,
        None,
        logical_operation,
        "video.submit",
        operation.provider_id,
        operation.profile_id,
        operation.model_id,
        operation.capability_snapshot_id,
        "f" * 64,
        "succeeded",
        cost_status="known",
        cost_value="1.0",
        cost_currency="CNY",
        cost_source="provider-billing",
        native_usage={"seconds": 1},
    )
    candidate = VideoTakeCandidate(
        project.id,
        episode.id,
        "shot",
        run.id,
        logical_operation,
        "source-version",
        0,
        "a" * 64,
        "shot-spec",
        1,
        "b" * 64,
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
    return project, episode, version


@pytest.mark.asyncio
async def test_media_owner_is_exact_idempotent_bounded_and_preview_stales() -> None:
    uow = InMemoryUnitOfWork()
    project, episode, version = await _asset_owner(uow)
    command_value = RecordInspectionCommand(
        project.id,
        version.id,
        version.revision,
        version.content_hash,
        "inspect:1",
        _metadata(version.content_hash),
        "ffprobe",
        "7.1",
    )
    unavailable = RuntimeResourceSnapshot(
        cpu_count=4,
        available_concurrency=4,
        memory_available_bytes=1024,
        memory_limit_bytes=2048,
        disk_free_bytes=None,
        disk_total_bytes=None,
        captured_at="2026-08-26T00:00:00+00:00",
        error="resource probe unavailable",
    )
    blocked_service = MediaOwnerService(
        lambda: uow,
        resilience=OperationsResilienceCoordinator(
            unavailable, capacity_snapshot(unavailable, project.id)
        ),
    )
    with pytest.raises(ValidationDomainError, match="resource_probe_unavailable"):
        await blocked_service.record_inspection(command_value)
    assert not uow.media_inspections and not uow.media_derivatives and not uow.preview_artifacts
    healthy = RuntimeResourceSnapshot(
        cpu_count=4,
        available_concurrency=4,
        memory_available_bytes=2048,
        memory_limit_bytes=4096,
        disk_free_bytes=8 * 1024 * 1024,
        disk_total_bytes=16 * 1024 * 1024,
        captured_at="2026-08-26T00:00:00+00:00",
    )
    service = MediaOwnerService(
        lambda: uow,
        resilience=OperationsResilienceCoordinator(healthy, capacity_snapshot(healthy, project.id)),
    )
    inspection = await service.record_inspection(command_value)
    assert inspection.admission_refs["operation"] == "media.inspect"
    assert (await service.record_inspection(command_value)).id == inspection.id
    with pytest.raises(ValidationDomainError, match="operation conflict"):
        await service.record_inspection(
            RecordInspectionCommand(
                project.id,
                version.id,
                version.revision,
                version.content_hash,
                "inspect:1",
                {**_metadata(version.content_hash), "frameCount": 61},
                "ffprobe",
                "7.1",
            )
        )

    derivative = await service.record_derivative(
        RecordDerivativeCommand(
            project.id,
            inspection.id,
            "proxy",
            "ready",
            {"maxWidth": 540, "maxFrames": 60},
            "derive:proxy:1",
            "ffmpeg",
            "7.1",
            object_ref={
                "profileId": "local",
                "objectKey": f"projects/{project.id}/proxy.mp4",
                "operationKey": "derive:proxy:1",
            },
            checksum="c" * 64,
            size_bytes=512,
        )
    )
    assert derivative.admission_refs["operation"] == "media.derivative"
    ready = await service.ready_derivatives(
        project.id, version.id, version.revision, version.content_hash
    )
    assert ready == (derivative,)
    cut = TimelineCut(episode.id, project.id)
    uow.timeline_cuts[episode.id] = cut
    preview = PreviewArtifact(
        project.id,
        episode.id,
        cut.id,
        cut.revision,
        cut.fingerprint(),
        "d" * 64,
        "ready",
        (derivative.id,),
    )
    preview = await service.record_preview(preview)
    assert preview.admission_refs["operation"] == "media.preview"
    assert preview.matches(cut.revision, cut.fingerprint(), "d" * 64)
    cut.revision += 1
    assert not preview.matches(cut.revision, cut.fingerprint(), "d" * 64)
    with pytest.raises(ValidationDomainError, match="claimed-vs-observed"):
        await service.record_inspection(
            RecordInspectionCommand(
                project.id,
                version.id,
                version.revision,
                version.content_hash,
                "inspect:mismatch",
                _metadata("f" * 64),
                "ffprobe",
                "7.1",
            )
        )


@pytest.mark.asyncio
async def test_media_dispatch_admission_source_and_generated_zero_side_effect() -> None:
    uow = InMemoryUnitOfWork()
    project, _episode, version = await _asset_owner(uow)
    asset = await uow.assets.get(version.asset_id)
    assert asset is not None
    asset.source_type = "user_upload"
    asset.authorization_status = "verified"
    ref = StoredObjectRef(
        project.id,
        "local",
        "workspace",
        version.storage_object.object_key,
        version.storage_object.size_bytes,
        version.storage_object.checksum,
        version.storage_object.mime_type,
        version.storage_object.e_tag,
        "media:source:1",
    )
    service = MediaOwnerService(lambda: uow)
    source = MediaDispatchAdmission(
        project.id,
        "uploaded_source",
        version.id,
        version.revision,
        version.content_hash,
        ref,
        "media:source:1",
    )
    event = await service.enqueue_dispatch(source)
    assert event["discriminator"] == "uploaded_source"
    assert await service.enqueue_dispatch(source) == event
    before = len(uow.outbox_events)
    with pytest.raises(ValidationDomainError, match="accepted/current"):
        await service.enqueue_dispatch(
            MediaDispatchAdmission(
                project.id,
                "generated_candidate",
                version.id,
                version.revision,
                version.content_hash,
                replace(ref, operation_key="media:generated:rejected"),
                "media:generated:rejected",
                episode_id="episode-foreign",
                shot_id="shot-foreign",
                candidate_id="candidate-missing",
            )
        )
    assert len(uow.outbox_events) == before


@pytest.mark.asyncio
async def test_media_activity_requires_the_durable_dispatch_admission() -> None:
    """Activities may revalidate an owner ledger but must not create one after dispatch."""
    uow = InMemoryUnitOfWork()
    project, _episode, version = await _asset_owner(uow)
    asset = await uow.assets.get(version.asset_id)
    assert asset is not None
    asset.source_type, asset.authorization_status = "user_upload", "verified"
    resource = RuntimeResourceSnapshot(2, 2, 1024, 2048, 4096, 8192, "now")
    resilience = OperationsResilienceCoordinator(resource, capacity_snapshot(resource, "*"))
    owner = MediaOwnerService(lambda: uow, resilience=resilience)
    operation_key = "media:durable:1"
    event = await owner.enqueue_dispatch(
        MediaDispatchAdmission(
            project.id,
            "uploaded_source",
            version.id,
            version.revision,
            version.content_hash,
            StoredObjectRef(
                project.id,
                "local",
                "workspace",
                version.storage_object.object_key,
                version.storage_object.size_bytes,
                version.storage_object.checksum,
                version.storage_object.mime_type,
                version.storage_object.e_tag,
                operation_key,
            ),
            operation_key,
        )
    )

    with pytest.raises(ValidationDomainError, match="dispatch admission is required"):
        owner.admit_activity(project.id, "media.inspect", operation_key)

    assert (
        owner.admit_activity(
            project.id,
            "media.inspect",
            operation_key,
            event["resourceAdmission"],
        )
        == event["resourceAdmission"]
    )


@pytest.mark.asyncio
async def test_generated_candidate_producer_uses_one_canonical_media_operation_key() -> None:
    uow = InMemoryUnitOfWork()
    _project, _episode, version = await _asset_owner(uow)
    candidate = next(iter(uow.video_take_candidates.values()))
    candidate = replace(candidate, status="accepted", revision=candidate.revision + 1)
    uow.video_take_candidates[candidate.id] = candidate
    owner = MediaOwnerService(lambda: uow)

    event = await owner.produce_generated_candidate(uow, candidate=candidate, asset_version=version)
    duplicate = await owner.produce_generated_candidate(
        uow, candidate=candidate, asset_version=version
    )

    expected_key = (
        f"media:generated:{candidate.id}:{candidate.revision}:{version.id}:"
        f"{version.revision}:{version.content_hash}"
    )
    assert event is duplicate
    assert event["operationKey"] == expected_key
    assert event["storedObjectRef"]["operationKey"] == expected_key
    assert event["discriminator"] == "generated_candidate"
    assert (
        len([item for item in uow.outbox_events if item.get("type") == "media.dispatch.requested"])
        == 1
    )


@pytest.mark.asyncio
async def test_generated_candidate_producer_accepts_scene_projection_dict() -> None:
    """Scenes owner hands the producer a canonical projection, not a domain object."""
    uow = InMemoryUnitOfWork()
    _project, _episode, version = await _asset_owner(uow)
    candidate = next(iter(uow.video_take_candidates.values()))
    candidate = replace(candidate, status="accepted", revision=candidate.revision + 1)
    projection = {
        "id": candidate.id,
        "projectId": candidate.project_id,
        "episodeId": candidate.episode_id,
        "targetId": candidate.target_id,
        "revision": candidate.revision,
        "status": candidate.status,
        "assetVersionId": candidate.asset_version_id,
        "assetVersionRevision": candidate.asset_version_revision,
        "assetVersionHash": candidate.asset_version_hash,
        "provenance": "media_review",
        "mediaKind": "image",
    }
    owner = MediaOwnerService(lambda: uow)

    event = await owner.produce_generated_candidate(
        uow, candidate=projection, asset_version=version
    )

    assert event["candidateId"] == candidate.id
    assert event["candidateRevision"] == candidate.revision
    assert event["assetVersionId"] == version.id


@pytest.mark.asyncio
async def test_scene_accept_produces_media_event() -> None:
    """The review projection carries its accepted state without mutating client input."""
    uow = InMemoryUnitOfWork()
    project, episode, version = await _asset_owner(uow)
    candidate = next(iter(uow.video_take_candidates.values()))
    shot = Shot("scene", project.id, episode.id, 1)
    shot.continuity_snapshot = ImmutableOwnerRef("continuity", 1, "a" * 64)
    uow.shots[shot.id] = shot
    projection = {
        "candidateId": candidate.id,
        "candidateRevision": candidate.revision,
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
    }
    owner = MediaOwnerService(lambda: uow)

    await _accept_current_media(
        uow,
        project_id=project.id,
        episode_id=episode.id,
        shot_id=shot.id,
        candidate=projection,
        expected_shot_revision=shot.revision,
        media_owner=owner,
    )

    event = next(
        item for item in uow.outbox_events if item.get("type") == "media.dispatch.requested"
    )
    assert event["candidateId"] == candidate.id
    assert event["operationKey"] == (
        f"media:generated:{candidate.id}:{candidate.revision}:{version.id}:"
        f"{version.revision}:{version.content_hash}"
    )
    assert "status" not in projection


@pytest.mark.asyncio
async def test_media_dispatch_freezes_route_and_fails_closed_before_temporal_start() -> None:
    class Starter:
        def __init__(self, unavailable: bool = False) -> None:
            self.calls: list[dict[str, object]] = []
            self.unavailable = unavailable

        async def start(self, payload: dict[str, object]) -> str:
            self.calls.append(payload)
            if self.unavailable:
                raise RuntimeError("media queue is unreachable")
            return "started"

    uow = InMemoryUnitOfWork()
    project, _episode, version = await _asset_owner(uow)
    asset = await uow.assets.get(version.asset_id)
    assert asset is not None
    asset.source_type, asset.authorization_status = "user_upload", "verified"
    ref = StoredObjectRef(
        project.id,
        "local",
        "workspace",
        version.storage_object.object_key,
        version.storage_object.size_bytes,
        version.storage_object.checksum,
        version.storage_object.mime_type,
        version.storage_object.e_tag,
        "media:route:1",
    )
    owner = MediaOwnerService(lambda: uow)
    with pytest.raises(StorageValidationError, match="must be verified"):
        await owner.enqueue_dispatch(
            MediaDispatchAdmission(
                project.id,
                "uploaded_source",
                version.id,
                version.revision,
                version.content_hash,
                {
                    "projectId": ref.project_id,
                    "profileId": ref.profile_id,
                    "bucket": ref.bucket,
                    "objectKey": ref.object_key,
                    "sizeBytes": ref.size_bytes,
                    "checksum": ref.checksum,
                    "mimeType": ref.mime_type,
                    "etag": ref.etag,
                    "operationKey": ref.operation_key,
                    "verified": False,
                },
                "media:route:1",
            )
        )
    event = await owner.enqueue_dispatch(
        MediaDispatchAdmission(
            project.id,
            "uploaded_source",
            version.id,
            version.revision,
            version.content_hash,
            ref,
            "media:route:1",
        )
    )
    assert {
        "executionRoute": event["executionRoute"],
        "workflowType": event["workflowType"],
        "taskQueue": event["taskQueue"],
        "schemaVersion": event["schemaVersion"],
    } == {
        "executionRoute": MEDIA_ROUTE,
        "workflowType": MEDIA_WORKFLOW_TYPE,
        "taskQueue": MEDIA_TASK_QUEUE,
        "schemaVersion": MEDIA_SCHEMA_VERSION,
    }

    starter = Starter()
    assert await MediaDispatchService(lambda: uow).dispatch_pending(starter) == {
        "dispatched": 1,
        "failed": 0,
    }
    assert len(starter.calls) == 1
    assert starter.calls[0]["workflowId"] == "media-" + hashlib.sha256(b"media:route:1").hexdigest()
    event_index = next(
        index
        for index, item in enumerate(uow.outbox_events)
        if item.get("operationKey") == "media:route:1"
    )
    assert uow.outbox_events[event_index]["status"] == "dispatched"

    # A restarted dispatcher must re-check source owner state before starting
    # a workflow; changing authorization leaves the durable event pending.
    asset.authorization_status = "pending"
    uow.outbox_events[event_index] = {**event, "status": "pending"}
    before = list(starter.calls)
    assert await MediaDispatchService(lambda: uow).dispatch_pending(starter) == {
        "dispatched": 0,
        "failed": 1,
    }
    assert starter.calls == before
    assert uow.outbox_events[event_index]["status"] == "pending"
    assert "lastDiagnostic" in uow.outbox_events[event_index]

    uow.outbox_events[event_index] = {
        **uow.outbox_events[event_index],
        "status": "pending",
        "operation": "unknown",
    }
    before = list(starter.calls)
    assert await MediaDispatchService(lambda: uow).dispatch_pending(starter) == {
        "dispatched": 0,
        "failed": 1,
    }
    assert starter.calls == before
    assert uow.outbox_events[event_index]["status"] == "pending"

    uow.outbox_events[event_index] = {
        **uow.outbox_events[event_index],
        "operation": "inspect",
        "taskQueue": "foreign-queue",
    }
    assert await MediaDispatchService(lambda: uow).dispatch_pending(starter) == {
        "dispatched": 0,
        "failed": 1,
    }
    assert starter.calls == before
    assert uow.outbox_events[event_index]["status"] == "pending"

    uow.outbox_events[event_index] = {
        **uow.outbox_events[event_index],
        "taskQueue": MEDIA_TASK_QUEUE,
        "resourceAdmission": None,
    }
    assert await MediaDispatchService(lambda: uow).dispatch_pending(starter) == {
        "dispatched": 0,
        "failed": 1,
    }
    assert starter.calls == before
    assert uow.outbox_events[event_index]["status"] == "pending"

    uow.outbox_events[event_index] = {
        **event,
        "status": "pending",
    }
    unavailable = Starter(unavailable=True)
    assert await MediaDispatchService(lambda: uow).dispatch_pending(unavailable) == {
        "dispatched": 0,
        "failed": 1,
    }
    assert uow.outbox_events[event_index]["status"] == "pending"


@pytest.mark.asyncio
async def test_temporal_media_starter_rejects_frozen_queue_schema_or_route_drift() -> None:
    class Client:
        def __init__(self) -> None:
            self.calls: list[dict[str, object]] = []

        async def start_workflow(
            self, workflow: str, arg: object, *, id: str, task_queue: str
        ) -> object:
            self.calls.append({"workflow": workflow, "arg": arg, "id": id, "taskQueue": task_queue})
            return object()

    client = Client()
    payload: dict[str, object] = {
        "workflowId": "media-stable-id",
        "executionRoute": MEDIA_ROUTE,
        "workflowType": MEDIA_WORKFLOW_TYPE,
        "taskQueue": MEDIA_TASK_QUEUE,
        "schemaVersion": MEDIA_SCHEMA_VERSION,
    }
    assert await TemporalMediaStarter(client).start(payload) == "started"
    assert len(client.calls) == 1
    for key, value in (
        ("taskQueue", "other"),
        ("schemaVersion", "2"),
        ("executionRoute", "legacy"),
    ):
        with pytest.raises(ValidationDomainError, match="media launch"):
            await TemporalMediaStarter(client).start({**payload, key: value})
    assert len(client.calls) == 1


@pytest.mark.asyncio
@pytest.mark.parametrize(
    "variant", ["pending", "rejected", "retake", "stale", "foreign", "current"]
)
async def test_generated_media_admission_variants_leave_all_downstream_owner_facts_unchanged(
    variant: str,
) -> None:
    uow = InMemoryUnitOfWork()
    project, episode, version = await _asset_owner(uow)
    candidate = next(iter(uow.video_take_candidates.values()))
    if variant in {"pending", "rejected", "retake"}:
        uow.video_take_candidates[candidate.id] = replace(candidate, status=variant)
    elif variant == "stale":
        uow.video_take_candidates[candidate.id] = replace(
            candidate, asset_version_revision=candidate.asset_version_revision + 1
        )
    elif variant == "foreign":
        uow.video_take_candidates[candidate.id] = replace(candidate, project_id="foreign-project")
    # The accepted candidate has no matching Scene/Shot current in the current variant.
    ref = StoredObjectRef(
        project.id,
        "local",
        "workspace",
        version.storage_object.object_key,
        version.storage_object.size_bytes,
        version.storage_object.checksum,
        version.storage_object.mime_type,
        version.storage_object.e_tag,
        f"media:generated:{variant}",
    )
    service = MediaOwnerService(lambda: uow)
    before_outbox = list(uow.outbox_events)

    with pytest.raises(ValidationDomainError, match="generated media (candidate|shot scope)"):
        await service.enqueue_dispatch(
            MediaDispatchAdmission(
                project.id,
                "generated_candidate",
                version.id,
                version.revision,
                version.content_hash,
                ref,
                f"media:generated:{variant}",
                episode_id=episode.id,
                shot_id="missing-current-shot",
                candidate_id=candidate.id,
            )
        )

    assert uow.outbox_events == before_outbox
    assert uow.media_inspections == {}
    assert uow.media_derivatives == {}
    assert uow.timeline_cuts == {}
    assert uow.export_batches == {}


@pytest.mark.asyncio
async def test_media_derivative_activity_verifies_each_output_before_marking_ready(
    tmp_path: Path,
) -> None:
    """A derivative is ready only after its concrete bounded object verifies."""
    uow = InMemoryUnitOfWork()
    project, _episode, version = await _asset_owner(uow)
    storage = LocalWorkspaceAdapter(tmp_path / "workspace")
    storage.put(version.storage_object.object_key, b"source", correlation_id="source")
    owner = MediaOwnerService(lambda: uow)
    inspection = await owner.record_inspection(
        RecordInspectionCommand(
            project.id,
            version.id,
            version.revision,
            version.content_hash,
            "inspect:derivatives",
            _metadata(version.content_hash),
            "ffprobe",
            "7.1",
        )
    )
    outputs = {
        kind: storage.put(
            f"projects/{project.id}/derivatives/{kind}.bin",
            kind.encode(),
            correlation_id=f"derive:{kind}",
        )
        for kind in ("proxy", "thumbnail", "keyframe_index", "waveform")
    }

    class Inspector:
        def derive(self, *_args: object, **_kwargs: object) -> tuple[dict[str, object], ...]:
            return tuple(
                {
                    "kind": kind,
                    "status": "succeeded",
                    "objectRef": stored.object_ref,
                    "checksum": stored.checksum,
                    "sizeBytes": stored.size_bytes,
                    "toolVersion": "7.1",
                    "metadata": {"schema": f"{kind}/v1"},
                    "retentionPolicy": "project-retention",
                    "retentionVersion": "7",
                    "licenseStatus": "approved",
                    "hold": False,
                }
                for kind, stored in outputs.items()
            )

    configure_media_activities(MediaActivityDependencies(owner, storage, Inspector()))
    result = await media_derivative(
        {
            "projectId": project.id,
            "inspectionId": inspection.id,
            "operationKey": "derive:all",
            "storedObjectRef": {
                "projectId": project.id,
                "profileId": "local",
                "bucket": "workspace",
                "objectKey": version.storage_object.object_key,
                "sizeBytes": version.storage_object.size_bytes,
                "checksum": version.storage_object.checksum,
                "mimeType": version.storage_object.mime_type,
                "operationKey": "derive:all",
            },
        }
    )

    assert len(result["derivativeIds"]) == 4
    derivatives = {item.kind: item for item in uow.media_derivatives.values()}
    assert set(derivatives) == {"proxy", "thumbnail", "keyframe_index", "waveform"}
    for kind, stored in outputs.items():
        derivative = derivatives[kind]
        assert derivative.status == "ready"
        assert derivative.checksum == stored.checksum
        assert derivative.size_bytes == stored.size_bytes
        assert derivative.object_ref == {
            "profileId": "local",
            "objectKey": stored.object_ref.removeprefix("workspace://"),
            "operationKey": f"derive:all:{kind}",
        }
        assert derivative.retention_policy == "project-retention"
        assert derivative.retention_version == "7"
        assert derivative.license_status == "approved"
        assert derivative.hold is False


@pytest.mark.asyncio
async def test_media_derivative_activity_records_failure_without_mutating_current_owner(
    tmp_path: Path,
) -> None:
    uow = InMemoryUnitOfWork()
    project, _episode, version = await _asset_owner(uow)
    storage = LocalWorkspaceAdapter(tmp_path / "workspace")
    storage.put(version.storage_object.object_key, b"source", correlation_id="source")
    owner = MediaOwnerService(lambda: uow)
    inspection = await owner.record_inspection(
        RecordInspectionCommand(
            project.id,
            version.id,
            version.revision,
            version.content_hash,
            "inspect:failure",
            _metadata(version.content_hash),
            "ffprobe",
            "7.1",
        )
    )

    class FailingInspector:
        def derive(self, *_args: object, **_kwargs: object) -> tuple[dict[str, object], ...]:
            return (
                {
                    "kind": "waveform",
                    "status": "failed",
                    "toolVersion": "7.1",
                    "diagnostic": "ffmpeg derivative failed",
                    "metadata": {"schema": "waveform/v1"},
                },
            )

    configure_media_activities(MediaActivityDependencies(owner, storage, FailingInspector()))
    result = await media_derivative(
        {
            "projectId": project.id,
            "inspectionId": inspection.id,
            "operationKey": "derive:failure",
            "storedObjectRef": {
                "projectId": project.id,
                "profileId": "local",
                "bucket": "workspace",
                "objectKey": version.storage_object.object_key,
                "sizeBytes": version.storage_object.size_bytes,
                "checksum": version.storage_object.checksum,
                "mimeType": version.storage_object.mime_type,
                "operationKey": "derive:failure",
            },
        }
    )

    assert len(result["derivativeIds"]) == 1
    derivative = next(iter(uow.media_derivatives.values()))
    assert derivative.kind == "waveform"
    assert derivative.status == "failed"
    assert derivative.raw_diagnostic == "ffmpeg derivative failed"
    assert uow.shots == {}


@pytest.mark.asyncio
async def test_media_render_requires_content_verified_duration_codec_and_container(
    tmp_path: Path,
) -> None:
    workspace = tmp_path / "render"
    workspace.mkdir()
    output = workspace / "opaque-output"

    class Renderer:
        def render(self, _request: RenderRequest, _workspace: Path) -> RenderResult:
            output.write_bytes(b"not-derived-from-extension")
            return RenderResult(
                output,
                "rendered",
                0,
                LoudnessMeasurement(-14.0, -1.0, "test", "1"),
            )

        def inspect_output(
            self, output_path: Path, checked_workspace: Path
        ) -> RenderOutputInspection:
            assert output_path == output
            assert checked_workspace == workspace
            return RenderOutputInspection("mp4", "h264", "aac", 1.0)

    configure_media_activities(
        MediaActivityDependencies(
            MediaOwnerService(lambda: InMemoryUnitOfWork()), None, None, Renderer()
        )
    )
    payload = {
        "workspace": str(workspace),
        "renderRequest": {
            "inputPaths": [str(workspace / "input")],
            "outputPath": str(output),
            "width": 1920,
            "height": 1080,
        },
        "expectedOutput": {
            "durationSeconds": 1.0,
            "container": "mp4",
            "videoCodec": "h264",
            "audioCodec": "aac",
        },
    }

    assert (await media_render(payload))["status"] == "succeeded"
    incomplete = {**payload, "expectedOutput": {"durationSeconds": 1.0}}
    assert (await media_render(incomplete))["diagnostic"] == "render_output_expectation_incomplete"


def _version_for_render() -> TimelineVersion:
    first = {
        "id": "00000000-0000-4000-8000-000000000201",
        "assetVersionId": "00000000-0000-4000-8000-000000000301",
        "assetVersionRevision": 0,
        "assetVersionHash": "a" * 64,
        "derivativeFingerprint": "b" * 64,
        "derivativeStatus": "ready",
        "inFrame": 0,
        "outFrame": 30,
        "durationFrames": 30,
        "timelineStart": 0,
        "transition": {"type": "cut", "durationFrames": 0},
    }
    second = {
        **first,
        "id": "00000000-0000-4000-8000-000000000202",
        "assetVersionId": "00000000-0000-4000-8000-000000000302",
        "timelineStart": 25,
        "transition": {"type": "crossfade", "durationFrames": 5},
    }
    cue = {
        "id": "00000000-0000-4000-8000-000000000401",
        "track": "music",
        "assetVersionId": "00000000-0000-4000-8000-000000000402",
        "assetVersionRevision": 0,
        "assetVersionHash": "c" * 64,
        "startFrame": 0,
        "durationFrames": 55,
        "trigger": "manual",
        "triggerRef": None,
        "priority": 10,
        "continuityRefs": [],
        "gainDb": -2.0,
        "mute": False,
        "solo": False,
        "fadeInFrames": 3,
        "fadeOutFrames": 3,
        "authorizationStatus": "authorized",
        "licenseStatus": "approved",
    }
    return TimelineVersion(
        "00000000-0000-4000-8000-000000000002",
        3,
        "Published",
        {
            "schema_version": "1.0.0",
            "clips": [first, second],
            "soundCues": [cue],
            "captions": [
                {
                    "id": "00000000-0000-4000-8000-000000000501",
                    "text": "Hello",
                    "startFrame": 0,
                    "endFrame": 30,
                }
            ],
            "ducking": {
                "enabled": True,
                "dialogueIntervals": [[10, 20]],
                "attenuationDb": 9.0,
                "attackFrames": 3,
                "releaseFrames": 6,
                "targetTracks": ["music"],
            },
        },
        "00000000-0000-4000-8000-000000000001",
        id="00000000-0000-4000-8000-000000000102",
    )


def test_canonical_render_plan_srt_ducking_crossfade_and_parity_gate() -> None:
    version = _version_for_render()
    plan = compile_render_plan(version)
    preview = compile_preview_manifest(plan, 3)
    graph, video_map, audio_map = compile_ffmpeg_filter_graph(plan)
    assert preview["renderPlanHash"] == plan.render_plan_hash
    assert "xfade=transition=fade" in graph
    assert "loudnorm=I=-14:TP=-1" in graph
    assert "volume='if(between" in graph
    assert video_map.startswith("[vjoin") and audio_map == "[aout]"
    assert b"00:00:00,000 --> 00:00:01,000" in render_srt(plan)
    assert (
        verify_parity(
            plan.render_plan_hash,
            plan.render_plan_hash,
            ssim=0.98,
            duration_delta_frames=1,
            caption_delta_frames=-1,
            audio_delta_frames=0,
        )["status"]
        == "passed"
    )
    with pytest.raises(ValidationDomainError, match="parity"):
        verify_parity(
            plan.render_plan_hash,
            "f" * 64,
            ssim=1.0,
            duration_delta_frames=0,
            caption_delta_frames=0,
            audio_delta_frames=0,
        )


class CountingRenderer(MockFfmpegRenderAdapter):
    def __init__(self) -> None:
        self.render_count = 0

    def render(self, request: object, workspace: Path) -> object:
        self.render_count += 1
        return super().render(request, workspace)


class FailOnceStorage:
    def __init__(self, target: LocalWorkspaceAdapter) -> None:
        self.target = target
        self.complete_count = 0
        self.failed = False

    def __getattr__(self, name: str) -> object:
        return getattr(self.target, name)

    def complete_multipart(self, *args: object, **kwargs: object) -> object:
        self.complete_count += 1
        if self.complete_count == 2 and not self.failed:
            self.failed = True
            raise RuntimeError("storage response unknown")
        return self.target.complete_multipart(*args, **kwargs)


class BoundedStorage(FailOnceStorage):
    def __init__(self, target: LocalWorkspaceAdapter) -> None:
        super().__init__(target)
        self.part_sizes: list[int] = []

    def capability(self, profile_revision: int = 1) -> StorageCapability:
        return StorageCapability(profile_revision, 1, 4, 10_000, 1024 * 1024)

    def upload_part(self, *args: object, **kwargs: object) -> object:
        content = args[2]
        assert isinstance(content, bytes)
        self.part_sizes.append(len(content))
        return self.target.upload_part(*args, **kwargs)

    def complete_multipart(self, *args: object, **kwargs: object) -> object:
        return self.target.complete_multipart(*args, **kwargs)


class DriftingCapabilityStorage:
    def __init__(self, target: LocalWorkspaceAdapter) -> None:
        self.target = target
        self.drifted = False

    def __getattr__(self, name: str) -> object:
        return getattr(self.target, name)

    @property
    def adapter_key(self) -> str:
        return self.target.adapter_key

    def capability(self, profile_revision: int = 1) -> StorageCapability:
        current = self.target.capability(profile_revision)
        if not self.drifted:
            return current
        return StorageCapability(
            current.profile_revision,
            current.min_part_size_bytes,
            current.max_part_size_bytes // 2,
            current.max_part_count,
            current.max_object_size_bytes,
        )


class RetryableReadStorage:
    def __init__(self, target: LocalWorkspaceAdapter) -> None:
        self.target = target

    def __getattr__(self, name: str) -> object:
        return getattr(self.target, name)

    def iter_chunks(self, object_ref: str, chunk_size: int = 1024 * 1024) -> object:
        raise StorageRetryableError("temporary storage read failure")


async def _worker_owner(
    storage: LocalWorkspaceAdapter,
    renderer_identity: dict[str, object] | None = None,
) -> tuple[InMemoryUnitOfWork, object, object, object, ExportService]:
    uow = InMemoryUnitOfWork()
    project, episode, version = await _asset_owner(uow)
    clip = {
        "id": "00000000-0000-4000-8000-000000000201",
        "assetVersionId": version.id,
        "assetVersionRevision": version.revision,
        "assetVersionHash": version.content_hash,
        "derivativeFingerprint": "d" * 64,
        "derivativeStatus": "ready",
        "inFrame": 0,
        "outFrame": 30,
        "durationFrames": 30,
        "timelineStart": 0,
        "transition": {"type": "cut", "durationFrames": 0},
    }
    timeline_version = TimelineVersion(
        episode.id,
        1,
        "Published",
        {
            "schema_version": "1.0.0",
            "clips": [clip],
            "soundCues": [],
            "captions": [],
            "ducking": None,
        },
        project.id,
    )
    uow.timeline_versions[timeline_version.id] = timeline_version
    storage.put(f"workspace://{version.storage_object.object_key}", b"source", "seed")
    exports = ExportService(
        lambda: uow,
        MockFfmpegRenderAdapter(),
        storage=storage,
        renderer_identity=renderer_identity,
    )
    batch = await exports.create_batch(
        project.id,
        [
            {
                "episodeId": episode.id,
                "timelineVersionId": timeline_version.id,
                "timelineVersionRevision": 1,
                "outputBaseName": "episode-01",
            }
        ],
        "light",
        "worker-batch",
        storage_profile_id="local-test-offline",
        storage_profile_revision=1,
    )
    return uow, project, episode, batch, exports


@pytest.mark.asyncio
async def test_explicit_composed_renderer_identity_survives_export_snapshot_and_replay(
    tmp_path: Path,
) -> None:
    storage = LocalWorkspaceAdapter(tmp_path / "storage")
    renderer_identity = {
        "profileId": "renderer-profile",
        "profileRevision": 4,
        "capabilitySnapshotId": "catalog-render-snapshot",
        "capabilityRevision": 9,
    }
    uow, project, _episode, batch, _exports = await _worker_owner(
        storage, renderer_identity=renderer_identity
    )
    snapshot = batch.jobs[0].execution_snapshot
    assert snapshot is not None
    assert snapshot.renderer_identity["profileId"] == "renderer-profile"
    assert snapshot.renderer_identity["profileRevision"] == 4
    assert snapshot.renderer_identity["capabilitySnapshotId"] == "catalog-render-snapshot"
    assert snapshot.renderer_identity["capabilityRevision"] == 9
    assert len(str(snapshot.renderer_identity["snapshotId"])) == 64

    renderer = CountingRenderer()
    worker = EpisodeExportWorker(
        lambda: uow,
        renderer,
        storage,
        renderer_identity=renderer_identity,
    )
    result = await worker.execute(
        ExecuteExportJobCommand(project.id, batch.jobs[0].id, tmp_path / "render", "identity")
    )
    assert result["status"] == "succeeded"
    assert renderer.render_count == 1


@pytest.mark.asyncio
async def test_export_worker_packages_three_artifacts_and_is_idempotent(tmp_path: Path) -> None:
    storage = LocalWorkspaceAdapter(tmp_path / "storage")
    uow, project, episode, batch, _exports = await _worker_owner(storage)
    renderer = CountingRenderer()
    worker = EpisodeExportWorker(lambda: uow, renderer, storage)
    workspace = tmp_path / "render"
    workspace.mkdir()
    command_value = ExecuteExportJobCommand(
        project.id,
        batch.jobs[0].id,
        workspace,
        "worker-1",
    )
    result = await worker.execute(command_value)
    assert result["status"] == "succeeded" and renderer.render_count == 1
    assert {item.status for item in batch.jobs[0].artifacts} == {"verified"}
    assert (await worker.execute(command_value))["reconciled"] is True
    assert renderer.render_count == 1
    expected_base = f"episode-01-{episode.id}-{batch.jobs[0].timeline_version_id}"
    assert (workspace / f"{expected_base}.mp4").is_file()
    assert (workspace / f"{expected_base}.srt").is_file()
    manifest_path = workspace / f"{expected_base}.light.json"
    manifest = json.loads(manifest_path.read_text())
    assert manifest["exportProfile"] == "light"
    assert {item["artifactType"] for item in manifest["references"]} == {"mp4", "srt"}
    assert not (workspace / "episode.mp4").exists()


@pytest.mark.asyncio
async def test_media_storage_terminal_handoff_uses_typed_owner_commands_and_is_retry_safe(
    tmp_path: Path,
) -> None:
    storage = LocalWorkspaceAdapter(tmp_path / "storage")
    content = b"verified terminal object"
    stored = storage.put("projects/p/exports/object.mp4", content, "media-terminal")

    class AssetOwner:
        def __init__(self) -> None:
            self.commands: list[object] = []

        async def get_reservation(self, _project_id: str, _reservation_id: str) -> object:
            return type(
                "Reservation",
                (),
                {"revision": 1, "operation_key": "asset-op", "storage_profile_id": "local"},
            )()

        async def complete_reservation(self, command: object) -> object:
            self.commands.append(command)
            return type("Version", (), {"id": "version-registered"})()

    class ExportOwner:
        def __init__(self) -> None:
            self.calls: list[dict[str, object]] = []
            self.artifact = type(
                "Artifact",
                (),
                {
                    "id": "artifact-registered",
                    "status": "verified",
                    "artifact_type": "mp4",
                    "size_bytes": stored.size_bytes,
                    "checksum": stored.checksum,
                    "operation_key": "export-op",
                    "mime_type": "video/mp4",
                    "storage_object_ref": {
                        "project_id": "p",
                        "profile_id": "local",
                        "bucket": "workspace",
                        "object_key": "projects/p/exports/object.mp4",
                        "size_bytes": stored.size_bytes,
                        "checksum": stored.checksum,
                        "mime_type": "video/mp4",
                        "operation_key": "export-op",
                        "verified": True,
                    },
                },
            )()

        async def register_artifact(self, **kwargs: object) -> object:
            self.calls.append(kwargs)
            raise RevisionConflictError("job-1", 4, 5)

        async def get_job(self, _project_id: str, _job_id: str) -> object:
            return type("Job", (), {"artifacts": [self.artifact]})()

    assets = AssetOwner()
    exports = ExportOwner()
    configure_media_activities(
        MediaActivityDependencies(
            MediaOwnerService(lambda: InMemoryUnitOfWork()),
            storage,
            None,
            assets=assets,
            exports=exports,
        )
    )
    ref = {
        "projectId": "p",
        "profileId": "local",
        "bucket": "workspace",
        "objectKey": "projects/p/exports/object.mp4",
        "sizeBytes": stored.size_bytes,
        "checksum": stored.checksum,
        "mimeType": "video/mp4",
        "etag": stored.etag,
        "operationKey": "asset-op",
        "verified": True,
    }
    asset_result = await media_storage_terminal_handoff(
        {
            "status": "verified",
            "projectId": "p",
            "operationKey": "asset-op",
            "objectRef": ref,
            "ownerHandoff": {
                "owner": "assets",
                "reservationId": "reservation-1",
                "contentHash": stored.checksum,
                "operationKey": "asset-op",
                "reservationRevision": 1,
            },
        }
    )
    assert asset_result == {
        "status": "registered",
        "owner": "assets",
        "assetVersionId": "version-registered",
    }
    assert len(assets.commands) == 1
    retry = await media_storage_terminal_handoff(
        {
            "status": "verified",
            "projectId": "p",
            "operationKey": "asset-op",
            "objectRef": ref,
            "ownerHandoff": {
                "owner": "assets",
                "reservationId": "reservation-1",
                "contentHash": stored.checksum,
                "operationKey": "asset-op",
                "reservationRevision": 1,
            },
        }
    )
    assert retry == asset_result and len(assets.commands) == 2

    export_result = await media_storage_terminal_handoff(
        {
            "status": "verified",
            "projectId": "p",
            "operationKey": "export-op",
            "objectRef": {**ref, "operationKey": "export-op"},
            "ownerHandoff": {
                "owner": "export",
                "jobId": "job-1",
                "artifactType": "mp4",
                "expectedRevision": 4,
                "storageProfileRevision": 2,
                "operationKey": "export-op",
                "packagingPhase": "registering",
            },
        }
    )
    assert export_result == {
        "status": "registered",
        "owner": "export",
        "artifactId": "artifact-registered",
        "artifactStatus": "verified",
    }
    assert exports.calls[0]["stored_object"].verified is True


@pytest.mark.asyncio
async def test_media_storage_terminal_handoff_fails_closed_without_explicit_target(
    tmp_path: Path,
) -> None:
    storage = LocalWorkspaceAdapter(tmp_path / "storage")
    stored = storage.put("projects/p/object.bin", b"object", "media-terminal-missing")
    configure_media_activities(
        MediaActivityDependencies(MediaOwnerService(lambda: InMemoryUnitOfWork()), storage, None)
    )
    with pytest.raises(ValidationDomainError, match="owner handoff target is required"):
        await media_storage_terminal_handoff(
            {
                "status": "verified",
                "projectId": "p",
                "operationKey": "missing-target",
                "objectRef": {
                    "projectId": "p",
                    "profileId": "local",
                    "bucket": "workspace",
                    "objectKey": "projects/p/object.bin",
                    "sizeBytes": stored.size_bytes,
                    "checksum": stored.checksum,
                    "mimeType": "application/octet-stream",
                    "etag": stored.etag,
                    "operationKey": "missing-target",
                    "verified": True,
                },
            }
        )


@pytest.mark.asyncio
async def test_media_storage_upload_returns_complete_verified_typed_reference(
    tmp_path: Path,
) -> None:
    output = tmp_path / "render.mp4"
    output.write_bytes(b"rendered")
    storage = LocalWorkspaceAdapter(tmp_path / "storage")
    configure_media_activities(
        MediaActivityDependencies(MediaOwnerService(lambda: InMemoryUnitOfWork()), storage, None)
    )

    result = await media_storage_upload(
        {
            "projectId": "p",
            "operationKey": "export-op",
            "outputPath": str(output),
            "objectKey": "projects/p/exports/render.mp4",
        }
    )

    assert result["status"] == "verified"
    reference = result["objectRef"]
    assert isinstance(reference, dict)
    assert reference["mimeType"] == "video/mp4"
    assert reference["verified"] is True


@pytest.mark.asyncio
async def test_export_worker_revalidates_frozen_resource_admission_before_render(
    tmp_path: Path,
) -> None:
    storage = LocalWorkspaceAdapter(tmp_path / "storage")
    uow, project, episode, batch, _exports = await _worker_owner(storage)
    snapshot = batch.jobs[0].execution_snapshot
    assert snapshot is not None
    resource = RuntimeResourceSnapshot(
        cpu_count=4,
        available_concurrency=4,
        memory_available_bytes=2_048,
        memory_limit_bytes=4_096,
        disk_free_bytes=8 * 1024 * 1024,
        disk_total_bytes=16 * 1024 * 1024,
        captured_at="2026-08-26T00:00:00+00:00",
    )
    initial = OperationsResilienceCoordinator(resource, capacity_snapshot(resource, project.id))
    frozen = initial.freeze(
        project.id,
        "export.render",
        f"export:{project.id}:worker-batch:{episode.id}:{snapshot.timeline_version_id}",
    )
    batch.jobs[0].execution_snapshot = replace(
        snapshot,
        admission_refs=admission_refs(frozen),
        snapshot_hash="",
    )
    changed = replace(resource, revision=2)
    renderer = CountingRenderer()
    worker = EpisodeExportWorker(
        lambda: uow,
        renderer,
        storage,
        resilience=OperationsResilienceCoordinator(changed, capacity_snapshot(changed, project.id)),
    )

    with pytest.raises(ValidationDomainError, match="resource_snapshot_stale"):
        await worker.execute(
            ExecuteExportJobCommand(project.id, batch.jobs[0].id, tmp_path / "render", "stale")
        )

    assert renderer.render_count == 0


@pytest.mark.asyncio
async def test_export_worker_reconciles_partial_upload_without_rerender(tmp_path: Path) -> None:
    target = LocalWorkspaceAdapter(tmp_path / "storage")
    uow, project, episode, batch, _exports = await _worker_owner(target)
    renderer = CountingRenderer()
    storage = FailOnceStorage(target)
    worker = EpisodeExportWorker(lambda: uow, renderer, storage)
    workspace = tmp_path / "render"
    workspace.mkdir()
    command_value = ExecuteExportJobCommand(
        project.id,
        batch.jobs[0].id,
        workspace,
        "worker-reconcile",
    )
    with pytest.raises(RuntimeError, match="unknown"):
        await worker.execute(command_value)
    assert batch.jobs[0].status == "packaging" and renderer.render_count == 1
    assert (await worker.execute(command_value))["status"] == "succeeded"
    assert renderer.render_count == 1


@pytest.mark.asyncio
async def test_export_worker_uses_frozen_bounded_multipart_parts(tmp_path: Path) -> None:
    target = LocalWorkspaceAdapter(tmp_path / "storage")
    storage = BoundedStorage(target)
    uow, project, _episode, batch, _exports = await _worker_owner(storage)
    renderer = CountingRenderer()
    worker = EpisodeExportWorker(lambda: uow, renderer, storage)
    workspace = tmp_path / "render"

    result = await worker.execute(
        ExecuteExportJobCommand(project.id, batch.jobs[0].id, workspace, "bounded-parts")
    )

    assert result["status"] == "succeeded"
    assert renderer.render_count == 1
    assert len(storage.part_sizes) > 3
    assert max(storage.part_sizes) <= 4
    source = Path(__file__).parents[1] / "src/video_agent_api/application/export_worker.py"
    assert ".read_bytes(" not in source.read_text(encoding="utf-8")


@pytest.mark.asyncio
async def test_export_worker_fails_job_when_frozen_storage_capability_drifts(
    tmp_path: Path,
) -> None:
    target = LocalWorkspaceAdapter(tmp_path / "storage")
    storage = DriftingCapabilityStorage(target)
    uow, project, _episode, batch, _exports = await _worker_owner(storage)
    storage.drifted = True
    renderer = CountingRenderer()
    worker = EpisodeExportWorker(lambda: uow, renderer, storage)

    with pytest.raises(ValidationDomainError, match="storage capability changed"):
        await worker.execute(
            ExecuteExportJobCommand(
                project.id, batch.jobs[0].id, tmp_path / "render", "storage-drift"
            )
        )

    job = batch.jobs[0]
    assert job.status == "failed"
    assert renderer.render_count == 0
    assert [(item.target_type, item.code) for item in job.diagnostics] == [
        ("storage", "storage_preflight_failed")
    ]


@pytest.mark.asyncio
async def test_export_worker_fails_job_when_frozen_input_content_is_invalid(
    tmp_path: Path,
) -> None:
    storage = LocalWorkspaceAdapter(tmp_path / "storage")
    uow, project, _episode, batch, _exports = await _worker_owner(storage)
    snapshot = batch.jobs[0].execution_snapshot
    assert snapshot is not None
    source = snapshot.inputs[0]
    (storage.root / source.object_key).write_bytes(b"corrupt")
    worker = EpisodeExportWorker(lambda: uow, CountingRenderer(), storage)

    with pytest.raises(ValidationDomainError, match="materialization verification"):
        await worker.execute(
            ExecuteExportJobCommand(
                project.id, batch.jobs[0].id, tmp_path / "render", "input-drift"
            )
        )

    job = batch.jobs[0]
    assert job.status == "failed"
    assert [(item.target_type, item.code) for item in job.diagnostics] == [
        ("storage", "input_materialization_failed")
    ]


@pytest.mark.asyncio
async def test_export_worker_leaves_retryable_storage_read_in_preflight(
    tmp_path: Path,
) -> None:
    target = LocalWorkspaceAdapter(tmp_path / "storage")
    uow, project, _episode, batch, _exports = await _worker_owner(target)
    worker = EpisodeExportWorker(lambda: uow, CountingRenderer(), RetryableReadStorage(target))

    with pytest.raises(StorageRetryableError, match="temporary storage read failure"):
        await worker.execute(
            ExecuteExportJobCommand(
                project.id, batch.jobs[0].id, tmp_path / "render", "retryable-read"
            )
        )

    job = batch.jobs[0]
    assert job.status == "preflighting"
    assert job.diagnostics == []


@pytest.mark.asyncio
async def test_export_worker_fails_job_when_execution_snapshot_is_missing(
    tmp_path: Path,
) -> None:
    storage = LocalWorkspaceAdapter(tmp_path / "storage")
    uow, project, _episode, batch, _exports = await _worker_owner(storage)
    batch.jobs[0].execution_snapshot = None
    worker = EpisodeExportWorker(lambda: uow, CountingRenderer(), storage)

    with pytest.raises(ValidationDomainError, match="execution snapshot is missing"):
        await worker.execute(
            ExecuteExportJobCommand(
                project.id, batch.jobs[0].id, tmp_path / "render", "missing-snapshot"
            )
        )

    job = batch.jobs[0]
    assert job.status == "failed"
    assert [(item.target_type, item.code) for item in job.diagnostics] == [
        ("timeline", "execution_snapshot_invalid")
    ]


def _config(url: str) -> Config:
    root = Path(__file__).parents[1]
    config = Config(str(root / "alembic.ini"))
    config.set_main_option("script_location", str(root / "alembic"))
    config.set_main_option("sqlalchemy.url", url)
    return config


@pytest.mark.asyncio
async def test_media_owner_sql_restart_round_trip(tmp_path: Path) -> None:
    url = f"sqlite:///{tmp_path / 'media-owner.db'}"
    command.upgrade(_config(url), "head")
    engine = create_async_engine(url.replace("sqlite://", "sqlite+aiosqlite://"))
    factory = make_sqlalchemy_uow_factory(async_sessionmaker(engine, expire_on_commit=False))
    projects = ProjectsEpisodesService(factory)
    project = await projects.create_project("P")
    assets = AssetsService(factory)
    asset = await assets.create_asset(CreateAssetCommand(project.id, "Take", "video"))
    asset.authorization_status, asset.license = "verified", "owned"
    version = await assets.append_version(
        AppendAssetVersionCommand(
            asset.id,
            StorageObject(
                "local",
                "workspace",
                f"projects/{project.id}/take.mp4",
                "video/mp4",
                1024,
                "a" * 64,
            ),
            "b" * 64,
        )
    )
    service = MediaOwnerService(factory)
    inspection = await service.record_inspection(
        RecordInspectionCommand(
            project.id,
            version.id,
            version.revision,
            version.content_hash,
            "inspect:sql",
            _metadata(version.content_hash),
            "ffprobe",
            "7.1",
        )
    )
    derivative = await service.record_derivative(
        RecordDerivativeCommand(
            project.id,
            inspection.id,
            "thumbnail",
            "ready",
            {"frame": 0, "maxWidth": 320},
            "derive:sql:thumbnail",
            "ffmpeg",
            "7.1",
            object_ref={
                "profileId": "local",
                "objectKey": f"projects/{project.id}/thumbnail.jpg",
                "operationKey": "derive:sql:thumbnail",
            },
            checksum="c" * 64,
            size_bytes=128,
        )
    )
    fresh = MediaOwnerService(factory)
    ready = await fresh.ready_derivatives(
        project.id, version.id, version.revision, version.content_hash
    )
    assert ready[0].id == derivative.id
    await engine.dispose()


def test_renderer_probe_checks_every_frozen_capability(monkeypatch: pytest.MonkeyPatch) -> None:
    adapter = SubprocessFfmpegRenderAdapter("/controlled/ffmpeg", "/controlled/ffprobe")

    def output(arguments: tuple[str, ...]) -> str:
        binary, option = arguments[0], arguments[-1]
        if option == "-version":
            return "ffprobe version 7.1\n" if binary.endswith("ffprobe") else "ffmpeg version 7.1\n"
        if option == "-decoders":
            return " V..... h264 H.264\n A..... aac AAC\n"
        if option == "-encoders":
            return " V..... libx264 H.264\n A..... aac AAC\n"
        if option == "-pix_fmts":
            return "IO... yuv420p 3 12\n"
        if option == "-formats":
            return " D  mp4 QuickTime / MOV\n E  mp4 MP4\n"
        raise AssertionError(arguments)

    monkeypatch.setattr(adapter, "_run", output)
    snapshot = adapter.probe()
    assert snapshot.supported
    assert snapshot.ffmpeg_version == "ffmpeg version 7.1"
    assert snapshot.ffprobe_version == "ffprobe version 7.1"

    def missing_aac_encoder(arguments: tuple[str, ...]) -> str:
        value = output(arguments)
        return value.replace(" A..... aac AAC\n", "") if arguments[-1] == "-encoders" else value

    monkeypatch.setattr(adapter, "_run", missing_aac_encoder)
    with pytest.raises(RendererCapabilityUnsupportedError, match="aac-encoder=False"):
        adapter.probe()
    with pytest.raises(RendererUnconfiguredError, match="FFMPEG_PATH and FFPROBE_PATH"):
        SubprocessFfmpegRenderAdapter(None, None).probe()


def test_renderer_executes_an_argument_vector_without_shell(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    adapter = SubprocessFfmpegRenderAdapter("/controlled/ffmpeg", "/controlled/ffprobe")
    monkeypatch.setattr(
        adapter,
        "_run",
        lambda arguments: (
            "ffmpeg version 7.1\n"
            if arguments[-1] == "-version"
            else (
                " V..... h264\n A..... aac\n"
                if arguments[-1] == "-decoders"
                else (
                    " V..... libx264\n A..... aac\n"
                    if arguments[-1] == "-encoders"
                    else (
                        "IO... yuv420p\n" if arguments[-1] == "-pix_fmts" else " D  mp4\n E  mp4\n"
                    )
                )
            )
        ),
    )
    source = tmp_path / "source.mp4"
    output = tmp_path / "output.mp4"
    source.write_bytes(b"source")
    calls: list[tuple[list[str], dict[str, object]]] = []

    def run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
        calls.append((arguments, kwargs))
        if "loudnorm=I=-14:TP=-1:LRA=11:print_format=json" in arguments:
            return subprocess.CompletedProcess(
                arguments,
                0,
                "",
                'diagnostic\n{"input_i":"-14.0","input_tp":"-1.0"}',
            )
        output.write_bytes(b"rendered")
        return subprocess.CompletedProcess(arguments, 0, "", "diagnostic")

    monkeypatch.setattr(subprocess, "run", run)
    request = RenderRequest(
        (source,),
        output,
        "[0:v]null[vout]",
        1080,
        1920,
        video_map="[vout]",
    )
    result = adapter.render(request, tmp_path)
    arguments, kwargs = calls[0]
    assert result.stderr == "diagnostic"
    assert kwargs.get("shell") is None
    assert arguments[0] == "/controlled/ffmpeg"
    assert arguments[arguments.index("-map") + 1] == "[vout]"
    assert arguments.index("lavfi") < arguments.index("-filter_complex")
    assert result.loudness.integrated_lufs == -14.0
    with pytest.raises(ValidationDomainError, match="forbidden"):
        adapter.render(
            RenderRequest((source,), output, "[0:v]null[vout]\nrm", 1080, 1920),
            tmp_path,
        )
