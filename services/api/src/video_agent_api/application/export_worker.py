"""Idempotent Media Worker orchestration for one Episode ExportJob."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any, cast

from video_agent_api.application.exports import ExportService
from video_agent_api.application.rendering import (
    build_light_manifest,
    compile_ffmpeg_filter_graph,
    render_plan_from_snapshot,
    render_srt,
)
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.domain.exports import ExportDiagnosticTarget, ExportJob
from video_agent_api.ports.contracts import (
    PartReceipt,
    StorageRetryableError,
    StorageWriteIntent,
    StoredObjectRef,
)
from video_agent_api.ports.rendering import FfmpegRenderPort, RenderRequest


@dataclass(frozen=True, slots=True)
class ExecuteExportJobCommand:
    project_id: str
    job_id: str
    workspace: Path
    correlation_id: str


class EpisodeExportWorker:
    """Run/reconcile a single job without merging Episodes or hiding external failures."""

    def __init__(
        self,
        uow_factory: Any,
        renderer: FfmpegRenderPort,
        storage: Any,
    ) -> None:
        self._uow_factory = uow_factory
        self._renderer = renderer
        self._storage = storage
        self._exports = ExportService(uow_factory, renderer)

    async def execute(self, command: ExecuteExportJobCommand) -> dict[str, object]:
        workspace = command.workspace.resolve()
        workspace.mkdir(parents=True, exist_ok=True)
        job = await self._exports.get_job(command.project_id, command.job_id)
        if job.status == "succeeded":
            return {"jobId": job.id, "status": job.status, "reconciled": True}
        if job.status in {"failed", "cancelled", "cancel_requested"}:
            raise ValidationDomainError("terminal export job requires an explicit member retry")
        if job.status == "queued":
            job = await self._exports.transition_job(
                command.project_id, job.id, "preflighting", job.revision
            )
        try:
            snapshot = job.execution_snapshot
            if snapshot is None:
                raise ValidationDomainError("export execution snapshot is missing")
            if (
                snapshot.project_id != command.project_id
                or snapshot.episode_id != job.episode_id
                or snapshot.timeline_version_id != job.timeline_version_id
            ):
                raise ValidationDomainError("export execution snapshot scope is invalid")
        except Exception as error:
            target = _diagnostic(job, "timeline", "execution_snapshot_invalid")
            await self._exports.record_job_failure(command.project_id, job.id, str(error), target)
            raise
        try:
            input_paths = self._materialize_inputs(snapshot, workspace)
        except StorageRetryableError:
            raise
        except Exception as error:
            target = _diagnostic(job, "storage", "input_materialization_failed")
            await self._exports.record_job_failure(command.project_id, job.id, str(error), target)
            raise
        try:
            capability = self._renderer.probe()
        except Exception as error:
            target = _diagnostic(job, "renderer", "renderer_probe_failed")
            await self._exports.record_job_failure(command.project_id, job.id, str(error), target)
            raise
        try:
            if _renderer_capability_payload(capability) != snapshot.renderer_capability:
                raise ValidationDomainError("renderer capability changed after export submission")
        except Exception as error:
            target = _diagnostic(job, "renderer", "renderer_capability_changed")
            await self._exports.record_job_failure(command.project_id, job.id, str(error), target)
            raise
        try:
            if getattr(self._storage, "adapter_key", None) != snapshot.storage_profile_snapshot.get(
                "adapterKey"
            ):
                raise ValidationDomainError("storage adapter changed after export submission")
            current_storage_capability = self._storage.capability(snapshot.storage_profile_revision)
            if (
                _storage_capability_payload(current_storage_capability)
                != snapshot.storage_capability
            ):
                raise ValidationDomainError("storage capability changed after export submission")
        except Exception as error:
            target = _diagnostic(job, "storage", "storage_preflight_failed")
            await self._exports.record_job_failure(command.project_id, job.id, str(error), target)
            raise
        try:
            plan = render_plan_from_snapshot(snapshot.render_plan, snapshot.render_plan_hash)
            expected_inputs = len(plan.clips) + len(plan.cues)
            if len(input_paths) != expected_inputs:
                raise ValidationDomainError("export inputs do not match the canonical RenderPlan")
            if job.render_plan_hash not in {None, plan.render_plan_hash}:
                raise ValidationDomainError("export RenderPlan changed after preflight")
        except Exception as error:
            target = _diagnostic(job, "timeline", "render_plan_preflight_failed")
            await self._exports.record_job_failure(command.project_id, job.id, str(error), target)
            raise
        from video_agent_api.domain.timeline import TimelineVersion

        version = TimelineVersion(
            snapshot.episode_id,
            1,
            "Frozen export snapshot",
            {
                "schema_version": snapshot.schema_version,
                "clips": list(plan.clips),
                "soundCues": list(plan.cues),
                "captions": list(plan.captions),
                "ducking": plan.ducking,
            },
            snapshot.project_id,
            id=snapshot.timeline_version_id,
            revision=snapshot.timeline_version_revision,
            schema_version=snapshot.schema_version,
        )
        output_base = f"{snapshot.output_base_name}-{job.episode_id}-{job.timeline_version_id}"
        mp4_path = workspace / f"{output_base}.mp4"
        srt_path = workspace / f"{output_base}.srt"
        manifest_path = workspace / f"{output_base}.light.json"
        if job.status == "preflighting":
            job = await self._exports.start_rendering(
                command.project_id, job.id, job.revision, plan.render_plan_hash
            )
        if job.status == "rendering":
            graph, video_map, audio_map = compile_ffmpeg_filter_graph(plan)
            try:
                result = self._renderer.render(
                    RenderRequest(
                        input_paths,
                        mp4_path,
                        graph,
                        plan.settings.width,
                        plan.settings.height,
                        plan.settings.fps,
                        video_map,
                        audio_map,
                    ),
                    workspace,
                )
            except Exception as error:
                target = _diagnostic(job, "renderer", "render_failed")
                await self._exports.record_job_failure(
                    command.project_id, job.id, str(error), target
                )
                raise
            srt_path.write_bytes(render_srt(plan))
            artifacts = {item.artifact_type: item for item in job.artifacts}
            audit_facts = {
                **snapshot.audit_facts,
                "loudness": {
                    "integratedLufs": result.loudness.integrated_lufs,
                    "truePeakDbtp": result.loudness.true_peak_dbtp,
                    "measuredBy": result.loudness.measured_by,
                    "measurementVersion": result.loudness.measurement_version,
                },
            }
            manifest = build_light_manifest(
                plan,
                version,
                audit_facts,
                artifacts["mp4"].id,
                artifacts["srt"].id,
                artifacts["light_manifest"].id,
            )
            manifest_path.write_text(
                json.dumps(manifest, sort_keys=True, separators=(",", ":")),
                encoding="utf-8",
            )
            job = await self._exports.complete_rendering(
                command.project_id,
                job.id,
                job.revision,
                plan.render_plan_hash,
                result.stderr,
            )

        if job.status != "packaging":
            raise ValidationDomainError("export job did not reach packaging")
        outputs = {
            "mp4": (mp4_path, "video/mp4"),
            "srt": (srt_path, "application/x-subrip"),
            "light_manifest": (manifest_path, "application/json"),
        }
        if any(not path.is_file() for path, _mime in outputs.values()):
            raise ValidationDomainError("packaging outputs are unavailable for reconciliation")

        try:
            if job.packaging_phase is None:
                job = await self._exports.set_packaging_phase(
                    command.project_id, job.id, "uploading", job.revision
                )
            object_refs = self._upload_or_reconcile(command, job.id, snapshot, outputs)
            if job.packaging_phase == "uploading":
                job = await self._exports.set_packaging_phase(
                    command.project_id, job.id, "verifying", job.revision
                )
            self._verify_outputs(outputs, object_refs)
            if job.packaging_phase == "verifying":
                job = await self._exports.set_packaging_phase(
                    command.project_id, job.id, "registering", job.revision
                )
            for artifact_type in ("mp4", "srt", "light_manifest"):
                path, _mime = outputs[artifact_type]
                checksum = _file_checksum(path)
                await self._exports.register_artifact(
                    command.project_id,
                    job.id,
                    artifact_type,
                    path.stat().st_size,
                    checksum,
                    True,
                    job.revision,
                    object_refs[artifact_type],
                    snapshot.storage_profile_revision,
                )
                job = await self._exports.get_job(command.project_id, job.id)
        except Exception:
            # Unknown/partial storage responses remain packaging for deterministic reconcile.
            raise
        job = await self._exports.transition_job(
            command.project_id, job.id, "succeeded", job.revision
        )
        return {
            "jobId": job.id,
            "status": job.status,
            "renderPlanHash": plan.render_plan_hash,
            "rendererCapability": {
                "ffmpegVersion": capability.ffmpeg_version,
                "ffprobeVersion": capability.ffprobe_version,
            },
            "artifacts": [
                {"id": item.id, "artifactType": item.artifact_type, "status": item.status}
                for item in job.artifacts
            ],
        }

    def _materialize_inputs(self, snapshot: Any, workspace: Path) -> tuple[Path, ...]:
        paths: list[Path] = []
        for index, source in enumerate(snapshot.inputs):
            suffix = _mime_suffix(source.mime_type)
            path = workspace / f"input-{index:05d}-{source.asset_version_id}{suffix}"
            if path.is_file() and _file_checksum(path) == source.checksum:
                paths.append(path)
                continue
            temporary = path.with_suffix(path.suffix + ".downloading")
            digest = hashlib.sha256()
            size_bytes = 0
            with temporary.open("wb") as target:
                for chunk in self._storage.iter_chunks(source.object_key, 1024 * 1024):
                    target.write(chunk)
                    digest.update(chunk)
                    size_bytes += len(chunk)
            if size_bytes != source.size_bytes or digest.hexdigest() != source.checksum:
                temporary.unlink(missing_ok=True)
                raise ValidationDomainError("export input materialization verification failed")
            temporary.replace(path)
            paths.append(path)
        return tuple(paths)

    def _upload_or_reconcile(
        self,
        command: ExecuteExportJobCommand,
        job_id: str,
        snapshot: Any,
        outputs: dict[str, tuple[Path, str]],
    ) -> dict[str, StoredObjectRef]:
        results: dict[str, StoredObjectRef] = {}
        for artifact_type, (path, mime_type) in outputs.items():
            size_bytes = path.stat().st_size
            checksum = _file_checksum(path)
            capability = _capability_from_snapshot(snapshot.storage_capability)
            profile_id = snapshot.storage_profile_id
            part_size = _select_part_size(size_bytes, capability)
            self._storage.admit_upload(
                size_bytes,
                part_size,
                profile_revision=capability.profile_revision,
            )
            operation_key = f"export-upload:{command.project_id}:{job_id}:{artifact_type}"
            intent = StorageWriteIntent(
                operation_key,
                command.project_id,
                profile_id,
                f"projects/{command.project_id}/exports/{job_id}/{path.name}",
                size_bytes,
                checksum,
                mime_type,
            )
            reconciled = self._storage.reconcile_multipart(intent, command.correlation_id)
            if reconciled is not None:
                results[artifact_type] = reconciled
                continue
            session = self._storage.resume_multipart(intent, command.correlation_id)
            receipts: list[PartReceipt] = []
            with path.open("rb") as source:
                part_number = 1
                while content := source.read(part_size):
                    part_checksum = hashlib.sha256(content).hexdigest()
                    receipt = PartReceipt(part_number, part_checksum, part_checksum, len(content))
                    uploaded = self._storage.upload_part(
                        session, receipt, content, command.correlation_id
                    )
                    receipts.append(uploaded)
                    part_number += 1
            results[artifact_type] = self._storage.complete_multipart(
                session, tuple(receipts), command.correlation_id
            )
        return results

    @staticmethod
    def _verify_outputs(
        outputs: dict[str, tuple[Path, str]], refs: dict[str, StoredObjectRef]
    ) -> None:
        for artifact_type, (path, mime_type) in outputs.items():
            ref = refs[artifact_type]
            if (
                not ref.verified
                or ref.size_bytes != path.stat().st_size
                or ref.checksum != _file_checksum(path)
                or ref.mime_type != mime_type
            ):
                raise ValidationDomainError("export storage verification failed")


def _file_checksum(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def _select_part_size(size_bytes: int, capability: Any) -> int:
    required = max(1, (size_bytes + capability.max_part_count - 1) // capability.max_part_count)
    preferred = 8 * 1024 * 1024
    part_size = max(capability.min_part_size_bytes, required)
    part_size = max(part_size, min(preferred, capability.max_part_size_bytes))
    if part_size > capability.max_part_size_bytes or not capability.supports(size_bytes, part_size):
        raise ValidationDomainError("export artifact exceeds frozen storage capability")
    return part_size


def _capability_from_snapshot(value: dict[str, int]) -> Any:
    from video_agent_api.ports.contracts import StorageCapability

    return StorageCapability(
        value["profileRevision"],
        value["minPartSizeBytes"],
        value["maxPartSizeBytes"],
        value["maxPartCount"],
        value["maxObjectSizeBytes"],
    )


def _storage_capability_payload(value: Any) -> dict[str, int]:
    return {
        "profileRevision": value.profile_revision,
        "minPartSizeBytes": value.min_part_size_bytes,
        "maxPartSizeBytes": value.max_part_size_bytes,
        "maxPartCount": value.max_part_count,
        "maxObjectSizeBytes": value.max_object_size_bytes,
    }


def _renderer_capability_payload(value: Any) -> dict[str, object]:
    return {
        "ffmpegVersion": value.ffmpeg_version,
        "ffprobeVersion": value.ffprobe_version,
        "h264Decoder": value.h264_decoder,
        "h264Encoder": value.h264_encoder,
        "aacDecoder": value.aac_decoder,
        "aacEncoder": value.aac_encoder,
        "yuv420p": value.yuv420p,
        "mp4Muxer": value.mp4_muxer,
        "mp4Demuxer": value.mp4_demuxer,
    }


def _mime_suffix(mime_type: str) -> str:
    return {
        "video/mp4": ".mp4",
        "audio/mpeg": ".mp3",
        "audio/wav": ".wav",
        "audio/x-wav": ".wav",
        "image/png": ".png",
        "image/jpeg": ".jpg",
    }.get(mime_type, ".bin")


def _diagnostic(job: ExportJob, target_type: str, code: str) -> ExportDiagnosticTarget:
    return ExportDiagnosticTarget(
        target_type=cast(Any, target_type),
        project_id=job.project_id,
        episode_id=job.episode_id,
        timeline_version_id=job.timeline_version_id,
        owner_id=None,
        owner_revision=None,
        field_path=None,
        route_token=hashlib.sha256(
            f"{job.project_id}:{job.episode_id}:{target_type}".encode()
        ).hexdigest(),
        code=code,
    )
