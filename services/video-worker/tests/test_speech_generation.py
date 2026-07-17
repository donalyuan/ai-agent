import json
from pathlib import Path
from types import SimpleNamespace
from uuid import UUID

import pytest
import video_worker.speech_generation as speech_generation

from video_worker.speech_generation import (
    AsrQueryResult,
    AudioInspectionResult,
    AudioInspectionError,
    LocalSpeechStorage,
    PendingAudioInspection,
    PendingSpeechTask,
    ProviderHttpResponse,
    SpeechProviderError,
    TimestampWord,
    TtsSynthesisRequest,
    TtsSynthesisResult,
    VolcengineAsrV3Provider,
    VolcengineTtsV3Provider,
    build_srt,
    inspect_audio_file,
    run_next_speech_task,
    run_next_audio_inspection,
)
from video_worker.model_registry import (
    SpeechModelRuntimeConfig,
    SpeechStagingRuntimeConfig,
)
from video_worker.tos_staging import TosStagingError


FIXTURES = Path(__file__).parent / "fixtures" / "speech"


def test_ffprobe_inspection_uses_real_file_digest_and_audio_stream(tmp_path: Path) -> None:
    audio = tmp_path / "fixture.wav"
    audio.write_bytes(b"RIFF" + b"x" * 64)
    calls: list[list[str]] = []

    def runner(command: list[str], timeout: int):
        calls.append(command)
        return SimpleNamespace(
            returncode=0,
            stdout=json.dumps(
                {
                    "format": {
                        "format_name": "wav",
                        "duration": "1.250",
                        "size": "68",
                    },
                    "streams": [
                        {
                            "codec_type": "audio",
                            "codec_name": "pcm_s16le",
                            "sample_rate": "24000",
                            "channels": 1,
                        }
                    ],
                }
            ),
            stderr="",
        )

    result = inspect_audio_file(audio, runner=runner)

    assert calls and calls[0][0] == "ffprobe"
    assert result.duration_ms == 1250
    assert result.file_size_bytes == 68
    assert result.container_format == "wav"
    assert result.audio_codec == "pcm_s16le"
    assert result.sample_rate_hz == 24000
    assert result.channel_count == 1
    assert len(result.source_sha256) == 64


def test_ffprobe_inspection_rejects_media_without_audio_stream(tmp_path: Path) -> None:
    media = tmp_path / "fixture.mp4"
    media.write_bytes(b"fixture")

    def runner(_command: list[str], timeout: int):
        return SimpleNamespace(
            returncode=0,
            stdout=json.dumps(
                {
                    "format": {"format_name": "mp4", "duration": "2.0", "size": "7"},
                    "streams": [{"codec_type": "video", "codec_name": "h264"}],
                }
            ),
            stderr="",
        )

    with pytest.raises(AudioInspectionError) as error:
        inspect_audio_file(media, runner=runner)

    assert error.value.code == "audio_stream_missing"


def test_audio_inspection_worker_persists_probe_result(tmp_path: Path) -> None:
    source = tmp_path / "uploads" / "voice.wav"
    source.parent.mkdir(parents=True)
    source.write_bytes(b"RIFFfixture")

    class Store:
        completed = None
        failed = None

        def claim_next_audio_inspection(self):
            return PendingAudioInspection(
                inspection_id="11111111-1111-4111-8111-111111111111",
                project_id="22222222-2222-4222-8222-222222222222",
                material_id="33333333-3333-4333-8333-333333333333",
                file_url="/assets/uploads/voice.wav",
            )

        def complete_audio_inspection(self, inspection_id, result):
            self.completed = (inspection_id, result)

        def fail_audio_inspection(self, inspection_id, error_code, error_summary):
            self.failed = (inspection_id, error_code, error_summary)

    result = AudioInspectionResult(
        source_sha256="a" * 64,
        file_size_bytes=11,
        duration_ms=800,
        container_format="wav",
        audio_codec="pcm_s16le",
        sample_rate_hz=24000,
        channel_count=1,
    )
    store = Store()

    assert run_next_audio_inspection(
        store,
        LocalSpeechStorage(tmp_path),
        audio_inspector=lambda path: result if path == source else None,
    )
    assert store.completed[1] == result
    assert store.failed is None


