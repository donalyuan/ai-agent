from __future__ import annotations

from dataclasses import dataclass
from typing import Callable
from urllib.parse import urlsplit


class ModelRegistryError(RuntimeError):
    def __init__(self, code: str, message: str):
        super().__init__(message)
        self.code = code


@dataclass(frozen=True)
class ImageModelRuntimeConfig:
    model_id: str
    display_name: str
    provider_name: str
    api_protocol: str
    protocol_version: str
    auth_scheme: str
    request_base_url: str
    upstream_model: str
    api_key: str
    api_secret: str | None
    timeout_seconds: int
    settings: dict[str, object]

    def snapshot(self) -> dict[str, object]:
        return {
            "model_id": self.model_id,
            "display_name": self.display_name,
            "model_type": "image",
            "provider_name": self.provider_name,
            "api_protocol": self.api_protocol,
            "protocol_version": self.protocol_version,
            "request_base_url": self.request_base_url,
            "upstream_model": self.upstream_model,
            "reasoning_effort": None,
            "timeout_seconds": self.timeout_seconds,
            "max_output_tokens": None,
            "settings": self.settings,
        }


class PostgresModelRegistry:
    def __init__(
        self,
        database_url: str,
        connection_factory: Callable[[], object] | None = None,
    ):
        self.database_url = database_url
        self._connection_factory = connection_factory

    def _connect(self):
        if self._connection_factory is not None:
            return self._connection_factory()
        import psycopg
        from psycopg.rows import dict_row

        return psycopg.connect(self.database_url, row_factory=dict_row)

    def resolve_enabled(
        self,
        model_id: str,
        expected_type: str,
    ) -> ImageModelRuntimeConfig:
        with self._connect() as connection:
            row = connection.execute(
                """
                SELECT id, display_name, model_type, provider_name, api_protocol,
                       protocol_version, auth_scheme, request_base_url, upstream_model,
                       api_key, api_secret, timeout_seconds, settings, status, deleted_at
                FROM ai_models
                WHERE id = %s
                """,
                (model_id,),
            ).fetchone()
        if row is None:
            raise ModelRegistryError("model_not_found", "模型不存在")
        if str(row["model_type"]) != expected_type:
            raise ModelRegistryError("model_type_mismatch", "模型类型不匹配")
        if str(row["status"]) != "enabled" or row["deleted_at"] is not None:
            raise ModelRegistryError("model_disabled", "模型已停用或删除")

        protocol = str(row["api_protocol"])
        auth_scheme = str(row["auth_scheme"])
        if protocol not in {"openai_images", "volcengine_ark_images"}:
            raise ModelRegistryError("invalid_model_config", "图片模型协议无效")
        expected_auth = "bearer"
        if auth_scheme != expected_auth:
            raise ModelRegistryError("invalid_model_config", "图片模型认证协议无效")

        api_key = str(row["api_key"] or "").strip()
        api_secret = str(row["api_secret"]).strip() if row["api_secret"] else None
        request_base_url = str(row["request_base_url"] or "").strip().rstrip("/")
        upstream_model = str(row["upstream_model"] or "").strip()
        timeout_seconds = int(row["timeout_seconds"])
        settings = row["settings"] or {}
        if (
            not api_key
            or not request_base_url.startswith(("http://", "https://"))
            or not upstream_model
            or timeout_seconds <= 0
            or not isinstance(settings, dict)
        ):
            raise ModelRegistryError("invalid_model_config", "图片模型配置无效")
        if protocol == "volcengine_ark_images":
            parsed_url = urlsplit(request_base_url)
            if (
                parsed_url.scheme not in {"http", "https"}
                or not parsed_url.netloc
                or parsed_url.query
                or parsed_url.fragment
                or not parsed_url.path.rstrip("/").endswith("/api/v3")
                or settings.get("max_images_per_request") != 1
            ):
                raise ModelRegistryError("invalid_model_config", "火山方舟图片模型配置无效")

        return ImageModelRuntimeConfig(
            model_id=str(row["id"]),
            display_name=str(row["display_name"]),
            provider_name=str(row["provider_name"]),
            api_protocol=protocol,
            protocol_version=str(row["protocol_version"]),
            auth_scheme=auth_scheme,
            request_base_url=request_base_url,
            upstream_model=upstream_model,
            api_key=api_key,
            api_secret=api_secret,
            timeout_seconds=timeout_seconds,
            settings=settings,
        )
