from __future__ import annotations

from base64 import b64decode
from collections.abc import Callable
from dataclasses import dataclass
from hashlib import sha256
from typing import Literal
from urllib.parse import urljoin

import httpx

from video_agent_api.ports.contracts import (
    AdapterNotConfiguredError,
    ModelSelection,
    PortResult,
    VideoGenerationPort,
)


@dataclass(slots=True)
class AgnesVideoProvider(VideoGenerationPort):
    configured: bool = False
    operations: dict[str, dict[str, object]] | None = None
    transport: Callable[[str, dict[str, object], ModelSelection, str], PortResult] | None = None
    base_url: str | None = None
    api_key: str | None = None
    http_transport: httpx.BaseTransport | None = None
    timeout_seconds: float = 30.0
    idempotency_key_header: str | None = None
    correlation_header: str | None = None

    def probe_capabilities(self, advertised: list[dict[str, object]]) -> dict[str, object]:
        """Return one account-observed stable mode; callers persist this snapshot."""
        candidates = [
            item
            for item in advertised
            if item.get("operation") == "submit"
            and item.get("version") != "2.5"
            and item.get("preview") is not True
        ]
        if not candidates:
            raise AdapterNotConfiguredError("agnes_video_capability_unconfigured")
        candidates.sort(key=lambda item: (item.get("version") != "2.0", str(item.get("id", ""))))
        selected = dict(candidates[0])
        mode_id = selected.get("id")
        if not isinstance(mode_id, str) or not mode_id:
            raise ValueError("agnes_video_probe_mode_id_missing")
        self.operations = {"submit": selected}
        return selected

    def _unconfigured(self) -> None:
        if not self.configured:
            raise AdapterNotConfiguredError("agnes_video_provider_unconfigured")

    def submit_video(
        self, prompt: str, selection: ModelSelection, correlation_id: str
    ) -> PortResult:
        self._unconfigured()
        return self._request("submit", {"prompt": prompt}, selection, correlation_id)

    def get_video_status(self, job_id: str, correlation_id: str) -> PortResult:
        self._unconfigured()
        return self._request(
            "poll", {"providerRequestId": job_id}, ModelSelection("", "", "", ""), correlation_id
        )

    def cancel_video(self, job_id: str, correlation_id: str) -> PortResult:
        self._unconfigured()
        return self._request(
            "cancel", {"providerRequestId": job_id}, ModelSelection("", "", "", ""), correlation_id
        )

    def _request(
        self,
        operation: str,
        payload: dict[str, object],
        selection: ModelSelection,
        correlation_id: str,
    ) -> PortResult:
        if self.operations is not None and operation not in self.operations:
            raise AdapterNotConfiguredError("agnes_video_operation_unconfigured")
        if self.transport is not None:
            result = self.transport(operation, payload, selection, correlation_id)
            if not isinstance(result, PortResult):
                raise ValueError("agnes_video_result_invalid")
            return result
        if not self.base_url or not self.api_key:
            raise AdapterNotConfiguredError("agnes_video_transport_unconfigured")
        if operation not in {"submit", "poll"}:
            raise AdapterNotConfiguredError("agnes_video_operation_unconfigured")
        base = self.base_url.rstrip("/") + "/"
        headers = {"Authorization": f"Bearer {self.api_key}"}
        if self.idempotency_key_header:
            headers[self.idempotency_key_header] = correlation_id
        if self.correlation_header:
            headers[self.correlation_header] = correlation_id
        with httpx.Client(transport=self.http_transport, follow_redirects=False) as client:
            if operation == "submit":
                response = client.post(
                    urljoin(base, "videos"),
                    headers=headers,
                    json={"model": selection.model_id, "prompt": payload["prompt"]},
                    timeout=self.timeout_seconds,
                )
            else:
                request_id = payload.get("providerRequestId")
                if not isinstance(request_id, str) or not request_id:
                    raise ValueError("agnes_video_request_id_invalid")
                response = client.get(
                    urljoin(base, f"videos/{request_id}"),
                    headers=headers,
                    timeout=self.timeout_seconds,
                )
        response.raise_for_status()
        body = response.json()
        if not isinstance(body, dict):
            raise ValueError("agnes_video_result_invalid")
        request_id = body.get("id") or body.get("video_id") or payload.get("providerRequestId")
        if not isinstance(request_id, str) or not request_id:
            raise ValueError("agnes_video_result_request_id_missing")
        return PortResult(request_id, correlation_id, dict(body))

    def validate_capability(self, operation: str, parameters: dict[str, object]) -> None:
        if self.operations is None or operation not in self.operations:
            raise AdapterNotConfiguredError("agnes_video_mode_unconfigured")
        capability = self.operations[operation]
        if capability.get("preview") is True or capability.get("version") == "2.5":
            raise AdapterNotConfiguredError("agnes_video_preview_not_supported")
        allowed = capability.get("parameters")
        if isinstance(allowed, dict):
            for key, value in parameters.items():
                if (
                    key in allowed
                    and isinstance(allowed[key], (list, tuple))
                    and value not in allowed[key]
                ):
                    raise ValueError("agnes_video_parameter_unsupported")

    @staticmethod
    def validate_video_media(
        content: bytes,
        mime_type: str,
        *,
        duration_seconds: float,
        width: int,
        height: int,
        max_bytes: int = 2 * 1024 * 1024 * 1024,
    ) -> tuple[str, str]:
        """Bound result bytes before StoragePort; no derivative metadata is produced here."""
        if mime_type not in {"video/mp4", "video/webm", "video/quicktime"}:
            raise ValueError("agnes_video_mime_invalid")
        if not content or len(content) > max_bytes:
            raise ValueError("agnes_video_size_invalid")
        if duration_seconds <= 0 or duration_seconds > 600:
            raise ValueError("agnes_video_duration_invalid")
        if width < 1 or height < 1 or width > 16384 or height > 16384:
            raise ValueError("agnes_video_dimensions_invalid")
        return mime_type, sha256(content).hexdigest()

    @staticmethod
    def decode_result_media(payload: dict[str, object]) -> tuple[bytes, str] | None:
        encoded = payload.get("base64")
        mime = payload.get("mimeType")
        if encoded is None and mime is None:
            return None
        if not isinstance(encoded, str) or not isinstance(mime, str):
            raise ValueError("agnes_video_result_invalid")
        try:
            return b64decode(encoded, validate=True), mime
        except Exception as error:
            raise ValueError("agnes_video_result_invalid") from error


