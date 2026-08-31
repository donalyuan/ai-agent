"""Explicit, allowlisted ffmpeg/ffprobe subprocess adapter."""

from __future__ import annotations

import json
import math
import os
import re
import subprocess
from pathlib import Path

from video_agent_api.domain.errors import (
    RendererCapabilityUnsupportedError,
    RendererUnconfiguredError,
    ValidationDomainError,
)
from video_agent_api.ports.rendering import (
    LoudnessMeasurement,
    RendererCapabilitySnapshot,
    RenderOutputInspection,
    RenderRequest,
    RenderResult,
)

_TIMEOUT_SECONDS = 20


class SubprocessFfmpegRenderAdapter:
    def __init__(self, ffmpeg_path: str | None, ffprobe_path: str | None) -> None:
        self._ffmpeg_path = ffmpeg_path
        self._ffprobe_path = ffprobe_path

    def probe(self) -> RendererCapabilitySnapshot:
        if not self._ffmpeg_path or not self._ffprobe_path:
            raise RendererUnconfiguredError(
                "renderer_unconfigured: FFMPEG_PATH and FFPROBE_PATH are required"
            )
        try:
            version = self._run((self._ffmpeg_path, "-version"))
            probe_version = self._run((self._ffprobe_path, "-version"))
            decoders = self._run((self._ffmpeg_path, "-hide_banner", "-decoders"))
            encoders = self._run((self._ffmpeg_path, "-hide_banner", "-encoders"))
            pixel_formats = self._run((self._ffmpeg_path, "-hide_banner", "-pix_fmts"))
            formats = self._run((self._ffmpeg_path, "-hide_banner", "-formats"))
        except (OSError, subprocess.SubprocessError) as error:
            raise RendererUnconfiguredError(f"renderer_unconfigured: {error}") from error
        snapshot = RendererCapabilitySnapshot(
            ffmpeg_version=version.splitlines()[0] if version else "unknown",
            ffprobe_version=probe_version.splitlines()[0] if probe_version else "unknown",
            h264_decoder=_codec_available(decoders, "h264"),
            h264_encoder=_codec_available(encoders, "libx264", "h264"),
            aac_decoder=_codec_available(decoders, "aac"),
            aac_encoder=_codec_available(encoders, "aac"),
            yuv420p="yuv420p" in pixel_formats,
            mp4_muxer=_format_available(formats, "E", "mp4"),
            mp4_demuxer=_format_available(formats, "D", "mp4"),
            raw_diagnostic="\n".join((version.splitlines()[0], probe_version.splitlines()[0])),
        )
        if not snapshot.supported:
            raise RendererCapabilityUnsupportedError(
                "renderer_capability_unsupported: "
                f"h264-decoder={snapshot.h264_decoder},h264-encoder={snapshot.h264_encoder},"
                f"aac-decoder={snapshot.aac_decoder},aac-encoder={snapshot.aac_encoder},"
                f"yuv420p={snapshot.yuv420p},mp4-mux={snapshot.mp4_muxer},"
                f"mp4-demux={snapshot.mp4_demuxer}"
            )
        return snapshot

    def render(self, request: RenderRequest, workspace: Path) -> RenderResult:
        capability = self.probe()
        workspace = workspace.resolve()
        if not workspace.is_dir() or request.fps != 30:
            raise ValidationDomainError("renderer workspace or fps is invalid")
        inputs = tuple(path.resolve() for path in request.input_paths)
        output = request.output_path.resolve()
        if not inputs or any(
            not path.is_file() or not path.is_relative_to(workspace) for path in inputs
        ):
            raise ValidationDomainError("renderer input must be an existing workspace file")
        if not output.is_relative_to(workspace) or output.suffix.lower() != ".mp4":
            raise ValidationDomainError("renderer output must be a workspace MP4")
        if any(character in request.filter_graph for character in ("\n", "\r", "\x00")):
            raise ValidationDomainError("renderer filter graph contains forbidden separators")
        arguments: list[str] = [str(self._ffmpeg_path), "-nostdin", "-y"]
        for path in inputs:
            arguments.extend(("-i", str(path)))
        silence_input_index: int | None = None
        if request.audio_map is None:
            silence_input_index = len(inputs)
            arguments.extend(
                (
                    "-f",
                    "lavfi",
                    "-i",
                    "anullsrc=channel_layout=stereo:sample_rate=48000",
                )
            )
        if request.filter_graph:
            arguments.extend(("-filter_complex", request.filter_graph))
        arguments.extend(("-map", request.video_map))
        if request.audio_map is not None:
            arguments.extend(("-map", request.audio_map))
        else:
            arguments.extend(("-map", f"{silence_input_index}:a", "-shortest"))
        arguments.extend(
            (
                "-r",
                "30",
                "-s",
                f"{request.width}x{request.height}",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
                "-ar",
                "48000",
                "-f",
                "mp4",
                str(output),
            )
        )
        completed = subprocess.run(
            arguments,
            check=False,
            cwd=workspace,
            env={"PATH": os.environ.get("PATH", "")},
            capture_output=True,
            text=True,
            timeout=_TIMEOUT_SECONDS,
        )
        if completed.returncode != 0:
            raise ValidationDomainError(f"renderer_failed: {completed.stderr}")
        if not output.is_file() or output.stat().st_size < 1:
            raise ValidationDomainError("renderer_failed: output is missing or empty")
        return RenderResult(
            output,
            completed.stderr,
            completed.returncode,
            self._measure_loudness(output, capability.ffmpeg_version),
        )

    def _measure_loudness(self, output: Path, measurement_version: str) -> LoudnessMeasurement:
        completed = subprocess.run(
            [
                str(self._ffmpeg_path),
                "-nostdin",
                "-hide_banner",
                "-i",
                str(output),
                "-af",
                "loudnorm=I=-14:TP=-1:LRA=11:print_format=json",
                "-f",
                "null",
                "-",
            ],
            check=False,
            env={"PATH": os.environ.get("PATH", "")},
            capture_output=True,
            text=True,
            timeout=_TIMEOUT_SECONDS,
        )
        if completed.returncode != 0:
            raise ValidationDomainError(f"renderer_loudness_measurement_failed: {completed.stderr}")
        blocks = re.findall(r"\{[^{}]+\}", completed.stderr, flags=re.DOTALL)
        for block in reversed(blocks):
            try:
                payload = json.loads(block)
                integrated = float(payload["input_i"])
                true_peak = float(payload["input_tp"])
            except (KeyError, TypeError, ValueError, json.JSONDecodeError):
                continue
            if math.isfinite(integrated) and math.isfinite(true_peak):
                return LoudnessMeasurement(
                    integrated,
                    true_peak,
                    "ffmpeg-loudnorm",
                    measurement_version,
                )
        raise ValidationDomainError(
            "renderer_loudness_measurement_failed: finite loudness JSON is missing"
        )

    def inspect_output(self, output_path: Path, workspace: Path) -> RenderOutputInspection:
        """Read bounded ffprobe fields; filename extensions never establish media identity."""
        workspace = workspace.resolve()
        output = output_path.resolve()
        if not output.is_file() or not output.is_relative_to(workspace):
            raise ValidationDomainError("render_output_invalid:path")
        try:
            completed = subprocess.run(
                [
                    str(self._ffprobe_path),
                    "-v",
                    "error",
                    "-show_entries",
                    "format=format_name,duration:stream=codec_type,codec_name",
                    "-of",
                    "json",
                    str(output),
                ],
                check=False,
                capture_output=True,
                text=True,
                timeout=_TIMEOUT_SECONDS,
            )
        except (OSError, subprocess.SubprocessError) as error:
            raise ValidationDomainError(
                f"render_output_invalid:ffprobe:{type(error).__name__}"
            ) from error
        if completed.returncode != 0 or len(completed.stdout) > 32 * 1024:
            raise ValidationDomainError("render_output_invalid:ffprobe")
        try:
            payload = json.loads(completed.stdout)
            formats = str(payload["format"]["format_name"]).split(",")
            duration = float(payload["format"]["duration"])
            streams = payload["streams"]
            video_codec = next(
                str(stream["codec_name"]) for stream in streams if stream["codec_type"] == "video"
            )
            audio_codec = next(
                str(stream["codec_name"]) for stream in streams if stream["codec_type"] == "audio"
            )
        except (KeyError, StopIteration, TypeError, ValueError, json.JSONDecodeError) as error:
            raise ValidationDomainError("render_output_invalid:media") from error
        if "mp4" not in formats or video_codec != "h264" or audio_codec != "aac":
            raise ValidationDomainError("render_output_invalid:container_or_codec")
        if not math.isfinite(duration) or duration <= 0:
            raise ValidationDomainError("render_output_invalid:duration")
        return RenderOutputInspection("mp4", video_codec, audio_codec, duration)

    def _run(self, arguments: tuple[str, ...]) -> str:
        completed = subprocess.run(
            arguments,
            check=True,
            capture_output=True,
            text=True,
            timeout=_TIMEOUT_SECONDS,
        )
        return completed.stdout + completed.stderr


