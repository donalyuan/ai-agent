use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::fmt;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct TosStagingToolConfig {
    pub id: Uuid,
    pub version: i64,
    pub is_enabled: bool,
    pub storage_provider: String,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub object_prefix: String,
    pub access_key: String,
    pub secret_key: String,
    pub signed_url_ttl_seconds: i32,
    pub max_file_bytes: i64,
    pub max_audio_duration_seconds: i32,
    pub last_check_status: String,
    pub last_check_requested_at: Option<DateTime<Utc>>,
    pub last_checked_at: Option<DateTime<Utc>>,
    pub last_check_error_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SaveTosStagingToolConfigInput {
    pub version: Option<i64>,
    pub enabled: bool,
    pub storage_provider: String,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub object_prefix: String,
    pub access_key: String,
    pub secret_key: String,
    pub signed_url_ttl_seconds: i32,
    pub max_file_bytes: i64,
    pub max_audio_duration_seconds: i32,
}

#[derive(Clone)]
pub struct PostgresTosStagingToolRepository {
    pool: PgPool,
}

impl PostgresTosStagingToolRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_current(
        &self,
    ) -> Result<Option<TosStagingToolConfig>, TosStagingToolRepositoryError> {
        let row = sqlx::query(&format!(
            "SELECT {TOS_CONFIG_COLUMNS} FROM tos_staging_tool_configs WHERE is_current = TRUE"
        ))
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(config_from_row))
    }

    pub async fn get_enabled_current(
        &self,
    ) -> Result<TosStagingToolConfig, TosStagingToolRepositoryError> {
        let config = self
            .get_current()
            .await?
            .ok_or(TosStagingToolRepositoryError::NotConfigured)?;
        if !config.is_enabled {
            return Err(TosStagingToolRepositoryError::Disabled);
        }
        if config.last_check_status != "succeeded" {
            return Err(TosStagingToolRepositoryError::CheckRequired);
        }
        Ok(config)
    }

    pub async fn pending_cleanup_count(&self) -> Result<i64, TosStagingToolRepositoryError> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM sound_subtitle_tasks WHERE staging_status = 'cleanup_pending'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn queue_connection_check(
        &self,
        expected_version: i64,
    ) -> Result<TosStagingToolConfig, TosStagingToolRepositoryError> {
        let row = sqlx::query(&format!(
            r#"
            UPDATE tos_staging_tool_configs
            SET last_check_status = 'queued', last_check_requested_at = NOW(),
                last_checked_at = NULL, last_check_error_summary = NULL,
                check_locked_at = NULL, check_worker_id = NULL, updated_at = NOW()
            WHERE is_current = TRUE AND version = $1
            RETURNING {TOS_CONFIG_COLUMNS}
            "#
        ))
        .bind(expected_version)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(row) => Ok(config_from_row(row)),
            None if self.get_current().await?.is_none() => {
                Err(TosStagingToolRepositoryError::NotConfigured)
            }
            None => Err(TosStagingToolRepositoryError::VersionConflict),
        }
    }

    pub async fn save(
        &self,
        input: SaveTosStagingToolConfigInput,
    ) -> Result<TosStagingToolConfig, TosStagingToolRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext('system-tos-staging-config'))")
            .execute(&mut *transaction)
            .await?;
        let current = sqlx::query(&format!(
            "SELECT {TOS_CONFIG_COLUMNS} FROM tos_staging_tool_configs WHERE is_current = TRUE FOR UPDATE"
        ))
        .fetch_optional(&mut *transaction)
        .await?
        .map(config_from_row);

        match (&current, input.version) {
            (None, None) => {}
            (Some(config), Some(version)) if config.version == version => {}
            (None, Some(_)) | (Some(_), None) | (Some(_), Some(_)) => {
                return Err(TosStagingToolRepositoryError::VersionConflict)
            }
        }

        let pending_cleanup_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sound_subtitle_tasks WHERE staging_status = 'cleanup_pending'",
        )
        .fetch_one(&mut *transaction)
        .await?;
        if pending_cleanup_count > 0 {
            return Err(TosStagingToolRepositoryError::CleanupPending {
                count: pending_cleanup_count,
            });
        }

        let normalized = normalize_config(input, current.as_ref())?;
        let connection_unchanged = current
            .as_ref()
            .is_some_and(|config| same_connection_config(config, &normalized));
        if normalized.enabled
            && (!connection_unchanged
                || current
                    .as_ref()
                    .is_none_or(|config| config.last_check_status != "succeeded"))
        {
            return Err(TosStagingToolRepositoryError::CheckRequired);
        }
        let retained_check = current.as_ref().filter(|config| {
            connection_unchanged
                && matches!(config.last_check_status.as_str(), "succeeded" | "failed")
        });
        let next_version = current.as_ref().map_or(1, |config| config.version + 1);
        if current.is_some() {
            sqlx::query(
                "UPDATE tos_staging_tool_configs SET is_current = FALSE WHERE is_current = TRUE",
            )
            .execute(&mut *transaction)
            .await?;
        }
        let row = sqlx::query(&format!(
            r#"
            INSERT INTO tos_staging_tool_configs (
                version, is_current, is_enabled, storage_provider, endpoint, region,
                bucket, object_prefix, access_key, secret_key, signed_url_ttl_seconds,
                max_file_bytes, max_audio_duration_seconds, last_check_status,
                last_check_requested_at, last_checked_at, last_check_error_summary
            )
            VALUES (
                $1, TRUE, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12,
                $13, $14, $15, $16
            )
            RETURNING {TOS_CONFIG_COLUMNS}
            "#
        ))
        .bind(next_version)
        .bind(normalized.enabled)
        .bind(normalized.storage_provider)
        .bind(normalized.endpoint)
        .bind(normalized.region)
        .bind(normalized.bucket)
        .bind(normalized.object_prefix)
        .bind(normalized.access_key)
        .bind(normalized.secret_key)
        .bind(normalized.signed_url_ttl_seconds)
        .bind(normalized.max_file_bytes)
        .bind(normalized.max_audio_duration_seconds)
        .bind(
            retained_check
                .map(|config| config.last_check_status.as_str())
                .unwrap_or("never"),
        )
        .bind(retained_check.and_then(|config| config.last_check_requested_at))
        .bind(retained_check.and_then(|config| config.last_checked_at))
        .bind(retained_check.and_then(|config| config.last_check_error_summary.as_deref()))
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(config_from_row(row))
    }
}