def test_tts_v3_provider_uses_confirmed_headers_and_parses_stream_timestamps() -> None:
    line = (FIXTURES / "tts_stream.ndjson").read_bytes().strip()
    captured: dict[str, object] = {}

    def post(url, headers, payload, timeout_seconds):
        captured.update(url=url, headers=headers, payload=payload, timeout=timeout_seconds)
        return ProviderHttpResponse(
            status_code=200,
            headers={"X-Tt-Logid": "fixture-log-id"},
            body_lines=[line],
        )

    provider = VolcengineTtsV3Provider(
        api_key="fixture-api-key",
        base_url="https://openspeech.bytedance.com/api/v3",
        resource_id="seed-tts-2.0",
        timeout_seconds=120,
        http_post=post,
    )
    result = provider.synthesize(
        TtsSynthesisRequest(
            request_id=UUID("22222222-2222-4222-8222-222222222222"),
            text="Hello",
            voice_type="en_fixture_voice",
            language="en-us",
            parameters={"audio_format": "mp3", "sample_rate": 24000},
        )
    )

    assert captured["url"] == (
        "https://openspeech.bytedance.com/api/v3/tts/unidirectional"
    )
    headers = captured["headers"]
    assert headers == {
        "Content-Type": "application/json",
        "X-Api-Key": "fixture-api-key",
        "X-Api-Resource-Id": "seed-tts-2.0",
        "X-Api-Request-Id": "22222222-2222-4222-8222-222222222222",
    }
    payload = captured["payload"]
    assert payload["req_params"]["text"] == "Hello"
    assert payload["req_params"]["speaker"] == "en_fixture_voice"
    assert payload["req_params"]["explicit_language"] == "en-us"
    assert "language" not in payload["req_params"]
    assert "emotion" not in payload["req_params"]
    assert "enable_subtitle" not in json.dumps(payload)
    assert result.audio_content.startswith(b"ID3")
    assert result.audio_format == "mp3"
    assert result.upstream_log_id == "fixture-log-id"
    assert result.words == [
        TimestampWord(text="Hello", start_ms=50, end_ms=350, confidence=0.99)
    ]
    assert "fixture-api-key" not in json.dumps(result.audit_snapshot())


def test_tts_v3_provider_exposes_whitelisted_http_error_details() -> None:
    body = json.dumps(
        {"header": {"code": 45000020, "message": "Permission denied"}}
    ).encode()

    def post(_url, _headers, _payload, _timeout_seconds):
        return ProviderHttpResponse(
            status_code=403,
            headers={"X-Tt-Logid": "20260717150632A1B2C3D4E5F60789"},
            body_lines=body.splitlines(),
            body_content=body,
        )

    provider = VolcengineTtsV3Provider(
        api_key="fixture-api-key",
        base_url="https://openspeech.bytedance.com/api/v3",
        resource_id="seed-tts-2.0",
        timeout_seconds=120,
        http_post=post,
    )

    with pytest.raises(SpeechProviderError) as captured:
        provider.synthesize(
            TtsSynthesisRequest(
                request_id=UUID("22222222-2222-4222-8222-222222222222"),
                text="Hello",
                voice_type="voice",
                language="en-us",
                parameters={"audio_format": "mp3", "sample_rate": 24000},
            )
        )

    assert captured.value.code == "tts_http_error"
    assert captured.value.retryable is False
    assert captured.value.error_details == {
        "http_status": 403,
        "provider_error_code": "45000020",
        "provider_error_message": "Permission denied",
    }
    assert captured.value.upstream_log_id == "20260717150632A1B2C3D4E5F60789"


def test_tts_provider_rejects_incomplete_media_and_missing_timestamps() -> None:
    def post(_url, _headers, _payload, _timeout_seconds):
        return ProviderHttpResponse(
            status_code=200,
            headers={},
            body_lines=[
                json.dumps({"code": 0, "data": "eA==", "sentence": {"words": []}}).encode()
            ],
        )

    provider = VolcengineTtsV3Provider(
        api_key="fixture-api-key",
        base_url="https://openspeech.bytedance.com/api/v3",
        resource_id="seed-tts-2.0",
        timeout_seconds=120,
        http_post=post,
    )
    with pytest.raises(SpeechProviderError) as error:
        provider.synthesize(
            TtsSynthesisRequest(
                request_id=UUID("22222222-2222-4222-8222-222222222222"),
                text="Hello",
                voice_type="voice",
                language="en-us",
                parameters={"audio_format": "mp3", "sample_rate": 24000},
            )
        )
    assert error.value.code == "tts_audio_invalid"


