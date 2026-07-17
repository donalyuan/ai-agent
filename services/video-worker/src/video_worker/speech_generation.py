from __future__ import annotations

import base64
import binascii
import hashlib
import json
import os
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable, Protocol
from urllib import error as urllib_error
from urllib import request as urllib_request
from urllib.parse import urlsplit
from uuid import UUID

from video_worker.model_registry import (
    SpeechModelRuntimeConfig,
    SpeechStagingRuntimeConfig,
)
from video_worker.tos_staging import TosAudioStaging, TosStagingConfig, TosStagingError


@dataclass(frozen=True)
class AudioInspectionResult:
    source_sha256: str
    file_size_bytes: int
    duration_ms: int
    container_format: str
    audio_codec: str
    sample_rate_hz: int
    channel_count: int


class AudioInspectionError(RuntimeError):
    def __init__(self, code: str, message: str) -> None:
        super().__init__(message)
        self.code = code


class SpeechProviderError(RuntimeError):
    def __init__(
        self,
        code: str,
        message: str,
        *,
        retryable: bool,
        error_details: dict[str, object] | None = None,
        upstream_log_id: str | None = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.retryable = retryable
        self.error_details = _normalize_error_details(error_details)
        self.upstream_log_id = _safe_trace_id(upstream_log_id)


@dataclass(frozen=True)
class ProviderHttpResponse:
    status_code: int
    headers: dict[str, str]
    body_lines: Iterable[bytes]
    body_content: bytes | None = None


@dataclass(frozen=True)
class TimestampWord:
    text: str
    start_ms: int
    end_ms: int
    confidence: float | None


@dataclass(frozen=True)
class TtsSynthesisRequest:
    request_id: UUID
    text: str
    voice_type: str
    language: str
    parameters: dict[str, object]


@dataclass(frozen=True)
class TtsSynthesisResult:
    audio_content: bytes
    audio_format: str
    words: list[TimestampWord]
    upstream_log_id: str | None

    def audit_snapshot(self) -> dict[str, object]:
        return {
            "audio_format": self.audio_format,
            "audio_size_bytes": len(self.audio_content),
            "word_count": len(self.words),
            "upstream_log_id": self.upstream_log_id,
        }


@dataclass(frozen=True)
class AsrSubmitResult:
    accepted: bool

    def audit_snapshot(self) -> dict[str, object]:
        return {"accepted": self.accepted}


@dataclass(frozen=True)
class AsrQueryResult:
    is_terminal: bool
    is_success: bool
    text: str
    words: list[TimestampWord]
    utterances: list[dict[str, object]]
    upstream_log_id: str | None
    error_code: str | None = None
    error_summary: str | None = None

    def audit_snapshot(self) -> dict[str, object]:
        return {
            "is_terminal": self.is_terminal,
            "is_success": self.is_success,
            "text": self.text,
            "word_count": len(self.words),
            "utterance_count": len(self.utterances),
            "upstream_log_id": self.upstream_log_id,
            "error_code": self.error_code,
            "error_summary": self.error_summary,
        }


@dataclass(frozen=True)
class PendingSpeechTask:
    task_id: str
    project_id: str
    task_type: str
    model_id: str
    tos_staging_config_id: str | None
    tos_staging_config_version: int | None
    request_id: str
    text_content: str
    voice_type: str | None
    language: str | None
    emotion: str | None
    parameters: dict[str, object]
    model_snapshot: dict[str, object]
    voice_snapshot: dict[str, object] | None
    confirmation_snapshot: dict[str, object]
    resource_usage: dict[str, object]
    source_audio_material_id: str | None = None
    source_file_url: str | None = None
    inspection_source_sha256: str | None = None
    inspection_duration_ms: int | None = None
    staging_status: str = "none"
    staging_object_key: str | None = None
    staging_source_sha256: str | None = None
    upstream_submitted: bool = False
    attempt_count: int = 0


@dataclass(frozen=True)
class PendingAudioInspection:
    inspection_id: str
    project_id: str
    material_id: str
    file_url: str


@dataclass(frozen=True)
class PendingTosCleanup:
    task_id: str
    tos_staging_config_id: str
    tos_staging_config_version: int
    object_key: str


@dataclass(frozen=True)
class SavedSpeechArtifact:
    file_url: str
    file_name: str
    file_path: Path
    file_size_bytes: int


class SpeechTaskStore(Protocol):
    def claim_next_speech_task(self) -> PendingSpeechTask | None: ...

    def complete_tts_task(self, **kwargs) -> None: ...

    def complete_asr_task(self, **kwargs) -> None: ...

    def fail_task(self, **kwargs) -> None: ...

    def record_asr_staging(
        self, task_id: str, object_key: str, source_sha256: str
    ) -> None: ...

    def record_asr_submitted(self, task_id: str, attempt_count: int) -> None: ...

    def defer_asr_task(self, task_id: str) -> None: ...

    def record_cleanup(
        self, task_id: str, succeeded: bool, error_summary: str | None
    ) -> None: ...


class AudioInspectionStore(Protocol):
    def claim_next_audio_inspection(self) -> PendingAudioInspection | None: ...

    def complete_audio_inspection(
        self, inspection_id: str, result: AudioInspectionResult
    ) -> None: ...

    def fail_audio_inspection(
        self, inspection_id: str, error_code: str, error_summary: str
    ) -> None: ...

class SpeechModelRegistry(Protocol):
    def resolve_speech(
        self, model_id: str, expected_protocol: str, expected_registry_version: int
    ) -> SpeechModelRuntimeConfig: ...

    def resolve_tos_staging(
        self, config_id: str, expected_version: int
    ) -> SpeechStagingRuntimeConfig: ...


class LocalSpeechStorage:
    def __init__(self, root: Path, public_prefix: str = "/assets") -> None:
        self.root = Path(root)
        self.public_prefix = public_prefix.rstrip("/")

    def source_path(self, file_url: str) -> Path:
        prefix = f"{self.public_prefix}/"
        if not file_url.startswith(prefix):
            raise AudioInspectionError(
                "audio_storage_unsupported", "音频不属于自管素材存储"
            )
        relative = Path(file_url[len(prefix) :])
        if relative.is_absolute() or ".." in relative.parts:
            raise AudioInspectionError("audio_path_invalid", "音频素材路径无效")
        path = (self.root / relative).resolve()
        root = self.root.resolve()
        if path != root and root not in path.parents:
            raise AudioInspectionError("audio_path_invalid", "音频素材路径越界")
        return path

    def save_audio(
        self, task_id: str, content: bytes, extension: str, *, preview: bool
    ) -> SavedSpeechArtifact:
        safe_task_id = str(UUID(task_id))
        safe_extension = extension.lower().lstrip(".")
        if safe_extension not in {"mp3", "wav", "ogg", "aac", "pcm"}:
            raise SpeechProviderError(
                "audio_format_unsupported", "输出音频格式无效", retryable=False
            )
        directory = "previews" if preview else "audio"
        filename = f"preview.{safe_extension}" if preview else f"tts.{safe_extension}"
        path = self.root / "generated" / directory / safe_task_id / filename
        _atomic_write(path, content)
        return SavedSpeechArtifact(
            file_url=f"{self.public_prefix}/generated/{directory}/{safe_task_id}/{filename}",
            file_name=filename,
            file_path=path,
            file_size_bytes=len(content),
        )

    def save_subtitle(self, task_id: str, content: str) -> SavedSpeechArtifact:
        safe_task_id = str(UUID(task_id))
        filename = "subtitles.srt"
        path = self.root / "generated" / "subtitles" / safe_task_id / filename
        encoded = content.encode("utf-8")
        _atomic_write(path, encoded)
        return SavedSpeechArtifact(
            file_url=f"{self.public_prefix}/generated/subtitles/{safe_task_id}/{filename}",
            file_name=filename,
            file_path=path,
            file_size_bytes=len(encoded),
        )

    def delete(self, artifact: SavedSpeechArtifact | None) -> None:
        if artifact is None:
            return
        try:
            artifact.file_path.unlink(missing_ok=True)
        except OSError:
            pass


def run_next_audio_inspection(
    store: AudioInspectionStore,
    storage: LocalSpeechStorage,
    *,
    audio_inspector: Callable[[Path], AudioInspectionResult] | None = None,
) -> bool:
    inspection = store.claim_next_audio_inspection()
    if inspection is None:
        return False
    inspect_audio = audio_inspector or inspect_audio_file
    try:
        source_path = storage.source_path(inspection.file_url)
        result = inspect_audio(source_path)
        store.complete_audio_inspection(inspection.inspection_id, result)
    except Exception as error:
        store.fail_audio_inspection(
            inspection.inspection_id,
            str(getattr(error, "code", "audio_inspection_failed")),
            _safe_error_summary(error),
        )
    return True


def run_next_speech_task(
    store: SpeechTaskStore,
    model_registry: SpeechModelRegistry,
    storage: LocalSpeechStorage,
    *,
    tts_provider_factory: Callable[[SpeechModelRuntimeConfig], object] | None = None,
    asr_provider_factory: Callable[[SpeechModelRuntimeConfig], object] | None = None,
    tos_staging_factory: Callable[[SpeechStagingRuntimeConfig], object] | None = None,
    audio_inspector: Callable[[Path], AudioInspectionResult] | None = None,
) -> bool:
    task = store.claim_next_speech_task()
    if task is None:
        return False
    inspect_audio = audio_inspector or inspect_audio_file
    expected_protocol = str(task.model_snapshot.get("api_protocol") or "")
    try:
        registry_version = int(task.model_snapshot.get("registry_version"))
        config = model_registry.resolve_speech(
            task.model_id, expected_protocol, registry_version
        )
        if task.task_type in {"tts", "tts_preview"}:
            _run_tts_task(
                store,
                task,
                config,
                storage,
                tts_provider_factory=tts_provider_factory,
                audio_inspector=inspect_audio,
            )
        elif task.task_type == "asr":
            if (
                task.tos_staging_config_id is None
                or task.tos_staging_config_version is None
            ):
                raise SpeechProviderError(
                    "tos_staging_config_missing",
                    "ASR 任务缺少锁定的系统 TOS 配置版本",
                    retryable=False,
                )
            staging_config = model_registry.resolve_tos_staging(
                task.tos_staging_config_id,
                task.tos_staging_config_version,
            )
            _run_asr_task(
                store,
                task,
                config,
                staging_config,
                storage,
                asr_provider_factory=asr_provider_factory,
                tos_staging_factory=tos_staging_factory,
                audio_inspector=inspect_audio,
            )
        else:
            raise SpeechProviderError(
                "task_type_unsupported", "Worker 不支持该声音任务类型", retryable=False
            )
    except Exception as error:
        code = str(getattr(error, "code", "speech_task_failed"))
        store.fail_task(
            task_id=task.task_id,
            error_code=code,
            error_summary=_safe_error_summary(error),
            attempt_count=max(1, task.attempt_count),
            cleanup_pending=False,
            error_details=getattr(error, "error_details", None),
            upstream_log_id=getattr(error, "upstream_log_id", None),
        )
    return True


def _run_tts_task(
    store: SpeechTaskStore,
    task: PendingSpeechTask,
    config: SpeechModelRuntimeConfig,
    storage: LocalSpeechStorage,
    *,
    tts_provider_factory: Callable[[SpeechModelRuntimeConfig], object] | None,
    audio_inspector: Callable[[Path], AudioInspectionResult],
) -> None:
    if config.api_protocol not in {"volcengine_tts_v3", "openai_audio_speech"}:
        raise SpeechProviderError(
            "model_protocol_mismatch", "TTS 任务模型协议不匹配", retryable=False
        )
    generate_subtitle = bool(task.confirmation_snapshot.get("generate_subtitle"))
    if config.api_protocol == "openai_audio_speech" and generate_subtitle:
        raise SpeechProviderError(
            "tts_word_timestamps_unsupported",
            "当前中转语音模型不支持 TTS 字词时间戳字幕",
            retryable=False,
        )
    factory = tts_provider_factory or _tts_provider_from_config
    provider = factory(config)
    request = TtsSynthesisRequest(
        request_id=UUID(task.request_id),
        text=task.text_content,
        voice_type=task.voice_type or "",
        language=task.language or "",
        parameters=task.parameters,
    )
    attempts = 0
    while True:
        attempts += 1
        try:
            result = provider.synthesize(request)
            break
        except SpeechProviderError as error:
            if not error.retryable or attempts >= 2:
                raise

    timeline = None
    subtitle_content = None
    if generate_subtitle:
        segments = task.confirmation_snapshot.get("subtitle_segments")
        if not isinstance(segments, list) or not all(
            isinstance(item, str) for item in segments
        ):
            raise SpeechProviderError(
                "subtitle_segments_missing", "确认快照缺少字幕断句", retryable=False
            )
        timeline, subtitle_content = build_srt(result.words, segments)

    audio_artifact = storage.save_audio(
        task.task_id,
        result.audio_content,
        result.audio_format,
        preview=task.task_type == "tts_preview",
    )
    subtitle_artifact = None
    try:
        inspection = audio_inspector(audio_artifact.file_path)
        if subtitle_content is not None:
            subtitle_artifact = storage.save_subtitle(task.task_id, subtitle_content)
        store.complete_tts_task(
            task=task,
            audio_artifact=audio_artifact,
            subtitle_artifact=subtitle_artifact,
            audio_inspection=inspection,
            timeline=timeline,
            words=[word.__dict__ for word in result.words],
            upstream_log_id=result.upstream_log_id,
            attempt_count=attempts,
        )
    except Exception:
        storage.delete(audio_artifact)
        storage.delete(subtitle_artifact)
        raise


def _run_asr_task(
    store: SpeechTaskStore,
    task: PendingSpeechTask,
    config: SpeechModelRuntimeConfig,
    staging_config: SpeechStagingRuntimeConfig,
    storage: LocalSpeechStorage,
    *,
    asr_provider_factory: Callable[[SpeechModelRuntimeConfig], object] | None,
    tos_staging_factory: Callable[[SpeechStagingRuntimeConfig], object] | None,
    audio_inspector: Callable[[Path], AudioInspectionResult],
) -> None:
    if config.api_protocol != "volcengine_asr_v3":
        raise SpeechProviderError(
            "model_protocol_mismatch", "ASR 任务模型协议不匹配", retryable=False
        )
    if not task.source_file_url or not task.inspection_source_sha256:
        raise AudioInspectionError("inspection_missing", "ASR 任务缺少音频检查快照")
    source_path = storage.source_path(task.source_file_url)
    actual = audio_inspector(source_path)
    if actual.source_sha256 != task.inspection_source_sha256:
        raise AudioInspectionError(
            "audio_source_changed", "音频源文件已变化，请重新检查并确认"
        )
    if (
        task.inspection_duration_ms is None
        or actual.duration_ms != task.inspection_duration_ms
    ):
        raise AudioInspectionError(
            "audio_duration_changed", "音频真实时长已变化，请重新检查并确认"
        )
    if actual.file_size_bytes > staging_config.max_file_bytes:
        raise AudioInspectionError("audio_too_large", "音频超过 TOS 暂存大小上限")
    if actual.duration_ms > staging_config.max_audio_duration_seconds * 1000:
        raise AudioInspectionError("audio_too_long", "音频超过 ASR 时长上限")

    staging_factory = tos_staging_factory or _tos_staging_from_config
    staging = staging_factory(staging_config)
    object_key = task.staging_object_key
    signed_url = None
    if task.staging_status == "uploaded" and object_key:
        signed_url = staging.signed_get_url(object_key)
    else:
        content = source_path.read_bytes()
        try:
            staged = staging.stage(
                project_id=UUID(task.project_id),
                task_id=UUID(task.task_id),
                content=content,
                extension=source_path.suffix.lstrip(".").lower(),
                content_type=_audio_content_type(source_path.suffix),
            )
        except TosStagingError as error:
            if error.object_key and error.source_sha256:
                store.record_asr_staging(
                    task.task_id, error.object_key, error.source_sha256
                )
                store.fail_task(
                    task_id=task.task_id,
                    error_code=error.code,
                    error_summary=_safe_error_summary(error),
                    attempt_count=max(1, task.attempt_count),
                    cleanup_pending=True,
                )
                _cleanup_tos(store, task.task_id, staging, error.object_key)
                return
            raise
        object_key = staged.object_key
        signed_url = staged.signed_get_url
        store.record_asr_staging(task.task_id, object_key, staged.source_sha256)

    provider_factory = asr_provider_factory or _asr_provider_from_config
    provider = provider_factory(config)
    request_id = UUID(task.request_id)
    provider_attempt_count = max(1, task.attempt_count)
    try:
        if not task.upstream_submitted:
            attempts = 0
            while True:
                attempts += 1
                try:
                    provider.submit(request_id, signed_url, actual.container_format)
                    break
                except SpeechProviderError as error:
                    if not error.retryable or attempts >= 2:
                        raise
            provider_attempt_count = attempts
            store.record_asr_submitted(task.task_id, attempts)
        query = provider.query(request_id)
        if not query.is_terminal:
            store.defer_asr_task(task.task_id)
            return
        if not query.is_success:
            store.fail_task(
                task_id=task.task_id,
                error_code=query.error_code or "asr_upstream_failed",
                error_summary=query.error_summary or "ASR 上游任务失败",
                attempt_count=provider_attempt_count,
                cleanup_pending=True,
                upstream_log_id=query.upstream_log_id,
            )
            _cleanup_tos(store, task.task_id, staging, object_key)
            return
        segments = [
            str(item.get("text") or "").strip()
            for item in query.utterances
            if str(item.get("text") or "").strip()
        ]
        timeline, srt = build_srt(query.words, segments)
        subtitle_artifact = storage.save_subtitle(task.task_id, srt)
        try:
            store.complete_asr_task(
                task=task,
                subtitle_artifact=subtitle_artifact,
                timeline=timeline,
                words=[word.__dict__ for word in query.words],
                transcript=query.text,
                upstream_log_id=query.upstream_log_id,
                attempt_count=provider_attempt_count,
            )
        except Exception:
            storage.delete(subtitle_artifact)
            raise
        _cleanup_tos(store, task.task_id, staging, object_key)
    except Exception as error:
        store.fail_task(
            task_id=task.task_id,
            error_code=str(getattr(error, "code", "asr_task_failed")),
            error_summary=_safe_error_summary(error),
            attempt_count=provider_attempt_count,
            cleanup_pending=True,
            error_details=getattr(error, "error_details", None),
            upstream_log_id=getattr(error, "upstream_log_id", None),
        )
        _cleanup_tos(store, task.task_id, staging, object_key)


def _cleanup_tos(
    store: SpeechTaskStore, task_id: str, staging: object, object_key: str | None
) -> None:
    if not object_key:
        return
    try:
        staging.cleanup(object_key)
    except Exception as error:
        store.record_cleanup(task_id, False, _safe_error_summary(error))
    else:
        store.record_cleanup(task_id, True, None)


def _tts_provider_from_config(config: SpeechModelRuntimeConfig) -> object:
    if config.api_protocol == "volcengine_tts_v3":
        return VolcengineTtsV3Provider(
            api_key=config.api_key,
            base_url=config.request_base_url,
            resource_id=str(config.settings.get("resource_id") or ""),
            timeout_seconds=config.timeout_seconds,
        )
    if config.api_protocol == "openai_audio_speech":
        return OpenAiAudioSpeechProvider(
            api_key=config.api_key,
            base_url=config.request_base_url,
            upstream_model=config.upstream_model,
            timeout_seconds=config.timeout_seconds,
        )
    raise SpeechProviderError(
        "model_protocol_mismatch", "TTS 任务模型协议不匹配", retryable=False
    )


def _asr_provider_from_config(config: SpeechModelRuntimeConfig) -> VolcengineAsrV3Provider:
    return VolcengineAsrV3Provider(
        api_key=config.api_key,
        base_url=config.request_base_url,
        resource_id=str(config.settings.get("resource_id") or ""),
        timeout_seconds=config.timeout_seconds,
    )


def _tos_staging_from_config(config: SpeechStagingRuntimeConfig) -> TosAudioStaging:
    return TosAudioStaging(
        TosStagingConfig(
            endpoint=config.endpoint,
            region=config.region,
            bucket=config.bucket,
            object_prefix=config.object_prefix,
            signed_url_ttl_seconds=config.signed_url_ttl_seconds,
            max_file_bytes=config.max_file_bytes,
            access_key=config.access_key,
            secret_key=config.secret_key,
        )
    )


class PostgresSpeechStore:
    def __init__(self, database_url: str, worker_id: str | None = None) -> None:
        self.database_url = database_url
        self.worker_id = worker_id or os.getenv("HOSTNAME", "speech-worker")[:160]

    def _connect(self):
        import psycopg
        from psycopg.rows import dict_row

        return psycopg.connect(self.database_url, row_factory=dict_row)

    def recover_stale_work(self, lease_seconds: int = 600) -> None:
        if lease_seconds < 60:
            raise ValueError("speech worker lease must be at least 60 seconds")
        with self._connect() as connection:
            with connection.transaction():
                connection.execute(
                    """
                    UPDATE audio_material_inspections
                    SET status = 'queued', locked_at = NULL, worker_id = NULL,
                        started_at = NULL, updated_at = NOW()
                    WHERE status = 'running'
                      AND locked_at < NOW() - make_interval(secs => %s)
                    """,
                    (lease_seconds,),
                )
                connection.execute(
                    """
                    UPDATE sound_subtitle_tasks
                    SET status = 'failed', error_code = 'worker_interrupted',
                        error_summary = 'TTS Worker 执行中断；为避免重复计费，必须人工确认后重试',
                        completed_at = NOW(), locked_at = NULL, worker_id = NULL,
                        updated_at = NOW()
                    WHERE status = 'running'
                      AND task_type IN ('tts', 'tts_preview')
                      AND locked_at < NOW() - make_interval(secs => %s)
                    """,
                    (lease_seconds,),
                )
                connection.execute(
                    """
                    UPDATE sound_subtitle_tasks
                    SET status = 'queued', locked_at = NULL, worker_id = NULL,
                        updated_at = NOW()
                    WHERE status = 'running'
                      AND task_type = 'asr'
                      AND locked_at < NOW() - make_interval(secs => %s)
                    """,
                    (lease_seconds,),
                )

    def claim_next_audio_inspection(self) -> PendingAudioInspection | None:
        with self._connect() as connection:
            with connection.transaction():
                row = connection.execute(
                    """
                    WITH candidate AS (
                        SELECT inspection.id
                        FROM audio_material_inspections inspection
                        JOIN materials material ON material.id = inspection.material_id
                        WHERE inspection.status = 'queued'
                          AND material.status = 'active'
                          AND material.material_type = 'audio'
                        ORDER BY inspection.created_at ASC, inspection.id ASC
                        FOR UPDATE OF inspection SKIP LOCKED
                        LIMIT 1
                    )
                    UPDATE audio_material_inspections inspection
                    SET status = 'running', locked_at = NOW(), worker_id = %s,
                        started_at = COALESCE(started_at, NOW()), updated_at = NOW()
                    FROM candidate
                    WHERE inspection.id = candidate.id
                    RETURNING inspection.id, inspection.project_id, inspection.material_id,
                              (SELECT file_url FROM materials WHERE id = inspection.material_id) AS file_url
                    """,
                    (self.worker_id,),
                ).fetchone()
        if row is None:
            return None
        return PendingAudioInspection(
            inspection_id=str(row["id"]),
            project_id=str(row["project_id"]),
            material_id=str(row["material_id"]),
            file_url=str(row["file_url"]),
        )

    def complete_audio_inspection(
        self, inspection_id: str, result: AudioInspectionResult
    ) -> None:
        with self._connect() as connection:
            updated = connection.execute(
                """
                UPDATE audio_material_inspections
                SET status = 'succeeded', source_sha256 = %s, file_size_bytes = %s,
                    duration_ms = %s, container_format = %s, audio_codec = %s,
                    sample_rate_hz = %s, channel_count = %s, error_code = NULL,
                    error_summary = NULL, completed_at = NOW(), locked_at = NULL,
                    worker_id = NULL, updated_at = NOW()
                WHERE id = %s AND status = 'running'
                """,
                (
                    result.source_sha256,
                    result.file_size_bytes,
                    result.duration_ms,
                    result.container_format,
                    result.audio_codec,
                    result.sample_rate_hz,
                    result.channel_count,
                    inspection_id,
                ),
            )
            if updated.rowcount != 1:
                raise RuntimeError("audio inspection is no longer running")

    def fail_audio_inspection(
        self, inspection_id: str, error_code: str, error_summary: str
    ) -> None:
        with self._connect() as connection:
            connection.execute(
                """
                UPDATE audio_material_inspections
                SET status = 'failed', error_code = %s, error_summary = %s,
                    completed_at = NOW(), locked_at = NULL, worker_id = NULL,
                    updated_at = NOW()
                WHERE id = %s AND status = 'running'
                """,
                (error_code[:120], error_summary[:1000], inspection_id),
            )

    def claim_next_speech_task(self) -> PendingSpeechTask | None:
        with self._connect() as connection:
            with connection.transaction():
                row = connection.execute(
                    """
                    WITH candidate AS (
                        SELECT task.id
                        FROM sound_subtitle_tasks task
                        WHERE task.status = 'queued'
                          AND task.task_type IN ('tts_preview', 'tts', 'asr')
                          AND (
                              SELECT COUNT(*)
                              FROM sound_subtitle_tasks running
                              WHERE running.project_id = task.project_id
                                AND running.status = 'running'
                          ) < 2
                        ORDER BY task.created_at ASC, task.id ASC
                        FOR UPDATE OF task SKIP LOCKED
                        LIMIT 1
                    ), claimed AS (
                        UPDATE sound_subtitle_tasks task
                        SET status = 'running', locked_at = NOW(), worker_id = %s,
                            started_at = COALESCE(started_at, NOW()), updated_at = NOW()
                        FROM candidate
                        WHERE task.id = candidate.id
                        RETURNING task.*
                    )
                    SELECT claimed.*,
                           material.file_url AS source_file_url,
                           inspection.source_sha256 AS inspection_source_sha256,
                           inspection.duration_ms AS inspection_duration_ms
                    FROM claimed
                    LEFT JOIN materials material ON material.id = claimed.source_audio_material_id
                    LEFT JOIN audio_material_inspections inspection
                        ON inspection.id = claimed.audio_inspection_id
                    """,
                    (self.worker_id,),
                ).fetchone()
        if row is None:
            return None
        return PendingSpeechTask(
            task_id=str(row["id"]),
            project_id=str(row["project_id"]),
            task_type=str(row["task_type"]),
            model_id=str(row["model_id"]),
            tos_staging_config_id=(
                str(row["tos_staging_config_id"])
                if row["tos_staging_config_id"]
                else None
            ),
            tos_staging_config_version=(
                int(row["tos_staging_config_version"])
                if row["tos_staging_config_version"] is not None
                else None
            ),
            request_id=str(row["request_id"]),
            text_content=str(row["text_content"] or ""),
            voice_type=str(row["voice_type"]) if row["voice_type"] else None,
            language=str(row["language"]) if row["language"] else None,
            emotion=str(row["emotion"]) if row["emotion"] else None,
            parameters=dict(row["parameters"] or {}),
            model_snapshot=dict(row["model_snapshot"] or {}),
            voice_snapshot=(dict(row["voice_snapshot"]) if row["voice_snapshot"] else None),
            confirmation_snapshot=dict(row["confirmation_snapshot"] or {}),
            resource_usage=dict(row["resource_usage"] or {}),
            source_audio_material_id=(
                str(row["source_audio_material_id"])
                if row["source_audio_material_id"]
                else None
            ),
            source_file_url=(str(row["source_file_url"]) if row["source_file_url"] else None),
            inspection_source_sha256=(
                str(row["inspection_source_sha256"])
                if row["inspection_source_sha256"]
                else None
            ),
            inspection_duration_ms=(
                int(row["inspection_duration_ms"])
                if row["inspection_duration_ms"] is not None
                else None
            ),
            staging_status=str(row["staging_status"]),
            staging_object_key=(
                str(row["staging_object_key"]) if row["staging_object_key"] else None
            ),
            staging_source_sha256=(
                str(row["staging_source_sha256"])
                if row["staging_source_sha256"]
                else None
            ),
            upstream_submitted=row["upstream_submitted_at"] is not None,
            attempt_count=int(row["attempt_count"]),
        )

    def complete_tts_task(
        self,
        *,
        task: PendingSpeechTask,
        audio_artifact: SavedSpeechArtifact,
        subtitle_artifact: SavedSpeechArtifact | None,
        audio_inspection: AudioInspectionResult,
        timeline: list[dict[str, object]] | None,
        words: list[dict[str, object]],
        upstream_log_id: str | None,
        attempt_count: int,
    ) -> None:
        from psycopg.types.json import Jsonb

        with self._connect() as connection:
            with connection.transaction():
                audio_material_id = None
                if task.task_type == "tts":
                    audio_material_id = connection.execute(
                        """
                        INSERT INTO materials (
                            project_id, material_type, file_url, file_name, tags, metadata, status
                        )
                        VALUES (%s, 'audio', %s, %s, ARRAY['AI生成', 'TTS']::text[], %s, 'active')
                        RETURNING id
                        """,
                        (
                            task.project_id,
                            audio_artifact.file_url,
                            audio_artifact.file_name,
                            Jsonb(
                                {
                                    "source": "ai_generated",
                                    "audio_usage": "tts",
                                    "generation_task_id": task.task_id,
                                    "model_snapshot": task.model_snapshot,
                                    "voice_snapshot": task.voice_snapshot or {},
                                    "text_snapshot": task.text_content,
                                    "duration_ms": audio_inspection.duration_ms,
                                    "file_size_bytes": audio_inspection.file_size_bytes,
                                    "source_sha256": audio_inspection.source_sha256,
                                    "container_format": audio_inspection.container_format,
                                    "audio_codec": audio_inspection.audio_codec,
                                    "sample_rate_hz": audio_inspection.sample_rate_hz,
                                    "channel_count": audio_inspection.channel_count,
                                    "resource_usage": task.resource_usage,
                                }
                            ),
                        ),
                    ).fetchone()["id"]
                subtitle_material_id = None
                if subtitle_artifact is not None:
                    subtitle_material_id = connection.execute(
                        """
                        INSERT INTO materials (
                            project_id, material_type, file_url, file_name, tags, metadata, status
                        )
                        VALUES (%s, 'subtitle', %s, %s, ARRAY['AI生成', 'SRT']::text[], %s, 'active')
                        RETURNING id
                        """,
                        (
                            task.project_id,
                            subtitle_artifact.file_url,
                            subtitle_artifact.file_name,
                            Jsonb(
                                {
                                    "source": "ai_generated",
                                    "generation_task_id": task.task_id,
                                    "alignment_source": "tts_timestamp",
                                    "source_audio_material_id": str(audio_material_id),
                                    "model_snapshot": task.model_snapshot,
                                    "voice_snapshot": task.voice_snapshot or {},
                                    "timeline": timeline or [],
                                    "resource_usage": task.resource_usage,
                                }
                            ),
                        ),
                    ).fetchone()["id"]
                result = {
                    "audio_file_url": audio_artifact.file_url,
                    "audio_file_size_bytes": audio_artifact.file_size_bytes,
                    "subtitle_file_url": (
                        subtitle_artifact.file_url if subtitle_artifact else None
                    ),
                    "word_count": len(words),
                    "duration_ms": audio_inspection.duration_ms,
                }
                updated = connection.execute(
                    """
                    UPDATE sound_subtitle_tasks
                    SET status = 'succeeded', output_audio_material_id = %s,
                        output_subtitle_material_id = %s, timeline = %s, result = %s,
                        upstream_log_id = %s, attempt_count = LEAST(max_attempts, %s),
                        error_code = NULL, error_summary = NULL,
                        error_details = '{}'::jsonb, completed_at = NOW(),
                        locked_at = NULL, worker_id = NULL, updated_at = NOW()
                    WHERE id = %s AND status = 'running'
                    """,
                    (
                        audio_material_id,
                        subtitle_material_id,
                        Jsonb(timeline) if timeline is not None else None,
                        Jsonb(result),
                        upstream_log_id,
                        attempt_count,
                        task.task_id,
                    ),
                )
                if updated.rowcount != 1:
                    raise RuntimeError("TTS task is no longer running")

    def complete_asr_task(
        self,
        *,
        task: PendingSpeechTask,
        subtitle_artifact: SavedSpeechArtifact,
        timeline: list[dict[str, object]],
        words: list[dict[str, object]],
        transcript: str,
        upstream_log_id: str | None,
        attempt_count: int,
    ) -> None:
        from psycopg.types.json import Jsonb

        with self._connect() as connection:
            with connection.transaction():
                subtitle_material_id = connection.execute(
                    """
                    INSERT INTO materials (
                        project_id, material_type, file_url, file_name, tags, metadata, status
                    )
                    VALUES (%s, 'subtitle', %s, %s, ARRAY['AI生成', 'ASR', 'SRT']::text[], %s, 'active')
                    RETURNING id
                    """,
                    (
                        task.project_id,
                        subtitle_artifact.file_url,
                        subtitle_artifact.file_name,
                        Jsonb(
                            {
                                "source": "ai_generated",
                                "generation_task_id": task.task_id,
                                "alignment_source": "asr",
                                "source_audio_material_id": task.source_audio_material_id,
                                "model_snapshot": task.model_snapshot,
                                "timeline": timeline,
                                "resource_usage": task.resource_usage,
                            }
                        ),
                    ),
                ).fetchone()["id"]
                updated = connection.execute(
                    """
                    UPDATE sound_subtitle_tasks
                    SET status = 'succeeded', output_subtitle_material_id = %s,
                        text_content = %s, timeline = %s, result = %s,
                        upstream_log_id = %s, attempt_count = LEAST(max_attempts, %s),
                        staging_status = 'cleanup_pending', error_code = NULL,
                        error_summary = NULL, error_details = '{}'::jsonb,
                        completed_at = NOW(), locked_at = NULL,
                        worker_id = NULL, updated_at = NOW()
                    WHERE id = %s AND status = 'running'
                    """,
                    (
                        subtitle_material_id,
                        transcript,
                        Jsonb(timeline),
                        Jsonb(
                            {
                                "subtitle_file_url": subtitle_artifact.file_url,
                                "word_count": len(words),
                                "transcript_character_count": len(transcript),
                            }
                        ),
                        upstream_log_id,
                        attempt_count,
                        task.task_id,
                    ),
                )
                if updated.rowcount != 1:
                    raise RuntimeError("ASR task is no longer running")

    def fail_task(
        self,
        *,
        task_id: str,
        error_code: str,
        error_summary: str,
        attempt_count: int,
        cleanup_pending: bool,
        error_details: dict[str, object] | None = None,
        upstream_log_id: str | None = None,
    ) -> None:
        from psycopg.types.json import Jsonb

        with self._connect() as connection:
            connection.execute(
                """
                UPDATE sound_subtitle_tasks
                SET status = 'failed', error_code = %s, error_summary = %s,
                    error_details = %s,
                    upstream_log_id = COALESCE(%s, upstream_log_id),
                    attempt_count = LEAST(max_attempts, %s),
                    staging_status = CASE
                        WHEN %s AND staging_object_key IS NOT NULL THEN 'cleanup_pending'
                        ELSE staging_status
                    END,
                    completed_at = NOW(), locked_at = NULL, worker_id = NULL,
                    updated_at = NOW()
                WHERE id = %s AND status IN ('queued', 'running')
                """,
                (
                    error_code[:120],
                    error_summary[:1000],
                    Jsonb(_normalize_error_details(error_details)),
                    _safe_trace_id(upstream_log_id),
                    attempt_count,
                    cleanup_pending,
                    task_id,
                ),
            )

    def record_asr_staging(
        self, task_id: str, object_key: str, source_sha256: str
    ) -> None:
        with self._connect() as connection:
            updated = connection.execute(
                """
                UPDATE sound_subtitle_tasks
                SET staging_object_key = %s, staging_source_sha256 = %s,
                    staging_status = 'uploaded', updated_at = NOW()
                WHERE id = %s AND status = 'running' AND task_type = 'asr'
                """,
                (object_key, source_sha256, task_id),
            )
            if updated.rowcount != 1:
                raise RuntimeError("ASR task cannot record TOS staging")

    def record_asr_submitted(self, task_id: str, attempt_count: int) -> None:
        with self._connect() as connection:
            updated = connection.execute(
                """
                UPDATE sound_subtitle_tasks
                SET upstream_submitted_at = COALESCE(upstream_submitted_at, NOW()),
                    attempt_count = LEAST(max_attempts, %s),
                    updated_at = NOW()
                WHERE id = %s AND status = 'running' AND task_type = 'asr'
                """,
                (attempt_count, task_id),
            )
            if updated.rowcount != 1:
                raise RuntimeError("ASR task cannot record submission")

    def defer_asr_task(self, task_id: str) -> None:
        with self._connect() as connection:
            connection.execute(
                """
                UPDATE sound_subtitle_tasks
                SET status = 'queued', locked_at = NULL, worker_id = NULL,
                    updated_at = NOW()
                WHERE id = %s AND status = 'running' AND upstream_submitted_at IS NOT NULL
                """,
                (task_id,),
            )

    def record_cleanup(
        self, task_id: str, succeeded: bool, error_summary: str | None
    ) -> None:
        with self._connect() as connection:
            connection.execute(
                """
                UPDATE sound_subtitle_tasks
                SET staging_status = CASE WHEN %s THEN 'cleaned' ELSE 'cleanup_pending' END,
                    cleanup_attempt_count = cleanup_attempt_count + 1,
                    cleanup_error_summary = CASE WHEN %s THEN NULL ELSE %s END,
                    locked_at = NULL, worker_id = NULL, updated_at = NOW()
                WHERE id = %s AND staging_status IN ('uploaded', 'cleanup_pending')
                """,
                (succeeded, succeeded, error_summary, task_id),
            )

    def claim_next_cleanup(self) -> PendingTosCleanup | None:
        with self._connect() as connection:
            with connection.transaction():
                row = connection.execute(
                    """
                    WITH candidate AS (
                        SELECT id
                        FROM sound_subtitle_tasks
                        WHERE staging_status = 'cleanup_pending'
                          AND status IN ('succeeded', 'failed', 'cancelled')
                          AND (
                              locked_at IS NULL
                              OR locked_at < NOW() - INTERVAL '10 minutes'
                          )
                        ORDER BY updated_at ASC, id ASC
                        FOR UPDATE SKIP LOCKED
                        LIMIT 1
                    )
                    UPDATE sound_subtitle_tasks task
                    SET locked_at = NOW(), worker_id = %s, updated_at = NOW()
                    FROM candidate
                    WHERE task.id = candidate.id
                    RETURNING task.id, task.tos_staging_config_id,
                              task.tos_staging_config_version, task.staging_object_key
                    """,
                    (self.worker_id,),
                ).fetchone()
        if row is None:
            return None
        return PendingTosCleanup(
            task_id=str(row["id"]),
            tos_staging_config_id=str(row["tos_staging_config_id"]),
            tos_staging_config_version=int(row["tos_staging_config_version"]),
            object_key=str(row["staging_object_key"]),
        )


def run_next_tos_cleanup(
    store: PostgresSpeechStore,
    model_registry: SpeechModelRegistry,
    *,
    tos_staging_factory: Callable[[SpeechStagingRuntimeConfig], object] | None = None,
) -> bool:
    cleanup = store.claim_next_cleanup()
    if cleanup is None:
        return False
    try:
        staging_config = model_registry.resolve_tos_staging(
            cleanup.tos_staging_config_id,
            cleanup.tos_staging_config_version,
        )
        factory = tos_staging_factory or _tos_staging_from_config
        factory(staging_config).cleanup(cleanup.object_key)
    except Exception as error:
        store.record_cleanup(cleanup.task_id, False, _safe_error_summary(error))
    else:
        store.record_cleanup(cleanup.task_id, True, None)
    return True


def inspect_audio_file(
    path: Path,
    *,
    runner: Callable[[list[str], int], object] | None = None,
) -> AudioInspectionResult:
    source = Path(path)
    if not source.is_file():
        raise AudioInspectionError("audio_file_missing", "自管音频文件不存在")
    file_size = source.stat().st_size
    if file_size <= 0:
        raise AudioInspectionError("audio_file_empty", "音频文件为空")
    digest = hashlib.sha256()
    with source.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)

    command = [
        "ffprobe",
        "-v",
        "error",
        "-show_entries",
        "format=format_name,duration,size:stream=codec_type,codec_name,sample_rate,channels,duration",
        "-of",
        "json",
        str(source),
    ]
    execute = runner or _run_ffprobe
    try:
        completed = execute(command, 30)
    except (OSError, subprocess.SubprocessError) as error:
        raise AudioInspectionError(
            "ffprobe_unavailable", f"ffprobe 执行失败: {error.__class__.__name__}"
        ) from error
    if int(getattr(completed, "returncode", 1)) != 0:
        raise AudioInspectionError("audio_probe_failed", "ffprobe 无法解析该音频")
    try:
        payload = json.loads(str(getattr(completed, "stdout", "")))
    except json.JSONDecodeError as error:
        raise AudioInspectionError("audio_probe_invalid", "ffprobe 返回无效 JSON") from error
    streams = payload.get("streams")
    if not isinstance(streams, list):
        streams = []
    audio_stream = next(
        (
            stream
            for stream in streams
            if isinstance(stream, dict) and stream.get("codec_type") == "audio"
        ),
        None,
    )
    if audio_stream is None:
        raise AudioInspectionError("audio_stream_missing", "媒体文件不包含音频流")
    format_payload = payload.get("format")
    if not isinstance(format_payload, dict):
        format_payload = {}
    duration = _positive_float(
        audio_stream.get("duration") or format_payload.get("duration"),
        "audio_duration_invalid",
        "无法读取真实音频时长",
    )
    sample_rate = _positive_int(
        audio_stream.get("sample_rate"),
        "audio_sample_rate_invalid",
        "无法读取音频采样率",
    )
    channels = _positive_int(
        audio_stream.get("channels"),
        "audio_channels_invalid",
        "无法读取音频声道数",
    )
    container_format = str(format_payload.get("format_name") or "").split(",")[0].strip()
    codec = str(audio_stream.get("codec_name") or "").strip()
    if not container_format or not codec:
        raise AudioInspectionError("audio_format_invalid", "无法读取音频格式或 codec")
    return AudioInspectionResult(
        source_sha256=digest.hexdigest(),
        file_size_bytes=file_size,
        duration_ms=max(1, round(duration * 1000)),
        container_format=container_format,
        audio_codec=codec,
        sample_rate_hz=sample_rate,
        channel_count=channels,
    )


