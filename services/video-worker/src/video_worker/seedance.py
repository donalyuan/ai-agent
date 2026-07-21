"""Seedance provider contract 与火山方舟真实异步任务客户端。"""

import json
import re
from dataclasses import dataclass
from typing import Any, Protocol
from urllib import error as urllib_error
from urllib import request as urllib_request
from urllib.parse import urlsplit
import uuid

CREATE_TASK_PATH = "/api/v3/contents/generations/tasks"
TASK_PATH = "/api/v3/contents/generations/tasks/{task_id}"


@dataclass(frozen=True)
class SeedanceImageInput:
    url: str
    role: str


@dataclass(frozen=True)
class SeedanceRequest:
    prompt: str
    images: list[SeedanceImageInput]
    duration_seconds: int
    aspect_ratio: str
    resolution: str
    generate_audio: bool = False
    model: str = "doubao-seedance-2-0-260128"

    def validate(self) -> None:
        if not self.prompt.strip() or len(self.prompt) > 500:
            raise ValueError("Seedance 提示词不得为空且不超过 500 字")
        if self.model.startswith("doubao-seedance-1-5-"):
            if not 4 <= self.duration_seconds <= 12:
                raise ValueError("Seedance 1.5 单任务时长必须在 4~12 秒")
            expected_roles = (
                ["first_frame"]
                if len(self.images) == 1
                else ["first_frame", "last_frame"]
            )
            if len(self.images) not in {1, 2} or [item.role for item in self.images] != expected_roles:
                raise ValueError("Seedance 1.5 只支持首帧或首尾帧输入")
        elif self.model.startswith("doubao-seedance-2-0-"):
            if not 4 <= self.duration_seconds <= 15:
                raise ValueError("Seedance 2.0 单任务时长必须在 4~15 秒")
            if not 1 <= len(self.images) <= 9 or any(
                item.role != "reference_image" for item in self.images
            ):
                raise ValueError("Seedance 2.0 参考图必须为 1~9 张 reference_image")
        else:
            raise ValueError("当前 Seedance 模型家族未实现")
        if self.resolution not in {"480p", "720p", "1080p"}:
            raise ValueError("Seedance 分辨率无效")
        if self.aspect_ratio not in {"16:9", "4:3", "1:1", "3:4", "9:16", "21:9", "adaptive"}:
            raise ValueError("Seedance 宽高比无效")
        if any(not item.url.startswith("https://") for item in self.images):
            raise ValueError("Seedance 图片必须使用 HTTPS URL")

    def payload(self) -> dict[str, object]:
        return {
            "model": self.model,
            "content": [
                {"type": "text", "text": self.prompt},
                *[
                    {
                        "type": "image_url",
                        "image_url": {"url": image.url},
                        "role": image.role,
                    }
                    for image in self.images
                ],
            ],
            "duration": self.duration_seconds,
            "ratio": self.aspect_ratio,
            "resolution": self.resolution,
            "generate_audio": self.generate_audio,
        }


@dataclass(frozen=True)
class SeedanceTask:
    task_id: str
    status: str
    request: dict[str, Any]
    output_url: str | None = None
    error: str | None = None

    @classmethod
    def from_payload(cls, payload: dict[str, Any]) -> "SeedanceTask":
        task_id = str(payload.get("id") or "").strip()
        raw_status = str(payload.get("status") or "").strip().lower()
        status_map = {
            "queued": "queued",
            "pending": "queued",
            "running": "running",
            "processing": "running",
            "succeeded": "succeeded",
            "success": "succeeded",
            "failed": "failed",
            "cancelled": "cancelled",
            "canceled": "cancelled",
            "expired": "failed",
        }
        if not task_id or raw_status not in status_map:
            raise SeedanceProviderError("response_invalid", "Seedance 返回的任务结构无效")
        content = payload.get("content")
        output_url = content.get("video_url") if isinstance(content, dict) else None
        if output_url is not None and not isinstance(output_url, str):
            raise SeedanceProviderError("response_invalid", "Seedance 视频 URL 无效")
        error = payload.get("error")
        return cls(
            task_id=task_id,
            status=status_map[raw_status],
            request={},
            output_url=output_url,
            error=str(error) if error is not None else None,
        )


