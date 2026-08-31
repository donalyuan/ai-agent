"""Media Worker-owned inspection, derivative, and preview facts."""

from __future__ import annotations

import hashlib
import json
from dataclasses import dataclass, field
from typing import Literal
from uuid import uuid4

from .errors import ValidationDomainError

MediaStatus = Literal["pending", "ready", "failed", "stale"]
DerivativeKind = Literal["proxy", "thumbnail", "keyframe_index", "waveform"]
_STATUSES = {"pending", "ready", "failed", "stale"}
_KINDS = {"proxy", "thumbnail", "keyframe_index", "waveform"}
_METADATA_FIELDS = {
    "mimeType",
    "sizeBytes",
    "checksum",
    "durationFrames",
    "timebase",
    "fpsNumerator",
    "fpsDenominator",
    "frameCount",
    "width",
    "height",
    "videoCodec",
    "pixelFormat",
    "audioTracks",
    "sampleRate",
    "channels",
}


def _hash(value: object, field_name: str) -> str:
    if not isinstance(value, str) or len(value) != 64:
        raise ValidationDomainError(f"{field_name} is invalid")
    try:
        int(value, 16)
    except ValueError as error:
        raise ValidationDomainError(f"{field_name} is invalid") from error
    return value.lower()


def source_fingerprint(asset_version_id: str, revision: int, source_hash: str) -> str:
    return hashlib.sha256(f"{asset_version_id}:{revision}:{source_hash}".encode()).hexdigest()


def _metadata(value: dict[str, object]) -> dict[str, object]:
    if set(value) != _METADATA_FIELDS:
        raise ValidationDomainError("canonical media metadata is incomplete or aliased")
    for name in (
        "sizeBytes",
        "durationFrames",
        "fpsNumerator",
        "fpsDenominator",
        "frameCount",
        "width",
        "height",
        "audioTracks",
        "sampleRate",
        "channels",
    ):
        item = value[name]
        if isinstance(item, bool) or not isinstance(item, int) or item < 0:
            raise ValidationDomainError(f"media metadata {name} is invalid")
    if value["fpsDenominator"] == 0:
        raise ValidationDomainError("media fps denominator is invalid")
    _hash(value["checksum"], "media checksum")
    for name in ("mimeType", "timebase", "videoCodec", "pixelFormat"):
        if not isinstance(value[name], str) or not str(value[name]).strip():
            raise ValidationDomainError(f"media metadata {name} is required")
    return dict(value)


@dataclass(frozen=True, slots=True)
class MediaInspection:
    project_id: str
    asset_version_id: str
    asset_version_revision: int
    source_hash: str
    status: MediaStatus
    metadata: dict[str, object]
    tool: str
    tool_version: str
    operation_key: str
    id: str = field(default_factory=lambda: str(uuid4()))
    revision: int = 1
    schema_version: str = "1.0.0"
    retention_policy: str = "phase-one"
    retention_version: str = "1"
    license_status: str = "approved"
    hold: bool = False
    raw_diagnostic: str | None = None
    admission_refs: dict[str, object] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if (
            not self.project_id
            or not self.asset_version_id
            or self.asset_version_revision < 0
            or self.status not in _STATUSES
            or not self.tool
            or not self.tool_version
            or not self.operation_key
        ):
            raise ValidationDomainError("media inspection identity is invalid")
        object.__setattr__(self, "source_hash", _hash(self.source_hash, "source hash"))
        object.__setattr__(self, "metadata", _metadata(self.metadata))
        object.__setattr__(self, "admission_refs", dict(self.admission_refs))
        if self.metadata["checksum"] != self.source_hash:
            raise ValidationDomainError("ffprobe claimed-vs-observed checksum mismatch")

    @property
    def source_fingerprint(self) -> str:
        return source_fingerprint(
            self.asset_version_id, self.asset_version_revision, self.source_hash
        )


@dataclass(frozen=True, slots=True)
class MediaDerivative:
    project_id: str
    inspection_id: str
    asset_version_id: str
    asset_version_revision: int
    source_hash: str
    source_fingerprint: str
    kind: DerivativeKind
    status: MediaStatus
    parameters: dict[str, object]
    tool: str
    tool_version: str
    operation_key: str
    id: str = field(default_factory=lambda: str(uuid4()))
    schema_version: str = "1.0.0"
    derivative_schema_version: str = "1.0.0"
    object_ref: dict[str, object] | None = None
    checksum: str | None = None
    size_bytes: int | None = None
    retention_policy: str = "phase-one"
    retention_version: str = "1"
    license_status: str = "approved"
    hold: bool = False
    raw_diagnostic: str | None = None
    admission_refs: dict[str, object] = field(default_factory=dict)

    def __post_init__(self) -> None:
        expected = source_fingerprint(
            self.asset_version_id, self.asset_version_revision, self.source_hash
        )
        if (
            not self.project_id
            or not self.inspection_id
            or self.kind not in _KINDS
            or self.status not in _STATUSES
            or self.source_fingerprint != expected
            or not self.tool
            or not self.tool_version
            or not self.operation_key
        ):
            raise ValidationDomainError("media derivative identity is invalid")
        _hash(self.source_hash, "derivative source hash")
        object.__setattr__(self, "admission_refs", dict(self.admission_refs))
        if self.status == "ready" and (
            self.object_ref is None
            or self.checksum is None
            or self.size_bytes is None
            or self.size_bytes < 0
        ):
            raise ValidationDomainError("ready derivative requires a verified bounded output")
        if self.checksum is not None:
            _hash(self.checksum, "derivative checksum")
        if self.object_ref is not None:
            allowed = {"profileId", "objectKey", "operationKey"}
            if set(self.object_ref) != allowed or any(
                not isinstance(self.object_ref[name], str) or not self.object_ref[name]
                for name in allowed
            ):
                raise ValidationDomainError("derivative object reference is invalid")


@dataclass(frozen=True, slots=True)
class PreviewArtifact:
    project_id: str
    episode_id: str
    cut_id: str
    cut_revision: int
    timeline_fingerprint: str
    render_plan_hash: str
    status: MediaStatus
    proxy_derivative_ids: tuple[str, ...]
    id: str = field(default_factory=lambda: str(uuid4()))
    schema_version: str = "1.0.0"
    raw_diagnostic: str | None = None
    admission_refs: dict[str, object] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if (
            not self.project_id
            or not self.episode_id
            or not self.cut_id
            or self.cut_revision < 1
            or self.status not in _STATUSES
            or len(self.proxy_derivative_ids) != len(set(self.proxy_derivative_ids))
        ):
            raise ValidationDomainError("preview artifact identity is invalid")
        _hash(self.timeline_fingerprint, "timeline fingerprint")
        _hash(self.render_plan_hash, "render plan hash")
        object.__setattr__(self, "admission_refs", dict(self.admission_refs))

    def matches(self, cut_revision: int, timeline_fingerprint: str, render_plan_hash: str) -> bool:
        return (
            self.status == "ready"
            and self.cut_revision == cut_revision
            and self.timeline_fingerprint == timeline_fingerprint
            and self.render_plan_hash == render_plan_hash
        )


def canonical_hash(value: object) -> str:
    return hashlib.sha256(
        json.dumps(value, sort_keys=True, separators=(",", ":"), default=str).encode()
    ).hexdigest()