class VolcengineTtsV3Provider:
    def __init__(
        self,
        *,
        api_key: str,
        base_url: str,
        resource_id: str,
        timeout_seconds: int,
        http_post: Callable[[str, dict[str, str], dict[str, object], int], ProviderHttpResponse]
        | None = None,
    ) -> None:
        if not api_key.strip():
            raise SpeechProviderError(
                "tts_config_invalid", "TTS API Key 不能为空", retryable=False
            )
        if resource_id != "seed-tts-2.0":
            raise SpeechProviderError(
                "tts_config_invalid", "TTS Resource ID 无效", retryable=False
            )
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")
        self.resource_id = resource_id
        self.timeout_seconds = timeout_seconds
        self.http_post = http_post or default_json_stream_post

    def synthesize(self, request: TtsSynthesisRequest) -> TtsSynthesisResult:
        audio_format = str(request.parameters.get("audio_format") or "").lower()
        sample_rate = request.parameters.get("sample_rate")
        audio_params: dict[str, object] = {
            "format": audio_format,
            "sample_rate": sample_rate,
        }
        for name, value in request.parameters.items():
            if name not in {"audio_format", "sample_rate"}:
                audio_params[name] = value
        payload: dict[str, object] = {
            "user": {"uid": str(request.request_id)},
            "req_params": {
                "text": request.text,
                "speaker": request.voice_type,
                "explicit_language": request.language,
                "audio_params": audio_params,
            },
        }
        response = self.http_post(
            f"{self.base_url}/tts/unidirectional",
            {
                "Content-Type": "application/json",
                "X-Api-Key": self.api_key,
                "X-Api-Resource-Id": self.resource_id,
                "X-Api-Request-Id": str(request.request_id),
            },
            payload,
            self.timeout_seconds,
        )
        _raise_for_http(response, "tts")
        audio_chunks: list[bytes] = []
        words: list[TimestampWord] = []
        for raw_line in response.body_lines:
            line = raw_line.decode("utf-8", errors="strict").strip()
            if not line:
                continue
            try:
                item = json.loads(line)
            except json.JSONDecodeError as error:
                raise SpeechProviderError(
                    "tts_stream_invalid", "TTS 流包含无效 JSON", retryable=True
                ) from error
            code = item.get("code")
            if code not in (None, 0):
                raise SpeechProviderError(
                    "tts_upstream_error",
                    _safe_upstream_message(item.get("message"), "TTS 上游返回失败"),
                    retryable=False,
                )
            encoded = item.get("data")
            if isinstance(encoded, str) and encoded:
                try:
                    audio_chunks.append(base64.b64decode(encoded, validate=True))
                except (ValueError, binascii.Error) as error:
                    raise SpeechProviderError(
                        "tts_stream_invalid", "TTS 音频块 base64 无效", retryable=True
                    ) from error
            sentence = item.get("sentence")
            if isinstance(sentence, dict):
                _append_tts_words(words, sentence.get("words"))
        audio_content = b"".join(audio_chunks)
        if not _valid_audio_signature(audio_content, audio_format):
            raise SpeechProviderError(
                "tts_audio_invalid", "TTS 返回的音频不完整或格式不匹配", retryable=True
            )
        upstream_log_id = _header_value(response.headers, "X-Tt-Logid")
        return TtsSynthesisResult(
            audio_content=audio_content,
            audio_format=audio_format,
            words=words,
            upstream_log_id=upstream_log_id,
        )