@dataclass(slots=True)
class VideoSubmissionState:
    logical_operation: str
    status: Literal[
        "pending", "submitted", "running", "succeeded", "failed", "cancelled", "submission_unknown"
    ] = "pending"
    provider_request_id: str | None = None
    result_candidate_id: str | None = None
    cancel_requested: bool = False
    observation_fingerprints: tuple[str, ...] = ()

    def mark_unknown(self) -> None:
        if self.status in {"pending", "submitted", "running"}:
            self.status = "submission_unknown"

    def observe(self, status: str, fingerprint: str, provider_request_id: str | None = None) -> str:
        if fingerprint in self.observation_fingerprints:
            return self.status
        self.observation_fingerprints = (*self.observation_fingerprints, fingerprint)
        if provider_request_id and self.provider_request_id is None:
            self.provider_request_id = provider_request_id
        precedence = {
            "pending": 0,
            "submitted": 1,
            "running": 2,
            "submission_unknown": 2,
            "cancelled": 3,
            "failed": 3,
            "succeeded": 3,
        }
        if self.cancel_requested or precedence.get(self.status, 0) >= 3:
            return self.status
        if self.status == "submission_unknown" and status in {"submitted", "running"}:
            return self.status
        if status in precedence and precedence[status] >= precedence.get(self.status, 0):
            self.status = status  # type: ignore[assignment]
        return self.status

    def request_cancel(self) -> str:
        if self.status in {"succeeded", "failed", "cancelled"}:
            return self.status
        self.cancel_requested = True
        self.status = "cancelled"
        return self.status

    def reconcile(self, provider_request_id: str | None, terminal: str | None) -> str:
        if provider_request_id and self.provider_request_id is None:
            self.provider_request_id = provider_request_id
        if self.status == "cancelled":
            return self.status
        if terminal in {"succeeded", "failed", "cancelled"}:
            self.status = terminal  # type: ignore[assignment]
        elif self.status == "submission_unknown" and provider_request_id is None:
            self.status = "submission_unknown"
        return self.status
