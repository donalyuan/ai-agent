from __future__ import annotations

import os
from pathlib import Path
from uuid import uuid4

import psycopg
import pytest
from psycopg.types.json import Jsonb

from video_worker.speech_generation import (
    AudioInspectionResult,
    PostgresSpeechStore,
    SavedSpeechArtifact,
)
from video_worker.tos_tool_check import PostgresTosCheckStore
from video_worker.voice_catalog import PostgresVoiceCatalogStore


MIGRATIONS = Path(os.getenv("BACKEND_MIGRATIONS_PATH", "/app/backend-migrations"))


def database_url() -> str:
    return os.getenv(
        "DATABASE_URL",
        "postgres://postgres:postgres@biga-postgres:5432/video_agent",
    )


def with_database_name(url: str, name: str) -> str:
    base, separator, query = url.partition("?")
    prefix = base.rsplit("/", 1)[0]
    return f"{prefix}/{name}{separator}{query}" if separator else f"{prefix}/{name}"


@pytest.fixture
def migrated_database_url():
    if not MIGRATIONS.is_dir():
        pytest.skip("backend migrations are not mounted")
    name = f"speech_worker_store_{uuid4().hex}"
    admin_url = with_database_name(database_url(), "postgres")
    test_url = with_database_name(database_url(), name)
    with psycopg.connect(admin_url, autocommit=True) as admin:
        admin.execute(f'CREATE DATABASE "{name}"')
    try:
        with psycopg.connect(test_url) as connection:
            for migration in sorted(MIGRATIONS.glob("*.sql")):
                connection.execute(migration.read_text(encoding="utf-8"))
                connection.commit()
        yield test_url
    finally:
        with psycopg.connect(admin_url, autocommit=True) as admin:
            admin.execute(
                "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = %s",
                (name,),
            )
            admin.execute(f'DROP DATABASE IF EXISTS "{name}"')


def seed_project_and_models(connection):
    project_id = connection.execute(
        "INSERT INTO projects (name) VALUES ('Worker contract') RETURNING id"
    ).fetchone()[0]
    tts_model_id = connection.execute(
        """
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, protocol_version,
            auth_scheme, request_base_url, upstream_model, api_key,
            catalog_access_key, catalog_secret_key, timeout_seconds, settings, status
        )
        VALUES (
            'TTS', 'speech', '火山引擎', 'volcengine_tts_v3', 'v3', 'api_key',
            'https://openspeech.bytedance.com/api/v3', 'doubao-seed-tts-2.0',
            'tts-key', 'catalog-ak', 'catalog-sk', 120, %s, 'enabled'
        )
        RETURNING id
        """,
        (
            Jsonb(
                {
                    "resource_id": "seed-tts-2.0",
                    "supported_audio_formats": ["wav"],
                    "default_audio_format": "wav",
                    "supported_sample_rates": [24000],
                    "default_sample_rate": 24000,
                    "max_input_characters": 3000,
                    "max_audio_duration_seconds": None,
                    "supports_word_timestamps": True,
                    "word_timestamp_languages": ["zh-cn"],
                    "catalog_sync_interval_minutes": 1440,
                    "parameters": {},
                }
            ),
        ),
    ).fetchone()[0]
    asr_model_id = connection.execute(
        """
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, protocol_version,
            auth_scheme, request_base_url, upstream_model, api_key, timeout_seconds,
            settings, status
        )
        VALUES (
            'ASR', 'speech', '火山引擎', 'volcengine_asr_v3', 'v3', 'api_key',
            'https://openspeech.bytedance.com/api/v3', 'doubao-seed-asr-2.0',
            'asr-key', 120, %s, 'enabled'
        )
        RETURNING id
        """,
        (
            Jsonb(
                {
                    "resource_id": "volc.seedasr.auc",
                    "supported_audio_formats": ["mp3", "wav"],
                    "default_audio_format": "mp3",
                    "supported_sample_rates": [24000],
                    "default_sample_rate": 24000,
                    "max_input_characters": None,
                    "max_audio_duration_seconds": 3600,
                    "supports_word_timestamps": True,
                    "word_timestamp_languages": ["*"],
                    "catalog_sync_interval_minutes": None,
                    "parameters": {},
                }
            ),
        ),
    ).fetchone()[0]
    tos_config_id, tos_config_version = connection.execute(
        """
        INSERT INTO tos_staging_tool_configs (
            version, is_current, is_enabled, storage_provider, endpoint, region,
            bucket, object_prefix, access_key, secret_key, signed_url_ttl_seconds,
            max_file_bytes, max_audio_duration_seconds
        ) VALUES (
            1, TRUE, TRUE, 'volcengine_tos',
            'https://tos-cn-beijing.volces.com', 'cn-beijing', 'private-bucket',
            'novex/asr', 'tos-ak', 'tos-sk', 600, 10485760, 3600
        ) RETURNING id, version
        """
    ).fetchone()
    return project_id, tts_model_id, asr_model_id, tos_config_id, tos_config_version


