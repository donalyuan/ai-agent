from __future__ import annotations

import json
import logging
from pathlib import Path

import pytest
from fastapi.testclient import TestClient
from pydantic import ValidationError

from video_agent_api.app import create_app
from video_agent_api.db import check_database
from video_agent_api.domain.schemas import RevisionUpdateCommand, WorkflowDraftBoundary
from video_agent_api.domain.services import (
    ImmutableVersionError,
    RevisionConflictError,
    WorkflowService,
    require_valid_transition,
)
from video_agent_api.logging import JsonFormatter, redact_event
from video_agent_api.ports.config import ProviderCatalog
from video_agent_api.ports.contracts import (
    AdapterNotConfiguredError,
    DisabledConfigurationError,
    ModelSelection,
)
from video_agent_api.ports.mocks import DeterministicMockProvider
from video_agent_api.ports.storage import LocalWorkspaceAdapter, TOSAdapter
from video_agent_api.skills.registry import SkillRegistry
from video_agent_api.skills.router import (
    RouteContext,
    SemanticAdapterUnavailableError,
    SkillRouter,
)


def test_domain_revision_state_and_immutable_versions() -> None:
    require_valid_transition("draft", "generated")
    with pytest.raises(ValueError, match="invalid state transition"):
        require_valid_transition("approved", "draft")

    workflow = WorkflowService()
    with pytest.raises(RevisionConflictError) as conflict:
        workflow.update_draft(draft_id="draft-1", expected_revision=1, actual_revision=2)
    assert conflict.value.current_revision == 2

    with pytest.raises(ImmutableVersionError):
        workflow.update_published_version("workflow-version-1")
    with pytest.raises(ValidationError):
        RevisionUpdateCommand(expected_revision=0)
    with pytest.raises(ValidationError):
        WorkflowDraftBoundary(
            id="draft-1",
            schema_version="1.0.0",
            revision=0,
            status="draft",
            project_id="project-1",
            scope_type="episode",
            scope_ids=[],
            definition={"nodes": []},
        )


def test_workflow_draft_boundary_consumes_the_contract_example() -> None:
    path = Path(__file__).parents[3] / "packages/contracts/examples/workflow-draft.valid.json"
    payload = json.loads(path.read_text(encoding="utf-8"))
    boundary = WorkflowDraftBoundary.model_validate(payload)
    assert boundary.model_dump(mode="json", by_alias=True) == payload
    with pytest.raises(ValidationError, match="Extra inputs are not permitted"):
        WorkflowDraftBoundary.model_validate({**payload, "unexpected": True})


@pytest.mark.parametrize(
    ("field", "invalid_value"),
    [
        ("id", "not-a-uuid"),
        ("projectId", "not-a-uuid"),
        ("schema_version", "v1"),
        ("revision", "0"),
        ("revision", False),
        ("revision", True),
        ("scopeType", "global"),
        (
            "scopeIds",
            [
                "00000000-0000-4000-8000-000000000003",
                "00000000-0000-4000-8000-000000000003",
            ],
        ),
        ("scopeIds", ["not-a-uuid"]),
    ],
)
def test_workflow_draft_boundary_rejects_schema_invalid_values(
    field: str, invalid_value: object
) -> None:
    path = Path(__file__).parents[3] / "packages/contracts/examples/workflow-draft.valid.json"
    payload = json.loads(path.read_text(encoding="utf-8"))

    with pytest.raises(ValidationError):
        WorkflowDraftBoundary.model_validate({**payload, field: invalid_value})


def test_api_dockerfile_uses_the_frozen_uv_lock() -> None:
    dockerfile = (Path(__file__).parents[1] / "Dockerfile").read_text(encoding="utf-8")

    assert "ghcr.io/astral-sh/uv:0.10.12" in dockerfile
    assert "uv lock --project /workspace/services/api --check" in dockerfile
    assert "uv sync" in dockerfile
    assert "--frozen" in dockerfile
    assert "--no-dev" in dockerfile
    assert "/workspace/services/api/.venv/bin" in dockerfile
    assert "pip install" not in dockerfile


def test_mock_provider_is_deterministic_and_configuration_is_explicit() -> None:
    provider = DeterministicMockProvider()
    selection = ModelSelection(
        provider_id="mock-provider",
        profile_id="mock-profile",
        model_id="mock-model",
        adapter_key="mock",
        default_parameters={"temperature": 0},
    )
    first = provider.generate_text(prompt="hello", selection=selection, correlation_id="trace-1")
    second = provider.generate_text(prompt="hello", selection=selection, correlation_id="trace-1")
    assert first == second
    assert first.request_id.startswith("mock-")

    catalog = ProviderCatalog.empty()
    with pytest.raises(AdapterNotConfiguredError):
        catalog.select("missing")
    catalog.add_profile(profile_id="disabled", enabled=False, selection=selection)
    with pytest.raises(DisabledConfigurationError):
        catalog.select("disabled")


