from pathlib import Path
from io import BytesIO
import hashlib
import json
import subprocess

import pytest

from video_worker.real_work_generation import (
    LocalWorkGenerationStorage,
    _image_type,
    _download_video,
    _select_tts_language,
    _seedance_generate_audio,
    _seedance_image_roles,
    build_real_compose_command,
    build_real_silent_compose_command,
    RealWorkProvider,
)
from video_worker.model_registry import SpeechModelRuntimeConfig
from video_worker.model_registry import VideoModelRuntimeConfig
from video_worker.seedance import SeedanceProviderError
from video_worker.speech_generation import SpeechProviderError, TtsSynthesisResult
from video_worker.tos_staging import StagedMediaObject
from video_worker.work_generation import (
    RealWorkGenerationLimits,
    UnknownSubmissionError,
    WorkGenerationConfigurationError,
    WorkStep,
)


def test_chinese_narration_requires_declared_chinese_voice_language():
    with pytest.raises(WorkGenerationConfigurationError, match="不支持中文"):
        _select_tts_language("这是中文旁白", [{"Language": "en"}])

    assert _select_tts_language(
        "这是中文旁白", [{"Language": "zh-cn"}]
    ) == "zh-cn"


def test_local_storage_rejects_traversal_and_non_asset_urls(tmp_path):
    storage = LocalWorkGenerationStorage(tmp_path)

    with pytest.raises(WorkGenerationConfigurationError):
        storage.source_path("/assets/../secret")
    with pytest.raises(WorkGenerationConfigurationError):
        storage.source_path("https://internal.example/file.png")


def test_image_signature_must_match_extension(tmp_path):
    path = Path(tmp_path / "reference.jpg")

    with pytest.raises(WorkGenerationConfigurationError, match="不匹配"):
        _image_type(path, b"\x89PNG\r\n\x1a\nfixture")


def test_seedance_image_roles_follow_locked_model_family():
    assert _seedance_image_roles("doubao-seedance-1-5-pro-251215", 1) == [
        "first_frame"
    ]
    assert _seedance_image_roles("doubao-seedance-1-5-pro-251215", 2) == [
        "first_frame",
        "last_frame",
    ]
    assert _seedance_image_roles("doubao-seedance-2-0-260128", 6) == [
        "reference_image"
    ] * 6
    with pytest.raises(WorkGenerationConfigurationError, match="最多 2 张"):
        _seedance_image_roles("doubao-seedance-1-5-pro-251215", 3)


def test_real_compose_produces_h264_and_aac_from_silent_video(tmp_path):
    video = tmp_path / "segment.mp4"
    audio = tmp_path / "tts.wav"
    subtitle = tmp_path / "subtitles.srt"
    output = tmp_path / "final.mp4"
    subprocess.run(
        [
            "ffmpeg", "-y", "-f", "lavfi", "-i",
            "testsrc2=size=320x180:rate=24:duration=4",
            "-an", "-c:v", "libx264", "-pix_fmt", "yuv420p", str(video),
        ],
        check=True,
        capture_output=True,
    )
    subprocess.run(
        [
            "ffmpeg", "-y", "-f", "lavfi", "-i",
            "sine=frequency=440:sample_rate=24000:duration=4", str(audio),
        ],
        check=True,
        capture_output=True,
    )
    subtitle.write_text("1\n00:00:00,000 --> 00:00:03,500\n真实作品\n", encoding="utf-8")

    subprocess.run(
        build_real_compose_command([video], audio, subtitle, output, 4),
        check=True,
        capture_output=True,
    )
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_streams", "-of", "json", str(output)],
        check=True,
        capture_output=True,
        text=True,
    )
    streams = json.loads(probe.stdout)["streams"]

    assert any(item["codec_type"] == "video" and item["codec_name"] == "h264" for item in streams)
    assert any(item["codec_type"] == "audio" and item["codec_name"] == "aac" for item in streams)