class SeedanceProviderError(RuntimeError):
    def __init__(
        self,
        code: str,
        message: str,
        *,
        status_code: int | None = None,
        provider_code: str | None = None,
        provider_message: str | None = None,
        request_id: str | None = None,
    ):
        super().__init__(message)
        self.code = code
        self.status_code = status_code
        self.provider_code = provider_code
        self.provider_message = provider_message
        self.request_id = request_id

    def audit_snapshot(self) -> dict[str, str | int]:
        snapshot: dict[str, str | int] = {"error_code": self.code}
        if self.status_code is not None:
            snapshot["http_status"] = self.status_code
        if self.provider_code:
            snapshot["provider_error_code"] = self.provider_code
        if self.provider_message:
            snapshot["provider_error_message"] = self.provider_message
        if self.request_id:
            snapshot["provider_request_id"] = self.request_id
        return snapshot


class SeedanceProvider(Protocol):
    def create(self, request: SeedanceRequest) -> SeedanceTask: ...
    def get(self, task_id: str) -> SeedanceTask: ...
    def cancel(self, task_id: str) -> SeedanceTask: ...


class FakeSeedanceProvider:
    """测试用 provider；记录完整请求但永不保存凭据。"""

    def __init__(self, responses: list[SeedanceTask] | None = None):
        self.tasks: dict[str, SeedanceTask] = {}
        self.responses = list(responses or [])
        self.calls: list[dict[str, Any]] = []

    def create(self, request: SeedanceRequest) -> SeedanceTask:
        request.validate()
        task_id = str(uuid.uuid4())
        payload = request.payload()
        self.calls.append(payload)
        task = self.responses.pop(0) if self.responses else SeedanceTask(task_id, "succeeded", payload, "https://fake.invalid/video.mp4")
        self.tasks[task_id] = task
        return task

    def get(self, task_id: str) -> SeedanceTask:
        if task_id not in self.tasks:
            raise KeyError(task_id)
        return self.tasks[task_id]

    def cancel(self, task_id: str) -> SeedanceTask:
        task = self.get(task_id)
        cancelled = SeedanceTask(task.task_id, "cancelled", task.request, task.output_url, task.error)
        self.tasks[task_id] = cancelled
        return cancelled


class ArkSeedanceProvider:
    """单次发送 Ark 请求；尤其禁止在客户端内部重试创建 POST。"""

    def __init__(
        self,
        *,
        api_key: str,
        base_url: str,
        timeout_seconds: int,
        open_request=None,
    ) -> None:
        normalized = base_url.strip().rstrip("/")
        parsed = urlsplit(normalized)
        if (
            not api_key.strip()
            or parsed.scheme != "https"
            or not parsed.netloc
            or parsed.query
            or parsed.fragment
            or not parsed.path.rstrip("/").endswith("/api/v3")
            or timeout_seconds <= 0
        ):
            raise SeedanceProviderError("config_invalid", "Seedance provider 配置无效")
        self.api_key = api_key.strip()
        self.base_url = normalized
        self.timeout_seconds = timeout_seconds
        self.open_request = open_request or urllib_request.urlopen

    def create(self, request: SeedanceRequest) -> SeedanceTask:
        request.validate()
        payload = request.payload()
        try:
            return self._send("POST", CREATE_TASK_PATH, payload)
        except SeedanceProviderError as error:
            if error.code in {"response_invalid", "response_too_large"} or (
                error.status_code is not None and error.status_code >= 500
            ):
                raise SeedanceProviderError(
                    "unknown_submission",
                    "Seedance 创建结果不确定",
                    status_code=error.status_code,
                    provider_code=error.provider_code,
                    provider_message=error.provider_message,
                    request_id=error.request_id,
                ) from error
            raise
        except Exception as error:
            raise SeedanceProviderError(
                "unknown_submission",
                f"Seedance 创建结果不确定: {error.__class__.__name__}",
            ) from error

    def get(self, task_id: str) -> SeedanceTask:
        return self._send("GET", TASK_PATH.format(task_id=_safe_task_id(task_id)))

    def cancel(self, task_id: str) -> SeedanceTask:
        return self._send("DELETE", TASK_PATH.format(task_id=_safe_task_id(task_id)))

    def _send(
        self,
        method: str,
        path: str,
        payload: dict[str, object] | None = None,
    ) -> SeedanceTask:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8") if payload is not None else None
        request = urllib_request.Request(
            f"{self.base_url}{path.removeprefix('/api/v3')}",
            data=body,
            method=method,
            headers={
                "Authorization": f"Bearer {self.api_key}",
                "Content-Type": "application/json",
            },
        )
        try:
            with self.open_request(request, timeout=self.timeout_seconds) as response:
                raw = response.read(1024 * 1024 + 1)
                if len(raw) > 1024 * 1024:
                    raise SeedanceProviderError("response_too_large", "Seedance 响应过大")
                status_code = int(getattr(response, "status", 200))
                response_headers = getattr(response, "headers", None)
        except urllib_error.HTTPError as error:
            raw = error.read(1024 * 1024 + 1)
            raise _http_provider_error(error.code, raw, error.headers) from error
        if not 200 <= status_code < 300:
            raise _http_provider_error(status_code, raw, response_headers)
        try:
            parsed = json.loads(raw)
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise SeedanceProviderError("response_invalid", "Seedance 返回无效 JSON") from error
        if not isinstance(parsed, dict):
            raise SeedanceProviderError("response_invalid", "Seedance 返回必须是 object")
        return SeedanceTask.from_payload(parsed)