class OpenAiAudioSpeechProvider:
    def __init__(
        self,
        *,
        api_key: str,
        base_url: str,
        upstream_model: str,
        timeout_seconds: int,
        http_post: Callable[
            [str, dict[str, str], dict[str, object], int], ProviderHttpResponse
        ]
        | None = None,
    ) -> None:
        normalized_base_url = base_url.strip().rstrip("/")
        parsed_url = urlsplit(normalized_base_url)
        if (
            not api_key.strip()
            or not upstream_model.strip()
            or timeout_seconds <= 0
            or parsed_url.scheme not in {"http", "https"}
            or not parsed_url.netloc
            or parsed_url.query
            or parsed_url.fragment
            or not parsed_url.path.rstrip("/").endswith("/v1")
        ):
            raise SpeechProviderError(
                "tts_config_invalid",
                "OpenAI Audio Speech 模型配置无效",
                retryable=False,
            )
        self.api_key = api_key
        self.base_url = normalized_base_url
        self.upstream_model = upstream_model.strip()
        self.timeout_seconds = timeout_seconds
        self.http_post = http_post or default_json_stream_post

    def synthesize(self, request: TtsSynthesisRequest) -> TtsSynthesisResult:
        audio_format = str(request.parameters.get("audio_format") or "").lower()
        if audio_format not in {"mp3", "wav", "ogg", "aac", "pcm", "raw"}:
            raise SpeechProviderError(
                "audio_format_unsupported", "输出音频格式无效", retryable=False
            )
        payload: dict[str, object] = {
            "model": self.upstream_model,
            "input": request.text,
            "voice": request.voice_type,
            "response_format": audio_format,
        }
        speed = request.parameters.get("speed_ratio")
        if speed is not None:
            if (
                isinstance(speed, bool)
                or not isinstance(speed, (int, float))
                or not 0.25 <= float(speed) <= 4.0
            ):
                raise SpeechProviderError(
                    "tts_speed_invalid", "TTS 语速参数无效", retryable=False
                )
            payload["speed"] = speed
        response = self.http_post(
            f"{self.base_url}/audio/speech",
            {
                "Authorization": f"Bearer {self.api_key}",
                "Content-Type": "application/json",
            },
            payload,
            self.timeout_seconds,
        )
        _raise_for_http(response, "tts")
        content_type = _header_value(response.headers, "Content-Type")
        if not _valid_audio_content_type(content_type, audio_format):
            raise SpeechProviderError(
                "tts_content_type_invalid",
                "TTS 返回的 Content-Type 与音频格式不匹配",
                retryable=True,
            )
        audio_content = response.body_content
        if audio_content is None or not _valid_audio_signature(
            audio_content, audio_format
        ):
            raise SpeechProviderError(
                "tts_audio_invalid",
                "TTS 返回的音频不完整或格式不匹配",
                retryable=True,
            )
        return TtsSynthesisResult(
            audio_content=audio_content,
            audio_format=audio_format,
            words=[],
            upstream_log_id=_gateway_request_id(response.headers),
        )


