from __future__ import annotations

import json
from base64 import b64encode

import httpx
import pytest

from video_agent_api.adapters.in_memory import InMemoryUnitOfWork
from video_agent_api.application.catalog import CatalogService, RecordProviderCallCommand
from video_agent_api.application.runtime_composition import CatalogRuntimeResolver
from video_agent_api.domain.catalog import CapabilitySnapshot, Model, Provider, ProviderProfile
from video_agent_api.domain.errors import ValidationDomainError
from video_agent_api.ports.contracts import AdapterNotConfiguredError, ModelSelection
from video_agent_api.providers.agnes import AgnesVideoProvider
from video_agent_api.providers.gpt_image import GPTImageProvider
from video_agent_api.providers.text import OpenAICompatibleTextModelAdapter

_PNG = (
    b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01"
    b"\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\x0dIDAT\x08\xd7"
    b"c\xf8\xcf\xc0\xf0\x1f\x00\x05\x00\x01\xff\x89\x99=\x1d\x00\x00\x00"
    b"\x00IEND\xaeB`\x82"
)


def test_text_transport_uses_catalog_parameters_without_unverified_correlation_header() -> None:
    seen: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request)
        return httpx.Response(
            200,
            json={"id": "text-1", "choices": [{"message": {"content": "{}"}}]},
            request=request,
        )

    adapter = OpenAICompatibleTextModelAdapter(
        "https://text.example.test",
        "secret",
        transport=httpx.MockTransport(handler),
    )
    result = adapter.generate_text(
        "prompt",
        ModelSelection("provider", "profile", "catalog-model", "openai", {"temperature": 0.2}),
        "correlation",
    )

    assert result.request_id == "text-1"
    assert seen[0].url.path == "/v1/chat/completions"
    assert seen[0].headers["authorization"] == "Bearer secret"
    assert "x-correlation-id" not in seen[0].headers
    assert json.loads(seen[0].content) == {
        "model": "catalog-model",
        "messages": [{"role": "user", "content": "prompt"}],
        "response_format": {"type": "json_object"},
        "temperature": 0.2,
    }


def test_provider_protocol_headers_are_only_sent_when_snapshot_declares_them() -> None:
    seen: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request)
        return httpx.Response(
            200,
            json={"id": "text-1", "choices": [{"message": {"content": "{}"}}]},
            request=request,
        )

    adapter = OpenAICompatibleTextModelAdapter(
        "https://text.example.test",
        "secret",
        transport=httpx.MockTransport(handler),
        idempotency_key_header="X-Provider-Idempotency",
        correlation_header="X-Provider-Correlation",
    )
    adapter.generate_text("prompt", ModelSelection("p", "q", "m", "openai"), "corr-1")
    assert seen[0].headers["x-provider-idempotency"] == "corr-1"
    assert seen[0].headers["x-provider-correlation"] == "corr-1"


def test_gpt_image_transport_uses_official_paths_and_b64_json_without_headers() -> None:
    seen: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request)
        return httpx.Response(
            200,
            json={"data": [{"b64_json": b64encode(_PNG).decode("ascii")}]},
            request=request,
        )

    adapter = GPTImageProvider(
        configured=True,
        base_url="https://images.example.test",
        api_key="secret",
        http_transport=httpx.MockTransport(handler),
    )
    selection = ModelSelection("provider", "profile", "gpt-image", "gpt", {"size": "1024x1024"})
    generated = adapter.generate_image("draw", selection, "correlation")
    edited = adapter.edit_image("edit", selection, "correlation")

    assert [request.url.path for request in seen] == [
        "/v1/images/generations",
        "/v1/images/edits",
    ]
    assert all(request.headers["authorization"] == "Bearer secret" for request in seen)
    assert all("x-correlation-id" not in request.headers for request in seen)
    assert generated.payload["base64"] == b64encode(_PNG).decode("ascii")
    assert generated.payload["width"] == 1 and generated.payload["height"] == 1
    assert edited.payload["mimeType"] == "image/png"


