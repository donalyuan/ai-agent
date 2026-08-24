"""Fail-open local observability with W3C context and allowlisted telemetry."""

from __future__ import annotations

import json
import re
import secrets
from collections import Counter
from collections.abc import Callable
from dataclasses import dataclass, field
from datetime import UTC, datetime
from urllib.parse import urlencode, urlparse

from fastapi import Request
from starlette.middleware.base import BaseHTTPMiddleware, RequestResponseEndpoint
from starlette.responses import Response
from starlette.types import ASGIApp

_TRACEPARENT = re.compile(r"^00-([0-9a-f]{32})-([0-9a-f]{16})-(0[01])$")
_SECRET = re.compile(r"(?i)(api[_-]?key|authorization|password|token|secret|credential)")
_SAFE_FIELDS = {
    "timestamp",
    "severity",
    "service",
    "event",
    "trace_id",
    "span_id",
    "operation",
    "outcome",
    "error_code",
    "correlation_id",
    "owner_revision",
    "owner_hash",
}
_METRIC_LABELS = {
    "service",
    "route",
    "method",
    "status_class",
    "operation",
    "owner_status",
    "provider_key",
    "model_key",
    "adapter_key",
    "error_class",
    "outcome",
}


@dataclass(frozen=True, slots=True)
class TraceContext:
    trace_id: str
    span_id: str
    sampled: bool = True
    tracestate: str | None = None

    @property
    def traceparent(self) -> str:
        return f"00-{self.trace_id}-{self.span_id}-{'01' if self.sampled else '00'}"


def parse_traceparent(value: str | None) -> TraceContext:
    match = _TRACEPARENT.fullmatch(value or "")
    if match and match[1] != "0" * 32 and match[2] != "0" * 16:
        return TraceContext(match[1], match[2], match[3] == "01")
    return TraceContext(secrets.token_hex(16), secrets.token_hex(8))


def child_context(parent: TraceContext) -> TraceContext:
    return TraceContext(parent.trace_id, secrets.token_hex(8), parent.sampled, parent.tracestate)


def inject_trace(carrier: dict[str, str], context: TraceContext) -> dict[str, str]:
    injected = {**carrier, "traceparent": context.traceparent}
    if context.tracestate:
        injected["tracestate"] = context.tracestate
    return injected


def extract_trace(carrier: dict[str, str]) -> TraceContext:
    return parse_traceparent(carrier.get("traceparent"))


def trace_viewer_url(base_url: str | None, trace_id: str) -> str | None:
    context = parse_traceparent(f"00-{trace_id}-{'1' * 16}-01")
    if context.trace_id != trace_id:
        return None
    if base_url is None:
        return None
    parsed = urlparse(base_url)
    if (
        parsed.scheme not in {"http", "https"}
        or parsed.hostname
        not in {
            "127.0.0.1",
            "localhost",
        }
        or parsed.query
        or parsed.fragment
    ):
        return None
    return f"{base_url.rstrip('/')}?{urlencode({'trace_id': trace_id})}"


@dataclass(slots=True)
class InMemoryTelemetry:
    logs: list[dict[str, object]] = field(default_factory=list)
    metrics: Counter[tuple[str, tuple[tuple[str, str], ...]]] = field(default_factory=Counter)
    spans: list[dict[str, object]] = field(default_factory=list)
    diagnostics: list[str] = field(default_factory=list)
    exporter_available: bool = True
    max_records: int = 10_000

    def log(self, event: str, context: TraceContext, **fields: object) -> None:
        record: dict[str, object] = {
            "timestamp": datetime.now(UTC).isoformat(),
            "severity": "INFO",
            "service": "api",
            "event": event,
            "trace_id": context.trace_id,
            "span_id": context.span_id,
        }
        for key, value in fields.items():
            if key in _SAFE_FIELDS and not _SECRET.search(str(value)):
                record[key] = value
        if len(self.logs) < self.max_records:
            self.logs.append(record)

    def span(
        self,
        name: str,
        context: TraceContext,
        *,
        parent_span_id: str | None = None,
        operation: str = "",
        outcome: str = "success",
    ) -> TraceContext:
        child = child_context(context)
        if len(self.spans) < self.max_records:
            self.spans.append(
                {
                    "name": name,
                    "trace_id": child.trace_id,
                    "span_id": child.span_id,
                    "parent_span_id": parent_span_id or context.span_id,
                    "operation": operation,
                    "outcome": outcome,
                }
            )
        return child

    def exporter_failure(self, diagnostic: str = "telemetry_export_unavailable") -> None:
        self.exporter_available = False
        if diagnostic not in self.diagnostics:
            self.diagnostics.append(diagnostic)

    def count(self, name: str, **labels: str) -> None:
        if any(key not in _METRIC_LABELS for key in labels):
            return
        if any(_SECRET.search(key) or key.endswith("_id") for key in labels):
            return
        bounded = tuple(sorted((key, value) for key, value in labels.items()))
        self.metrics[(name, bounded)] += 1

    def json_lines(self) -> str:
        return "\n".join(json.dumps(item, sort_keys=True) for item in self.logs)


def observe_call[T](
    telemetry: InMemoryTelemetry,
    context: TraceContext,
    operation: str,
    function: Callable[[], T],
) -> T:
    """Instrument an adapter call without changing its result or retry semantics."""
    try:
        result = function()
    except Exception as error:
        telemetry.log(
            "adapter.operation",
            context,
            operation=operation,
            outcome="error",
            error_code=type(error).__name__,
        )
        telemetry.count("adapter_operations_total", operation=operation, outcome="error")
        raise
    telemetry.log("adapter.operation", context, operation=operation, outcome="success")
    telemetry.count("adapter_operations_total", operation=operation, outcome="success")
    return result


class TraceMiddleware(BaseHTTPMiddleware):
    def __init__(self, app: ASGIApp, telemetry: InMemoryTelemetry) -> None:
        super().__init__(app)
        self.telemetry = telemetry

    async def dispatch(self, request: Request, call_next: RequestResponseEndpoint) -> Response:
        raw_traceparent = request.headers.get("traceparent")
        parent = parse_traceparent(raw_traceparent)
        context = TraceContext(
            parent.trace_id,
            secrets.token_hex(8),
            parent.sampled,
            request.headers.get("tracestate"),
        )
        request.state.trace_context = context
        if raw_traceparent and parent.trace_id not in raw_traceparent:
            self.telemetry.log(
                "trace.invalid_header",
                context,
                operation="http.request",
                outcome="rejected",
                error_code="invalid_traceparent",
            )
        try:
            response = await call_next(request)
            outcome = "success" if response.status_code < 500 else "error"
        except Exception:
            self.telemetry.count(
                "http_requests_total", method=request.method, route="unmatched", outcome="error"
            )
            raise
        response.headers["traceparent"] = context.traceparent
        response.headers["x-trace-id"] = context.trace_id
        self.telemetry.span(
            "http.request",
            context,
            parent_span_id=parent.span_id,
            operation=request.method,
            outcome=outcome,
        )
        self.telemetry.log("http.request", context, operation=request.method, outcome=outcome)
        route = request.scope.get("route")
        route_template = getattr(route, "path", "unmatched")
        self.telemetry.count(
            "http_requests_total",
            method=request.method,
            route=route_template,
            outcome=outcome,
        )
        return response
