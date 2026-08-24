from __future__ import annotations

import json
from dataclasses import dataclass
from hashlib import sha256
from urllib.parse import urljoin

import httpx

from video_agent_api.ports.contracts import (
    AdapterNotConfiguredError,
    ModelSelection,
    PortResult,
    TextModelPort,
)
from video_agent_api.ports.mocks import build_mock_text_output


@dataclass(slots=True)
class MockTextModelProvider(TextModelPort):
    """本地结构化候选生成器；不声称真实模型成功。"""

    def generate_text(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        digest = sha256(prompt.encode()).hexdigest()
        return PortResult(
            f"mock-text-{digest[:12]}",
            correlation_id,
            {
                "status": "mock",
                "schema_version": "1.0.0",
                "promptHash": digest,
                "modelId": selection.model_id,
                "payload": build_mock_text_output(prompt),
            },
        )


@dataclass(slots=True)
class OpenAICompatibleTextModelAdapter(TextModelPort):
    """OpenAI-compatible transport; disabled until an explicit profile is configured."""

    base_url: str | None = None
    api_key: str | None = None
    timeout_seconds: float = 30.0
    max_retries: int = 2
    transport: httpx.BaseTransport | None = None

    def _configured(self) -> tuple[str, str]:
        if not self.base_url or not self.api_key:
            raise AdapterNotConfiguredError("agentscope_text_runtime_unconfigured")
        return self.base_url.rstrip("/") + "/", self.api_key

    def list_models(self) -> list[dict[str, object]]:
        base, key = self._configured()
        with httpx.Client(transport=self.transport, follow_redirects=False) as client:
            response = client.get(
                urljoin(base, "v1/models"),
                headers={"Authorization": f"Bearer {key}"},
                timeout=self.timeout_seconds,
            )
        response.raise_for_status()
        payload = response.json()
        data = payload.get("data") if isinstance(payload, dict) else None
        return [item for item in data if isinstance(item, dict)] if isinstance(data, list) else []

    def generate_text(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        base, key = self._configured()
        payload = {
            "model": selection.model_id,
            "messages": [{"role": "user", "content": prompt}],
            "response_format": {"type": "json_object"},
        }
        last_error: Exception | None = None
        with httpx.Client(transport=self.transport, follow_redirects=False) as client:
            for attempt in range(self.max_retries + 1):
                try:
                    response = client.post(
                        urljoin(base, "v1/chat/completions"),
                        headers={
                            "Authorization": f"Bearer {key}",
                            "X-Correlation-ID": correlation_id,
                        },
                        json=payload,
                        timeout=self.timeout_seconds,
                    )
                except (httpx.TimeoutException, httpx.TransportError) as error:
                    last_error = error
                    if attempt < self.max_retries:
                        continue
                    raise
                if response.status_code == 429 or response.status_code >= 500:
                    if attempt < self.max_retries:
                        continue
                response.raise_for_status()
                body = response.json()
                choices = body.get("choices") if isinstance(body, dict) else None
                content = choices[0].get("message", {}).get("content") if choices else None
                if not isinstance(content, str):
                    raise ValueError("structured_response_missing")
                parsed = json.loads(content)
                if not isinstance(parsed, dict):
                    raise ValueError("structured_response_not_object")
                usage = body.get("usage") if isinstance(body, dict) else None
                request_id = str(
                    response.headers.get("x-request-id") or body.get("id") or "unknown"
                )
                return PortResult(
                    request_id,
                    correlation_id,
                    {"status": "live", "payload": parsed, "usage": usage or {}},
                )
        raise RuntimeError("text adapter failed") from last_error