def test_silent_mode_disables_seedance_audio_and_adds_silent_aac(tmp_path):
    video = tmp_path / "segment.mp4"
    output = tmp_path / "silent-final.mp4"
    subprocess.run(
        [
            "ffmpeg", "-y", "-f", "lavfi", "-i",
            "testsrc2=size=320x180:rate=24:duration=4",
            "-an", "-c:v", "libx264", "-pix_fmt", "yuv420p", str(video),
        ],
        check=True,
        capture_output=True,
    )

    assert _seedance_generate_audio("silent") is False
    assert _seedance_generate_audio("independent_tts") is False
    assert _seedance_generate_audio("seedance_original") is True
    subprocess.run(
        build_real_silent_compose_command([video], output, 4),
        check=True,
        capture_output=True,
    )
    probe = subprocess.run(
        ["ffprobe", "-v", "error", "-show_streams", "-of", "json", str(output)],
        check=True,
        capture_output=True,
        text=True,
    )
    streams = json.loads(probe.stdout)["streams"]

    assert any(item["codec_type"] == "video" and item["codec_name"] == "h264" for item in streams)
    assert any(item["codec_type"] == "audio" and item["codec_name"] == "aac" for item in streams)


def _silent_video_bytes(tmp_path) -> bytes:
    video = tmp_path / "download-source.mp4"
    subprocess.run(
        [
            "ffmpeg", "-y", "-f", "lavfi", "-i",
            "testsrc2=size=320x180:rate=24:duration=4",
            "-an", "-c:v", "libx264", "-pix_fmt", "yuv420p", str(video),
        ],
        check=True,
        capture_output=True,
    )
    return video.read_bytes()


def test_video_download_enforces_size_and_media_validation(tmp_path, monkeypatch):
    content = _silent_video_bytes(tmp_path)

    class Response(BytesIO):
        def __enter__(self):
            return self

        def __exit__(self, *_args):
            self.close()

    monkeypatch.setattr(
        "video_worker.real_work_generation.urllib_request.urlopen",
        lambda *_args, **_kwargs: Response(content),
    )
    output = tmp_path / "downloaded.mp4"
    inspection = _download_video("https://result.example/video.mp4", output, len(content) + 1)
    assert inspection["video_codec"] == "h264"
    assert output.is_file()

    with pytest.raises(WorkGenerationConfigurationError, match="大小限制"):
        _download_video("https://result.example/video.mp4", tmp_path / "large.mp4", 10)

    monkeypatch.setattr(
        "video_worker.real_work_generation.urllib_request.urlopen",
        lambda *_args, **_kwargs: Response(b"not-video"),
    )
    with pytest.raises(WorkGenerationConfigurationError, match="有效媒体"):
        _download_video("https://result.example/video.mp4", tmp_path / "invalid.mp4", 100)


class CapturingStore:
    def __init__(self):
        self.calls = []

    def register_generated_material(self, step, **kwargs):
        self.calls.append(kwargs)
        step.result_material_ids = [str(kwargs["material_id"])]


class TtsRegistry:
    def __init__(self, config):
        self.config = config

    def resolve_speech(self, model_id, protocol, version):
        assert (model_id, protocol, version) == ("tts-model", "volcengine_tts_v3", 2)
        return self.config


def _tts_step() -> WorkStep:
    return WorkStep(
        step_id="257b8705-b200-4dd8-8a7b-91ea9c0e1d74",
        run_id="9fd81164-d187-4690-a199-454265b66e0a",
        step_type="tts",
        run_model_snapshot={
            "tts_model_id": "tts-model",
            "tts_registry_version": 2,
            "tts_api_protocol": "volcengine_tts_v3",
        },
        voice_snapshot={"voice_type": "zh_voice", "catalog_version": 6},
        voice_languages=[{"Language": "zh-cn"}],
        voice_catalog_version=6,
        voice_available=True,
        work_input_snapshot={
            "narration_override": "覆盖后的精简旁白。",
            "scenes": [{"sequence": 1, "narration": "不应发送的原旁白。"}],
        },
        run_resource_usage={
            "video_task_count": 1,
            "video_seconds": 15,
            "tts_characters": 8,
            "asr_seconds": 0,
        },
        project_id="27636a3a-fd82-4472-942d-c63d04361ec6",
        work_title="测试作品",
    )


