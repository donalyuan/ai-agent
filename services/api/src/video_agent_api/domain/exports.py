"""Canonical RenderPlan, per-Episode export, artifact, and diagnostic contracts."""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import asdict, dataclass, field
from datetime import UTC, datetime
from typing import Literal
from uuid import uuid4

from .errors import ValidationDomainError

ExportStatus = Literal[
    "queued",
    "preflighting",
    "rendering",
    "packaging",
    "succeeded",
    "failed",
    "cancel_requested",
    "cancelled",
]
ArtifactType = Literal["mp4", "srt", "light_manifest"]
DiagnosticTargetType = Literal[
    "timeline",
    "clip",
    "caption",
    "sound_cue",
    "asset_version",
    "renderer",
    "storage",
    "artifact",
]
_SAFE_NAME = re.compile(r"^[A-Za-z0-9._-]{1,120}$")
_TARGETS = {
    "timeline",
    "clip",
    "caption",
    "sound_cue",
    "asset_version",
    "renderer",
    "storage",
    "artifact",
}


def export_temporal_workflow_id(
    project_id: str,
    batch_id: str,
    job_id: str,
    logical_operation: str,
) -> str:
    if not all((project_id, batch_id, job_id, logical_operation)):
        raise ValidationDomainError("export Temporal workflow scope is incomplete")
    fingerprint = _hash(
        {
            "projectId": project_id,
            "batchId": batch_id,
            "jobId": job_id,
            "logicalOperation": logical_operation,
        }
    )
    return f"episode-export-{fingerprint}"


@dataclass(slots=True)
class ExportDispatchOutbox:
    project_id: str
    batch_id: str
    job_id: str
    logical_operation: str
    id: str = field(default_factory=lambda: str(uuid4()))
    status: Literal["pending", "dispatched"] = "pending"
    attempts: int = 0
    last_error: str | None = None
    dispatched_at: str | None = None
    revision: int = 1
    schema_version: str = "1.0.0"
    workflow_id: str = ""

    def __post_init__(self) -> None:
        expected = export_temporal_workflow_id(
            self.project_id,
            self.batch_id,
            self.job_id,
            self.logical_operation,
        )
        if not self.workflow_id:
            self.workflow_id = expected
        if self.workflow_id != expected or self.status not in {"pending", "dispatched"}:
            raise ValidationDomainError("export dispatch identity or status is invalid")
        if self.attempts < 0 or self.revision < 1:
            raise ValidationDomainError("export dispatch counters are invalid")

    def dispatched(self) -> None:
        if self.status == "dispatched":
            return
        self.attempts += 1
        self.status = "dispatched"
        self.last_error = None
        self.dispatched_at = datetime.now(UTC).isoformat()
        self.revision += 1

    def failed_attempt(self, error: str) -> None:
        if self.status != "pending" or not error:
            raise ValidationDomainError("export dispatch failure state is invalid")
        self.attempts += 1
        self.last_error = error[:4000]
        self.revision += 1


@dataclass(frozen=True, slots=True)
class ExportSettings:
    aspect_ratio: Literal["9:16", "16:9", "1:1"] = "9:16"
    width: int = 1080
    height: int = 1920
    fps: int = 30
    container: Literal["mp4"] = "mp4"
    video_codec: Literal["h264"] = "h264"
    pixel_format: Literal["yuv420p"] = "yuv420p"
    audio_codec: Literal["aac"] = "aac"
    sample_rate: int = 48_000
    subtitle_encoding: Literal["UTF-8"] = "UTF-8"

    def __post_init__(self) -> None:
        dimensions = {"9:16": (1080, 1920), "16:9": (1920, 1080), "1:1": (1080, 1080)}
        if (
            self.aspect_ratio not in dimensions
            or (self.width, self.height) != dimensions[self.aspect_ratio]
            or self.fps != 30
            or self.container != "mp4"
            or self.video_codec != "h264"
            or self.pixel_format != "yuv420p"
            or self.audio_codec != "aac"
            or self.sample_rate != 48_000
            or self.subtitle_encoding != "UTF-8"
        ):
            raise ValidationDomainError("export settings violate the frozen MVP-A contract")


