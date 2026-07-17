use crate::repositories::{SaveTosStagingToolConfigInput, TosStagingToolConfig};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub(super) struct SaveTosStagingToolRequest {
    pub(super) version: Option<i64>,
    pub(super) enabled: bool,
    pub(super) storage_provider: String,
    pub(super) endpoint: String,
    pub(super) region: String,
    pub(super) bucket: String,
    pub(super) object_prefix: String,
    #[serde(default)]
    pub(super) access_key: String,
    #[serde(default)]
    pub(super) secret_key: String,
    pub(super) signed_url_ttl_seconds: i32,
    pub(super) max_file_bytes: i64,
    pub(super) max_audio_duration_seconds: i32,
}

#[derive(Debug, Deserialize)]
pub(super) struct CheckTosStagingToolRequest {
    pub(super) version: i64,
}

impl From<SaveTosStagingToolRequest> for SaveTosStagingToolConfigInput {
    fn from(request: SaveTosStagingToolRequest) -> Self {
        Self {
            version: request.version,
            enabled: request.enabled,
            storage_provider: request.storage_provider,
            endpoint: request.endpoint,
            region: request.region,
            bucket: request.bucket,
            object_prefix: request.object_prefix,
            access_key: request.access_key,
            secret_key: request.secret_key,
            signed_url_ttl_seconds: request.signed_url_ttl_seconds,
            max_file_bytes: request.max_file_bytes,
            max_audio_duration_seconds: request.max_audio_duration_seconds,
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct TosStagingToolAdminResponse {
    configured: bool,
    config_id: Option<Uuid>,
    version: Option<i64>,
    enabled: bool,
    storage_provider: Option<String>,
    endpoint: Option<String>,
    region: Option<String>,
    bucket: Option<String>,
    object_prefix: Option<String>,
    access_key_masked: Option<String>,
    secret_key_masked: Option<String>,
    access_key_configured: bool,
    secret_key_configured: bool,
    signed_url_ttl_seconds: Option<i32>,
    max_file_bytes: Option<i64>,
    max_audio_duration_seconds: Option<i32>,
    pending_cleanup_count: i64,
    last_check_status: String,
    last_check_requested_at: Option<DateTime<Utc>>,
    last_checked_at: Option<DateTime<Utc>>,
    last_check_error_summary: Option<String>,
    created_at: Option<DateTime<Utc>>,
    updated_at: Option<DateTime<Utc>>,
}

impl TosStagingToolAdminResponse {
    pub(super) fn unconfigured(pending_cleanup_count: i64) -> Self {
        Self {
            configured: false,
            config_id: None,
            version: None,
            enabled: false,
            storage_provider: None,
            endpoint: None,
            region: None,
            bucket: None,
            object_prefix: None,
            access_key_masked: None,
            secret_key_masked: None,
            access_key_configured: false,
            secret_key_configured: false,
            signed_url_ttl_seconds: None,
            max_file_bytes: None,
            max_audio_duration_seconds: None,
            pending_cleanup_count,
            last_check_status: "never".to_string(),
            last_check_requested_at: None,
            last_checked_at: None,
            last_check_error_summary: None,
            created_at: None,
            updated_at: None,
        }
    }

    pub(super) fn configured(config: TosStagingToolConfig, pending_cleanup_count: i64) -> Self {
        Self {
            configured: true,
            config_id: Some(config.id),
            version: Some(config.version),
            enabled: config.is_enabled,
            storage_provider: Some(config.storage_provider),
            endpoint: Some(config.endpoint),
            region: Some(config.region),
            bucket: Some(config.bucket),
            object_prefix: Some(config.object_prefix),
            access_key_masked: Some(mask_credential(&config.access_key)),
            secret_key_masked: Some(mask_credential(&config.secret_key)),
            access_key_configured: !config.access_key.is_empty(),
            secret_key_configured: !config.secret_key.is_empty(),
            signed_url_ttl_seconds: Some(config.signed_url_ttl_seconds),
            max_file_bytes: Some(config.max_file_bytes),
            max_audio_duration_seconds: Some(config.max_audio_duration_seconds),
            pending_cleanup_count,
            last_check_status: config.last_check_status,
            last_check_requested_at: config.last_check_requested_at,
            last_checked_at: config.last_checked_at,
            last_check_error_summary: config.last_check_error_summary,
            created_at: Some(config.created_at),
            updated_at: Some(config.updated_at),
        }
    }
}

fn mask_credential(value: &str) -> String {
    let characters: Vec<char> = value.chars().collect();
    if characters.len() <= 8 {
        return "****".to_string();
    }
    format!(
        "{}****{}",
        characters[..4].iter().collect::<String>(),
        characters[characters.len() - 4..]
            .iter()
            .collect::<String>()
    )
}