def _speech_config() -> SpeechModelRuntimeConfig:
    return SpeechModelRuntimeConfig(
        model_id="tts-model",
        display_name="TTS",
        provider_name="火山引擎",
        api_protocol="volcengine_tts_v3",
        protocol_version="v3",
        auth_scheme="api_key",
        request_base_url="https://openspeech.bytedance.com/api/v3",
        upstream_model="doubao-seed-tts-2.0",
        api_key="secret",
        timeout_seconds=120,
        settings={
            "resource_id": "seed-tts-2.0",
            "default_audio_format": "mp3",
            "default_sample_rate": 24000,
        },
        registry_version=2,
    )


def test_real_tts_calls_provider_once_and_registers_audio(tmp_path):
    audio = tmp_path / "fixture.mp3"
    subprocess.run(
        ["ffmpeg", "-y", "-f", "lavfi", "-i", "sine=duration=1", str(audio)],
        check=True,
        capture_output=True,
    )
    calls = []

    class TtsProvider:
        def synthesize(self, request):
            calls.append(request)
            return TtsSynthesisResult(audio.read_bytes(), "mp3", [], "log-id")

    store = CapturingStore()
    provider = RealWorkProvider(
        store,
        TtsRegistry(_speech_config()),
        RealWorkGenerationLimits({"9fd81164-d187-4690-a199-454265b66e0a"}),
        LocalWorkGenerationStorage(tmp_path / "assets"),
        tts_factory=lambda _config: TtsProvider(),
    )
    step = _tts_step()

    upstream_id = provider.submit(step)

    assert upstream_id.startswith("local-tts-")
    assert len(calls) == 1
    assert calls[0].language == "zh-cn"
    assert calls[0].text == "覆盖后的精简旁白。"
    assert store.calls[0]["artifact_role"] == "tts_audio"


def test_real_tts_never_automatically_retries_uncertain_failure(tmp_path):
    calls = []

    class FailingTtsProvider:
        def synthesize(self, _request):
            calls.append(1)
            raise TimeoutError("uncertain")

    provider = RealWorkProvider(
        CapturingStore(),
        TtsRegistry(_speech_config()),
        RealWorkGenerationLimits({"9fd81164-d187-4690-a199-454265b66e0a"}),
        LocalWorkGenerationStorage(tmp_path / "assets"),
        tts_factory=lambda _config: FailingTtsProvider(),
    )

    with pytest.raises(UnknownSubmissionError):
        provider.submit(_tts_step())

    assert calls == [1]


def test_real_tts_preserves_definitive_provider_error_for_audit(tmp_path):
    class RejectedTtsProvider:
        def synthesize(self, _request):
            raise SpeechProviderError(
                "tts_http_error",
                "语音供应商返回 HTTP 400",
                retryable=False,
                error_details={
                    "http_status": 400,
                    "provider_error_code": "InvalidParameter",
                    "provider_error_message": "voice is invalid",
                },
                upstream_log_id="safe-log-id",
            )

    provider = RealWorkProvider(
        CapturingStore(),
        TtsRegistry(_speech_config()),
        RealWorkGenerationLimits({"9fd81164-d187-4690-a199-454265b66e0a"}),
        LocalWorkGenerationStorage(tmp_path / "assets"),
        tts_factory=lambda _config: RejectedTtsProvider(),
    )

    with pytest.raises(SpeechProviderError) as captured:
        provider.submit(_tts_step())

    assert captured.value.retryable is False
    assert provider.output_audit() == {
        "error_code": "tts_http_error",
        "retryable": False,
        "error_details": {
            "http_status": 400,
            "provider_error_code": "InvalidParameter",
            "provider_error_message": "voice is invalid",
        },
        "upstream_log_id": "safe-log-id",
    }