def test_agnes_transport_only_exposes_evidenced_submit_and_poll() -> None:
    seen: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        seen.append(request)
        return httpx.Response(200, json={"id": "video-1", "status": "running"}, request=request)

    adapter = AgnesVideoProvider(
        configured=True,
        base_url="https://apihub.agnes-ai.com/v1",
        api_key="secret",
        http_transport=httpx.MockTransport(handler),
    )
    selection = ModelSelection("provider", "profile", "agnes-model", "agnes")
    submitted = adapter.submit_video("scene", selection, "correlation")
    polled = adapter.get_video_status("video-1", "correlation")

    assert submitted.request_id == "video-1"
    assert polled.payload["status"] == "running"
    assert [(request.method, request.url.path) for request in seen] == [
        ("POST", "/v1/videos"),
        ("GET", "/v1/videos/video-1"),
    ]
    assert all(request.headers["authorization"] == "Bearer secret" for request in seen)
    assert all("x-correlation-id" not in request.headers for request in seen)
    with pytest.raises(AdapterNotConfiguredError, match="operation_unconfigured"):
        adapter.cancel_video("video-1", "correlation")


@pytest.mark.asyncio
async def test_owner_ledger_reservation_is_released_after_terminal_path() -> None:
    uow = InMemoryUnitOfWork()
    provider = Provider("Live", "openai", "approved", "MVP-A", True, True)
    profile = ProviderProfile(
        provider.id,
        "Live",
        "openai",
        enabled=True,
        explicit_live_opt_in=True,
        credential_status="configured",
    )
    model = Model(profile.id, "catalog-model", enabled=True)
    profile.capability_snapshots["text.generate"] = CapabilitySnapshot(
        provider.id,
        profile.id,
        "text.generate",
        profile.revision,
        True,
        (),
        "probe",
        model.id,
    )
    uow.providers[provider.id] = provider
    uow.profiles[profile.id] = profile
    uow.models[model.id] = model
    resolver = CatalogRuntimeResolver(lambda: uow)

    identity = await resolver.resolve_invocation(profile.id, model.id, "text.generate")
    assert identity.operation == "text.generate"
    assert profile.active_operations == {}
    catalog = CatalogService(lambda: uow)
    recorded = await catalog.record_provider_call(
        RecordProviderCallCommand(
            project_id="project",
            run_id="run",
            node_run_id="node",
            logical_operation="text.generate:1",
            operation="text.generate",
            provider_id=provider.id,
            profile_id=profile.id,
            model_id=model.id,
            request_fingerprint="f" * 64,
            capability_snapshot_id=profile.capability_snapshots["text.generate"].id,
        )
    )
    assert recorded.status == "pending"
    assert profile.active_operations == {"text.generate": 1}
    with pytest.raises(ValidationDomainError, match="provider_operation_concurrency_exhausted"):
        await catalog.record_provider_call(
            RecordProviderCallCommand(
                project_id="project",
                run_id="run",
                node_run_id="node",
                logical_operation="text.generate:2",
                operation="text.generate",
                provider_id=provider.id,
                profile_id=profile.id,
                model_id=model.id,
                request_fingerprint="e" * 64,
                capability_snapshot_id=profile.capability_snapshots["text.generate"].id,
            )
        )
    assert len(uow.provider_calls) == 1
    claimed, acquired = await catalog.claim_provider_call(
        "run", "text.generate:1", expected_operation="text.generate"
    )
    assert acquired and claimed.status == "unknown"
    assert claimed.outbound_correlation
    assert profile.active_operations == {"text.generate": 1}
    await catalog.finalize_provider_call(
        "run", "text.generate:1", status="succeeded", provider_request_id="remote-1"
    )
    assert uow.profiles[profile.id].active_operations == {}


@pytest.mark.asyncio
async def test_resolver_identity_does_not_reserve_before_owner_durable_attempt() -> None:
    uow = InMemoryUnitOfWork()
    provider = Provider("Live", "openai", "approved", "MVP-A", True, True)
    profile = ProviderProfile(
        provider.id,
        "Live",
        "openai",
        enabled=True,
        explicit_live_opt_in=True,
        credential_status="configured",
    )
    model = Model(profile.id, "catalog-model", enabled=True)
    profile.capability_snapshots["text.generate"] = CapabilitySnapshot(
        provider.id,
        profile.id,
        "text.generate",
        profile.revision,
        True,
        (),
        "probe",
        model.id,
    )
    uow.providers[provider.id] = provider
    uow.profiles[profile.id] = profile
    uow.models[model.id] = model

    identity = await CatalogRuntimeResolver(lambda: uow).resolve_invocation(
        profile.id, model.id, "text.generate"
    )

    # Resolving a frozen identity may reject before the owner ledger exists, but it
    # must never leak a concurrency slot when the caller abandons before persistence.
    assert identity.operation == "text.generate"
    assert profile.active_operations == {}
