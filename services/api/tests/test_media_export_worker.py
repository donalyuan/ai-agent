from __future__ import annotations

import hashlib
import json
import subprocess
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
from video_agent_api.adapters.sqlalchemy import make_sqlalchemy_uow_factory
from video_agent_api.application.assets import (
    AppendAssetVersionCommand,
    AssetsService,
    CreateAssetCommand,
)
from video_agent_api.application.export_worker import (
    EpisodeExportWorker,
    ExecuteExportJobCommand,
)
from video_agent_api.application.exports import ExportService
from video_agent_api.application.media import (
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
from video_agent_api.domain.assets import StorageObject
from video_agent_api.domain.catalog import default_skill_revisions
from video_agent_api.domain.errors import (
    RendererCapabilityUnsupportedError,
    RendererUnconfiguredError,
    ValidationDomainError,
)
from video_agent_api.domain.media import PreviewArtifact
from video_agent_api.domain.provider_ops import ProviderCall
from video_agent_api.domain.runs import WorkflowRun
from video_agent_api.domain.timeline import TimelineCut, TimelineVersion
from video_agent_api.domain.video_generation import VideoOperation, VideoTakeCandidate
from video_agent_api.ports.contracts import StorageCapability, StorageRetryableError
from video_agent_api.ports.rendering import RenderRequest
from video_agent_api.ports.storage import LocalWorkspaceAdapter


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
    service = MediaOwnerService(lambda: uow)
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
    inspection = await service.record_inspection(command_value)
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
    await service.record_preview(preview)
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
    exports = ExportService(lambda: uow, MockFfmpegRenderAdapter(), storage=storage)
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