def test_voice_catalog_store_waits_after_failure_and_accepts_null_collections(
    migrated_database_url: str,
) -> None:
    with psycopg.connect(migrated_database_url) as connection:
        _, tts_model_id, _, _, _ = seed_project_and_models(connection)
        shared_model_id = connection.execute(
            """
            INSERT INTO ai_models (
                display_name, model_type, provider_name, api_protocol, protocol_version,
                auth_scheme, request_base_url, upstream_model, api_key,
                voice_catalog_source_model_id, timeout_seconds, settings, status
            )
            SELECT
                'TTS Gateway', 'speech', '中转服务', 'volcengine_tts_v3', 'v3',
                'api_key', 'https://speech-gateway.example.com/api/v3', upstream_model,
                'gateway-key', id, timeout_seconds, settings, 'enabled'
            FROM ai_models
            WHERE id = %s
            RETURNING id
            """,
            (tts_model_id,),
        ).fetchone()[0]
        connection.execute(
            """
            INSERT INTO voice_catalog_syncs (
                model_id, trigger_source, status, error_summary, completed_at
            ) VALUES (%s, 'workspace', 'failed', 'fixture failure', NOW())
            """,
            (tts_model_id,),
        )
        connection.commit()

    store = PostgresVoiceCatalogStore(migrated_database_url)
    assert store.enqueue_due_syncs() == 0

    with psycopg.connect(migrated_database_url) as connection:
        connection.execute(
            """
            UPDATE voice_catalog_syncs
            SET requested_at = NOW() - INTERVAL '2 days',
                completed_at = NOW() - INTERVAL '2 days'
            WHERE model_id = %s
            """,
            (tts_model_id,),
        )
        connection.commit()

    assert store.enqueue_due_syncs() == 1
    with psycopg.connect(migrated_database_url) as connection:
        shared_sync_count = connection.execute(
            "SELECT COUNT(*) FROM voice_catalog_syncs WHERE model_id = %s",
            (shared_model_id,),
        ).fetchone()[0]
    assert shared_sync_count == 0
    job = store.claim_next_sync()
    assert job is not None
    store.replace_catalog(
        job.sync_id,
        job.model_id,
        [
            {
                "VoiceType": "nullable_voice",
                "Name": "Nullable voice",
                "ResourceID": "seed-tts-2.0",
                "Categories": None,
                "NormalLabels": None,
                "SpecialLabels": None,
                "Languages": None,
                "Emotions": None,
            }
        ],
    )

    with psycopg.connect(migrated_database_url) as connection:
        row = connection.execute(
            """
            SELECT categories, normal_labels, special_labels, languages, emotions
            FROM voice_catalog_entries
            WHERE model_id = %s AND voice_type = 'nullable_voice'
            """,
            (tts_model_id,),
        ).fetchone()
    assert row == ([], [], [], [], [])