class MockFfmpegRenderAdapter:
    """Deterministic test-only renderer; production composition never selects it implicitly."""

    def probe(self) -> RendererCapabilitySnapshot:
        return RendererCapabilitySnapshot(
            "mock-ffmpeg",
            "mock-ffprobe",
            True,
            True,
            True,
            True,
            True,
            True,
            True,
            "explicit test adapter",
        )

    def render(self, request: RenderRequest, workspace: Path) -> RenderResult:
        del workspace
        request.output_path.write_bytes(b"mock-mp4")
        return RenderResult(
            request.output_path,
            "mock",
            0,
            LoudnessMeasurement(-14.0, -1.0, "mock-ffmpeg-loudnorm", "1"),
        )

    def inspect_output(self, output_path: Path, workspace: Path) -> RenderOutputInspection:
        if not output_path.is_file() or not output_path.resolve().is_relative_to(
            workspace.resolve()
        ):
            raise ValidationDomainError("render_output_invalid:path")
        return RenderOutputInspection("mp4", "h264", "aac", 1.0)


def _codec_available(output: str, *names: str) -> bool:
    lines = output.lower().splitlines()
    return any(any(name in line.split() for name in names) for line in lines)


def _format_available(output: str, mode: str, name: str) -> bool:
    for line in output.splitlines():
        stripped = line.strip()
        if len(stripped) > 3 and mode in stripped[:2]:
            aliases = {alias for token in stripped[2:].split() for alias in token.split(",")}
            if name in aliases:
                return True
    return False