class VolcengineAsrV3Provider:
    def __init__(
        self,
        *,
        api_key: str,
        base_url: str,
        resource_id: str,
        timeout_seconds: int,
        http_post: Callable[[str, dict[str, str], dict[str, object], int], ProviderHttpResponse]
        | None = None,
    ) -> None:
        if not api_key.strip() or resource_id != "volc.seedasr.auc":
            raise SpeechProviderError(
                "asr_config_invalid", "ASR 模型配置无效", retryable=False
            )
        self.api_key = api_key
        self.base_url = base_url.rstrip("/")
        self.resource_id = resource_id
        self.timeout_seconds = timeout_seconds
        self.http_post = http_post or default_json_stream_post

    def submit(self, request_id: UUID, signed_audio_url: str, audio_format: str) -> AsrSubmitResult:
        if not signed_audio_url.startswith("https://"):
            raise SpeechProviderError(
                "asr_audio_url_invalid", "ASR 音频 URL 必须使用 HTTPS", retryable=False
            )
        response = self.http_post(
            f"{self.base_url}/auc/bigmodel/submit",
            self._headers(request_id),
            {
                "user": {"uid": str(request_id)},
                "audio": {"url": signed_audio_url, "format": audio_format},
                "request": {
                    "model_name": "bigmodel",
                    "enable_itn": True,
                    "enable_punc": True,
                    "show_utterances": True,
                },
            },
            self.timeout_seconds,
        )
        _raise_for_http(response, "asr")
        status_code = _header_value(response.headers, "X-Api-Status-Code")
        if status_code not in (None, "20000000"):
            raise SpeechProviderError(
                "asr_submit_failed",
                "ASR 上游拒绝任务",
                retryable=_retryable_asr_status(status_code),
            )
        return AsrSubmitResult(accepted=True)

    def query(self, request_id: UUID) -> AsrQueryResult:
        response = self.http_post(
            f"{self.base_url}/auc/bigmodel/query",
            self._headers(request_id),
            {},
            self.timeout_seconds,
        )
        _raise_for_http(response, "asr")
        status_code = _header_value(response.headers, "X-Api-Status-Code")
        upstream_log_id = _header_value(response.headers, "X-Tt-Logid")
        if status_code in {"20000001", "20000002"}:
            return AsrQueryResult(False, False, "", [], [], upstream_log_id)
        body = b"\n".join(response.body_lines).strip()
        payload: dict[str, object] = {}
        if body:
            try:
                parsed = json.loads(body)
            except json.JSONDecodeError as error:
                raise SpeechProviderError(
                    "asr_response_invalid", "ASR 查询返回无效 JSON", retryable=True
                ) from error
            if not isinstance(parsed, dict):
                raise SpeechProviderError(
                    "asr_response_invalid", "ASR 查询结果必须是 object", retryable=True
                )
            payload = parsed
        if status_code not in (None, "20000000"):
            return AsrQueryResult(
                True,
                False,
                "",
                [],
                [],
                upstream_log_id,
                error_code=status_code or "asr_query_failed",
                error_summary="ASR 上游任务失败",
            )
        result = payload.get("result")
        if not isinstance(result, dict):
            raise SpeechProviderError(
                "asr_response_invalid", "ASR 成功结果缺少 result", retryable=True
            )
        utterances_value = result.get("utterances")
        utterances = utterances_value if isinstance(utterances_value, list) else []
        words = _asr_words(utterances)
        if not words:
            raise SpeechProviderError(
                "timestamps_unavailable", "ASR 未返回可信字词时间戳", retryable=False
            )
        return AsrQueryResult(
            True,
            True,
            str(result.get("text") or ""),
            words,
            [item for item in utterances if isinstance(item, dict)],
            upstream_log_id,
        )

    def _headers(self, request_id: UUID) -> dict[str, str]:
        return {
            "Content-Type": "application/json",
            "X-Api-Key": self.api_key,
            "X-Api-Resource-Id": self.resource_id,
            "X-Api-Request-Id": str(request_id),
        }


