from pathlib import Path
import json
import subprocess

import pytest

from video_worker.ffmpeg import MixTrack, build_ffmpeg_command, render_srt


def test_ffmpeg_command_uses_h264_aac_and_external_subtitle(tmp_path: Path):
    video = tmp_path / "scene.mp4"
    subtitle = tmp_path / "scene.srt"
    video.write_bytes(b"video")
    subtitle.write_text("1\n00:00:00,000 --> 00:00:01,000\n字幕\n")
    command = build_ffmpeg_command([video], tmp_path / "out.mp4", 4, subtitle, False)
    assert "libx264" in command
    assert "aac" in command
    assert "subtitles=" not in " ".join(command)
    assert "concat=n=1" in " ".join(command)


def test_ffmpeg_rejects_missing_required_input(tmp_path: Path):
    with pytest.raises(FileNotFoundError):
        build_ffmpeg_command([tmp_path / "missing.mp4"], tmp_path / "out.mp4", 4)


def test_audio_track_must_fit_output(tmp_path: Path):
    audio = tmp_path / "bgm.mp3"
    audio.write_bytes(b"audio")
    with pytest.raises(ValueError):
        build_ffmpeg_command([audio], tmp_path / "out.mp4", 4, tracks=[MixTrack(audio, 0, 5)])


def test_multitrack_mix_and_ducking_are_explicit(tmp_path: Path):
    video = tmp_path / "scene.mp4"
    voice = tmp_path / "voice.mp3"
    video.write_bytes(b"video")
    voice.write_bytes(b"audio")
    command = build_ffmpeg_command([video], tmp_path / "out.mp4", 4, tracks=[MixTrack(voice, 0, 4)], duck_original_audio=True)
    filters = " ".join(command)
    assert "sidechaincompress" in filters
    assert "amix=" in filters


def test_srt_is_generated_independently_from_burn_setting():
    assert render_srt([(0, 1250, "第一句")]) == "1\n00:00:00,000 --> 00:00:01,250\n第一句\n"


def test_media_fixture_outputs_mp4_h264_aac_with_expected_duration(tmp_path: Path):
    source = tmp_path / "source.mp4"
    output = tmp_path / "output.mp4"
    subprocess.run([
        "ffmpeg", "-y", "-f", "lavfi", "-i", "color=c=black:s=320x180:d=4",
        "-f", "lavfi", "-i", "sine=frequency=440:duration=4", "-c:v", "libx264", "-c:a", "aac", str(source),
    ], check=True, capture_output=True)
    subprocess.run(build_ffmpeg_command([source], output, 4), check=True, capture_output=True)
    probe = subprocess.run([
        "ffprobe", "-v", "error", "-show_entries", "format=format_name,duration:stream=codec_type,codec_name",
        "-of", "json", str(output),
    ], check=True, capture_output=True, text=True)
    metadata = json.loads(probe.stdout)
    codecs = {(stream["codec_type"], stream["codec_name"]) for stream in metadata["streams"]}
    assert ("video", "h264") in codecs
    assert ("audio", "aac") in codecs
    assert abs(float(metadata["format"]["duration"]) - 4) < 0.1