def test_postgres_store_claims_and_completes_tts_asr_and_cleanup(
    migrated_database_url: str, tmp_path: Path
) -> None:
    with psycopg.connect(migrated_database_url) as connection:
        project_id, tts_model_id, asr_model_id, tos_config_id, tos_config_version = (
            seed_project_and_models(connection)
        )
        connection.execute(
            """
            UPDATE tos_staging_tool_configs
            SET last_check_status = 'queued', last_check_requested_at = NOW()
            WHERE id = %s
            """,
            (tos_config_id,),
        )
        source_material_id = connection.execute(
            """
            INSERT INTO materials (project_id, material_type, file_url, file_name, status)
            VALUES (%s, 'audio', '/assets/uploads/source.mp3', 'source.mp3', 'active')
            RETURNING id
            """,
            (project_id,),
        ).fetchone()[0]
        inspection_id = connection.execute(
            """
            INSERT INTO audio_material_inspections (
                project_id, material_id, idempotency_key
            ) VALUES (%s, %s, 'inspect') RETURNING id
            """,
            (project_id, source_material_id),
        ).fetchone()[0]
        connection.commit()

    check_store = PostgresTosCheckStore(
        migrated_database_url, worker_id="contract-check-worker"
    )
    pending_check = check_store.claim_next_check()
    assert pending_check is not None
    assert pending_check.config_id == str(tos_config_id)
    assert pending_check.version == tos_config_version
    check_store.complete_check(
        pending_check.config_id,
        pending_check.version,
        succeeded=True,
        error_summary=None,
    )

    store = PostgresSpeechStore(migrated_database_url, worker_id="contract-worker")
    inspection = store.claim_next_audio_inspection()
    assert inspection is not None and inspection.inspection_id == str(inspection_id)
    probe = AudioInspectionResult(
        source_sha256="a" * 64,
        file_size_bytes=100,
        duration_ms=1000,
        container_format="mp3",
        audio_codec="mp3",
        sample_rate_hz=24000,
        channel_count=1,
    )
    store.complete_audio_inspection(str(inspection_id), probe)

    with psycopg.connect(migrated_database_url) as connection:
        tts_task_id = connection.execute(
            """
            INSERT INTO sound_subtitle_tasks (
                project_id, task_type, model_id, text_content, voice_type, language,
                emotion, parameters, model_snapshot, voice_snapshot,
                confirmation_snapshot, resource_usage, idempotency_key
            )
            VALUES (%s, 'tts', %s, '你好', 'voice', 'zh-cn', 'neutral', %s, %s,
                    %s, %s, %s, 'tts')
            RETURNING id
            """,
            (
                project_id,
                tts_model_id,
                Jsonb({"audio_format": "wav", "sample_rate": 24000}),
                Jsonb({"api_protocol": "volcengine_tts_v3", "registry_version": 1}),
                Jsonb({"name": "voice"}),
                Jsonb({"generate_subtitle": True, "subtitle_segments": ["你好"]}),
                Jsonb({"character_count": 2, "task_count": 2}),
            ),
        ).fetchone()[0]
        connection.commit()
    tts_task = store.claim_next_speech_task()
    assert tts_task is not None and tts_task.task_id == str(tts_task_id)
    audio_path = tmp_path / "tts.wav"
    audio_path.write_bytes(b"RIFFfixtureWAVE")
    subtitle_path = tmp_path / "tts.srt"
    subtitle_path.write_text("1\n00:00:00,000 --> 00:00:01,000\n你好\n")
    store.complete_tts_task(
        task=tts_task,
        audio_artifact=SavedSpeechArtifact("/assets/tts.wav", "tts.wav", audio_path, 15),
        subtitle_artifact=SavedSpeechArtifact(
            "/assets/tts.srt", "tts.srt", subtitle_path, subtitle_path.stat().st_size
        ),
        audio_inspection=probe,
        timeline=[{"index": 1, "start_ms": 0, "end_ms": 1000, "text": "你好"}],
        words=[{"text": "你好", "start_ms": 0, "end_ms": 1000}],
        upstream_log_id="tts-log",
        attempt_count=1,
    )

    with psycopg.connect(migrated_database_url) as connection:
        asr_task_id = connection.execute(
            """
            INSERT INTO sound_subtitle_tasks (
                project_id, task_type, model_id, audio_inspection_id,
                source_audio_material_id, tos_staging_config_id,
                tos_staging_config_version, parameters, model_snapshot,
                confirmation_snapshot, resource_usage, idempotency_key
            )
            VALUES (%s, 'asr', %s, %s, %s, %s, %s, %s, %s, %s, %s, 'asr')
            RETURNING id
            """,
            (
                project_id,
                asr_model_id,
                inspection_id,
                source_material_id,
                tos_config_id,
                tos_config_version,
                Jsonb({"audio_format": "mp3"}),
                Jsonb({"api_protocol": "volcengine_asr_v3", "registry_version": 1}),
                Jsonb({"generate_subtitle": True}),
                Jsonb({"audio_duration_ms": 1000, "task_count": 1}),
            ),
        ).fetchone()[0]
        connection.commit()
    asr_task = store.claim_next_speech_task()
    assert asr_task is not None and asr_task.task_id == str(asr_task_id)
    assert asr_task.tos_staging_config_id == str(tos_config_id)
    assert asr_task.tos_staging_config_version == tos_config_version
    object_key = f"novex/asr/{project_id}/{asr_task_id}/{'a' * 64}.mp3"
    store.record_asr_staging(str(asr_task_id), object_key, "a" * 64)
    store.record_asr_submitted(str(asr_task_id), 1)
    store.defer_asr_task(str(asr_task_id))
    resumed = store.claim_next_speech_task()
    assert resumed is not None and resumed.upstream_submitted is True
    asr_subtitle_path = tmp_path / "asr.srt"
    asr_subtitle_path.write_text("1\n00:00:00,000 --> 00:00:01,000\nHello\n")
    store.complete_asr_task(
        task=resumed,
        subtitle_artifact=SavedSpeechArtifact(
            "/assets/asr.srt",
            "asr.srt",
            asr_subtitle_path,
            asr_subtitle_path.stat().st_size,
        ),
        timeline=[{"index": 1, "start_ms": 0, "end_ms": 1000, "text": "Hello"}],
        words=[{"text": "Hello", "start_ms": 0, "end_ms": 1000}],
        transcript="Hello",
        upstream_log_id="asr-log",
        attempt_count=1,
    )
    store.record_cleanup(str(asr_task_id), True, None)

    with psycopg.connect(migrated_database_url) as connection:
        task_rows = connection.execute(
            """
            SELECT task_type, status, staging_status,
                   output_audio_material_id IS NOT NULL,
                   output_subtitle_material_id IS NOT NULL
            FROM sound_subtitle_tasks
            ORDER BY created_at, id
            """
        ).fetchall()
        assert task_rows == [
            ("tts", "succeeded", "none", True, True),
            ("asr", "succeeded", "cleaned", False, True),
        ]
        assert connection.execute("SELECT COUNT(*) FROM materials").fetchone()[0] == 4