def build_srt(
    words: list[TimestampWord],
    segments: list[str],
) -> tuple[list[dict[str, object]], str]:
    if not words:
        raise SpeechProviderError(
            "timestamps_unavailable", "供应商未返回可信字词时间戳", retryable=False
        )
    cleaned_segments = [segment.strip() for segment in segments if segment.strip()]
    if not cleaned_segments:
        raise SpeechProviderError(
            "subtitle_segments_missing", "字幕断句不能为空", retryable=False
        )
    timeline: list[dict[str, object]] = []
    word_index = 0
    for index, segment in enumerate(cleaned_segments, start=1):
        target = _alignment_text(segment)
        if not target:
            continue
        start_index = word_index
        accumulated = ""
        while word_index < len(words) and len(accumulated) < len(target):
            word = words[word_index]
            if word.end_ms <= word.start_ms or word.start_ms < 0:
                raise SpeechProviderError(
                    "timestamps_invalid", "字词时间戳无效", retryable=False
                )
            accumulated += _alignment_text(word.text)
            word_index += 1
        if accumulated != target or start_index == word_index:
            raise SpeechProviderError(
                "subtitle_alignment_mismatch",
                "字幕断句无法与供应商字词边界对齐",
                retryable=False,
            )
        timeline.append(
            {
                "index": index,
                "start_ms": words[start_index].start_ms,
                "end_ms": words[word_index - 1].end_ms,
                "text": segment,
            }
        )
    if word_index != len(words):
        raise SpeechProviderError(
            "subtitle_alignment_mismatch",
            "字幕断句未覆盖全部供应商字词时间戳",
            retryable=False,
        )
    blocks = [
        f"{item['index']}\n{_srt_time(int(item['start_ms']))} --> {_srt_time(int(item['end_ms']))}\n{item['text']}"
        for item in timeline
    ]
    return timeline, "\n\n".join(blocks) + "\n"