def test_openai_audio_speech_provider_uses_bearer_and_exact_standard_payload() -> None:
    captured: dict[str, object] = {}
    audio = b"ID3\x04\x00\x00\n\x00\x00\x00\x21gateway-audio"

    def post(url, headers, payload, timeout_seconds):
        captured.update(url=url, headers=headers, payload=payload, timeout=timeout_seconds)
        return ProviderHttpResponse(
            status_code=200,
            headers={
                "Content-Type": "application/octet-stream",
                "X-Request-Id": "gateway-request-id",
            },
            body_lines=audio.splitlines(),
            body_content=audio,
        )

    provider = speech_generation.OpenAiAudioSpeechProvider(
        api_key="gateway-api-key",
        base_url="https://speech-gateway.example.com/v1",
        upstream_model="doubao-seed-tts-2.0",
        timeout_seconds=120,
        http_post=post,
    )
    result = provider.synthesize(
        TtsSynthesisRequest(
            request_id=UUID("22222222-2222-4222-8222-222222222222"),
            text="Hello",
            voice_type="en_fixture_voice",
            language="en-us",
            parameters={
                "audio_format": "mp3",
                "sample_rate": 24000,
                "speed_ratio": 1.25,
            },
        )
    )

    assert captured == {
        "url": "https://speech-gateway.example.com/v1/audio/speech",
        "headers": {
            "Authorization": "Bearer gateway-api-key",
            "Content-Type": "application/json",
        },
        "payload": {
            "model": "doubao-seed-tts-2.0",
            "input": "Hello",
            "voice": "en_fixture_voice",
            "response_format": "mp3",
            "speed": 1.25,
        },
        "timeout": 120,
    }
    assert result.audio_content == audio
    assert result.audio_format == "mp3"
    assert result.words == []
    assert result.upstream_log_id == "gateway-request-id"
    assert "gateway-api-key" not in json.dumps(result.audit_snapshot())


def test_default_http_adapter_preserves_raw_binary_body(monkeypatch) -> None:
    audio = b"ID3\x04\x00\x00\n\x00\x00\x00\x21gateway-audio"

    class Response:
        status = 200
        headers = {"Content-Type": "audio/mpeg"}

        def __enter__(self):
            return self

        def __exit__(self, *_args):
            return None

        def read(self):
            return audio

    monkeypatch.setattr(
        speech_generation.urllib_request,
        "urlopen",
        lambda _request, timeout: Response() if timeout == 120 else None,
    )

    response = speech_generation.default_json_stream_post(
        "https://speech-gateway.example.com/v1/audio/speech",
        {"Content-Type": "application/json"},
        {"input": "Hello"},
        120,
    )

    assert response.body_content == audio
    assert list(response.body_lines) == audio.splitlines()


@pytest.mark.parametrize(
    ("content_type", "audio", "code"),
    [
        ("application/json", b"ID3valid", "tts_content_type_invalid"),
        ("audio/mpeg", b"not-an-mp3", "tts_audio_invalid"),
    ],
)
def test_openai_audio_speech_provider_rejects_non_audio_responses(
    content_type, audio, code
) -> None:
    def post(_url, _headers, _payload, _timeout_seconds):
        return ProviderHttpResponse(
            200,
            {"Content-Type": content_type},
            audio.splitlines(),
            body_content=audio,
        )

    provider = speech_generation.OpenAiAudioSpeechProvider(
        api_key="gateway-api-key",
        base_url="https://speech-gateway.example.com/v1",
        upstream_model="doubao-seed-tts-2.0",
        timeout_seconds=120,
        http_post=post,
    )

    with pytest.raises(SpeechProviderError) as captured:
        provider.synthesize(
            TtsSynthesisRequest(
                request_id=UUID("22222222-2222-4222-8222-222222222222"),
                text="Hello",
                voice_type="voice",
                language="en-us",
                parameters={"audio_format": "mp3"},
            )
        )

    assert captured.value.code == code
    assert captured.value.retryable is True


