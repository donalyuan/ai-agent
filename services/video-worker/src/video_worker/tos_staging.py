import hashlib
import io
import re
from dataclasses import dataclass
from pathlib import PurePosixPath
from typing import Callable, Protocol
from urllib.request import urlopen
from uuid import UUID

import tos
from tos.enum import HttpMethodType


class TosStagingError(RuntimeError):
    def __init__(
        self,
        code: str,
        message: str,
        *,
        retryable: bool,
        object_key: str | None = None,
        source_sha256: str | None = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.retryable = retryable
        self.object_key = object_key
        self.source_sha256 = source_sha256


@dataclass(frozen=True)
class TosStagingConfig:
    endpoint: str
    region: str
    bucket: str
    object_prefix: str
    signed_url_ttl_seconds: int
    max_file_bytes: int
    access_key: str
    secret_key: str


@dataclass(frozen=True)
class StagedAudioObject:
    object_key: str
    source_sha256: str
    source_size_bytes: int
    signed_get_url: str

    def audit_snapshot(self) -> dict[str, str | int]:
        return {
            "object_key": self.object_key,
            "source_sha256": self.source_sha256,
            "source_size_bytes": self.source_size_bytes,
        }


class TosClient(Protocol):
    def head_bucket(self, bucket: str): ...

    def put_object(self, bucket: str, key: str, **kwargs): ...

    def pre_signed_url(self, http_method, bucket: str, key: str, **kwargs): ...

    def delete_object(self, bucket: str, key: str): ...


class TosAudioStaging:
    def __init__(self, config: TosStagingConfig, client: TosClient | None = None) -> None:
        _validate_config(config)
        self.config = config
        self.client = client or tos.TosClientV2(
            config.access_key,
            config.secret_key,
            config.endpoint,
            config.region,
        )

    def stage(
        self,
        *,
        project_id: UUID,
        task_id: UUID,
        content: bytes,
        extension: str,
        content_type: str,
    ) -> StagedAudioObject:
        if not content:
            raise TosStagingError("empty_audio", "暂存音频不能为空", retryable=False)
        if len(content) > self.config.max_file_bytes:
            raise TosStagingError(
                "audio_too_large",
                "暂存音频超过模型配置的大小限制",
                retryable=False,
            )
        source_sha256 = hashlib.sha256(content).hexdigest()
        object_key = deterministic_object_key(
            prefix=self.config.object_prefix,
            project_id=project_id,
            task_id=task_id,
            source_sha256=source_sha256,
            extension=extension,
        )
        try:
            self.client.put_object(
                self.config.bucket,
                object_key,
                content=io.BytesIO(content),
                content_length=len(content),
                content_type=content_type,
                forbid_overwrite=True,
            )
            signed_url = self.signed_get_url(object_key)
        except Exception as error:
            if isinstance(error, TosStagingError):
                error.object_key = object_key
                error.source_sha256 = source_sha256
                raise
            raise TosStagingError(
                "tos_stage_failed",
                f"TOS 暂存失败: {error.__class__.__name__}",
                retryable=True,
                object_key=object_key,
                source_sha256=source_sha256,
            ) from error
        return StagedAudioObject(
            object_key=object_key,
            source_sha256=source_sha256,
            source_size_bytes=len(content),
            signed_get_url=signed_url,
        )

    def signed_get_url(self, object_key: str) -> str:
        if not _valid_object_key(object_key, self.config.object_prefix):
            raise TosStagingError(
                "tos_object_key_invalid", "TOS 临时对象键无效", retryable=False
            )
        try:
            signed = self.client.pre_signed_url(
                HttpMethodType.Http_Method_Get,
                self.config.bucket,
                object_key,
                expires=self.config.signed_url_ttl_seconds,
            )
        except Exception as error:
            raise TosStagingError(
                "tos_presign_failed",
                f"TOS 签名失败: {error.__class__.__name__}",
                retryable=True,
            ) from error
        signed_url = str(getattr(signed, "signed_url", ""))
        if not signed_url.startswith("https://"):
            raise TosStagingError(
                "tos_signed_url_invalid",
                "TOS 未返回有效 HTTPS 签名 URL",
                retryable=False,
            )
        return signed_url

    def cleanup(self, object_key: str) -> None:
        if not _valid_object_key(object_key, self.config.object_prefix):
            raise TosStagingError(
                "tos_object_key_invalid",
                "拒绝删除不属于当前暂存前缀的对象",
                retryable=False,
            )
        try:
            self.client.delete_object(self.config.bucket, object_key)
        except Exception as error:
            raise TosStagingError(
                "tos_cleanup_failed",
                f"TOS 临时对象清理失败: {error.__class__.__name__}",
                retryable=True,
            ) from error


class TosConnectionChecker:
    def __init__(
        self,
        config: TosStagingConfig,
        client: TosClient | None = None,
        signed_url_reader: Callable[[str], bytes] | None = None,
    ) -> None:
        _validate_config(config)
        self.config = config
        self.client = client or tos.TosClientV2(
            config.access_key,
            config.secret_key,
            config.endpoint,
            config.region,
        )
        self.signed_url_reader = signed_url_reader or _read_signed_url

    def check(self) -> None:
        probe = b"novex-tos-capability-check"
        object_key = str(
            PurePosixPath(
                self.config.object_prefix.strip().strip("/"),
                ".novex-connection-check",
                "probe.txt",
            )
        )
        upload_attempted = False
        failure: Exception | None = None
        try:
            self.client.head_bucket(self.config.bucket)
            upload_attempted = True
            self.client.put_object(
                self.config.bucket,
                object_key,
                content=io.BytesIO(probe),
                content_length=len(probe),
                content_type="text/plain",
                forbid_overwrite=False,
            )
            signed = self.client.pre_signed_url(
                HttpMethodType.Http_Method_Get,
                self.config.bucket,
                object_key,
                expires=self.config.signed_url_ttl_seconds,
            )
            signed_url = str(getattr(signed, "signed_url", ""))
            if not signed_url.startswith("https://"):
                raise RuntimeError("invalid signed URL")
            if self.signed_url_reader(signed_url) != probe:
                raise RuntimeError("signed URL content mismatch")
        except Exception as error:
            failure = error
        if upload_attempted:
            try:
                self.client.delete_object(self.config.bucket, object_key)
            except Exception as cleanup_error:
                failure = cleanup_error
        if failure is not None:
            raise TosStagingError(
                "tos_connection_check_failed",
                f"TOS Bucket 能力检查失败: {failure.__class__.__name__}",
                retryable=True,
            ) from failure


def deterministic_object_key(
    *,
    prefix: str,
    project_id: UUID,
    task_id: UUID,
    source_sha256: str,
    extension: str,
) -> str:
    normalized_prefix = prefix.strip().strip("/")
    normalized_extension = extension.lower().lstrip(".")
    if not re.fullmatch(r"[a-z0-9]{1,10}", normalized_extension):
        raise TosStagingError("invalid_audio_extension", "音频扩展名无效", retryable=False)
    if not re.fullmatch(r"[0-9a-f]{64}", source_sha256):
        raise TosStagingError("invalid_audio_digest", "音频摘要无效", retryable=False)
    parts = [part for part in [normalized_prefix, str(project_id), str(task_id)] if part]
    return str(PurePosixPath(*parts, f"{source_sha256}.{normalized_extension}"))


def _validate_config(config: TosStagingConfig) -> None:
    if not config.endpoint.startswith("https://"):
        raise TosStagingError("tos_config_invalid", "TOS endpoint 必须使用 HTTPS", retryable=False)
    for value in (config.region, config.bucket, config.access_key, config.secret_key):
        if not value.strip():
            raise TosStagingError("tos_config_invalid", "TOS 暂存配置不完整", retryable=False)
    if not 60 <= config.signed_url_ttl_seconds <= 3600:
        raise TosStagingError(
            "tos_config_invalid", "TOS 签名有效期必须为 60-3600 秒", retryable=False
        )
    if config.max_file_bytes <= 0:
        raise TosStagingError("tos_config_invalid", "TOS 文件上限必须为正数", retryable=False)


def _valid_object_key(object_key: str, prefix: str) -> bool:
    normalized_prefix = prefix.strip().strip("/")
    if not normalized_prefix:
        return False
    return object_key.startswith(f"{normalized_prefix}/") and ".." not in object_key.split("/")


def _read_signed_url(url: str) -> bytes:
    with urlopen(url, timeout=15) as response:
        return response.read(1024)