def default_json_stream_post(
    url: str,
    headers: dict[str, str],
    payload: dict[str, object],
    timeout_seconds: int,
) -> ProviderHttpResponse:
    request = urllib_request.Request(
        url,
        data=json.dumps(payload, ensure_ascii=False).encode("utf-8"),
        headers=headers,
        method="POST",
    )
    try:
        with urllib_request.urlopen(request, timeout=timeout_seconds) as response:
            body_content = response.read()
            return ProviderHttpResponse(
                status_code=int(response.status),
                headers={key: value for key, value in response.headers.items()},
                body_lines=body_content.splitlines(),
                body_content=body_content,
            )
    except urllib_error.HTTPError as error:
        body_content = error.read(4096)
        return ProviderHttpResponse(
            status_code=error.code,
            headers={key: value for key, value in error.headers.items()},
            body_lines=body_content.splitlines(),
            body_content=body_content,
        )
    except (urllib_error.URLError, TimeoutError) as error:
        raise SpeechProviderError(
            "speech_network_error",
            f"语音供应商网络错误: {error.__class__.__name__}",
            retryable=True,
        ) from error


def _run_ffprobe(command: list[str], timeout: int):
    return subprocess.run(
        command,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )


def _positive_float(value: object, code: str, message: str) -> float:
    try:
        parsed = float(value)
    except (TypeError, ValueError) as error:
        raise AudioInspectionError(code, message) from error
    if parsed <= 0:
        raise AudioInspectionError(code, message)
    return parsed


