from __future__ import annotations

import pytest

from video_worker.model_registry import (
    ModelRegistryError,
    PostgresModelRegistry,
)


MODEL_ID = "00000000-0000-0000-0000-000000000101"
TOS_CONFIG_ID = "00000000-0000-0000-0000-000000000201"


class FakeConnection:
    def __init__(self, row: dict[str, object] | None):
        self.row = row
        self.params = None

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return None

    def execute(self, _query: str, params: tuple[str]):
        self.params = params
        return self

    def fetchone(self):
        return self.row


def image_model_row(**overrides: object) -> dict[str, object]:
    row: dict[str, object] = {
        "id": MODEL_ID,
        "display_name": "OpenAI 图片",
        "model_type": "image",
        "provider_name": "OpenAI",
        "api_protocol": "openai_images",
        "protocol_version": "v1",
        "auth_scheme": "bearer",
        "request_base_url": "https://api.example.com/v1",
        "upstream_model": "gpt-image-test",
        "api_key": "test-key",
        "api_secret": None,
        "timeout_seconds": 45,
        "settings": {
            "supported_sizes": ["1024x1024"],
            "default_size": "1024x1024",
            "max_images_per_request": 4,
        },
        "status": "enabled",
        "deleted_at": None,
    }
    row.update(overrides)
    return row


def speech_model_row(**overrides: object) -> dict[str, object]:
    row: dict[str, object] = {
        "id": MODEL_ID,
        "display_name": "Doubao ASR",
        "model_type": "speech",
        "provider_name": "火山引擎",
        "api_protocol": "volcengine_asr_v3",
        "protocol_version": "v3",
        "auth_scheme": "api_key",
        "request_base_url": "https://openspeech.bytedance.com/api/v3",
        "upstream_model": "doubao-seed-asr-2.0",
        "api_key": "speech-key",
        "api_secret": None,
        "timeout_seconds": 120,
        "settings": {
            "resource_id": "volc.seedasr.auc",
            "supported_audio_formats": ["mp3", "wav"],
            "supports_word_timestamps": True,
            "word_timestamp_languages": ["*"],
            "max_audio_duration_seconds": 3600,
            "parameters": {},
        },
        "status": "enabled",
        "deleted_at": None,
        "version": 3,
    }
    row.update(overrides)
    return row


def video_model_row(**overrides: object) -> dict[str, object]:
    row: dict[str, object] = {
        "id": MODEL_ID,
        "display_name": "Seedance 1.5",
        "model_type": "video",
        "provider_name": "火山引擎",
        "api_protocol": "volcengine_ark_video",
        "protocol_version": "v1",
        "auth_scheme": "bearer",
        "request_base_url": "https://ark.cn-beijing.volces.com/api/v3",
        "upstream_model": "doubao-seedance-1-5-pro-251215",
        "api_key": "video-key",
        "timeout_seconds": 300,
        "settings": {
            "resolutions": ["720p", "1080p"],
            "aspect_ratios": ["16:9", "9:16", "1:1"],
            "generate_audio": True,
            "max_prompt_chars": 500,
            "max_duration_seconds": 15,
            "max_reference_images": 9,
            "min_duration_seconds": 4,
        },
        "status": "enabled",
        "deleted_at": None,
        "version": 4,
    }
    row.update(overrides)
    return row


def openai_audio_speech_model_row(**overrides: object) -> dict[str, object]:
    row = speech_model_row(
        display_name="Doubao TTS Gateway",
        provider_name="ZeekAI",
        api_protocol="openai_audio_speech",
        protocol_version="v1",
        auth_scheme="bearer",
        request_base_url="https://speech-gateway.example.com/v1",
        upstream_model="doubao-seed-tts-2.0",
        settings={
            "resource_id": "seed-tts-2.0",
            "supported_audio_formats": ["mp3", "wav"],
            "supports_word_timestamps": False,
            "word_timestamp_languages": [],
            "parameters": {
                "speed_ratio": {
                    "type": "number",
                    "minimum": 0.25,
                    "maximum": 4.0,
                }
            },
        },
    )
    row.update(overrides)
    return row