@dataclass(frozen=True, slots=True)
class ExportDiagnosticTarget:
    target_type: DiagnosticTargetType
    project_id: str
    episode_id: str
    timeline_version_id: str | None
    owner_id: str | None
    owner_revision: int | None
    field_path: str | None
    route_token: str
    code: str
    id: str = field(default_factory=lambda: str(uuid4()))
    schema_version: str = "1.0.0"

    def __post_init__(self) -> None:
        if self.target_type not in _TARGETS or not self.project_id or not self.episode_id:
            raise ValidationDomainError("export diagnostic target scope is invalid")
        if self.target_type in {"clip", "caption", "sound_cue", "asset_version", "artifact"}:
            if not self.owner_id or self.owner_revision is None:
                raise ValidationDomainError("export diagnostic owner target is incomplete")
        if len(self.route_token) < 16 or not self.code:
            raise ValidationDomainError("export diagnostic route token or code is invalid")
        if self.field_path and ("[" in self.field_path or "message" in self.field_path.lower()):
            raise ValidationDomainError("message or array positions are not owner targets")


@dataclass(frozen=True, slots=True)
class RenderPlan:
    project_id: str
    episode_id: str
    timeline_version_id: str
    clips: tuple[dict[str, object], ...]
    cues: tuple[dict[str, object], ...]
    captions: tuple[dict[str, object], ...]
    ducking: dict[str, object] | None = None
    settings: ExportSettings = field(default_factory=ExportSettings)
    render_plan_hash: str = ""

    def __post_init__(self) -> None:
        if not self.project_id or not self.episode_id or not self.timeline_version_id:
            raise ValidationDomainError("render plan scope is required")
        if not self.render_plan_hash:
            object.__setattr__(self, "render_plan_hash", _hash(self.canonical_payload()))

    @property
    def fps(self) -> int:
        return self.settings.fps

    @property
    def format(self) -> str:
        return self.settings.container

    def canonical_payload(self) -> dict[str, object]:
        return {
            "projectId": self.project_id,
            "episodeId": self.episode_id,
            "timelineVersionId": self.timeline_version_id,
            "clips": self.clips,
            "soundCues": self.cues,
            "captions": self.captions,
            "ducking": self.ducking,
            "settings": {
                "aspectRatio": self.settings.aspect_ratio,
                "width": self.settings.width,
                "height": self.settings.height,
                "fps": self.settings.fps,
                "container": self.settings.container,
                "videoCodec": self.settings.video_codec,
                "pixelFormat": self.settings.pixel_format,
                "audioCodec": self.settings.audio_codec,
                "sampleRate": self.settings.sample_rate,
                "subtitleEncoding": self.settings.subtitle_encoding,
            },
        }


def _hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), default=str).encode()
    ).hexdigest()


@dataclass(frozen=True, slots=True)
class ExportArtifact:
    export_job_id: str
    artifact_type: ArtifactType
    status: Literal["pending", "verified", "failed", "held"]
    size_bytes: int | None = None
    checksum: str | None = None
    retention_policy: str = "phase-one"
    retention_version: str = "1"
    hold: bool = False
    id: str = field(default_factory=lambda: str(uuid4()))
    storage_object_ref: dict[str, object] | None = None
    operation_key: str | None = None
    storage_profile_revision: int | None = None
    mime_type: str | None = None
    license_status: str = "approved"
    expires_at: str = "2999-12-31T23:59:59+00:00"

    def __post_init__(self) -> None:
        mime_by_type = {
            "mp4": "video/mp4",
            "srt": "application/x-subrip",
            "light_manifest": "application/json",
        }
        if self.artifact_type not in mime_by_type or self.status not in {
            "pending",
            "verified",
            "failed",
            "held",
        }:
            raise ValidationDomainError("export artifact type or status is invalid")
        if self.size_bytes is not None and self.size_bytes < 0:
            raise ValidationDomainError("export artifact size is invalid")
        if self.checksum is not None and (
            len(self.checksum) != 64
            or any(char not in "0123456789abcdef" for char in self.checksum)
        ):
            raise ValidationDomainError("export artifact checksum is invalid")
        if self.mime_type is not None and self.mime_type != mime_by_type[self.artifact_type]:
            raise ValidationDomainError("export artifact MIME does not match its type")
        try:
            expires = datetime.fromisoformat(self.expires_at.replace("Z", "+00:00"))
        except ValueError as error:
            raise ValidationDomainError("export artifact expiry is invalid") from error
        if expires.tzinfo is None:
            raise ValidationDomainError("export artifact expiry must include timezone")

    def downloadable(self, now: datetime) -> bool:
        expires = datetime.fromisoformat(self.expires_at.replace("Z", "+00:00"))
        return (
            self.status == "verified"
            and not self.hold
            and self.license_status == "approved"
            and expires.astimezone(UTC) > now.astimezone(UTC)
            and self.storage_object_ref is not None
        )


