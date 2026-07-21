import json
from io import BytesIO
from urllib import error as urllib_error

import pytest

from video_worker.seedance import (
    ArkSeedanceProvider,
    FakeSeedanceProvider,
    SeedanceImageInput,
    SeedanceProviderError,
    SeedanceRequest,
    SeedanceTask,
    sanitize_model_snapshot,
)


def image(url="https://image", role="reference_image"):
    return SeedanceImageInput(url, role)


def test_fake_seedance_contract_and_audio_flag():
    provider = FakeSeedanceProvider()
    task = provider.create(SeedanceRequest("镜头动作", [image()], 4, "16:9", "1080p", True))
    assert task.status == "succeeded"
    assert provider.calls[0]["generate_audio"] is True
    assert provider.calls[0]["ratio"] == "16:9"
    assert "aspect_ratio" not in provider.calls[0]
    assert provider.calls[0]["content"][1]["role"] == "reference_image"
    assert provider.calls[0]["model"] == "doubao-seedance-2-0-260128"
    assert provider.get(task.task_id).task_id == task.task_id
    assert provider.cancel(task.task_id).status == "cancelled"


@pytest.mark.parametrize("duration", [3, 16])
def test_seedance_rejects_invalid_duration(duration):
    with pytest.raises(ValueError):
        SeedanceRequest("prompt", [image()], duration, "16:9", "1080p").validate()


def test_seedance_15_uses_first_and_last_frame_contract_and_twelve_second_limit():
    request = SeedanceRequest(
        "镜头动作",
        [image(role="first_frame"), image("https://last", "last_frame")],
        12,
        "16:9",
        "1080p",
        model="doubao-seedance-1-5-pro-251215",
    )

    request.validate()
    assert [item["role"] for item in request.payload()["content"][1:]] == [
        "first_frame",
        "last_frame",
    ]
    with pytest.raises(ValueError, match="4~12"):
        SeedanceRequest(
            "镜头动作",
            [image(role="first_frame")],
            13,
            "16:9",
            "1080p",
            model="doubao-seedance-1-5-pro-251215",
        ).validate()


def test_snapshot_does_not_keep_credentials():
    assert sanitize_model_snapshot({"model": "seedance", "api_key": "secret", "token": "x"}) == {"model": "seedance"}


class FakeHttpResponse:
    def __init__(self, status: int, payload: dict[str, object]):
        self.status = status
        self.payload = payload

    def read(self, _limit: int = -1) -> bytes:
        return json.dumps(self.payload).encode()

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        return None


def test_real_seedance_uses_official_create_query_and_cancel_contract():
    calls = []
    responses = [
        FakeHttpResponse(200, {"id": "task-1", "status": "queued"}),
        FakeHttpResponse(200, {"id": "task-1", "status": "succeeded", "content": {"video_url": "https://result.example/video.mp4"}}),
        FakeHttpResponse(200, {"id": "task-1", "status": "cancelled"}),
    ]

    def open_request(request, timeout):
        calls.append((request, timeout))
        return responses.pop(0)

    provider = ArkSeedanceProvider(
        api_key="secret",
        base_url="https://ark.cn-beijing.volces.com/api/v3",
        timeout_seconds=300,
        open_request=open_request,
    )
    created = provider.create(SeedanceRequest("镜头动作", [image("https://tos.example/image.png?signature=x")], 15, "16:9", "1080p"))
    queried = provider.get(created.task_id)
    cancelled = provider.cancel(created.task_id)

    create_payload = json.loads(calls[0][0].data)
    assert calls[0][0].method == "POST"
    assert calls[0][0].full_url.endswith("/contents/generations/tasks")
    assert create_payload["ratio"] == "16:9"
    assert "aspect_ratio" not in create_payload
    assert create_payload["content"][1]["role"] == "reference_image"
    assert calls[1][0].method == "GET"
    assert calls[2][0].method == "DELETE"
    assert queried.output_url == "https://result.example/video.mp4"
    assert cancelled.status == "cancelled"


def test_real_seedance_does_not_retry_uncertain_post():
    calls = []

    def fail_once(request, timeout):
        calls.append((request, timeout))
        raise TimeoutError("timeout")

    provider = ArkSeedanceProvider(
        api_key="secret",
        base_url="https://ark.cn-beijing.volces.com/api/v3",
        timeout_seconds=300,
        open_request=fail_once,
    )

    with pytest.raises(SeedanceProviderError) as captured:
        provider.create(SeedanceRequest("镜头动作", [image("https://tos.example/image.png")], 15, "16:9", "1080p"))

    assert captured.value.code == "unknown_submission"
    assert len(calls) == 1


def test_real_seedance_preserves_only_sanitized_http_error_details():
    payload = {
        "error": {
            "code": "InvalidEndpointOrModel.NotFound",
            "message": (
                "model not found; reference="
                "https://tos.example/image.png?X-Tos-Signature=secret"
            ),
            "debug_request": {"authorization": "Bearer secret"},
        },
        "request_id": "req-ark-404",
    }

    def reject_request(request, timeout):
        raise urllib_error.HTTPError(
            request.full_url,
            404,
            "Not Found",
            {"x-tt-logid": "header-log-id"},
            BytesIO(json.dumps(payload).encode()),
        )

    provider = ArkSeedanceProvider(
        api_key="secret",
        base_url="https://ark.cn-beijing.volces.com/api/v3",
        timeout_seconds=300,
        open_request=reject_request,
    )

    with pytest.raises(SeedanceProviderError) as captured:
        provider.create(
            SeedanceRequest(
                "镜头动作",
                [image("https://tos.example/image.png?X-Tos-Signature=secret")],
                15,
                "16:9",
                "1080p",
            )
        )

    error = captured.value
    assert error.code == "http_error"
    assert error.status_code == 404
    assert error.provider_code == "InvalidEndpointOrModel.NotFound"
    assert error.provider_message == "model not found; reference=[REDACTED_URL]"
    assert error.request_id == "req-ark-404"
    assert error.audit_snapshot() == {
        "error_code": "http_error",
        "http_status": 404,
        "provider_error_code": "InvalidEndpointOrModel.NotFound",
        "provider_error_message": "model not found; reference=[REDACTED_URL]",
        "provider_request_id": "req-ark-404",
    }
    assert "secret" not in json.dumps(error.audit_snapshot())


def test_real_seedance_treats_success_without_task_id_as_uncertain_submission():
    calls = []

    def invalid_success(request, timeout):
        calls.append((request, timeout))
        return FakeHttpResponse(200, {"status": "queued"})

    provider = ArkSeedanceProvider(
        api_key="secret",
        base_url="https://ark.cn-beijing.volces.com/api/v3",
        timeout_seconds=300,
        open_request=invalid_success,
    )

    with pytest.raises(SeedanceProviderError) as captured:
        provider.create(SeedanceRequest("镜头动作", [image("https://tos.example/image.png")], 15, "16:9", "1080p"))

    assert captured.value.code == "unknown_submission"
    assert len(calls) == 1


@pytest.mark.parametrize("status", ["queued", "running", "succeeded", "failed", "cancelled"])
def test_task_status_mapping_keeps_supported_states(status):
    task = SeedanceTask.from_payload({"id": "task-1", "status": status})
    assert task.status == status