const TOS_CONFIG_COLUMNS: &str = r#"
    id, version, is_enabled, storage_provider, endpoint, region, bucket,
    object_prefix, access_key, secret_key, signed_url_ttl_seconds,
    max_file_bytes, max_audio_duration_seconds, last_check_status,
    last_check_requested_at, last_checked_at, last_check_error_summary,
    created_at, updated_at
"#;

fn config_from_row(row: sqlx::postgres::PgRow) -> TosStagingToolConfig {
    TosStagingToolConfig {
        id: row.get("id"),
        version: row.get("version"),
        is_enabled: row.get("is_enabled"),
        storage_provider: row.get("storage_provider"),
        endpoint: row.get("endpoint"),
        region: row.get("region"),
        bucket: row.get("bucket"),
        object_prefix: row.get("object_prefix"),
        access_key: row.get("access_key"),
        secret_key: row.get("secret_key"),
        signed_url_ttl_seconds: row.get("signed_url_ttl_seconds"),
        max_file_bytes: row.get("max_file_bytes"),
        max_audio_duration_seconds: row.get("max_audio_duration_seconds"),
        last_check_status: row.get("last_check_status"),
        last_check_requested_at: row.get("last_check_requested_at"),
        last_checked_at: row.get("last_checked_at"),
        last_check_error_summary: row.get("last_check_error_summary"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn normalize_config(
    input: SaveTosStagingToolConfigInput,
    current: Option<&TosStagingToolConfig>,
) -> Result<SaveTosStagingToolConfigInput, TosStagingToolRepositoryError> {
    let endpoint = normalize_endpoint(&input.endpoint)?;
    let access_key = input.access_key.trim();
    let secret_key = input.secret_key.trim();
    let (access_key, secret_key) = match (access_key.is_empty(), secret_key.is_empty()) {
        (true, true) => current
            .map(|config| (config.access_key.clone(), config.secret_key.clone()))
            .ok_or_else(|| invalid("首次配置必须填写 Access Key 和 Secret Key"))?,
        (false, false) => (access_key.to_string(), secret_key.to_string()),
        _ => return Err(invalid("Access Key 和 Secret Key 必须同时填写或同时留空")),
    };
    let storage_provider = input.storage_provider.trim().to_string();
    let region = input.region.trim().to_string();
    let bucket = input.bucket.trim().to_string();
    let object_prefix = input.object_prefix.trim().trim_matches('/').to_string();
    if storage_provider != "volcengine_tos"
        || region.is_empty()
        || bucket.is_empty()
        || object_prefix.is_empty()
    {
        return Err(invalid("TOS 存储位置配置不完整"));
    }
    if !(60..=3600).contains(&input.signed_url_ttl_seconds)
        || input.max_file_bytes <= 0
        || input.max_audio_duration_seconds <= 0
    {
        return Err(invalid("TOS 暂存限制无效"));
    }
    Ok(SaveTosStagingToolConfigInput {
        version: input.version,
        enabled: input.enabled,
        storage_provider,
        endpoint,
        region,
        bucket,
        object_prefix,
        access_key,
        secret_key,
        signed_url_ttl_seconds: input.signed_url_ttl_seconds,
        max_file_bytes: input.max_file_bytes,
        max_audio_duration_seconds: input.max_audio_duration_seconds,
    })
}

fn same_connection_config(
    current: &TosStagingToolConfig,
    next: &SaveTosStagingToolConfigInput,
) -> bool {
    current.storage_provider == next.storage_provider
        && current.endpoint == next.endpoint
        && current.region == next.region
        && current.bucket == next.bucket
        && current.object_prefix == next.object_prefix
        && current.access_key == next.access_key
        && current.secret_key == next.secret_key
        && current.signed_url_ttl_seconds == next.signed_url_ttl_seconds
        && current.max_file_bytes == next.max_file_bytes
        && current.max_audio_duration_seconds == next.max_audio_duration_seconds
}

fn normalize_endpoint(value: &str) -> Result<String, TosStagingToolRepositoryError> {
    let mut url = Url::parse(value.trim()).map_err(|_| invalid("TOS Endpoint 无效"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid("TOS Endpoint 必须为不含查询参数的 HTTPS 地址"));
    }
    let path = url.path().trim_end_matches('/').to_string();
    url.set_path(&path);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

fn invalid(message: &str) -> TosStagingToolRepositoryError {
    TosStagingToolRepositoryError::InvalidConfig(message.to_string())
}

#[derive(Debug, PartialEq)]
pub enum TosStagingToolRepositoryError {
    NotConfigured,
    Disabled,
    CheckRequired,
    VersionConflict,
    CleanupPending { count: i64 },
    InvalidConfig(String),
    Storage(String),
}

impl From<sqlx::Error> for TosStagingToolRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for TosStagingToolRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotConfigured => formatter.write_str("system TOS staging tool is not configured"),
            Self::Disabled => formatter.write_str("system TOS staging tool is disabled"),
            Self::CheckRequired => {
                formatter.write_str("system TOS staging tool connection check is required")
            }
            Self::VersionConflict => {
                formatter.write_str("system TOS staging tool version conflict")
            }
            Self::CleanupPending { count } => {
                write!(formatter, "TOS staging objects pending cleanup: {count}")
            }
            Self::InvalidConfig(message) => {
                write!(formatter, "invalid TOS staging config: {message}")
            }
            Self::Storage(message) => write!(formatter, "TOS staging storage error: {message}"),
        }
    }
}

impl std::error::Error for TosStagingToolRepositoryError {}
