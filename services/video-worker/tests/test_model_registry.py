from __future__ import annotations

import pytest

from video_worker.model_registry import (
    ModelRegistryError,
    PostgresModelRegistry,
)


MODEL_ID = "00000000-0000-0000-0000-000000000101"


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