@dataclass(frozen=True, slots=True)
class ExportInputSnapshot:
    asset_version_id: str
    asset_version_revision: int
    asset_version_hash: str
    object_key: str
    mime_type: str
    size_bytes: int
    checksum: str
    bucket: str
    storage_provider: str

    def __post_init__(self) -> None:
        if (
            not self.asset_version_id
            or self.asset_version_revision < 0
            or len(self.asset_version_hash) != 64
            or not self.object_key
            or self.object_key.startswith(("/", "\\"))
            or "/" not in self.mime_type
            or self.size_bytes < 0
            or len(self.checksum) != 64
            or not self.bucket
            or not self.storage_provider
        ):
            raise ValidationDomainError("export input snapshot is invalid")


@dataclass(frozen=True, slots=True)
class ExportExecutionSnapshot:
    project_id: str
    episode_id: str
    timeline_version_id: str
    timeline_version_revision: int
    timeline_version_hash: str
    render_plan_hash: str
    output_base_name: str
    storage_profile_id: str
    storage_profile_revision: int
    storage_profile_snapshot: dict[str, object]
    storage_profile_snapshot_hash: str
    storage_capability: dict[str, int]
    renderer_capability: dict[str, object]
    render_plan: dict[str, object]
    inputs: tuple[ExportInputSnapshot, ...]
    audit_facts: dict[str, object]
    schema_version: str = "1.0.0"
    snapshot_hash: str = ""

    def __post_init__(self) -> None:
        required_capability = {
            "profileRevision",
            "minPartSizeBytes",
            "maxPartSizeBytes",
            "maxPartCount",
            "maxObjectSizeBytes",
        }
        required_renderer = {
            "ffmpegVersion",
            "ffprobeVersion",
            "h264Decoder",
            "h264Encoder",
            "aacDecoder",
            "aacEncoder",
            "yuv420p",
            "mp4Muxer",
            "mp4Demuxer",
        }
        if (
            not all((self.project_id, self.episode_id, self.timeline_version_id))
            or self.timeline_version_revision < 1
            or len(self.timeline_version_hash) != 64
            or len(self.render_plan_hash) != 64
            or not _SAFE_NAME.fullmatch(self.output_base_name)
            or not self.storage_profile_id
            or self.storage_profile_revision < 1
            or not self.storage_profile_snapshot
            or len(self.storage_profile_snapshot_hash) != 64
            or set(self.storage_capability) != required_capability
            or set(self.renderer_capability) != required_renderer
            or not self.render_plan
            or not self.inputs
            or not self.audit_facts
        ):
            raise ValidationDomainError("export execution snapshot is incomplete")
        if any(
            isinstance(value, bool) or not isinstance(value, int) or value < 1
            for value in self.storage_capability.values()
        ):
            raise ValidationDomainError("export storage capability snapshot is invalid")
        if any(
            not isinstance(self.renderer_capability[field], bool)
            for field in required_renderer - {"ffmpegVersion", "ffprobeVersion"}
        ) or any(
            not isinstance(self.renderer_capability[field], str)
            or not str(self.renderer_capability[field]).strip()
            for field in {"ffmpegVersion", "ffprobeVersion"}
        ):
            raise ValidationDomainError("export renderer capability snapshot is invalid")
        if not all(
            bool(self.renderer_capability[field])
            for field in required_renderer - {"ffmpegVersion", "ffprobeVersion"}
        ):
            raise ValidationDomainError("export renderer capability snapshot is unsupported")
        if _hash({**self.storage_profile_snapshot, "capability": self.storage_capability}) != (
            self.storage_profile_snapshot_hash
        ):
            raise ValidationDomainError("export StorageProfile snapshot hash is invalid")
        if _hash(self.render_plan) != self.render_plan_hash:
            raise ValidationDomainError("export RenderPlan snapshot hash is invalid")
        expected_hash = _hash(
            {
                "projectId": self.project_id,
                "episodeId": self.episode_id,
                "timelineVersionId": self.timeline_version_id,
                "timelineVersionRevision": self.timeline_version_revision,
                "timelineVersionHash": self.timeline_version_hash,
                "renderPlanHash": self.render_plan_hash,
                "outputBaseName": self.output_base_name,
                "storageProfileId": self.storage_profile_id,
                "storageProfileRevision": self.storage_profile_revision,
                "storageProfileSnapshot": self.storage_profile_snapshot,
                "storageProfileSnapshotHash": self.storage_profile_snapshot_hash,
                "storageCapability": self.storage_capability,
                "rendererCapability": self.renderer_capability,
                "renderPlan": self.render_plan,
                "inputs": [asdict(item) for item in self.inputs],
                "auditFacts": self.audit_facts,
                "schemaVersion": self.schema_version,
            }
        )
        if not self.snapshot_hash:
            object.__setattr__(self, "snapshot_hash", expected_hash)
        elif self.snapshot_hash != expected_hash:
            raise ValidationDomainError("export execution snapshot hash is invalid")


