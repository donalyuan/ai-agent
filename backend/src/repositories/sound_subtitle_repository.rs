use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Row};
use std::fmt;
use uuid::Uuid;

pub const MAX_IN_FLIGHT_SOUND_TASKS_PER_PROJECT: i64 = 2;

#[derive(Clone, Debug, PartialEq)]
pub struct AudioMaterialInspection {
    pub id: Uuid,
    pub project_id: Uuid,
    pub material_id: Uuid,
    pub status: String,
    pub idempotency_key: String,
    pub source_sha256: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub duration_ms: Option<i64>,
    pub container_format: Option<String>,
    pub audio_codec: Option<String>,
    pub sample_rate_hz: Option<i32>,
    pub channel_count: Option<i32>,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundSubtitleTask {
    pub id: Uuid,
    pub project_id: Uuid,
    pub parent_task_id: Option<Uuid>,
    pub task_type: String,
    pub status: String,
    pub model_id: Uuid,
    pub tos_staging_config_id: Option<Uuid>,
    pub tos_staging_config_version: Option<i64>,
    pub audio_inspection_id: Option<Uuid>,
    pub source_audio_material_id: Option<Uuid>,
    pub source_script_id: Option<Uuid>,
    pub source_script_snapshot: Option<Value>,
    pub output_audio_material_id: Option<Uuid>,
    pub output_subtitle_material_id: Option<Uuid>,
    pub text_content: String,
    pub voice_type: Option<String>,
    pub language: Option<String>,
    pub emotion: Option<String>,
    pub parameters: Value,
    pub model_snapshot: Option<Value>,
    pub voice_snapshot: Option<Value>,
    pub confirmation_snapshot: Value,
    pub resource_usage: Value,
    pub timeline: Option<Value>,
    pub result: Option<Value>,
    pub idempotency_key: String,
    pub request_id: Uuid,
    pub upstream_log_id: Option<String>,
    pub upstream_submitted_at: Option<DateTime<Utc>>,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub error_details: Value,
    pub staging_object_key: Option<String>,
    pub staging_source_sha256: Option<String>,
    pub staging_status: String,
    pub cleanup_attempt_count: i32,
    pub cleanup_error_summary: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct CreateSoundSubtitleTaskInput {
    pub project_id: Uuid,
    pub parent_task_id: Option<Uuid>,
    pub task_type: String,
    pub model_id: Uuid,
    pub tos_staging_config_id: Option<Uuid>,
    pub tos_staging_config_version: Option<i64>,
    pub audio_inspection_id: Option<Uuid>,
    pub source_audio_material_id: Option<Uuid>,
    pub source_script_id: Option<Uuid>,
    pub source_script_snapshot: Option<Value>,
    pub text_content: String,
    pub voice_type: Option<String>,
    pub language: Option<String>,
    pub emotion: Option<String>,
    pub parameters: Value,
    pub model_snapshot: Value,
    pub voice_snapshot: Option<Value>,
    pub confirmation_snapshot: Value,
    pub resource_usage: Value,
    pub idempotency_key: String,
}

#[derive(Clone)]
pub struct PostgresSoundSubtitleRepository {
    pool: PgPool,
}

impl PostgresSoundSubtitleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn request_audio_inspection(
        &self,
        project_id: Uuid,
        material_id: Uuid,
        idempotency_key: &str,
    ) -> Result<(AudioMaterialInspection, bool), SoundSubtitleRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('audio-inspection:' || $1::text))")
            .bind(material_id)
            .execute(&mut *transaction)
            .await?;

        if let Some(row) = sqlx::query(&format!(
            r#"
            SELECT {INSPECTION_COLUMNS}
            FROM audio_material_inspections
            WHERE project_id = $1 AND material_id = $2 AND idempotency_key = $3
            "#
        ))
        .bind(project_id)
        .bind(material_id)
        .bind(idempotency_key)
        .fetch_optional(&mut *transaction)
        .await?
        {
            let inspection = inspection_from_row(row);
            transaction.commit().await?;
            return Ok((inspection, false));
        }

