from __future__ import annotations

from pathlib import Path

import pytest
import yaml

from video_agent_api.app import create_app
from video_agent_api.ports.contracts import AdapterNotConfiguredError, ModelSelection
from video_agent_api.ports.mocks import DeterministicMockProvider
from video_agent_api.ports.storage import LocalWorkspaceAdapter, TOSAdapter
from video_agent_api.runtime import RuntimeSettings, build_runtime


def _example_environment() -> dict[str, str]:
    path = Path(__file__).parents[3] / ".env.example"
    return dict(
        line.split("=", 1)
        for line in path.read_text(encoding="utf-8").splitlines()
        if line and not line.startswith("#")
    )


def test_example_environment_selects_mock_and_local_workspace() -> None:
    settings = RuntimeSettings.from_mapping(_example_environment())
    runtime = build_runtime(settings)

    assert runtime.provider_mode == "mock"
    assert runtime.storage_mode == "local_workspace"
    assert isinstance(runtime.provider, DeterministicMockProvider)
    assert isinstance(runtime.storage, LocalWorkspaceAdapter)
    assert runtime.storage.root == Path("/workspace/data/workspaces")


@pytest.mark.parametrize(
    ("provider_mode", "storage_mode", "message"),
    [
        ("unknown", "local_workspace", "unsupported provider mode"),
        ("mock", "unknown", "unsupported storage mode"),
    ],
)
def test_runtime_rejects_unknown_modes_without_fallback(
    provider_mode: str, storage_mode: str, message: str, tmp_path: Path
) -> None:
    settings = RuntimeSettings(provider_mode, storage_mode, tmp_path)

    with pytest.raises(AdapterNotConfiguredError, match=message):
        build_runtime(settings)


def test_runtime_exposes_explicit_unconfigured_tos(tmp_path: Path) -> None:
    runtime = build_runtime(RuntimeSettings("mock", "tos", tmp_path))

    assert isinstance(runtime.storage, TOSAdapter)
    with pytest.raises(AdapterNotConfiguredError, match="not configured"):
        runtime.storage.put("asset.bin", b"content", correlation_id="trace-tos")


def test_api_consumes_runtime_settings(tmp_path: Path) -> None:
    app = create_app(
        readiness_probe=lambda: True,
        settings=RuntimeSettings("mock", "local_workspace", tmp_path),
    )

    assert app.state.runtime.provider_mode == "mock"
    assert app.state.runtime.storage_mode == "local_workspace"


def test_compose_passes_runtime_modes_to_api_and_all_workers() -> None:
    path = Path(__file__).parents[3] / "infra/compose/compose.yaml"
    compose = yaml.safe_load(path.read_text(encoding="utf-8"))

    for service_name in ("api", "agent", "generation", "media"):
        environment = compose["services"][service_name]["environment"]
        assert environment["PROVIDER_MODE"] == "${PROVIDER_MODE}"
        assert environment["STORAGE_MODE"] == "${STORAGE_MODE}"
        assert environment["WORKSPACE_ROOT"] == "${WORKSPACE_ROOT}"


def test_mock_provider_and_storage_emit_structured_boundary_events(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    provider_events: list[tuple[str, str | None, dict[str, object]]] = []
    storage_events: list[tuple[str, str | None, dict[str, object]]] = []

    def capture_provider(event: str, *, correlation_id: str | None = None, **data: object) -> None:
        provider_events.append((event, correlation_id, data))

    def capture_storage(event: str, *, correlation_id: str | None = None, **data: object) -> None:
        storage_events.append((event, correlation_id, data))

    monkeypatch.setattr("video_agent_api.ports.mocks.log_event", capture_provider)
    monkeypatch.setattr("video_agent_api.ports.storage.log_event", capture_storage)

    selection = ModelSelection("mock", "profile", "model", "mock")
    provider = DeterministicMockProvider()
    provider.generate_text("hello", selection, correlation_id="trace-provider")
    with pytest.raises(RuntimeError, match="explicit error"):
        provider.generate_text("__mock_error__", selection, correlation_id="trace-error")

    storage = LocalWorkspaceAdapter(tmp_path)
    storage.put("asset.txt", b"content", correlation_id="trace-storage")
    with pytest.raises(ValueError, match="escapes workspace root"):
        storage.put("../escape.txt", b"content", correlation_id="trace-escape")
    with pytest.raises(AdapterNotConfiguredError):
        TOSAdapter().put("asset.bin", b"content", correlation_id="trace-tos")

    assert provider_events == [
        (
            "provider.call",
            "trace-provider",
            {"operation": "text.generate", "adapter": "mock", "result": "success"},
        ),
        (
            "provider.call",
            "trace-error",
            {
                "operation": "text.generate",
                "adapter": "mock",
                "result": "error",
                "error_type": "RuntimeError",
            },
        ),
    ]
    assert storage_events == [
        (
            "storage.call",
            "trace-storage",
            {"operation": "put", "adapter": "local_workspace", "result": "success"},
        ),
        (
            "storage.call",
            "trace-escape",
            {
                "operation": "put",
                "adapter": "local_workspace",
                "result": "error",
                "error_type": "ValueError",
            },
        ),
        (
            "storage.call",
            "trace-tos",
            {
                "operation": "put",
                "adapter": "tos",
                "result": "error",
                "error_type": "AdapterNotConfiguredError",
            },
        ),
    ]