@pytest.mark.parametrize(
    ("status_code", "retryable"),
    [(401, False), (403, False), (429, True), (500, True)],
)
def test_openai_audio_speech_provider_classifies_http_errors(
    status_code, retryable
) -> None:
    def post(_url, _headers, _payload, _timeout_seconds):
        return ProviderHttpResponse(
            status_code,
            {"Content-Type": "application/json"},
            [b'{"error":"redacted"}'],
            body_content=b'{"error":"redacted"}',
        )

    provider = speech_generation.OpenAiAudioSpeechProvider(
        api_key="gateway-api-key",
        base_url="https://speech-gateway.example.com/v1",
        upstream_model="doubao-seed-tts-2.0",
        timeout_seconds=120,
        http_post=post,
    )

    with pytest.raises(SpeechProviderError) as captured:
        provider.synthesize(
            TtsSynthesisRequest(
                request_id=UUID("22222222-2222-4222-8222-222222222222"),
                text="Hello",
                voice_type="voice",
                language="en-us",
                parameters={"audio_format": "mp3"},
            )
        )

    assert captured.value.code == "tts_http_error"
    assert captured.value.retryable is retryable


def test_openai_audio_speech_provider_redacts_secret_values_in_error_message() -> None:
    body = json.dumps(
        {
            "error": {
                "code": "permission_denied",
                "message": "Authorization: Bearer gateway-secret-token",
            }
        }
    ).encode()

    def post(_url, _headers, _payload, _timeout_seconds):
        return ProviderHttpResponse(
            403,
            {
                "Content-Type": "application/json",
                "X-OneAPI-Request-Id": "gateway-log-id",
            },
            body.splitlines(),
            body_content=body,
        )

    provider = speech_generation.OpenAiAudioSpeechProvider(
        api_key="gateway-api-key",
        base_url="https://speech-gateway.example.com/v1",
        upstream_model="doubao-seed-tts-2.0",
        timeout_seconds=120,
        http_post=post,
    )

    with pytest.raises(SpeechProviderError) as captured:
        provider.synthesize(
            TtsSynthesisRequest(
                request_id=UUID("22222222-2222-4222-8222-222222222222"),
                text="Hello",
                voice_type="voice",
                language="en-us",
                parameters={"audio_format": "mp3"},
            )
        )

    serialized = json.dumps(captured.value.error_details)
    assert captured.value.error_details == {
        "http_status": 403,
        "provider_error_code": "permission_denied",
        "provider_error_message": "Authorization: [REDACTED]",
    }
    assert captured.value.upstream_log_id == "gateway-log-id"
    assert "gateway-secret-token" not in serialized
    assert "gateway-api-key" not in serialized


def test_srt_segments_follow_vendor_word_boundaries() -> None:
    words = [
        TimestampWord("你", 0, 200, 0.99),
        TimestampWord("好", 210, 400, 0.99),
        TimestampWord("世", 500, 700, 0.99),
        TimestampWord("界", 710, 900, 0.99),
    ]

    timeline, srt = build_srt(words, ["你好", "世界"])

    assert timeline == [
        {"index": 1, "start_ms": 0, "end_ms": 400, "text": "你好"},
        {"index": 2, "start_ms": 500, "end_ms": 900, "text": "世界"},
    ]
    assert srt == (
        "1\n00:00:00,000 --> 00:00:00,400\n你好\n\n"
        "2\n00:00:00,500 --> 00:00:00,900\n世界\n"
    )


def test_srt_refuses_fabricated_alignment() -> None:
    with pytest.raises(SpeechProviderError) as error:
        build_srt([], ["没有时间戳"])
    assert error.value.code == "timestamps_unavailable"

    with pytest.raises(SpeechProviderError) as error:
        build_srt([TimestampWord("Hello", 0, 200, 1.0)], ["Different"])
    assert error.value.code == "subtitle_alignment_mismatch"