        if let Some(row) = sqlx::query(&format!(
            r#"
            SELECT {INSPECTION_COLUMNS}
            FROM audio_material_inspections
            WHERE material_id = $1 AND status IN ('queued', 'running')
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            "#
        ))
        .bind(material_id)
        .fetch_optional(&mut *transaction)
        .await?
        {
            let inspection = inspection_from_row(row);
            transaction.commit().await?;
            return Ok((inspection, false));
        }

        let row = sqlx::query(&format!(
            r#"
            INSERT INTO audio_material_inspections (
                project_id, material_id, idempotency_key
            )
            VALUES ($1, $2, $3)
            RETURNING {INSPECTION_COLUMNS}
            "#
        ))
        .bind(project_id)
        .bind(material_id)
        .bind(idempotency_key)
        .fetch_one(&mut *transaction)
        .await?;
        let inspection = inspection_from_row(row);
        transaction.commit().await?;
        Ok((inspection, true))
    }

    pub async fn get_audio_inspection(
        &self,
        inspection_id: Uuid,
    ) -> Result<AudioMaterialInspection, SoundSubtitleRepositoryError> {
        let row = sqlx::query(&format!(
            "SELECT {INSPECTION_COLUMNS} FROM audio_material_inspections WHERE id = $1"
        ))
        .bind(inspection_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SoundSubtitleRepositoryError::InspectionNotFound(
            inspection_id,
        ))?;
        Ok(inspection_from_row(row))
    }

    pub async fn latest_audio_inspection(
        &self,
        material_id: Uuid,
    ) -> Result<Option<AudioMaterialInspection>, SoundSubtitleRepositoryError> {
        let row = sqlx::query(&format!(
            r#"
            SELECT {INSPECTION_COLUMNS}
            FROM audio_material_inspections
            WHERE material_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            "#
        ))
        .bind(material_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(inspection_from_row))
    }

    pub async fn create_or_reuse_task(
        &self,
        input: CreateSoundSubtitleTaskInput,
    ) -> Result<(SoundSubtitleTask, bool), SoundSubtitleRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('sound-task:' || $1::text))")
            .bind(input.project_id)
            .execute(&mut *transaction)
            .await?;

        if let Some(row) = sqlx::query(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM sound_subtitle_tasks
            WHERE project_id = $1 AND task_type = $2 AND idempotency_key = $3
            "#
        ))
        .bind(input.project_id)
        .bind(&input.task_type)
        .bind(&input.idempotency_key)
        .fetch_optional(&mut *transaction)
        .await?
        {
            let existing = task_from_row(row);
            if existing.confirmation_snapshot != input.confirmation_snapshot {
                return Err(SoundSubtitleRepositoryError::IdempotencyConflict);
            }
            transaction.commit().await?;
            return Ok((existing, false));
        }

        let in_flight: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM sound_subtitle_tasks
            WHERE project_id = $1 AND status IN ('queued', 'running')
            "#,
        )
        .bind(input.project_id)
        .fetch_one(&mut *transaction)
        .await?;
        if in_flight >= MAX_IN_FLIGHT_SOUND_TASKS_PER_PROJECT {
            return Err(SoundSubtitleRepositoryError::ConcurrencyLimit {
                limit: MAX_IN_FLIGHT_SOUND_TASKS_PER_PROJECT,
            });
        }

        let row = sqlx::query(&format!(
            r#"
            INSERT INTO sound_subtitle_tasks (
                project_id, parent_task_id, task_type, model_id,
                tos_staging_config_id, tos_staging_config_version, audio_inspection_id,
                source_audio_material_id, source_script_id, source_script_snapshot,
                text_content, voice_type, language, emotion, parameters, model_snapshot,
                voice_snapshot, confirmation_snapshot, resource_usage, idempotency_key
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20
            )
            RETURNING {TASK_COLUMNS}
            "#
        ))
        .bind(input.project_id)
        .bind(input.parent_task_id)
        .bind(input.task_type)
        .bind(input.model_id)
        .bind(input.tos_staging_config_id)
        .bind(input.tos_staging_config_version)
        .bind(input.audio_inspection_id)
        .bind(input.source_audio_material_id)
        .bind(input.source_script_id)
        .bind(input.source_script_snapshot)
        .bind(input.text_content)
        .bind(input.voice_type)
        .bind(input.language)
        .bind(input.emotion)
        .bind(input.parameters)
        .bind(input.model_snapshot)
        .bind(input.voice_snapshot)
        .bind(input.confirmation_snapshot)
        .bind(input.resource_usage)
        .bind(input.idempotency_key)
        .fetch_one(&mut *transaction)
        .await?;
        let task = task_from_row(row);
        transaction.commit().await?;
        Ok((task, true))
    }

    pub async fn get_task(
        &self,
        project_id: Uuid,
        task_id: Uuid,
    ) -> Result<SoundSubtitleTask, SoundSubtitleRepositoryError> {
        let row = sqlx::query(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM sound_subtitle_tasks
            WHERE id = $1 AND project_id = $2
            "#
        ))
        .bind(task_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(SoundSubtitleRepositoryError::TaskNotFound(task_id))?;
        Ok(task_from_row(row))
    }

    pub async fn list_tasks(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<SoundSubtitleTask>, SoundSubtitleRepositoryError> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM sound_subtitle_tasks
            WHERE project_id = $1
            ORDER BY created_at DESC, id DESC
            "#
        ))
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(task_from_row).collect())
    }

    pub async fn cancel_task(
        &self,
        project_id: Uuid,
        task_id: Uuid,
    ) -> Result<SoundSubtitleTask, SoundSubtitleRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let current = sqlx::query(&format!(
            r#"
            SELECT {TASK_COLUMNS}
            FROM sound_subtitle_tasks
            WHERE id = $1 AND project_id = $2
            FOR UPDATE
            "#
        ))
        .bind(task_id)
        .bind(project_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(SoundSubtitleRepositoryError::TaskNotFound(task_id))?;
        let current = task_from_row(current);
        if current.status == "cancelled" {
            transaction.commit().await?;
            return Ok(current);
        }
        if current.status != "queued" {
            return Err(SoundSubtitleRepositoryError::TaskNotCancellable(task_id));
        }
        let row = sqlx::query(&format!(
            r#"
            UPDATE sound_subtitle_tasks
            SET status = 'cancelled', completed_at = NOW(),
                staging_status = CASE
                    WHEN staging_status = 'uploaded' THEN 'cleanup_pending'
                    ELSE staging_status
                END,
                locked_at = NULL, worker_id = NULL, updated_at = NOW()
            WHERE id = $1
            RETURNING {TASK_COLUMNS}
            "#
        ))
        .bind(task_id)
        .fetch_one(&mut *transaction)
        .await?;
        let task = task_from_row(row);
        transaction.commit().await?;
        Ok(task)
    }
}

