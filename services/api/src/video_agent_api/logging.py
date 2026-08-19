"""不依赖日志供应商的结构化 JSON 事件与脱敏。"""

from __future__ import annotations

import json
import logging
from typing import Any

_SENSITIVE_KEYS = {
    "api_key",
    "apikey",
    "authorization",
    "credential",
    "credentials",
    "secret",
    "token",
    "response",
    "raw_response",
}


def redact_event(value: Any) -> Any:
    """递归清除密钥、认证头和原始 Provider 响应。"""
    if isinstance(value, dict):
        return {
            key: "[REDACTED]" if key.lower() in _SENSITIVE_KEYS else redact_event(item)
            for key, item in value.items()
        }
    if isinstance(value, list):
        return [redact_event(item) for item in value]
    if isinstance(value, tuple):
        return tuple(redact_event(item) for item in value)
    return value


class JsonFormatter(logging.Formatter):
    """只序列化已脱敏字段，避免将 Provider 原始响应带入普通日志。"""

    def format(self, record: logging.LogRecord) -> str:
        payload = {
            "event": getattr(record, "event", record.getMessage()),
            "level": record.levelname.lower(),
            "logger": record.name,
            "correlation_id": getattr(record, "correlation_id", None),
            "data": redact_event(getattr(record, "event_data", {})),
        }
        return json.dumps(payload, ensure_ascii=False, default=str, sort_keys=True)


def configure_structured_logging() -> logging.Logger:
    logger = logging.getLogger("video_agent")
    if not logger.handlers:
        handler = logging.StreamHandler()
        handler.setFormatter(JsonFormatter())
        logger.addHandler(handler)
    logger.setLevel(logging.INFO)
    logger.propagate = False
    return logger


def log_event(event: str, *, correlation_id: str | None = None, **data: Any) -> None:
    configure_structured_logging().info(
        event,
        extra={"event": event, "correlation_id": correlation_id, "event_data": data},
    )