def test_asr_v3_reuses_request_id_and_never_returns_signed_url_in_audit() -> None:
    calls: list[tuple[str, dict, dict]] = []
    fixture = json.loads((FIXTURES / "asr_result.json").read_text(encoding="utf-8"))

    def post(url, headers, payload, timeout_seconds):
        calls.append((url, headers, payload))
        if url.endswith("/submit"):
            return ProviderHttpResponse(200, {"X-Api-Status-Code": "20000000"}, [])
        return ProviderHttpResponse(
            200,
            {"X-Api-Status-Code": "20000000", "X-Tt-Logid": "asr-log"},
            [json.dumps(fixture).encode()],
        )

    provider = VolcengineAsrV3Provider(
        api_key="fixture-api-key",
        base_url="https://openspeech.bytedance.com/api/v3",
        resource_id="volc.seedasr.auc",
        timeout_seconds=120,
        http_post=post,
    )
    request_id = UUID("22222222-2222-4222-8222-222222222222")
    signed_url = "https://tos.example.invalid/file.mp3?X-Tos-Signature=secret"

    submitted = provider.submit(request_id, signed_url, "mp3")
    queried = provider.query(request_id)

    assert submitted.accepted is True
    assert calls[0][0].endswith("/auc/bigmodel/submit")
    assert calls[1][0].endswith("/auc/bigmodel/query")
    for _, headers, _ in calls:
        assert headers["X-Api-Request-Id"] == str(request_id)
        assert headers["X-Api-Resource-Id"] == "volc.seedasr.auc"
    assert calls[0][2]["audio"]["url"] == signed_url
    assert queried.is_terminal is True
    assert queried.is_success is True
    assert queried.words[0] == TimestampWord("Hello", 100, 500, None)
    serialized_audit = json.dumps(queried.audit_snapshot())
    assert "X-Tos-Signature" not in serialized_audit
    assert "fixture-api-key" not in serialized_audit


class MemorySpeechStore:
    def __init__(self, task: PendingSpeechTask):
        self.task = task
        self.completed = None
        self.failed = None
        self.staged = None
        self.submitted = False
        self.deferred = False
        self.cleanup = None

    def claim_next_speech_task(self):
        task, self.task = self.task, None
        return task

    def complete_tts_task(self, **kwargs):
        self.completed = kwargs

    def complete_asr_task(self, **kwargs):
        self.completed = kwargs

    def fail_task(self, **kwargs):
        self.failed = kwargs

    def record_asr_staging(self, task_id, object_key, source_sha256):
        self.staged = (task_id, object_key, source_sha256)

    def record_asr_submitted(self, task_id, attempt_count):
        self.submitted = True

    def defer_asr_task(self, task_id):
        self.deferred = True

    def record_cleanup(self, task_id, succeeded, error_summary):
        self.cleanup = (task_id, succeeded, error_summary)


class FakeSpeechRegistry:
    def __init__(self, config, tos_config=None):
        self.config = config
        self.tos_config = tos_config

    def resolve_speech(self, model_id, protocol, version):
        assert model_id == self.config.model_id
        assert protocol == self.config.api_protocol
        assert version == self.config.registry_version
        return self.config

    def resolve_tos_staging(self, config_id, version):
        assert self.tos_config is not None
        assert config_id == self.tos_config.config_id
        assert version == self.tos_config.version
        return self.tos_config


def speech_config(protocol: str) -> SpeechModelRuntimeConfig:
    is_asr = protocol == "volcengine_asr_v3"
    is_openai_tts = protocol == "openai_audio_speech"
    return SpeechModelRuntimeConfig(
        model_id="11111111-1111-4111-8111-111111111111",
        display_name="测试语音模型",
        provider_name="ZeekAI" if is_openai_tts else "火山引擎",
        api_protocol=protocol,
        protocol_version="v1" if is_openai_tts else "v3",
        auth_scheme="bearer" if is_openai_tts else "api_key",
        request_base_url=(
            "https://speech-gateway.example.com/v1"
            if is_openai_tts
            else "https://openspeech.bytedance.com/api/v3"
        ),
        upstream_model="doubao-seed-asr-2.0" if is_asr else "doubao-seed-tts-2.0",
        api_key="speech-key",
        timeout_seconds=120,
        settings={
            "resource_id": "volc.seedasr.auc" if is_asr else "seed-tts-2.0",
            "supported_audio_formats": ["wav", "mp3"],
            "supports_word_timestamps": not is_openai_tts,
            "word_timestamp_languages": (
                [] if is_openai_tts else ["*" if is_asr else "zh-cn"]
            ),
            "parameters": {},
        },
        registry_version=3,
    )


