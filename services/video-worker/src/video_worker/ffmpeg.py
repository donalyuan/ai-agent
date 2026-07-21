"""FFmpeg 合成契约；命令构造可测试，执行失败不得伪造成功。"""

from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class MixTrack:
    path: Path
    start_seconds: float
    end_seconds: float
    volume: float = 1.0
    fade_in_seconds: float = 0.0
    fade_out_seconds: float = 0.0

    def validate(self, duration_seconds: float) -> None:
        if not self.path.exists():
            raise FileNotFoundError(str(self.path))
        if self.start_seconds < 0 or self.end_seconds <= self.start_seconds or self.end_seconds > duration_seconds:
            raise ValueError("音频时间范围必须在成片范围内")
        if self.volume < 0:
            raise ValueError("音量不能为负数")


def build_ffmpeg_command(
    video_paths: list[Path],
    output_path: Path,
    duration_seconds: int,
    subtitle_path: Path | None = None,
    burn_subtitles: bool = True,
    tracks: list[MixTrack] | None = None,
    duck_original_audio: bool = False,
) -> list[str]:
    if not video_paths or any(not path.exists() for path in video_paths):
        raise FileNotFoundError("缺少必需视频片段")
    if duration_seconds < 4 or duration_seconds > 60:
        raise ValueError("成片时长必须在 4~60 秒")
    if burn_subtitles and subtitle_path is not None and not subtitle_path.exists():
        raise FileNotFoundError("烧录字幕输入不存在")
    for track in tracks or []:
        track.validate(duration_seconds)
    command = ["ffmpeg", "-y"]
    for path in video_paths:
        command.extend(["-i", str(path)])
    for track in tracks or []:
        command.extend(["-i", str(track.path)])
    filter_parts = []
    video_inputs = "".join(f"[{index}:v:0]" for index in range(len(video_paths)))
    filter_parts.append(f"{video_inputs}concat=n={len(video_paths)}:v=1:a=0[vcat]")
    video_label = "vcat"
    if burn_subtitles and subtitle_path is not None:
        filter_parts.append(f"[vcat]subtitles={subtitle_path}[vout]")
        video_label = "vout"
    track_labels: list[str] = []
    for index, track in enumerate(tracks or []):
        input_index = len(video_paths) + index
        delay_ms = round(track.start_seconds * 1000)
        label = f"track{index}"
        filter_parts.append(
            f"[{input_index}:a:0]atrim=0:{track.end_seconds - track.start_seconds},"
            f"adelay={delay_ms}|{delay_ms},volume={track.volume}[{label}]"
        )
        track_labels.append(label)
    audio_label: str | None = None
    if track_labels:
        if duck_original_audio:
            filter_parts.append(f"[0:a:0][{track_labels[0]}]sidechaincompress=threshold=0.08:ratio=8[ducked]")
            mix_inputs = "[ducked]" + "".join(f"[{label}]" for label in track_labels)
        else:
            mix_inputs = "[0:a:0]" + "".join(f"[{label}]" for label in track_labels)
        filter_parts.append(f"{mix_inputs}amix=inputs={len(track_labels) + 1}:duration=longest[aout]")
        audio_label = "aout"
    command.extend(["-filter_complex", ";".join(filter_parts), "-map", f"[{video_label}]"])
    if audio_label:
        command.extend(["-map", f"[{audio_label}]"])
    else:
        command.extend(["-map", "0:a:0?"])
    command.extend(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-t", str(duration_seconds)])
    command.append(str(output_path))
    return command


def verify_output(path: Path) -> None:
    if not path.exists() or path.stat().st_size <= 0:
        raise RuntimeError("FFmpeg 未产生有效成片")


def render_srt(segments: list[tuple[int, int, str]]) -> str:
    """将毫秒时间轴确定性写成 SRT；无论是否烧录都保存该文本。"""

    lines: list[str] = []
    previous_end = 0
    for index, (start_ms, end_ms, text) in enumerate(segments, start=1):
        if start_ms < previous_end or end_ms <= start_ms or not text.strip():
            raise ValueError("字幕时间轴无效")
        lines.extend([str(index), f"{_srt_time(start_ms)} --> {_srt_time(end_ms)}", text.strip(), ""])
        previous_end = end_ms
    return "\n".join(lines)


def _srt_time(milliseconds: int) -> str:
    hours, remainder = divmod(milliseconds, 3_600_000)
    minutes, remainder = divmod(remainder, 60_000)
    seconds, millis = divmod(remainder, 1_000)
    return f"{hours:02}:{minutes:02}:{seconds:02},{millis:03}"