def test_definitive_seedance_create_failure_is_audited_and_cleans_staging(tmp_path):
    source = tmp_path / "assets" / "reference.png"
    source.parent.mkdir(parents=True)
    content = b"\x89PNG\r\n\x1a\nfixture"
    source.write_bytes(content)
    cleaned = []

    class VideoRegistry:
        def resolve_video(self, model_id, version):
            assert (model_id, version) == ("video-model", 1)
            return VideoModelRuntimeConfig(
                model_id="video-model",
                display_name="Seedance",
                provider_name="火山引擎",
                api_protocol="volcengine_ark_video",
                protocol_version="v1",
                auth_scheme="bearer",
                request_base_url="https://ark.cn-beijing.volces.com/api/v3",
                upstream_model="doubao-seedance-2-0-test",
                api_key="secret",
                timeout_seconds=300,
                settings={
                    "aspect_ratios": ["16:9"],
                    "resolutions": ["1080p"],
                },
                registry_version=1,
            )

        def resolve_tos_staging(self, config_id, version):
            assert (config_id, version) == ("tos-config", 2)
            return object()

    class Staging:
        def stage_media(self, **_kwargs):
            return StagedMediaObject(
                "prefix/work-generation/reference.png",
                hashlib.sha256(content).hexdigest(),
                len(content),
                "https://tos.example/reference.png?signature=secret",
            )

        def cleanup(self, object_key):
            cleaned.append(object_key)

    class RejectedSeedance:
        def create(self, _request):
            raise SeedanceProviderError(
                "http_error",
                "Seedance HTTP 404",
                status_code=404,
                provider_code="InvalidEndpointOrModel.NotFound",
                provider_message="model not found",
                request_id="req-ark-404",
            )

    step = WorkStep(
        step_id="fcb6c3df-152f-46ed-ab3b-45c263f7ad05",
        run_id="3cc84910-74ed-4549-b64e-599256f75f42",
        step_type="video_segment",
        input_snapshot={
            "reference_image_ids": ["reference-material"],
            "duration_seconds": 15,
            "prompt": "镜头动作",
        },
        parameter_snapshot={
            "output": {"aspect_ratio": "16:9", "resolution": "1080p"}
        },
        timeline_snapshot={"audio_mode": "silent"},
        run_model_snapshot={
            "video_model_id": "video-model",
            "video_registry_version": 1,
            "tos_staging_config_id": "tos-config",
            "tos_staging_config_version": 2,
        },
        run_resource_usage={
            "video_task_count": 1,
            "video_seconds": 15,
            "tts_characters": 0,
            "asr_seconds": 0,
        },
        work_input_snapshot={
            "scenes": [
                {
                    "sequence": 1,
                    "image_material_id": "reference-material",
                    "image_url": "/assets/reference.png",
                }
            ]
        },
        project_id="27636a3a-fd82-4472-942d-c63d04361ec6",
        work_title="测试作品",
    )
    provider = RealWorkProvider(
        CapturingStore(),
        VideoRegistry(),
        RealWorkGenerationLimits({step.run_id}),
        LocalWorkGenerationStorage(tmp_path / "assets"),
        seedance_factory=lambda _config: RejectedSeedance(),
        tos_factory=lambda _config: Staging(),
        signed_url_reader=lambda _url, _limit: content,
    )

    with pytest.raises(SeedanceProviderError):
        provider.submit(step)

    assert cleaned == ["prefix/work-generation/reference.png"]
    assert provider.output_audit() == {
        "provider": "volcengine_ark_video",
        "error_code": "http_error",
        "http_status": 404,
        "provider_error_code": "InvalidEndpointOrModel.NotFound",
        "provider_error_message": "model not found",
        "provider_request_id": "req-ark-404",
        "staging_cleanup": "succeeded",
    }
