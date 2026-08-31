from __future__ import annotations

import json
from pathlib import Path

from fastapi import FastAPI
from fastapi.testclient import TestClient

from video_agent_api.observability import (
    InMemoryTelemetry,
    TraceMiddleware,
    child_context,
    extract_trace,
    inject_trace,
    observe_call,
    parse_traceparent,
    trace_viewer_url,
)
from video_agent_api.resilience import (
    CapacitySnapshot,
    OperationsResilienceCoordinator,
    RuntimeResourceSnapshot,
    admit,
    aggregate_capacity,
    capacity_snapshot,
    check_recovery_requirements,
    cleanup_allowed,
    probe_resources,
    verify_restore,
)


def _snapshot(**overrides: object) -> RuntimeResourceSnapshot:
    values: dict[str, object] = {
        "cpu_count": 4,
        "available_concurrency": 4,
        "memory_available_bytes": 8_000,
        "memory_limit_bytes": 16_000,
        "disk_free_bytes": 5_000,
        "disk_total_bytes": 10_000,
        "captured_at": "2026-08-23T00:00:00+00:00",
    }
    values.update(overrides)
    return RuntimeResourceSnapshot(**values)  # type: ignore[arg-type]


def test_resource_probe_and_capacity_projection_are_read_only(tmp_path: Path) -> None:
    snapshot = probe_resources(tmp_path)
    capacity = capacity_snapshot(snapshot, "local_workspace")
    assert snapshot.schema_version == "1.0.0"
    assert snapshot.cpu_count >= 1
    assert capacity.limit_bytes == snapshot.disk_total_bytes
    assert capacity.observed_bytes == (snapshot.disk_total_bytes or 0) - (
        snapshot.disk_free_bytes or 0
    )

    unavailable = probe_resources(tmp_path / "missing")
    decision = admit(unavailable)
    assert decision.allowed is False
    assert decision.diagnostic == "resource_probe_unavailable"


def test_admission_distinguishes_capability_soft_and_hard_limits() -> None:
    unsupported = admit(_snapshot(cpu_count=1), min_cpu=2)
    assert unsupported.diagnostic == "resource_capability_unsupported"
    assert unsupported.required == 2

    soft = admit(_snapshot(disk_free_bytes=900), soft_ratio=0.1, hard_ratio=0.05)
    assert soft.allowed is True
    assert soft.warning == "resource_capacity_soft_limit"

    hard = admit(_snapshot(disk_free_bytes=400), required_bytes=1)
    assert hard.allowed is False
    assert hard.diagnostic == "resource_capacity_hard_limit"


def test_operations_resilience_freezes_deterministic_revalidatable_admission() -> None:
    resource = _snapshot(revision=7, config_revision=3)
    capacity = capacity_snapshot(resource, "project-1")
    coordinator = OperationsResilienceCoordinator(resource, capacity)

    accepted = coordinator.freeze("project-1", "image.generate", "op-1", required_bytes=10)
    assert accepted.allowed is True
    assert accepted.reference.startswith("resilience:")
    assert accepted.resource_hash == coordinator.resource_hash
    assert accepted.capacity_hash == coordinator.capacity_hash
    assert coordinator.revalidate(accepted).allowed is True

    changed = OperationsResilienceCoordinator(_snapshot(revision=8), capacity)
    stale = changed.revalidate(accepted)
    assert stale.allowed is False
    assert stale.diagnostic == "resource_snapshot_stale"


def test_admission_revalidates_across_api_worker_reprobe_with_new_capture_time() -> None:
    api_resource = _snapshot(captured_at="2026-08-26T00:00:00+00:00", revision=7)
    api_capacity = capacity_snapshot(api_resource, "project-1")
    api = OperationsResilienceCoordinator(api_resource, api_capacity)
    frozen = api.freeze("project-1", "media.dispatch", "media:source:1")

    worker_resource = _snapshot(captured_at="2026-08-26T00:01:00+00:00", revision=7)
    worker_capacity = capacity_snapshot(worker_resource, "project-1")
    worker = OperationsResilienceCoordinator(worker_resource, worker_capacity)

    assert worker.resource_hash == api.resource_hash
    assert worker.capacity_hash == api.capacity_hash
    assert worker.revalidate(frozen).allowed is True


def test_capacity_aggregation_and_recovery_are_owner_safe() -> None:
    snapshots = [
        CapacitySnapshot("workspace", 40, 100, 60, "2026-08-23T00:00:00+00:00", "local"),
        CapacitySnapshot("derivative", 20, 50, 30, "2026-08-23T00:00:01+00:00", "worker"),
    ]
    aggregate = aggregate_capacity(snapshots)
    assert aggregate.observed_bytes == 60
    assert aggregate.limit_bytes == 150
    assert aggregate.available_bytes == 90
    assert (
        check_recovery_requirements(
            required=("postgres", "manifest"), available=("postgres",)
        ).status
        == "blocked"
    )
    assert cleanup_allowed(referenced=False, hold=False, retention_expired=True)
    assert not cleanup_allowed(referenced=True, hold=False, retention_expired=True)


