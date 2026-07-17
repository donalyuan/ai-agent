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


@dataclass(frozen=True)
class SpeechStagingRuntimeConfig:
    config_id: str
    version: int
    storage_provider: str
    endpoint: str
    region: str
    bucket: str
    object_prefix: str
    access_key: str
    secret_key: str
    signed_url_ttl_seconds: int
    max_file_bytes: int
    max_audio_duration_seconds: int


@dataclass(frozen=True)
class SpeechModelRuntimeConfig:
    model_id: str
    display_name: str
    provider_name: str
    api_protocol: str
    protocol_version: str
    auth_scheme: str
    request_base_url: str
    upstream_model: str
    api_key: str
    timeout_seconds: int
    settings: dict[str, object]
    registry_version: int

    def snapshot(self) -> dict[str, object]:
        return {
            "model_id": self.model_id,
            "display_name": self.display_name,
            "model_type": "speech",
            "provider_name": self.provider_name,
            "api_protocol": self.api_protocol,
            "protocol_version": self.protocol_version,
            "request_base_url": self.request_base_url,
            "upstream_model": self.upstream_model,
            "reasoning_effort": None,
            "timeout_seconds": self.timeout_seconds,
            "max_output_tokens": None,
            "settings": self.settings,
            "registry_version": self.registry_version,
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

    def resolve_speech(
        self,
        model_id: str,
        expected_protocol: str,
        expected_registry_version: int,
    ) -> SpeechModelRuntimeConfig:
        with self._connect() as connection:
            row = connection.execute(
                """
                SELECT id, display_name, model_type, provider_name, api_protocol,
                       protocol_version, auth_scheme, request_base_url, upstream_model,
                       api_key, timeout_seconds, settings, status, deleted_at, version
                FROM ai_models
                WHERE id = %s
                """,
                (model_id,),
            ).fetchone()
        if row is None:
            raise ModelRegistryError("model_not_found", "语音模型不存在")
        if str(row["model_type"]) != "speech":
            raise ModelRegistryError("model_type_mismatch", "模型类型不匹配")
        if str(row["status"]) != "enabled" or row["deleted_at"] is not None:
            raise ModelRegistryError("model_disabled", "语音模型已停用或删除")
        protocol = str(row["api_protocol"])
        if protocol != expected_protocol:
            raise ModelRegistryError("model_protocol_mismatch", "语音模型协议已变化")
        registry_version = int(row["version"])
        if registry_version != expected_registry_version:
            raise ModelRegistryError("model_version_changed", "语音模型配置已变化，请重新确认")

        request_base_url = str(row["request_base_url"] or "").strip().rstrip("/")
        api_key = str(row["api_key"] or "").strip()
        upstream_model = str(row["upstream_model"] or "").strip()
        timeout_seconds = int(row["timeout_seconds"])
        settings = row["settings"] or {}
        expected_auth_scheme = (
            "bearer" if protocol == "openai_audio_speech" else "api_key"
        )
        if (
            protocol
            not in {
                "volcengine_tts_v3",
                "openai_audio_speech",
                "volcengine_asr_v3",
            }
            or str(row["auth_scheme"]) != expected_auth_scheme
            or not api_key
            or not upstream_model
            or timeout_seconds <= 0
            or not isinstance(settings, dict)
        ):
            raise ModelRegistryError("invalid_model_config", "语音模型配置无效")
        parsed_url = urlsplit(request_base_url)
        if (
            parsed_url.scheme not in {"http", "https"}
            or not parsed_url.netloc
            or parsed_url.query
            or parsed_url.fragment
            or not parsed_url.path.rstrip("/").endswith(
                "/v1" if protocol == "openai_audio_speech" else "/api/v3"
            )
        ):
            raise ModelRegistryError("invalid_model_config", "语音模型请求根地址无效")
        expected_resource = (
            "seed-tts-2.0"
            if protocol in {"volcengine_tts_v3", "openai_audio_speech"}
            else "volc.seedasr.auc"
        )
        timestamp_languages = settings.get("word_timestamp_languages")
        supports_word_timestamps = settings.get("supports_word_timestamps")
        valid_timestamp_capability = (
            supports_word_timestamps is False and timestamp_languages == []
            if protocol == "openai_audio_speech"
            else supports_word_timestamps is True
            and isinstance(timestamp_languages, list)
            and bool(timestamp_languages)
        )
        if (
            settings.get("resource_id") != expected_resource
            or not isinstance(settings.get("supported_audio_formats"), list)
            or not settings["supported_audio_formats"]
            or not valid_timestamp_capability
        ):
            raise ModelRegistryError("invalid_model_config", "语音模型能力配置无效")

        return SpeechModelRuntimeConfig(
            model_id=str(row["id"]),
            display_name=str(row["display_name"]),
            provider_name=str(row["provider_name"]),
            api_protocol=protocol,
            protocol_version=str(row["protocol_version"]),
            auth_scheme=str(row["auth_scheme"]),
            request_base_url=request_base_url,
            upstream_model=upstream_model,
            api_key=api_key,
            timeout_seconds=timeout_seconds,
            settings=settings,
            registry_version=registry_version,
        )

    def resolve_tos_staging(
        self,
        config_id: str,
        expected_version: int,
    ) -> SpeechStagingRuntimeConfig:
        with self._connect() as connection:
            row = connection.execute(
                """
                SELECT id, version, storage_provider, endpoint, region, bucket,
                       object_prefix, access_key, secret_key, signed_url_ttl_seconds,
                       max_file_bytes, max_audio_duration_seconds
                FROM tos_staging_tool_configs
                WHERE id = %s
                """,
                (config_id,),
            ).fetchone()
        if row is None:
            raise ModelRegistryError(
                "tos_staging_config_not_found", "任务锁定的 TOS 配置版本不存在"
            )
        try:
            config = SpeechStagingRuntimeConfig(
                config_id=str(row["id"]),
                version=int(row["version"]),
                storage_provider=str(row["storage_provider"] or ""),
                endpoint=str(row["endpoint"] or ""),
                region=str(row["region"] or ""),
                bucket=str(row["bucket"] or ""),
                object_prefix=str(row["object_prefix"] or ""),
                access_key=str(row["access_key"] or ""),
                secret_key=str(row["secret_key"] or ""),
                signed_url_ttl_seconds=int(row["signed_url_ttl_seconds"]),
                max_file_bytes=int(row["max_file_bytes"]),
                max_audio_duration_seconds=int(row["max_audio_duration_seconds"]),
            )
        except (TypeError, ValueError) as error:
            raise ModelRegistryError(
                "invalid_tos_staging_config", "任务锁定的 TOS 配置无效"
            ) from error
        if config.version != expected_version:
            raise ModelRegistryError(
                "tos_staging_version_changed", "任务锁定的 TOS 配置版本不匹配"
            )
        if (
            config.storage_provider != "volcengine_tos"
            or not config.endpoint.startswith("https://")
            or not all(
                value.strip()
                for value in (
                    config.region,
                    config.bucket,
                    config.object_prefix,
                    config.access_key,
                    config.secret_key,
                )
            )
            or not 60 <= config.signed_url_ttl_seconds <= 3600
            or config.max_file_bytes <= 0
            or config.max_audio_duration_seconds <= 0
        ):
            raise ModelRegistryError(
                "invalid_tos_staging_config", "任务锁定的 TOS 配置无效"
            )
        return config