const INSPECTION_COLUMNS: &str = r#"
    id, project_id, material_id, status, idempotency_key, source_sha256,
    file_size_bytes, duration_ms, container_format, audio_codec, sample_rate_hz,
    channel_count, error_code, error_summary, started_at, completed_at,
    created_at, updated_at
"#;

const TASK_COLUMNS: &str = r#"
    id, project_id, parent_task_id, task_type, status, model_id,
    tos_staging_config_id, tos_staging_config_version,
    audio_inspection_id, source_audio_material_id, source_script_id,
    source_script_snapshot, output_audio_material_id, output_subtitle_material_id,
    text_content, voice_type, language, emotion,
    parameters, model_snapshot, voice_snapshot, confirmation_snapshot,
    resource_usage, timeline, result, idempotency_key, request_id,
    upstream_log_id, upstream_submitted_at, attempt_count, max_attempts,
    error_code, error_summary, error_details, staging_object_key, staging_source_sha256,
    staging_status, cleanup_attempt_count, cleanup_error_summary, started_at,
    completed_at, created_at, updated_at
"#;

fn inspection_from_row(row: PgRow) -> AudioMaterialInspection {
    AudioMaterialInspection {
        id: row.get("id"),
        project_id: row.get("project_id"),
        material_id: row.get("material_id"),
        status: row.get("status"),
        idempotency_key: row.get("idempotency_key"),
        source_sha256: row.get("source_sha256"),
        file_size_bytes: row.get("file_size_bytes"),
        duration_ms: row.get("duration_ms"),
        container_format: row.get("container_format"),
        audio_codec: row.get("audio_codec"),
        sample_rate_hz: row.get("sample_rate_hz"),
        channel_count: row.get("channel_count"),
        error_code: row.get("error_code"),
        error_summary: row.get("error_summary"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn task_from_row(row: PgRow) -> SoundSubtitleTask {
    SoundSubtitleTask {
        id: row.get("id"),
        project_id: row.get("project_id"),
        parent_task_id: row.get("parent_task_id"),
        task_type: row.get("task_type"),
        status: row.get("status"),
        model_id: row.get("model_id"),
        tos_staging_config_id: row.get("tos_staging_config_id"),
        tos_staging_config_version: row.get("tos_staging_config_version"),
        audio_inspection_id: row.get("audio_inspection_id"),
        source_audio_material_id: row.get("source_audio_material_id"),
        source_script_id: row.get("source_script_id"),
        source_script_snapshot: row.get("source_script_snapshot"),
        output_audio_material_id: row.get("output_audio_material_id"),
        output_subtitle_material_id: row.get("output_subtitle_material_id"),
        text_content: row.get("text_content"),
        voice_type: row.get("voice_type"),
        language: row.get("language"),
        emotion: row.get("emotion"),
        parameters: row.get("parameters"),
        model_snapshot: row.get("model_snapshot"),
        voice_snapshot: row.get("voice_snapshot"),
        confirmation_snapshot: row.get("confirmation_snapshot"),
        resource_usage: row.get("resource_usage"),
        timeline: row.get("timeline"),
        result: row.get("result"),
        idempotency_key: row.get("idempotency_key"),
        request_id: row.get("request_id"),
        upstream_log_id: row.get("upstream_log_id"),
        upstream_submitted_at: row.get("upstream_submitted_at"),
        attempt_count: row.get("attempt_count"),
        max_attempts: row.get("max_attempts"),
        error_code: row.get("error_code"),
        error_summary: row.get("error_summary"),
        error_details: row.get("error_details"),
        staging_object_key: row.get("staging_object_key"),
        staging_source_sha256: row.get("staging_source_sha256"),
        staging_status: row.get("staging_status"),
        cleanup_attempt_count: row.get("cleanup_attempt_count"),
        cleanup_error_summary: row.get("cleanup_error_summary"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[derive(Debug)]
pub enum SoundSubtitleRepositoryError {
    InspectionNotFound(Uuid),
    TaskNotFound(Uuid),
    TaskNotCancellable(Uuid),
    IdempotencyConflict,
    ConcurrencyLimit { limit: i64 },
    Storage(String),
}

impl From<sqlx::Error> for SoundSubtitleRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for SoundSubtitleRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InspectionNotFound(id) => write!(formatter, "audio inspection not found: {id}"),
            Self::TaskNotFound(id) => write!(formatter, "sound task not found: {id}"),
            Self::TaskNotCancellable(id) => {
                write!(formatter, "sound task is not cancellable: {id}")
            }
            Self::IdempotencyConflict => formatter.write_str("idempotency key payload conflict"),
            Self::ConcurrencyLimit { limit } => {
                write!(formatter, "sound task concurrency limit reached: {limit}")
            }
            Self::Storage(message) => write!(formatter, "sound task storage error: {message}"),
        }
    }
}

impl std::error::Error for SoundSubtitleRepositoryError {}