def _safe_task_id(task_id: str) -> str:
    normalized = task_id.strip()
    if not normalized or any(char not in "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_" for char in normalized):
        raise SeedanceProviderError("task_id_invalid", "Seedance task ID 无效")
    return normalized


def _http_provider_error(status_code: int, raw: bytes, headers: Any) -> SeedanceProviderError:
    provider_code: str | None = None
    provider_message: str | None = None
    request_id: str | None = None
    if len(raw) <= 1024 * 1024:
        try:
            parsed = json.loads(raw)
        except (UnicodeDecodeError, json.JSONDecodeError):
            parsed = None
        if isinstance(parsed, dict):
            error = parsed.get("error")
            details = error if isinstance(error, dict) else parsed
            provider_code = _safe_identifier(details.get("code"), 160)
            provider_message = _sanitize_provider_text(details.get("message"), 1000)
            request_id = _safe_identifier(
                parsed.get("request_id") or details.get("request_id"), 200
            )
    if request_id is None and headers is not None:
        for header in ("x-request-id", "x-tt-logid", "x-tt-trace-id"):
            request_id = _safe_identifier(headers.get(header), 200)
            if request_id:
                break
    summary = f"Seedance HTTP {status_code}"
    if provider_code:
        summary = f"{summary} [{provider_code}]"
    if provider_message:
        summary = f"{summary}: {provider_message}"
    return SeedanceProviderError(
        "http_error",
        summary,
        status_code=status_code,
        provider_code=provider_code,
        provider_message=provider_message,
        request_id=request_id,
    )


def _safe_identifier(value: Any, max_length: int) -> str | None:
    if not isinstance(value, str):
        return None
    normalized = value.strip()
    if not normalized or len(normalized) > max_length:
        return None
    if not re.fullmatch(r"[A-Za-z0-9._:/-]+", normalized):
        return None
    return normalized


def _sanitize_provider_text(value: Any, max_length: int) -> str | None:
    if not isinstance(value, str):
        return None
    normalized = " ".join(value.strip().split())
    if not normalized:
        return None
    normalized = re.sub(r"https?://[^\s\"'<>]+", "[REDACTED_URL]", normalized)
    normalized = re.sub(r"(?i)\bBearer\s+[^\s,;]+", "Bearer [REDACTED]", normalized)
    normalized = re.sub(
        r"(?i)\b(api[_-]?key|access[_-]?key|secret[_-]?key|token|signature)"
        r"\s*[:=]\s*[^\s,;]+",
        r"\1=[REDACTED]",
        normalized,
    )
    return normalized[:max_length]


def sanitize_model_snapshot(snapshot: dict[str, Any]) -> dict[str, Any]:
    """模型快照只保留可审计字段，防止密钥进入运行记录。"""

    forbidden = {"api_key", "access_key", "secret_key", "authorization", "token", "password", "cookie"}
    return {key: value for key, value in snapshot.items() if key.lower() not in forbidden}
