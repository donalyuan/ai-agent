"""Controlled renderer boundary shared by preview and final export."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Protocol


@dataclass(frozen=True, slots=True)
class RendererCapabilitySnapshot:
    ffmpeg_version: str
    ffprobe_version: str
    h264_decoder: bool
    h264_encoder: bool
    aac_decoder: bool
    aac_encoder: bool
    yuv420p: bool
    mp4_muxer: bool
    mp4_demuxer: bool
    raw_diagnostic: str

    @property
    def supported(self) -> bool:
        return all(
            (
                self.h264_decoder,
                self.h264_encoder,
                self.aac_decoder,
                self.aac_encoder,
                self.yuv420p,
                self.mp4_muxer,
                self.mp4_demuxer,
            )
        )


@dataclass(frozen=True, slots=True)
class RenderRequest:
    input_paths: tuple[Path, ...]
    output_path: Path
    filter_graph: str
    width: int
    height: int
    fps: int = 30
    video_map: str = "[vout]"
    audio_map: str | None = None


@dataclass(frozen=True, slots=True)
class LoudnessMeasurement:
    integrated_lufs: float
    true_peak_dbtp: float
    measured_by: str
    measurement_version: str


@dataclass(frozen=True, slots=True)
class RenderResult:
    output_path: Path
    stderr: str
    return_code: int
    loudness: LoudnessMeasurement


class FfmpegRenderPort(Protocol):
    def probe(self) -> RendererCapabilitySnapshot: ...

    def render(self, request: RenderRequest, workspace: Path) -> RenderResult: ...