def test_restore_evidence_requires_exact_identity_and_stable_operator() -> None:
    operator = "11111111-1111-4111-8111-111111111111"
    passed = verify_restore(
        "restore:fixture:1",
        "a" * 64,
        "a" * 64,
        "etag-1",
        "etag-1",
        2,
        operator_uuid=operator,
        correlation_id="restore-correlation-1",
    )
    assert passed.status == "passed"
    assert passed.diagnostic is None

    mismatch = verify_restore(
        "restore:fixture:1",
        "a" * 64,
        "b" * 64,
        "etag-1",
        "etag-2",
        2,
        operator_uuid=operator,
    )
    assert mismatch.status == "failed"
    assert mismatch.diagnostic == "restore_object_identity_mismatch"


def test_trace_context_logs_and_metrics_are_safe_and_bounded() -> None:
    invalid = parse_traceparent("00-not-a-trace")
    assert len(invalid.trace_id) == 32
    assert invalid.trace_id != "0" * 32

    telemetry = InMemoryTelemetry()
    telemetry.log(
        "provider.failed",
        invalid,
        operation="image.generate",
        outcome="error",
        error_code="authentication_failed",
        credential="api-key-secret",
    )
    line = json.loads(telemetry.json_lines())
    assert line["error_code"] == "authentication_failed"
    assert "credential" not in line
    assert "api-key-secret" not in telemetry.json_lines()

    telemetry.count("provider_operations_total", operation="image", outcome="error")
    telemetry.count("provider_operations_total", project_id="unbounded")
    assert sum(telemetry.metrics.values()) == 1


def test_http_trace_uses_route_template_and_exporter_is_fail_open() -> None:
    telemetry = InMemoryTelemetry()
    app = FastAPI()
    app.add_middleware(TraceMiddleware, telemetry=telemetry)

    @app.get("/items/{item_id}")
    def item(item_id: str) -> dict[str, str]:
        return {"id": item_id}

    with TestClient(app) as client:
        response = client.get(
            "/items/owner-123",
            headers={"traceparent": "00-" + "a" * 32 + "-" + "b" * 16 + "-01"},
        )
    assert response.status_code == 200
    assert response.headers["x-trace-id"] == "a" * 32
    labels = next(iter(telemetry.metrics))[1]
    assert ("route", "/items/{item_id}") in labels
    assert "owner-123" not in repr(telemetry.metrics)


def test_async_carrier_adapter_wrapper_and_viewer_are_safe() -> None:
    root = parse_traceparent("00-" + "a" * 32 + "-" + "b" * 16 + "-01")
    carrier = inject_trace({}, root)
    extracted = extract_trace(carrier)
    worker = child_context(extracted)
    assert worker.trace_id == root.trace_id
    assert worker.span_id != root.span_id

    telemetry = InMemoryTelemetry()
    assert observe_call(telemetry, worker, "storage.stat", lambda: "ok") == "ok"

    def fail() -> None:
        raise RuntimeError("provider secret must not be logged")

    try:
        observe_call(telemetry, worker, "provider.submit", fail)
    except RuntimeError:
        pass
    assert "provider secret" not in telemetry.json_lines()
    assert trace_viewer_url("http://127.0.0.1:16686/search", root.trace_id) is not None
    assert trace_viewer_url("https://telemetry.example.com", root.trace_id) is None
    assert trace_viewer_url("javascript:alert(1)", root.trace_id) is None
    assert trace_viewer_url("http://127.0.0.1:16686/search?unsafe=1", root.trace_id) is None


def test_span_lineage_and_exporter_failure_are_fail_open() -> None:
    telemetry = InMemoryTelemetry()
    root = parse_traceparent("00-" + "c" * 32 + "-" + "d" * 16 + "-01")
    child = telemetry.span("workflow.start", root, operation="text.generate")
    grandchild = telemetry.span(
        "provider.submit", child, parent_span_id=child.span_id, operation="image.generate"
    )
    telemetry.exporter_failure()
    assert grandchild.trace_id == root.trace_id
    assert telemetry.spans[-1]["parent_span_id"] == child.span_id
    assert telemetry.diagnostics == ["telemetry_export_unavailable"]
    assert telemetry.exporter_available is False


def test_telemetry_projection_is_bounded_and_not_a_business_ledger() -> None:
    telemetry = InMemoryTelemetry()
    app = FastAPI()
    app.add_middleware(TraceMiddleware, telemetry=telemetry)

    @app.get("/diagnostics")
    def diagnostics() -> dict[str, object]:
        return {
            "traceId": "a" * 32,
            "runId": "run-secret-free-reference",
            "prompt": "must never be emitted",
        }

    with TestClient(app) as client:
        response = client.get("/diagnostics")
    assert response.status_code == 200
    assert len(telemetry.spans) == 1
    assert "must never be emitted" not in telemetry.json_lines()