def _positive_int(value: object, code: str, message: str) -> int:
    try:
        parsed = int(value)
    except (TypeError, ValueError) as error:
        raise AudioInspectionError(code, message) from error
    if parsed <= 0:
        raise AudioInspectionError(code, message)
    return parsed


def _append_tts_words(target: list[TimestampWord], value: object) -> None:
    if not isinstance(value, list):
        return
    for item in value:
        if not isinstance(item, dict):
            continue
        text = str(item.get("word") or item.get("text") or "").strip()
        if not text:
            continue
        try:
            start_ms = round(float(item.get("startTime")) * 1000)
            end_ms = round(float(item.get("endTime")) * 1000)
        except (TypeError, ValueError):
            continue
        confidence_value = item.get("confidence")
        confidence = (
            float(confidence_value)
            if isinstance(confidence_value, (int, float))
            else None
        )
        word = TimestampWord(text, start_ms, end_ms, confidence)
        if not target or target[-1] != word:
            target.append(word)


def _asr_words(utterances: list[object]) -> list[TimestampWord]:
    words: list[TimestampWord] = []
    for utterance in utterances:
        if not isinstance(utterance, dict):
            continue
        values = utterance.get("words")
        if not isinstance(values, list):
            continue
        for item in values:
            if not isinstance(item, dict):
                continue
            text = str(item.get("text") or item.get("word") or "").strip()
            try:
                start_ms = int(item.get("start_time"))
                end_ms = int(item.get("end_time"))
            except (TypeError, ValueError):
                continue
            if text and start_ms >= 0 and end_ms > start_ms:
                words.append(TimestampWord(text, start_ms, end_ms, None))
    return words