def tos_config_row(**overrides: object) -> dict[str, object]:
    row: dict[str, object] = {
        "id": TOS_CONFIG_ID,
        "version": 5,
        "storage_provider": "volcengine_tos",
        "endpoint": "https://tos-cn-beijing.volces.com",
        "region": "cn-beijing",
        "bucket": "private-bucket",
        "object_prefix": "novex/asr",
        "access_key": "tos-ak",
        "secret_key": "tos-sk",
        "signed_url_ttl_seconds": 600,
        "max_file_bytes": 10485760,
        "max_audio_duration_seconds": 3600,
    }
    row.update(overrides)
    return row


def registry_for(row: dict[str, object] | None) -> tuple[PostgresModelRegistry, FakeConnection]:
    connection = FakeConnection(row)
    registry = PostgresModelRegistry(
        "postgres://unused",
        connection_factory=lambda: connection,
    )
    return registry, connection


def test_loader_returns_openai_images_runtime_config_and_safe_snapshot():
    registry, connection = registry_for(image_model_row())

    config = registry.resolve_enabled(MODEL_ID, "image")

    assert connection.params == (MODEL_ID,)
    assert config.api_protocol == "openai_images"
    assert config.api_key == "test-key"
    assert config.settings["default_size"] == "1024x1024"
    snapshot = config.snapshot()
    assert snapshot["model_id"] == MODEL_ID
    assert "api_key" not in snapshot
    assert "api_secret" not in snapshot


def test_loader_returns_volcengine_ark_images_with_bearer_authentication():
    registry, _ = registry_for(
        image_model_row(
            display_name="Seedream Ark",
            provider_name="火山引擎",
            api_protocol="volcengine_ark_images",
            protocol_version="v3",
            auth_scheme="bearer",
            request_base_url="https://ark.cn-beijing.volces.com/api/v3",
            upstream_model="doubao-seedream-5-0-260128",
            settings={
                "supported_sizes": [],
                "default_size": None,
                "max_images_per_request": 1,
            },
        )
    )

    config = registry.resolve_enabled(MODEL_ID, "image")

    assert config.api_protocol == "volcengine_ark_images"
    assert config.auth_scheme == "bearer"
    assert config.request_base_url == "https://ark.cn-beijing.volces.com/api/v3"


@pytest.mark.parametrize(
    ("row", "code"),
    [
        (None, "model_not_found"),
        (image_model_row(status="disabled"), "model_disabled"),
        (image_model_row(model_type="text"), "model_type_mismatch"),
        (image_model_row(api_protocol="openai_responses"), "invalid_model_config"),
        (image_model_row(api_protocol="jimeng_visual"), "invalid_model_config"),
        (
            image_model_row(
                api_protocol="volcengine_ark_images",
                auth_scheme="access_key_secret",
            ),
            "invalid_model_config",
        ),
        (
            image_model_row(
                api_protocol="volcengine_ark_images",
                request_base_url="https://ark.cn-beijing.volces.com/api/v3?region=test",
                settings={"max_images_per_request": 1},
            ),
            "invalid_model_config",
        ),
        (
            image_model_row(
                api_protocol="volcengine_ark_images",
                request_base_url="https://ark.cn-beijing.volces.com/v1",
                settings={"max_images_per_request": 1},
            ),
            "invalid_model_config",
        ),
        (
            image_model_row(
                api_protocol="volcengine_ark_images",
                settings={"max_images_per_request": 4},
            ),
            "invalid_model_config",
        ),
        (image_model_row(api_protocol="runway_api"), "invalid_model_config"),
    ],
)
def test_loader_rejects_unavailable_or_invalid_models(row, code):
    registry, _ = registry_for(row)

    with pytest.raises(ModelRegistryError) as captured:
        registry.resolve_enabled(MODEL_ID, "image")

    assert captured.value.code == code


def test_speech_loader_requires_locked_model_version_without_tos_fields():
    registry, connection = registry_for(speech_model_row())

    config = registry.resolve_speech(MODEL_ID, "volcengine_asr_v3", 3)

    assert connection.params == (MODEL_ID,)
    assert config.registry_version == 3
    assert config.api_key == "speech-key"
    snapshot = config.snapshot()
    assert snapshot["registry_version"] == 3
    serialized = str(snapshot)
    assert "speech-key" not in serialized
    assert "staging" not in serialized


def test_video_loader_requires_locked_version_and_returns_safe_snapshot():
    registry, connection = registry_for(video_model_row())

    config = registry.resolve_video(MODEL_ID, 4)

    assert connection.params == (MODEL_ID,)
    assert config.api_protocol == "volcengine_ark_video"
    assert config.api_key == "video-key"
    assert config.settings["max_reference_images"] == 2
    assert config.settings["max_duration_seconds"] == 12
    assert config.settings["reference_image_mode"] == "first_last_frames"
    assert config.snapshot()["registry_version"] == 4
    assert "video-key" not in str(config.snapshot())