def tos_staging_config() -> SpeechStagingRuntimeConfig:
    return SpeechStagingRuntimeConfig(
        config_id="66666666-6666-4666-8666-666666666666",
        version=5,
        storage_provider="volcengine_tos",
        endpoint="https://tos-cn-beijing.volces.com",
        region="cn-beijing",
        bucket="private-bucket",
        object_prefix="novex/asr",
        access_key="tos-ak",
        secret_key="tos-sk",
        signed_url_ttl_seconds=600,
        max_file_bytes=1024 * 1024,
        max_audio_duration_seconds=3600,
    )


def pending_tts_task() -> PendingSpeechTask:
    return PendingSpeechTask(
        task_id="22222222-2222-4222-8222-222222222222",
        project_id="33333333-3333-4333-8333-333333333333",
        task_type="tts",
        model_id="11111111-1111-4111-8111-111111111111",
        tos_staging_config_id=None,
        tos_staging_config_version=None,
        request_id="44444444-4444-4444-8444-444444444444",
        text_content="你好",
        voice_type="fixture-voice",
        language="zh-cn",
        emotion=None,
        parameters={"audio_format": "wav", "sample_rate": 24000},
        model_snapshot={"api_protocol": "volcengine_tts_v3", "registry_version": 3},
        voice_snapshot={"name": "测试音色"},
        confirmation_snapshot={
            "generate_subtitle": True,
            "subtitle_segments": ["你好"],
        },
        resource_usage={"character_count": 2, "task_count": 2},
    )


def test_tts_worker_retries_once_then_atomically_records_audio_and_srt(tmp_path: Path) -> None:
    task = pending_tts_task()
    store = MemorySpeechStore(task)
    calls = 0
    wav = b"RIFF" + (32).to_bytes(4, "little") + b"WAVE" + b"fmt " + b"x" * 32

    class Provider:
        def synthesize(self, _request):
            nonlocal calls
            calls += 1
            if calls == 1:
                raise SpeechProviderError("temporary", "临时错误", retryable=True)
            return TtsSynthesisResult(
                audio_content=wav,
                audio_format="wav",
                words=[
                    TimestampWord("你", 0, 200, 1.0),
                    TimestampWord("好", 210, 400, 1.0),
                ],
                upstream_log_id="tts-log",
            )

    inspection = AudioInspectionResult(
        source_sha256="a" * 64,
        file_size_bytes=len(wav),
        duration_ms=400,
        container_format="wav",
        audio_codec="pcm_s16le",
        sample_rate_hz=24000,
        channel_count=1,
    )
    processed = run_next_speech_task(
        store,
        FakeSpeechRegistry(speech_config("volcengine_tts_v3")),
        LocalSpeechStorage(tmp_path),
        tts_provider_factory=lambda _config: Provider(),
        audio_inspector=lambda _path: inspection,
    )

    assert processed is True
    assert calls == 2
    assert store.failed is None
    assert store.completed["attempt_count"] == 2
    assert store.completed["upstream_log_id"] == "tts-log"
    assert store.completed["audio_artifact"].file_url.endswith("/tts.wav")
    assert store.completed["subtitle_artifact"].file_url.endswith("/subtitles.srt")
    assert store.completed["timeline"][0]["text"] == "你好"


def test_tts_worker_passes_structured_provider_error_to_store(tmp_path: Path) -> None:
    task = pending_tts_task()
    store = MemorySpeechStore(task)

    class Provider:
        def synthesize(self, _request):
            raise SpeechProviderError(
                "tts_http_error",
                "语音供应商返回 HTTP 403",
                retryable=False,
                error_details={
                    "http_status": 403,
                    "provider_error_code": "45000020",
                    "provider_error_message": "Permission denied",
                },
                upstream_log_id="provider-log-id",
            )

    run_next_speech_task(
        store,
        FakeSpeechRegistry(speech_config("volcengine_tts_v3")),
        LocalSpeechStorage(tmp_path),
        tts_provider_factory=lambda _config: Provider(),
        audio_inspector=lambda _path: pytest.fail("失败任务不应检查音频"),
    )

    assert store.completed is None
    assert store.failed["error_details"] == {
        "http_status": 403,
        "provider_error_code": "45000020",
        "provider_error_message": "Permission denied",
    }
    assert store.failed["upstream_log_id"] == "provider-log-id"


