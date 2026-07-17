from __future__ import annotations

import os
from typing import Callable, Protocol

from video_worker.model_registry import SpeechStagingRuntimeConfig
from video_worker.tos_staging import (
    TosConnectionChecker,
    TosStagingConfig,
    TosStagingError,
)


class TosCheckStore(Protocol):
    def claim_next_check(self) -> SpeechStagingRuntimeConfig | None: ...

    def complete_check(
        self,
        config_id: str,
        version: int,
        *,
        succeeded: bool,
        error_summary: str | None,
    ) -> None: ...


class PostgresTosCheckStore:
    def __init__(self, database_url: str, worker_id: str | None = None) -> None:
        self.database_url = database_url
        self.worker_id = worker_id or os.getenv("HOSTNAME", "tos-check-worker")[:160]

    def _connect(self):
        import psycopg
        from psycopg.rows import dict_row

        return psycopg.connect(self.database_url, row_factory=dict_row)

    def recover_stale_checks(self, lease_seconds: int = 600) -> None:
        if lease_seconds < 60:
            raise ValueError("TOS check worker lease must be at least 60 seconds")
        with self._connect() as connection:
            connection.execute(
                """
                UPDATE tos_staging_tool_configs
                SET last_check_status = 'queued', check_locked_at = NULL,
                    check_worker_id = NULL, updated_at = NOW()
                WHERE last_check_status = 'running'
                  AND check_locked_at < NOW() - make_interval(secs => %s)
                """,
                (lease_seconds,),
            )

    def claim_next_check(self) -> SpeechStagingRuntimeConfig | None:
        with self._connect() as connection:
            with connection.transaction():
                row = connection.execute(
                    """
                    WITH candidate AS (
                        SELECT id
                        FROM tos_staging_tool_configs
                        WHERE last_check_status = 'queued'
                        ORDER BY last_check_requested_at, id
                        FOR UPDATE SKIP LOCKED
                        LIMIT 1
                    )
                    UPDATE tos_staging_tool_configs config
                    SET last_check_status = 'running', check_locked_at = NOW(),
                        check_worker_id = %s, updated_at = NOW()
                    FROM candidate
                    WHERE config.id = candidate.id
                    RETURNING config.id, config.version, config.storage_provider,
                              config.endpoint, config.region, config.bucket,
                              config.object_prefix, config.access_key, config.secret_key,
                              config.signed_url_ttl_seconds, config.max_file_bytes,
                              config.max_audio_duration_seconds
                    """,
                    (self.worker_id,),
                ).fetchone()
        if row is None:
            return None
        return SpeechStagingRuntimeConfig(
            config_id=str(row["id"]),
            version=int(row["version"]),
            storage_provider=str(row["storage_provider"]),
            endpoint=str(row["endpoint"]),
            region=str(row["region"]),
            bucket=str(row["bucket"]),
            object_prefix=str(row["object_prefix"]),
            access_key=str(row["access_key"]),
            secret_key=str(row["secret_key"]),
            signed_url_ttl_seconds=int(row["signed_url_ttl_seconds"]),
            max_file_bytes=int(row["max_file_bytes"]),
            max_audio_duration_seconds=int(row["max_audio_duration_seconds"]),
        )

    def complete_check(
        self,
        config_id: str,
        version: int,
        *,
        succeeded: bool,
        error_summary: str | None,
    ) -> None:
        with self._connect() as connection:
            updated = connection.execute(
                """
                UPDATE tos_staging_tool_configs
                SET last_check_status = CASE WHEN %s THEN 'succeeded' ELSE 'failed' END,
                    last_checked_at = NOW(),
                    last_check_error_summary = CASE WHEN %s THEN NULL ELSE %s END,
                    check_locked_at = NULL, check_worker_id = NULL, updated_at = NOW()
                WHERE id = %s AND version = %s AND last_check_status = 'running'
                  AND check_worker_id = %s
                """,
                (
                    succeeded,
                    succeeded,
                    error_summary,
                    config_id,
                    version,
                    self.worker_id,
                ),
            )
            if updated.rowcount != 1:
                raise RuntimeError("TOS connection check is no longer running")


def run_next_tos_connection_check(
    store: TosCheckStore,
    *,
    checker_factory: Callable[[SpeechStagingRuntimeConfig], object] | None = None,
) -> bool:
    config = store.claim_next_check()
    if config is None:
        return False
    factory = checker_factory or _checker_from_config
    try:
        factory(config).check()
    except Exception as error:
        store.complete_check(
            config.config_id,
            config.version,
            succeeded=False,
            error_summary=_safe_check_error(error),
        )
    else:
        store.complete_check(
            config.config_id,
            config.version,
            succeeded=True,
            error_summary=None,
        )
    return True


def _checker_from_config(config: SpeechStagingRuntimeConfig) -> TosConnectionChecker:
    return TosConnectionChecker(
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


def _safe_check_error(error: Exception) -> str:
    message = str(error) if isinstance(error, TosStagingError) else error.__class__.__name__
    return f"TOS Bucket 能力检查失败: {message}"[:1000]