def test_local_workspace_uses_abstract_references_and_rejects_escape(tmp_path: Path) -> None:
    storage = LocalWorkspaceAdapter(tmp_path / "workspace")
    stored = storage.put("project/asset.txt", b"hello", correlation_id="trace-2")
    assert stored.object_ref == "workspace://project/asset.txt"
    assert storage.get(stored.object_ref) == b"hello"
    assert str(tmp_path) not in stored.object_ref
    with pytest.raises(ValueError, match="escapes workspace root"):
        storage.put("../escape.txt", b"no", correlation_id="trace-2")
    with pytest.raises(AdapterNotConfiguredError):
        TOSAdapter().stat("tos://unconfigured/object")
    upload = storage.create_multipart_session("project/large.bin", correlation_id="trace-2")
    first = storage.upload_part(upload.session_id, 1, b"hel", correlation_id="trace-2")
    second = storage.upload_part(upload.session_id, 2, b"lo", correlation_id="trace-2")
    completed = storage.complete_multipart_session(
        upload.session_id, [first, second], correlation_id="trace-2"
    )
    assert storage.get(completed.object_ref) == b"hello"


def test_skill_registry_and_router_are_deterministic(tmp_path: Path) -> None:
    skill_dir = tmp_path / "skills" / "drama"
    skill_dir.mkdir(parents=True)
    (skill_dir / "manifest.yaml").write_text(
        "\n".join(
            [
                "name: drama",
                "version: 1.0.0",
                "source_commit: abc123",
                "license: MIT",
                "enabled: true",
                "stages: [script]",
                "project_types: [short_drama]",
                "capabilities: [scene_writing]",
                "allowed_tools: [text_model]",
                "target_models: [configured-model]",
                "input_schema: {type: object}",
                "output_schema: {type: object}",
                "priority: 10",
            ]
        ),
        encoding="utf-8",
    )
    (skill_dir / "SKILL.md").write_text("# Drama\n", encoding="utf-8")
    registry = SkillRegistry(tmp_path / "skills")
    registry.load()
    assert registry.resolve("drama", "1.0.0").source_commit == "abc123"
    assert registry.read("drama", "1.0.0") == "# Drama\n"

    context = RouteContext(
        project_type="short_drama",
        stage="script",
        target_model="configured-model",
        query="write a scene",
        allowed_tools={"text_model"},
        allowed_licenses={"MIT"},
    )
    first = SkillRouter(registry).route(context)
    second = SkillRouter(registry).route(context)
    assert [candidate.name for candidate in first.candidates] == ["drama"]
    assert first == second
    assert first.selected is None
    assert first.needs_manual_selection is True

    class UnavailableAdapter:
        def rank(self, query: str, candidates: object) -> tuple[object, float]:
            raise SemanticAdapterUnavailableError("not installed")

    unavailable = SkillRouter(registry, UnavailableAdapter()).route(context)
    assert unavailable.needs_manual_selection is True
    assert (
        unavailable.fallback_reason
        == "semantic_adapter_unavailable:SemanticAdapterUnavailableError"
    )

    class LowConfidenceAdapter:
        def rank(
            self, query: str, candidates: tuple[object, ...]
        ) -> tuple[tuple[object, ...], float]:
            return candidates, 0.2

    low_confidence = SkillRouter(registry, LowConfidenceAdapter()).route(context)
    assert low_confidence.needs_manual_selection is True
    assert low_confidence.fallback_reason == "semantic_adapter_low_confidence"

    tie_dir = tmp_path / "skills" / "tie"
    tie_dir.mkdir()
    (tie_dir / "manifest.yaml").write_text(
        "\n".join(
            [
                "name: tie",
                "version: 1.0.0",
                "source_commit: def456",
                "license: MIT",
                "enabled: true",
                "stages: [script]",
                "project_types: [short_drama]",
                "capabilities: [scene_writing]",
                "allowed_tools: [text_model]",
                "target_models: [configured-model]",
                "input_schema: {type: object}",
                "output_schema: {type: object}",
                "priority: 10",
            ]
        ),
        encoding="utf-8",
    )
    registry.load()
    assert SkillRouter(registry).route(context).needs_manual_selection is True

    class BrokenAdapter:
        def rank(self, query: str, candidates: object) -> tuple[object, float]:
            raise RuntimeError("unexpected semantic adapter error")

    with pytest.raises(RuntimeError, match="unexpected semantic adapter error"):
        SkillRouter(registry, BrokenAdapter()).route(context)


def test_health_and_structured_logging_do_not_expose_secrets() -> None:
    app = create_app(readiness_probe=lambda: True)
    client = TestClient(app)
    assert client.get("/v1/health/live").json() == {"status": "live"}
    assert client.get("/v1/health/ready").json() == {"status": "ready"}
    event = redact_event(
        {"api_key": "secret", "authorization": "Bearer secret", "response": {"private": "x"}}
    )
    assert "secret" not in str(event)
    assert event["api_key"] == "[REDACTED]"
    record = logging.makeLogRecord(
        {
            "msg": "provider.call",
            "levelno": logging.INFO,
            "levelname": "INFO",
            "event": "provider.call",
            "event_data": {"authorization": "Bearer secret", "result": "safe"},
        }
    )
    serialized = JsonFormatter().format(record)
    assert "secret" not in serialized
    assert json.loads(serialized)["event"] == "provider.call"


async def test_database_readiness_probe_uses_a_non_mutating_query() -> None:
    assert await check_database("sqlite+aiosqlite:///:memory:") is True