def test_openai_audio_speech_task_dispatches_audio_only_without_timestamps(
    tmp_path: Path,
) -> None:
    task = PendingSpeechTask(
        **{
            **pending_tts_task().__dict__,
            "model_snapshot": {
                "api_protocol": "openai_audio_speech",
                "registry_version": 3,
            },
            "confirmation_snapshot": {"generate_subtitle": False},
        }
    )
    store = MemorySpeechStore(task)
    audio = b"ID3gateway-audio"

    class Provider:
        def synthesize(self, _request):
            return TtsSynthesisResult(audio, "mp3", [], "gateway-request-id")

    inspection = AudioInspectionResult(
        source_sha256="a" * 64,
        file_size_bytes=len(audio),
        duration_ms=400,
        container_format="mp3",
        audio_codec="mp3",
        sample_rate_hz=24000,
        channel_count=1,
    )

    processed = run_next_speech_task(
        store,
        FakeSpeechRegistry(speech_config("openai_audio_speech")),
        LocalSpeechStorage(tmp_path),
        tts_provider_factory=lambda _config: Provider(),
        audio_inspector=lambda _path: inspection,
    )

    assert processed is True
    assert store.failed is None
    assert store.completed["words"] == []
    assert store.completed["subtitle_artifact"] is None
    assert store.completed["audio_artifact"].file_url.endswith("/tts.mp3")


def test_openai_audio_speech_task_rejects_subtitles_before_provider_call(
    tmp_path: Path,
) -> None:
    task = PendingSpeechTask(
        **{
            **pending_tts_task().__dict__,
            "model_snapshot": {
                "api_protocol": "openai_audio_speech",
                "registry_version": 3,
            },
        }
    )
    store = MemorySpeechStore(task)

    run_next_speech_task(
        store,
        FakeSpeechRegistry(speech_config("openai_audio_speech")),
        LocalSpeechStorage(tmp_path),
        tts_provider_factory=lambda _config: pytest.fail("中转 TTS 不应被调用"),
        audio_inspector=lambda _path: pytest.fail("音频检查不应被调用"),
    )

    assert store.completed is None
    assert store.failed["error_code"] == "tts_word_timestamps_unsupported"


def test_tts_provider_factory_dispatches_openai_audio_speech() -> None:
    provider = speech_generation._tts_provider_from_config(
        speech_config("openai_audio_speech")
    )

    assert isinstance(provider, speech_generation.OpenAiAudioSpeechProvider)


def pending_asr_task(source_file_url: str, source_sha256: str) -> PendingSpeechTask:
    return PendingSpeechTask(
        task_id="22222222-2222-4222-8222-222222222222",
        project_id="33333333-3333-4333-8333-333333333333",
        task_type="asr",
        model_id="11111111-1111-4111-8111-111111111111",
        tos_staging_config_id="66666666-6666-4666-8666-666666666666",
        tos_staging_config_version=5,
        request_id="44444444-4444-4444-8444-444444444444",
        text_content="",
        voice_type=None,
        language=None,
        emotion=None,
        parameters={"audio_format": "mp3"},
        model_snapshot={"api_protocol": "volcengine_asr_v3", "registry_version": 3},
        voice_snapshot=None,
        confirmation_snapshot={"generate_subtitle": True},
        resource_usage={"audio_duration_ms": 1000, "task_count": 1},
        source_audio_material_id="55555555-5555-4555-8555-555555555555",
        source_file_url=source_file_url,
        inspection_source_sha256=source_sha256,
        inspection_duration_ms=1000,
        staging_status="none",
    )