@dataclass(slots=True)
class ExportJob:
    project_id: str
    episode_id: str
    timeline_version_id: str
    status: ExportStatus = "queued"
    revision: int = 1
    packaging_phase: Literal["uploading", "verifying", "registering"] | None = None
    artifacts: list[ExportArtifact] = field(default_factory=list)
    id: str = field(default_factory=lambda: str(uuid4()))
    batch_id: str = ""
    logical_operation: str = "initial"
    render_plan_hash: str | None = None
    renderer_diagnostic: str | None = None
    diagnostics: list[ExportDiagnosticTarget] = field(default_factory=list)
    execution_snapshot: ExportExecutionSnapshot | None = None

    def transition(self, target: ExportStatus) -> None:
        allowed = {
            "queued": {"preflighting", "cancel_requested"},
            "preflighting": {"rendering", "failed", "cancel_requested"},
            "rendering": {"packaging", "failed", "cancel_requested"},
            "packaging": {"succeeded", "failed", "cancel_requested"},
            "cancel_requested": {"cancelled"},
            "succeeded": set(),
            "failed": set(),
            "cancelled": set(),
        }
        if target not in allowed[self.status]:
            raise ValidationDomainError(f"invalid export transition: {self.status}->{target}")
        if target == "succeeded":
            verified = {item.artifact_type for item in self.artifacts if item.status == "verified"}
            if verified != {"mp4", "srt", "light_manifest"}:
                raise ValidationDomainError("all three verified artifacts are required")
        self.status = target
        self.revision += 1

    def set_packaging_phase(self, phase: Literal["uploading", "verifying", "registering"]) -> None:
        if self.status != "packaging" or phase not in {"uploading", "verifying", "registering"}:
            raise ValidationDomainError("packaging phase requires packaging status")
        self.packaging_phase = phase
        self.revision += 1

    def append_artifact(self, artifact: ExportArtifact) -> None:
        if artifact.export_job_id != self.id:
            raise ValidationDomainError("export artifact belongs to another job")
        if any(item.artifact_type == artifact.artifact_type for item in self.artifacts):
            raise ValidationDomainError("duplicate export artifact type")
        self.artifacts.append(artifact)
        self.revision += 1


@dataclass(frozen=True, slots=True)
class EpisodeExportSelection:
    episode_id: str
    timeline_version_id: str
    timeline_version_revision: int
    output_base_name: str

    def __post_init__(self) -> None:
        if (
            not self.episode_id
            or not self.timeline_version_id
            or self.timeline_version_revision != 1
            or not _SAFE_NAME.fullmatch(self.output_base_name)
        ):
            raise ValidationDomainError("export selection is invalid")


@dataclass(slots=True)
class EpisodeExportBatch:
    project_id: str
    selections: tuple[EpisodeExportSelection, ...]
    export_profile: Literal["light", "portable"] = "light"
    idempotency_key: str = ""
    status: Literal["queued", "succeeded", "partially_failed", "failed"] = "queued"
    jobs: list[ExportJob] = field(default_factory=list)
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    schema_version: str = "1.0.0"
    settings: ExportSettings = field(default_factory=ExportSettings)

    def __post_init__(self) -> None:
        episode_ids = [item.episode_id for item in self.selections]
        names = [item.output_base_name for item in self.selections]
        if not self.project_id or not self.selections or len(episode_ids) != len(set(episode_ids)):
            raise ValidationDomainError("export selection must be non-empty and unique")
        if len(names) != len(set(names)):
            raise ValidationDomainError("export output names must be unique")
        if self.export_profile != "light":
            if self.export_profile == "portable":
                raise ValidationDomainError("portable export is MVP-B")
            raise ValidationDomainError("exportProfile is invalid")
        if not self.idempotency_key:
            self.idempotency_key = self.id

    def summarize(self) -> None:
        statuses = {job.status for job in self.jobs}
        if statuses == {"succeeded"}:
            self.status = "succeeded"
        elif "failed" in statuses and "succeeded" in statuses:
            self.status = "partially_failed"
        elif statuses == {"failed"}:
            self.status = "failed"
        else:
            self.status = "queued"
        self.revision += 1