def _valid_audio_signature(content: bytes, audio_format: str) -> bool:
    if not content:
        return False
    if audio_format == "mp3":
        return content.startswith(b"ID3") or (
            len(content) >= 2 and content[0] == 0xFF and content[1] & 0xE0 == 0xE0
        )
    if audio_format == "wav":
        return len(content) >= 12 and content[:4] == b"RIFF" and content[8:12] == b"WAVE"
    if audio_format == "ogg":
        return content.startswith(b"OggS")
    if audio_format == "aac":
        return len(content) >= 2 and content[0] == 0xFF and content[1] & 0xF0 == 0xF0
    if audio_format in {"pcm", "raw"}:
        return len(content) >= 2 and len(content) % 2 == 0
    return False


def _valid_audio_content_type(content_type: str | None, audio_format: str) -> bool:
    if not content_type:
        return False
    media_type = content_type.split(";", 1)[0].strip().lower()
    expected = {
        "mp3": {"audio/mpeg", "audio/mp3"},
        "wav": {"audio/wav", "audio/wave", "audio/x-wav"},
        "ogg": {"audio/ogg", "application/ogg"},
        "aac": {"audio/aac", "audio/aacp", "audio/x-aac"},
        "pcm": {"audio/pcm", "audio/l16"},
        "raw": {"audio/pcm", "audio/l16"},
    }
    return media_type == "application/octet-stream" or media_type in expected.get(
        audio_format, set()
    )


def _raise_for_http(response: ProviderHttpResponse, provider: str) -> None:
    status_code = response.status_code
    if 200 <= status_code < 300:
        return
    error_details: dict[str, object] = {"http_status": status_code}
    payload = _provider_error_payload(response)
    if payload is not None:
        provider_code, provider_message = _extract_provider_error(payload)
        if provider_code is not None:
            error_details["provider_error_code"] = provider_code
        if provider_message is not None:
            error_details["provider_error_message"] = provider_message
    raise SpeechProviderError(
        f"{provider}_http_error",
        f"语音供应商返回 HTTP {status_code}",
        retryable=status_code == 429 or 500 <= status_code <= 599,
        error_details=error_details,
        upstream_log_id=(
            _header_value(response.headers, "X-Tt-Logid")
            or _gateway_request_id(response.headers)
        ),
    )


def _provider_error_payload(response: ProviderHttpResponse) -> dict[str, object] | None:
    limit = 64 * 1024
    if response.body_content is not None:
        if len(response.body_content) > limit:
            return None
        content = response.body_content
    else:
        chunks: list[bytes] = []
        size = 0
        for line in response.body_lines:
            size += len(line) + 1
            if size > limit:
                return None
            chunks.append(line)
        content = b"\n".join(chunks)
    if not content.strip():
        return None
    try:
        payload = json.loads(content)
    except (UnicodeDecodeError, json.JSONDecodeError):
        return None
    return payload if isinstance(payload, dict) else None


def _extract_provider_error(
    payload: dict[str, object],
) -> tuple[str | None, str | None]:
    candidates = [payload]
    candidates.extend(
        value
        for key in ("header", "error")
        if isinstance((value := payload.get(key)), dict)
    )
    provider_code = None
    provider_message = None
    for candidate in candidates:
        if provider_code is None:
            for key in ("code", "error_code", "errorCode"):
                value = candidate.get(key)
                if isinstance(value, bool) or not isinstance(value, (str, int, float)):
                    continue
                normalized = " ".join(str(value).split())[:120]
                if normalized:
                    provider_code = normalized
                    break
        if provider_message is None:
            for key in ("message", "msg", "error_message", "errorMessage"):
                value = candidate.get(key)
                if not isinstance(value, str) or not value.strip():
                    continue
                provider_message = _redact_sensitive_message(value)
                break
    return provider_code, provider_message


def _normalize_error_details(
    details: dict[str, object] | None,
) -> dict[str, object]:
    if not isinstance(details, dict):
        return {}
    normalized: dict[str, object] = {}
    status = details.get("http_status")
    if isinstance(status, int) and not isinstance(status, bool) and 100 <= status <= 599:
        normalized["http_status"] = status
    code = details.get("provider_error_code")
    if isinstance(code, (str, int, float)) and not isinstance(code, bool):
        safe_code = " ".join(str(code).split())[:120]
        if safe_code:
            normalized["provider_error_code"] = safe_code
    message = details.get("provider_error_message")
    if isinstance(message, str) and message.strip():
        normalized["provider_error_message"] = _redact_sensitive_message(message)
    return normalized


def _redact_sensitive_message(value: str) -> str:
    message = " ".join(value.split())[:1000]
    named_secret = re.compile(
        r"(?i)\b(authorization|x-api-key|api[ _-]?key|access[ _-]?key|secret[ _-]?key)"
        r"\s*[:=]\s*(?:bearer\s+)?[^\s,;]+"
    )
    message = named_secret.sub(lambda match: f"{match.group(1)}: [REDACTED]", message)
    return re.sub(r"(?i)\bbearer\s+[^\s,;]+", "Bearer [REDACTED]", message)[:500]


def _safe_trace_id(value: object) -> str | None:
    if not isinstance(value, str) or not value.strip():
        return None
    return " ".join(value.split())[:240]


def _retryable_asr_status(status_code: str | None) -> bool:
    return bool(status_code and status_code.startswith("5"))


def _header_value(headers: dict[str, str], name: str) -> str | None:
    expected = name.lower()
    return next((value for key, value in headers.items() if key.lower() == expected), None)


def _gateway_request_id(headers: dict[str, str]) -> str | None:
    return _header_value(headers, "X-Request-Id") or _header_value(
        headers, "X-OneAPI-Request-Id"
    )


def _safe_upstream_message(value: object, fallback: str) -> str:
    if not isinstance(value, str) or not value.strip():
        return fallback
    return _redact_sensitive_message(value)


def _alignment_text(value: str) -> str:
    return "".join(character for character in value if not character.isspace())


def _srt_time(milliseconds: int) -> str:
    hours, remainder = divmod(milliseconds, 3_600_000)
    minutes, remainder = divmod(remainder, 60_000)
    seconds, millis = divmod(remainder, 1000)
    return f"{hours:02d}:{minutes:02d}:{seconds:02d},{millis:03d}"


def _atomic_write(path: Path, content: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    try:
        temporary.write_bytes(content)
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def _safe_error_summary(error: Exception) -> str:
    message = " ".join((str(error) or error.__class__.__name__).split())
    if "?" in message and "http" in message.lower():
        message = message.split("?", 1)[0]
    return message[:1000]


def _audio_content_type(suffix: str) -> str:
    return {
        ".mp3": "audio/mpeg",
        ".wav": "audio/wav",
        ".ogg": "audio/ogg",
        ".aac": "audio/aac",
        ".m4a": "audio/mp4",
        ".flac": "audio/flac",
    }.get(suffix.lower(), "application/octet-stream")
