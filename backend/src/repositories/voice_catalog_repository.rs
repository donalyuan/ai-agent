use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceCatalogSync {
    pub id: Uuid,
    pub model_id: Uuid,
    pub trigger_source: String,
    pub status: String,
    pub page_limit: i32,
    pub page_count: i32,
    pub speaker_count: i32,
    pub error_summary: Option<String>,
    pub requested_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceCatalogEntry {
    pub id: Uuid,
    pub model_id: Uuid,
    pub voice_type: String,
    pub resource_id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    pub gender: Option<String>,
    pub age: Option<String>,
    pub categories: Value,
    pub normal_labels: Vec<String>,
    pub special_labels: Vec<String>,
    pub trial_url: Option<String>,
    pub short_trial_url: Option<String>,
    pub languages: Value,
    pub emotions: Value,
    pub description: String,
    pub is_available: bool,
    pub catalog_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceCatalogSnapshot {
    pub model_id: Uuid,
    pub source_model_id: Uuid,
    pub model_settings: Value,
    pub last_sync: Option<VoiceCatalogSync>,
    pub voices: Vec<VoiceCatalogEntry>,
}

#[derive(Clone)]
pub struct PostgresVoiceCatalogRepository {
    pool: PgPool,
}

impl PostgresVoiceCatalogRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn request_sync(
        &self,
        model_id: Uuid,
        trigger_source: &str,
    ) -> Result<(VoiceCatalogSync, bool), VoiceCatalogRepositoryError> {
        if !matches!(trigger_source, "admin" | "scheduled" | "workspace") {
            return Err(VoiceCatalogRepositoryError::InvalidRequest(
                "unknown sync trigger".to_string(),
            ));
        }
        let mut transaction = self.pool.begin().await?;
        let model = sqlx::query(
            r#"
            SELECT selected.model_type, selected.api_protocol, selected.upstream_model,
                   selected.settings, selected.status, selected.deleted_at,
                   source.id AS source_model_id,
                   source.model_type AS source_model_type,
                   source.api_protocol AS source_api_protocol,
                   source.upstream_model AS source_upstream_model,
                   source.settings AS source_settings,
                   source.status AS source_status,
                   source.deleted_at AS source_deleted_at,
                   source.voice_catalog_source_model_id AS nested_source_model_id,
                   source.catalog_access_key, source.catalog_secret_key
            FROM ai_models selected
            JOIN ai_models source
              ON source.id = COALESCE(selected.voice_catalog_source_model_id, selected.id)
            WHERE selected.id = $1
            FOR UPDATE OF selected, source
            "#,
        )
        .bind(model_id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(VoiceCatalogRepositoryError::ModelNotFound(model_id))?;
        let source_model_id = model.get::<Uuid, _>("source_model_id");
        let selected_settings = model.get::<Value, _>("settings");
        let source_settings = model.get::<Value, _>("source_settings");
        let valid_model = model.get::<String, _>("model_type") == "speech"
            && matches!(
                model.get::<String, _>("api_protocol").as_str(),
                "volcengine_tts_v3" | "openai_audio_speech"
            )
            && model.get::<String, _>("status") == "enabled"
            && model
                .get::<Option<DateTime<Utc>>, _>("deleted_at")
                .is_none()
            && model.get::<String, _>("source_model_type") == "speech"
            && model.get::<String, _>("source_api_protocol") == "volcengine_tts_v3"
            && model.get::<String, _>("source_status") == "enabled"
            && model
                .get::<Option<DateTime<Utc>>, _>("source_deleted_at")
                .is_none()
            && model
                .get::<Option<Uuid>, _>("nested_source_model_id")
                .is_none()
            && model.get::<String, _>("upstream_model").trim()
                == model.get::<String, _>("source_upstream_model").trim()
            && catalog_resource_id(&selected_settings) == catalog_resource_id(&source_settings)
            && model
                .get::<Option<String>, _>("catalog_access_key")
                .is_some_and(|value| !value.trim().is_empty())
            && model
                .get::<Option<String>, _>("catalog_secret_key")
                .is_some_and(|value| !value.trim().is_empty());
        if !valid_model {
            return Err(VoiceCatalogRepositoryError::ModelUnavailable(model_id));
        }

        if let Some(row) = sqlx::query(
            r#"
            SELECT id, model_id, trigger_source, status, page_limit, page_count,
                   speaker_count, error_summary, requested_at, started_at,
                   completed_at, created_at, updated_at
            FROM voice_catalog_syncs
            WHERE model_id = $1 AND status IN ('queued', 'running')
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(source_model_id)
        .fetch_optional(&mut *transaction)
        .await?
        {
            let sync = sync_from_row(row);
            transaction.commit().await?;
            return Ok((sync, false));
        }

        let row = sqlx::query(
            r#"
            INSERT INTO voice_catalog_syncs (model_id, trigger_source)
            VALUES ($1, $2)
            RETURNING id, model_id, trigger_source, status, page_limit, page_count,
                      speaker_count, error_summary, requested_at, started_at,
                      completed_at, created_at, updated_at
            "#,
        )
        .bind(source_model_id)
        .bind(trigger_source)
        .fetch_one(&mut *transaction)
        .await?;
        let sync = sync_from_row(row);
        transaction.commit().await?;
        Ok((sync, true))
    }

    pub async fn catalog(
        &self,
        model_id: Uuid,
        include_unavailable: bool,
    ) -> Result<VoiceCatalogSnapshot, VoiceCatalogRepositoryError> {
        let model = sqlx::query(
            r#"
            SELECT selected.settings, selected.upstream_model,
                   source.id AS source_model_id,
                   source.model_type AS source_model_type,
                   source.api_protocol AS source_api_protocol,
                   source.upstream_model AS source_upstream_model,
                   source.settings AS source_settings,
                   source.deleted_at AS source_deleted_at,
                   source.voice_catalog_source_model_id AS nested_source_model_id
            FROM ai_models selected
            JOIN ai_models source
              ON source.id = COALESCE(selected.voice_catalog_source_model_id, selected.id)
            WHERE selected.id = $1
              AND selected.model_type = 'speech'
              AND selected.api_protocol IN ('volcengine_tts_v3', 'openai_audio_speech')
              AND selected.deleted_at IS NULL
            "#,
        )
        .bind(model_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(VoiceCatalogRepositoryError::ModelNotFound(model_id))?;
        let model_settings = model.get::<Value, _>("settings");
        let source_model_id = model.get::<Uuid, _>("source_model_id");
        let source_settings = model.get::<Value, _>("source_settings");
        let valid_binding = model.get::<String, _>("source_model_type") == "speech"
            && model.get::<String, _>("source_api_protocol") == "volcengine_tts_v3"
            && model
                .get::<Option<DateTime<Utc>>, _>("source_deleted_at")
                .is_none()
            && model
                .get::<Option<Uuid>, _>("nested_source_model_id")
                .is_none()
            && model.get::<String, _>("upstream_model").trim()
                == model.get::<String, _>("source_upstream_model").trim()
            && catalog_resource_id(&model_settings) == catalog_resource_id(&source_settings);
        if !valid_binding {
            return Err(VoiceCatalogRepositoryError::ModelUnavailable(model_id));
        }
        let last_sync = sqlx::query(
            r#"
            SELECT id, model_id, trigger_source, status, page_limit, page_count,
                   speaker_count, error_summary, requested_at, started_at,
                   completed_at, created_at, updated_at
            FROM voice_catalog_syncs
            WHERE model_id = $1
            ORDER BY created_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(source_model_id)
        .fetch_optional(&self.pool)
        .await?
        .map(sync_from_row);
        let rows = sqlx::query(
            r#"
            SELECT id, model_id, voice_type, resource_id, name, avatar_url,
                   gender, age, categories, normal_labels, special_labels,
                   trial_url, short_trial_url, languages, emotions, description,
                   is_available, catalog_version, created_at, updated_at
            FROM voice_catalog_entries
            WHERE model_id = $1 AND ($2 OR is_available = TRUE)
            ORDER BY is_available DESC, name ASC, voice_type ASC
            "#,
        )
        .bind(source_model_id)
        .bind(include_unavailable)
        .fetch_all(&self.pool)
        .await?;
        Ok(VoiceCatalogSnapshot {
            model_id,
            source_model_id,
            model_settings,
            last_sync,
            voices: rows.into_iter().map(entry_from_row).collect(),
        })
    }
}

fn catalog_resource_id(settings: &Value) -> Option<&str> {
    settings
        .get("resource_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn sync_from_row(row: sqlx::postgres::PgRow) -> VoiceCatalogSync {
    VoiceCatalogSync {
        id: row.get("id"),
        model_id: row.get("model_id"),
        trigger_source: row.get("trigger_source"),
        status: row.get("status"),
        page_limit: row.get("page_limit"),
        page_count: row.get("page_count"),
        speaker_count: row.get("speaker_count"),
        error_summary: row.get("error_summary"),
        requested_at: row.get("requested_at"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn entry_from_row(row: sqlx::postgres::PgRow) -> VoiceCatalogEntry {
    VoiceCatalogEntry {
        id: row.get("id"),
        model_id: row.get("model_id"),
        voice_type: row.get("voice_type"),
        resource_id: row.get("resource_id"),
        name: row.get("name"),
        avatar_url: row.get("avatar_url"),
        gender: row.get("gender"),
        age: row.get("age"),
        categories: row.get("categories"),
        normal_labels: row.get("normal_labels"),
        special_labels: row.get("special_labels"),
        trial_url: row.get("trial_url"),
        short_trial_url: row.get("short_trial_url"),
        languages: row.get("languages"),
        emotions: row.get("emotions"),
        description: row.get("description"),
        is_available: row.get("is_available"),
        catalog_version: row.get("catalog_version"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

#[derive(Debug)]
pub enum VoiceCatalogRepositoryError {
    ModelNotFound(Uuid),
    ModelUnavailable(Uuid),
    InvalidRequest(String),
    Storage(String),
}

impl From<sqlx::Error> for VoiceCatalogRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for VoiceCatalogRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ModelNotFound(id) => write!(formatter, "speech model not found: {id}"),
            Self::ModelUnavailable(id) => write!(formatter, "speech model is unavailable: {id}"),
            Self::InvalidRequest(message) => formatter.write_str(message),
            Self::Storage(message) => write!(formatter, "voice catalog storage error: {message}"),
        }
    }
}

impl std::error::Error for VoiceCatalogRepositoryError {}