@pytest.mark.parametrize(
    ("row", "code"),
    [
        (video_model_row(version=5), "model_version_changed"),
        (video_model_row(status="disabled"), "model_disabled"),
        (video_model_row(auth_scheme="api_key"), "invalid_model_config"),
        (video_model_row(request_base_url="http://ark.example/api/v3"), "invalid_model_config"),
        (video_model_row(settings={"max_reference_images": 10}), "invalid_model_config"),
    ],
)
def test_video_loader_rejects_changed_or_unsafe_config(row, code):
    registry, _ = registry_for(row)

    with pytest.raises(ModelRegistryError) as captured:
        registry.resolve_video(MODEL_ID, 4)

    assert captured.value.code == code


def test_speech_loader_accepts_openai_audio_speech_bearer_runtime():
    registry, connection = registry_for(openai_audio_speech_model_row())

    config = registry.resolve_speech(MODEL_ID, "openai_audio_speech", 3)

    assert connection.params == (MODEL_ID,)
    assert config.api_protocol == "openai_audio_speech"
    assert config.auth_scheme == "bearer"
    assert config.request_base_url == "https://speech-gateway.example.com/v1"
    assert config.settings["supports_word_timestamps"] is False
    assert config.settings["word_timestamp_languages"] == []
    assert "speech-key" not in str(config.snapshot())


@pytest.mark.parametrize(
    "row",
    [
        openai_audio_speech_model_row(auth_scheme="api_key"),
        openai_audio_speech_model_row(
            request_base_url="https://speech-gateway.example.com/v1/audio/speech"
        ),
        openai_audio_speech_model_row(
            settings={
                "resource_id": "seed-tts-2.0",
                "supported_audio_formats": ["mp3"],
                "supports_word_timestamps": True,
                "word_timestamp_languages": ["zh-cn"],
            }
        ),
    ],
)
def test_speech_loader_rejects_invalid_openai_audio_speech_runtime(row):
    registry, _ = registry_for(row)

    with pytest.raises(ModelRegistryError) as captured:
        registry.resolve_speech(MODEL_ID, "openai_audio_speech", 3)

    assert captured.value.code == "invalid_model_config"


def test_tos_loader_resolves_locked_historical_version_independently():
    registry, connection = registry_for(tos_config_row())

    config = registry.resolve_tos_staging(TOS_CONFIG_ID, 5)

    assert connection.params == (TOS_CONFIG_ID,)
    assert config.config_id == TOS_CONFIG_ID
    assert config.version == 5
    assert config.bucket == "private-bucket"
    assert config.access_key == "tos-ak"


@pytest.mark.parametrize(
    ("row", "protocol", "version", "code"),
    [
        (speech_model_row(version=4), "volcengine_asr_v3", 3, "model_version_changed"),
        (speech_model_row(api_protocol="volcengine_tts_v3"), "volcengine_asr_v3", 3, "model_protocol_mismatch"),
        (speech_model_row(api_key=None), "volcengine_asr_v3", 3, "invalid_model_config"),
        (speech_model_row(status="disabled"), "volcengine_asr_v3", 3, "model_disabled"),
    ],
)
def test_speech_loader_rejects_changed_or_invalid_runtime(row, protocol, version, code):
    registry, _ = registry_for(row)

    with pytest.raises(ModelRegistryError) as captured:
        registry.resolve_speech(MODEL_ID, protocol, version)

    assert captured.value.code == code


@pytest.mark.parametrize(
    ("row", "version", "code"),
    [
        (None, 5, "tos_staging_config_not_found"),
        (tos_config_row(), 4, "tos_staging_version_changed"),
        (tos_config_row(endpoint="http://tos.invalid"), 5, "invalid_tos_staging_config"),
        (tos_config_row(secret_key=""), 5, "invalid_tos_staging_config"),
    ],
)
def test_tos_loader_rejects_missing_changed_or_invalid_locked_config(row, version, code):
    registry, _ = registry_for(row)

    with pytest.raises(ModelRegistryError) as captured:
        registry.resolve_tos_staging(TOS_CONFIG_ID, version)

    assert captured.value.code == code