def test_asr_worker_blocks_changed_source_before_tos_upload(tmp_path: Path) -> None:
    source = tmp_path / "source.mp3"
    source.write_bytes(b"ID3source")
    task = pending_asr_task("/assets/source.mp3", "b" * 64)
    store = MemorySpeechStore(task)

    class Staging:
        called = False

        def stage(self, **_kwargs):
            self.called = True

    staging = Staging()
    actual = AudioInspectionResult(
        source_sha256="a" * 64,
        file_size_bytes=9,
        duration_ms=1000,
        container_format="mp3",
        audio_codec="mp3",
        sample_rate_hz=24000,
        channel_count=1,
    )

    run_next_speech_task(
        store,
        FakeSpeechRegistry(speech_config("volcengine_asr_v3"), tos_staging_config()),
        LocalSpeechStorage(tmp_path),
        asr_provider_factory=lambda _config: pytest.fail("ASR 不应被调用"),
        tos_staging_factory=lambda _config: staging,
        audio_inspector=lambda _path: actual,
    )

    assert staging.called is False
    assert store.failed["error_code"] == "audio_source_changed"


def test_asr_worker_cleans_uncertain_upload_when_presign_fails(tmp_path: Path) -> None:
    source = tmp_path / "source.mp3"
    source.write_bytes(b"ID3source")
    task = pending_asr_task("/assets/source.mp3", "a" * 64)
    store = MemorySpeechStore(task)
    inspection = AudioInspectionResult(
        source_sha256="a" * 64,
        file_size_bytes=9,
        duration_ms=1000,
        container_format="mp3",
        audio_codec="mp3",
        sample_rate_hz=24000,
        channel_count=1,
    )

    class Staging:
        deleted = []

        def stage(self, **_kwargs):
            raise TosStagingError(
                "tos_presign_failed",
                "签名失败",
                retryable=True,
                object_key="novex/asr/project/task/source.mp3",
                source_sha256="a" * 64,
            )

        def cleanup(self, key):
            self.deleted.append(key)

    staging = Staging()
    run_next_speech_task(
        store,
        FakeSpeechRegistry(speech_config("volcengine_asr_v3"), tos_staging_config()),
        LocalSpeechStorage(tmp_path),
        asr_provider_factory=lambda _config: pytest.fail("ASR 不应被调用"),
        tos_staging_factory=lambda _config: staging,
        audio_inspector=lambda _path: inspection,
    )

    assert store.staged[1] == "novex/asr/project/task/source.mp3"
    assert store.failed["error_code"] == "tos_presign_failed"
    assert staging.deleted == ["novex/asr/project/task/source.mp3"]
    assert store.cleanup == (task.task_id, True, None)


def test_asr_worker_resumes_query_without_resubmission_and_cleans_tos(tmp_path: Path) -> None:
    source = tmp_path / "source.mp3"
    source.write_bytes(b"ID3source")
    task = pending_asr_task("/assets/source.mp3", "a" * 64)
    task = PendingSpeechTask(
        **{
            **task.__dict__,
            "staging_status": "uploaded",
            "staging_object_key": "novex/asr/project/task/source.mp3",
            "upstream_submitted": True,
        }
    )
    store = MemorySpeechStore(task)
    inspection = AudioInspectionResult(
        source_sha256="a" * 64,
        file_size_bytes=9,
        duration_ms=1000,
        container_format="mp3",
        audio_codec="mp3",
        sample_rate_hz=24000,
        channel_count=1,
    )

    class Staging:
        def __init__(self):
            self.deleted = []

        def signed_get_url(self, key):
            return f"https://tos.invalid/{key}?signature=secret"

        def cleanup(self, key):
            self.deleted.append(key)

    class Provider:
        submit_calls = 0

        def submit(self, *_args):
            self.submit_calls += 1

        def query(self, _request_id):
            return AsrQueryResult(
                True,
                True,
                "Hello world.",
                [
                    TimestampWord("Hello", 0, 400, None),
                    TimestampWord("world.", 450, 900, None),
                ],
                [{"text": "Hello world."}],
                "asr-log",
            )

    staging = Staging()
    provider = Provider()
    run_next_speech_task(
        store,
        FakeSpeechRegistry(speech_config("volcengine_asr_v3"), tos_staging_config()),
        LocalSpeechStorage(tmp_path),
        asr_provider_factory=lambda _config: provider,
        tos_staging_factory=lambda _config: staging,
        audio_inspector=lambda _path: inspection,
    )

    assert provider.submit_calls == 0
    assert store.completed["subtitle_artifact"].file_url.endswith("/subtitles.srt")
    assert staging.deleted == [task.staging_object_key]
    assert store.cleanup == (task.task_id, True, None)
