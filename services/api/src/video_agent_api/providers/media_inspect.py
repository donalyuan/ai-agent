"""Media Worker inspection/derivative boundary.

This module intentionally has no Provider or Timeline dependency.  The default implementation
only records a bounded source fingerprint; a real ffprobe/ffmpeg adapter can be injected later.
"""

from __future__ import annotations

import struct
from dataclasses import dataclass
from hashlib import sha256
from pathlib import Path

from video_agent_api.ports.contracts import MediaDerivativePort, MediaInspectPort, StoredObject


@dataclass(slots=True)
class LocalMediaInspector(MediaInspectPort, MediaDerivativePort):
    tool_version: str = "unconfigured"
    derivative_schema_version: str = "1.0.0"
    workspace_root: Path | None = None

    def _path(self, object_ref: str) -> Path | None:
        if self.workspace_root is None or not object_ref.startswith("workspace://"):
            return None
        root = self.workspace_root.resolve()
        path = (root / object_ref.removeprefix("workspace://")).resolve()
        if root not in path.parents:
            return None
        return path

    def inspect(self, stored_object: StoredObject, correlation_id: str) -> dict[str, object]:
        del correlation_id
        path = self._path(stored_object.object_ref)
        if path is not None and path.exists():
            with path.open("rb") as source:
                if source.read(8) == b"\x89PNG\r\n\x1a\n":
                    length, kind = struct.unpack(">I4s", source.read(8))
                    if kind == b"IHDR" and length == 13:
                        width, height = struct.unpack(">II", source.read(8))
                        return {
                            "status": "inspected",
                            "mediaType": "image/png",
                            "width": width,
                            "height": height,
                            "sourceObjectRef": stored_object.object_ref,
                            "sourceChecksum": stored_object.checksum,
                            "sourceSizeBytes": stored_object.size_bytes,
                            "toolVersion": self.tool_version,
                            "derivativeSchemaVersion": self.derivative_schema_version,
                        }
        return {
            "status": "pending",
            "sourceObjectRef": stored_object.object_ref,
            "sourceChecksum": stored_object.checksum,
            "sourceSizeBytes": stored_object.size_bytes,
            "toolVersion": self.tool_version,
            "derivativeSchemaVersion": self.derivative_schema_version,
        }

    def derive(
        self,
        stored_object: StoredObject,
        metadata: dict[str, object],
        correlation_id: str,
    ) -> tuple[dict[str, object], ...]:
        del correlation_id
        source_fingerprint = sha256(
            f"{stored_object.object_ref}:{stored_object.checksum}:{stored_object.size_bytes}".encode()
        ).hexdigest()
        path = self._path(stored_object.object_ref)
        if path is not None and metadata.get("mediaType") == "image/png":
            if self.workspace_root is None:
                raise RuntimeError("workspace root is unavailable")
            root = self.workspace_root.resolve()
            proxy = root / ".derivatives" / source_fingerprint / "proxy.png"
            proxy.parent.mkdir(parents=True, exist_ok=True)
            critical = {b"IHDR", b"PLTE", b"IDAT", b"IEND"}
            with path.open("rb") as source, proxy.open("wb") as target:
                target.write(source.read(8))
                while header := source.read(8):
                    length, kind = struct.unpack(">I4s", header)
                    if kind in critical:
                        target.write(header)
                        remaining = length + 4
                        while remaining:
                            chunk = source.read(min(1024 * 1024, remaining))
                            if not chunk:
                                raise ValueError("truncated PNG chunk")
                            target.write(chunk)
                            remaining -= len(chunk)
                    else:
                        source.seek(length + 4, 1)
                    if kind == b"IEND":
                        break
            proxy_checksum = sha256(proxy.read_bytes()).hexdigest()
            return (
                {
                    "kind": "normalized_metadata",
                    "status": "succeeded",
                    "sourceChecksum": stored_object.checksum,
                    "sourceFingerprint": source_fingerprint,
                    "toolVersion": self.tool_version,
                    "derivativeSchemaVersion": self.derivative_schema_version,
                    "metadata": dict(metadata),
                    "retentionPolicy": "diagnostic-30d",
                    "retentionVersion": "1",
                    "hold": False,
                },
                {
                    "kind": "proxy",
                    "status": "succeeded",
                    "sourceChecksum": stored_object.checksum,
                    "sourceFingerprint": source_fingerprint,
                    "objectRef": f"workspace://{proxy.relative_to(root).as_posix()}",
                    "checksum": proxy_checksum,
                    "sizeBytes": proxy.stat().st_size,
                    "toolVersion": self.tool_version,
                    "derivativeSchemaVersion": self.derivative_schema_version,
                    "metadata": dict(metadata),
                    "retentionPolicy": "diagnostic-30d",
                    "retentionVersion": "1",
                    "hold": False,
                },
            )
        return tuple(
            {
                "kind": kind,
                "status": "pending",
                "sourceChecksum": stored_object.checksum,
                "sourceFingerprint": source_fingerprint,
                "toolVersion": self.tool_version,
                "derivativeSchemaVersion": self.derivative_schema_version,
                "metadata": dict(metadata),
                "retentionPolicy": "diagnostic-30d",
                "retentionVersion": "1",
                "hold": False,
            }
            for kind in ("normalized_metadata", "proxy", "thumbnail", "keyframe_index", "waveform")
        )
