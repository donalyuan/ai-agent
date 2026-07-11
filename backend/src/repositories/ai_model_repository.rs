use async_trait::async_trait;
use chrono::{DateTime, Utc};
use novex_model::{
    ApiProtocol, AuthScheme, ModelExecutionSnapshot, ModelRuntimeConfig, ModelSettings, ModelType,
};
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder, Row, Transaction};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AiModelStatus {
    Enabled,
    Disabled,
    Deleted,
}

impl AiModelStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Deleted => "deleted",
        }
    }
}

impl TryFrom<&str> for AiModelStatus {
    type Error = AiModelRepositoryError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "enabled" => Ok(Self::Enabled),
            "disabled" => Ok(Self::Disabled),
            "deleted" => Ok(Self::Deleted),
            _ => Err(AiModelRepositoryError::Storage(format!(
                "unknown model status: {value}"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AiModel {
    pub id: Uuid,
    pub display_name: String,
    pub model_type: ModelType,
    pub provider_name: String,
    pub api_protocol: ApiProtocol,
    pub protocol_version: String,
    pub auth_scheme: AuthScheme,
    pub request_base_url: String,
    pub upstream_model: String,
    pub api_key: String,
    pub api_secret: Option<String>,
    pub timeout_seconds: i32,
    pub reasoning_effort: Option<String>,
    pub max_output_tokens: Option<i32>,
    pub settings: Value,
    pub sort_order: i32,
    pub remark: String,
    pub status: AiModelStatus,
    pub is_default: bool,
    pub last_call_status: String,
    pub last_call_at: Option<DateTime<Utc>>,
    pub last_error_summary: Option<String>,
    pub source: String,
    pub source_key: Option<String>,
    pub version: i64,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AiModelListFilter {
    pub model_type: Option<ModelType>,
    pub status: Option<AiModelStatus>,
    pub provider_name: Option<String>,
    pub api_protocol: Option<ApiProtocol>,
    pub search: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateAiModelInput {
    pub display_name: String,
    pub model_type: ModelType,
    pub provider_name: String,
    pub api_protocol: ApiProtocol,
    pub protocol_version: String,
    pub auth_scheme: AuthScheme,
    pub request_base_url: String,
    pub upstream_model: String,
    pub api_key: String,
    pub api_secret: Option<String>,
    pub timeout_seconds: i32,
    pub reasoning_effort: Option<String>,
    pub max_output_tokens: Option<i32>,
    pub settings: Value,
    pub sort_order: i32,
    pub remark: String,
    pub status: AiModelStatus,
    pub source: String,
    pub source_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateAiModelInput {
    pub version: i64,
    pub model_type: ModelType,
    pub display_name: String,
    pub provider_name: String,
    pub api_protocol: ApiProtocol,
    pub protocol_version: String,
    pub auth_scheme: AuthScheme,
    pub request_base_url: String,
    pub upstream_model: String,
    /// `None` means preserve the current plaintext credential.
    pub api_key: Option<String>,
    /// `None` means preserve the current optional plaintext credential.
    pub api_secret: Option<String>,
    pub timeout_seconds: i32,
    pub reasoning_effort: Option<String>,
    pub max_output_tokens: Option<i32>,
    pub settings: Value,
    pub sort_order: i32,
    pub remark: String,
    pub replacement_model_id: Option<Uuid>,
    pub allow_no_default: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChangeAiModelStatusInput {
    pub id: Uuid,
    pub version: i64,
    pub status: AiModelStatus,
    pub replacement_model_id: Option<Uuid>,
    pub allow_no_default: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeleteAiModelInput {
    pub id: Uuid,
    pub version: i64,
    pub replacement_model_id: Option<Uuid>,
    pub allow_no_default: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DeleteAiModelOutcome {
    Physical,
    Logical(AiModel),
}

#[derive(Clone)]
pub struct PostgresAiModelRepository {
    pool: PgPool,
}

impl PostgresAiModelRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
pub trait AiModelRepository: Send + Sync {
    async fn create(&self, input: CreateAiModelInput) -> Result<AiModel, AiModelRepositoryError>;
    async fn get(&self, id: Uuid) -> Result<AiModel, AiModelRepositoryError>;
    async fn list(
        &self,
        filter: AiModelListFilter,
    ) -> Result<Vec<AiModel>, AiModelRepositoryError>;
    async fn update(
        &self,
        id: Uuid,
        input: UpdateAiModelInput,
    ) -> Result<AiModel, AiModelRepositoryError>;
    async fn set_default(
        &self,
        id: Uuid,
        version: i64,
    ) -> Result<AiModel, AiModelRepositoryError>;
    async fn change_status(
        &self,
        input: ChangeAiModelStatusInput,
    ) -> Result<AiModel, AiModelRepositoryError>;
    async fn delete(
        &self,
        input: DeleteAiModelInput,
    ) -> Result<DeleteAiModelOutcome, AiModelRepositoryError>;
    async fn resolve_enabled(
        &self,
        id: Uuid,
        expected_type: ModelType,
    ) -> Result<ModelRuntimeConfig, AiModelRepositoryError>;
    async fn list_enabled_options(
        &self,
        model_type: ModelType,
    ) -> Result<Vec<AiModel>, AiModelRepositoryError>;
}

#[async_trait]
impl AiModelRepository for PostgresAiModelRepository {
    async fn create(&self, input: CreateAiModelInput) -> Result<AiModel, AiModelRepositoryError> {
        validate_configuration(ModelConfiguration {
            model_type: input.model_type,
            api_protocol: input.api_protocol,
            auth_scheme: input.auth_scheme,
            display_name: &input.display_name,
            provider_name: &input.provider_name,
            request_base_url: &input.request_base_url,
            upstream_model: &input.upstream_model,
            api_key: &input.api_key,
            api_secret: input.api_secret.as_deref(),
            timeout_seconds: input.timeout_seconds,
            reasoning_effort: input.reasoning_effort.as_deref(),
            max_output_tokens: input.max_output_tokens,
            settings: &input.settings,
        })?;
        if input.status == AiModelStatus::Deleted {
            return Err(AiModelRepositoryError::InvalidConfig(
                "new model cannot start deleted".to_string(),
            ));
        }
        if !matches!(input.source.as_str(), "admin" | "environment_import") {
            return Err(AiModelRepositoryError::InvalidConfig(
                "unknown model source".to_string(),
            ));
        }

        let mut transaction = self.pool.begin().await?;
        lock_model_type(&mut transaction, input.model_type).await?;
        let enabled_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM ai_models WHERE model_type = $1 AND status = 'enabled' AND deleted_at IS NULL",
        )
        .bind(input.model_type.as_str())
        .fetch_one(&mut *transaction)
        .await?;
        let is_default = input.status == AiModelStatus::Enabled && enabled_count == 0;

        let row = sqlx::query(&format!(
            r#"
            INSERT INTO ai_models (
                display_name, model_type, provider_name, api_protocol, protocol_version,
                auth_scheme, request_base_url, upstream_model, api_key, api_secret,
                timeout_seconds, reasoning_effort, max_output_tokens, settings, sort_order,
                remark, status, is_default, source, source_key
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20
            )
            RETURNING {MODEL_COLUMNS}
            "#
        ))
        .bind(input.display_name.trim())
        .bind(input.model_type.as_str())
        .bind(input.provider_name.trim())
        .bind(input.api_protocol.as_str())
        .bind(input.protocol_version.trim())
        .bind(input.auth_scheme.as_str())
        .bind(normalize_base_url(&input.request_base_url))
        .bind(input.upstream_model.trim())
        .bind(input.api_key)
        .bind(input.api_secret.filter(|value| !value.is_empty()))
        .bind(input.timeout_seconds)
        .bind(normalize_optional(input.reasoning_effort))
        .bind(input.max_output_tokens)
        .bind(input.settings)
        .bind(input.sort_order)
        .bind(input.remark)
        .bind(input.status.as_str())
        .bind(is_default)
        .bind(input.source)
        .bind(normalize_optional(input.source_key))
        .fetch_one(&mut *transaction)
        .await?;
        let model = model_from_row(row)?;
        transaction.commit().await?;
        Ok(model)
    }

    async fn get(&self, id: Uuid) -> Result<AiModel, AiModelRepositoryError> {
        let row = sqlx::query(&format!(
            "SELECT {MODEL_COLUMNS} FROM ai_models WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AiModelRepositoryError::NotFound(id))?;
        model_from_row(row)
    }

    async fn list(
        &self,
        filter: AiModelListFilter,
    ) -> Result<Vec<AiModel>, AiModelRepositoryError> {
        let mut query = QueryBuilder::<Postgres>::new(format!(
            "SELECT {MODEL_COLUMNS} FROM ai_models WHERE 1 = 1"
        ));
        if let Some(status) = filter.status {
            query.push(" AND status = ").push_bind(status.as_str());
        } else {
            query.push(" AND deleted_at IS NULL");
        }
        if let Some(model_type) = filter.model_type {
            query
                .push(" AND model_type = ")
                .push_bind(model_type.as_str());
        }
        if let Some(provider_name) = non_empty(filter.provider_name) {
            query
                .push(" AND LOWER(provider_name) = LOWER(")
                .push_bind(provider_name)
                .push(")");
        }
        if let Some(api_protocol) = filter.api_protocol {
            query
                .push(" AND api_protocol = ")
                .push_bind(api_protocol.as_str());
        }
        if let Some(search) = non_empty(filter.search) {
            let pattern = format!("%{search}%");
            query
                .push(" AND (display_name ILIKE ")
                .push_bind(pattern.clone())
                .push(" OR upstream_model ILIKE ")
                .push_bind(pattern.clone())
                .push(" OR provider_name ILIKE ")
                .push_bind(pattern)
                .push(")");
        }
        query.push(" ORDER BY is_default DESC, sort_order ASC, updated_at DESC, id ASC");
        query
            .build()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(model_from_row)
            .collect()
    }

    async fn update(
        &self,
        id: Uuid,
        input: UpdateAiModelInput,
    ) -> Result<AiModel, AiModelRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let current = get_for_update(&mut transaction, id).await?;
        ensure_version(&current, input.version)?;
        if current.status == AiModelStatus::Deleted {
            return Err(AiModelRepositoryError::Disabled(id));
        }
        let api_key = input
            .api_key
            .as_ref()
            .filter(|value| !value.is_empty())
            .unwrap_or(&current.api_key)
            .clone();
        let api_secret = input
            .api_secret
            .as_ref()
            .filter(|value| !value.is_empty())
            .cloned()
            .or_else(|| current.api_secret.clone());
        lock_model_types(&mut transaction, current.model_type, input.model_type).await?;
        validate_configuration(ModelConfiguration {
            model_type: input.model_type,
            api_protocol: input.api_protocol,
            auth_scheme: input.auth_scheme,
            display_name: &input.display_name,
            provider_name: &input.provider_name,
            request_base_url: &input.request_base_url,
            upstream_model: &input.upstream_model,
            api_key: &api_key,
            api_secret: api_secret.as_deref(),
            timeout_seconds: input.timeout_seconds,
            reasoning_effort: input.reasoning_effort.as_deref(),
            max_output_tokens: input.max_output_tokens,
            settings: &input.settings,
        })?;

        let is_default = if input.model_type == current.model_type {
            current.is_default
        } else {
            if current.is_default {
                replace_or_clear_default(
                    &mut transaction,
                    &current,
                    input.replacement_model_id,
                    input.allow_no_default,
                )
                .await?;
            }
            let enabled_in_new_type = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM ai_models WHERE model_type = $1 AND status = 'enabled' AND deleted_at IS NULL AND id <> $2",
            )
            .bind(input.model_type.as_str())
            .bind(id)
            .fetch_one(&mut *transaction)
            .await?;
            current.status == AiModelStatus::Enabled && enabled_in_new_type == 0
        };

        let row = sqlx::query(&format!(
            r#"
            UPDATE ai_models
            SET model_type = $2, display_name = $3, provider_name = $4, api_protocol = $5,
                protocol_version = $6, auth_scheme = $7, request_base_url = $8,
                upstream_model = $9, api_key = $10, api_secret = $11,
                timeout_seconds = $12, reasoning_effort = $13, max_output_tokens = $14,
                settings = $15, sort_order = $16, remark = $17, is_default = $18,
                version = version + 1
            WHERE id = $1 AND version = $19
            RETURNING {MODEL_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(input.model_type.as_str())
        .bind(input.display_name.trim())
        .bind(input.provider_name.trim())
        .bind(input.api_protocol.as_str())
        .bind(input.protocol_version.trim())
        .bind(input.auth_scheme.as_str())
        .bind(normalize_base_url(&input.request_base_url))
        .bind(input.upstream_model.trim())
        .bind(api_key)
        .bind(api_secret)
        .bind(input.timeout_seconds)
        .bind(normalize_optional(input.reasoning_effort))
        .bind(input.max_output_tokens)
        .bind(input.settings)
        .bind(input.sort_order)
        .bind(input.remark)
        .bind(is_default)
        .bind(input.version)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(AiModelRepositoryError::VersionConflict(id))?;
        let model = model_from_row(row)?;
        transaction.commit().await?;
        Ok(model)
    }

    async fn set_default(
        &self,
        id: Uuid,
        version: i64,
    ) -> Result<AiModel, AiModelRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let current = get_for_update(&mut transaction, id).await?;
        ensure_version(&current, version)?;
        ensure_enabled(&current)?;
        lock_model_type(&mut transaction, current.model_type).await?;
        if !current.is_default {
            clear_other_defaults(&mut transaction, current.model_type, id).await?;
        }
        let row = sqlx::query(&format!(
            r#"
            UPDATE ai_models
            SET is_default = TRUE, version = version + 1
            WHERE id = $1 AND version = $2
            RETURNING {MODEL_COLUMNS}
            "#
        ))
        .bind(id)
        .bind(version)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(AiModelRepositoryError::VersionConflict(id))?;
        let model = model_from_row(row)?;
        transaction.commit().await?;
        Ok(model)
    }

    async fn change_status(
        &self,
        input: ChangeAiModelStatusInput,
    ) -> Result<AiModel, AiModelRepositoryError> {
        if input.status == AiModelStatus::Deleted {
            return Err(AiModelRepositoryError::InvalidConfig(
                "use delete operation for deleted status".to_string(),
            ));
        }
        let mut transaction = self.pool.begin().await?;
        let current = get_for_update(&mut transaction, input.id).await?;
        ensure_version(&current, input.version)?;
        if current.status == AiModelStatus::Deleted {
            return Err(AiModelRepositoryError::Disabled(input.id));
        }
        lock_model_type(&mut transaction, current.model_type).await?;

        let make_default = if input.status == AiModelStatus::Enabled {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM ai_models WHERE model_type = $1 AND status = 'enabled' AND deleted_at IS NULL AND id <> $2",
            )
            .bind(current.model_type.as_str())
            .bind(current.id)
            .fetch_one(&mut *transaction)
            .await?
                == 0
        } else {
            if current.is_default {
                replace_or_clear_default(
                    &mut transaction,
                    &current,
                    input.replacement_model_id,
                    input.allow_no_default,
                )
                .await?;
            }
            false
        };

        let row = sqlx::query(&format!(
            r#"
            UPDATE ai_models
            SET status = $2, is_default = $3, version = version + 1
            WHERE id = $1 AND version = $4
            RETURNING {MODEL_COLUMNS}
            "#
        ))
        .bind(input.id)
        .bind(input.status.as_str())
        .bind(make_default)
        .bind(input.version)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or(AiModelRepositoryError::VersionConflict(input.id))?;
        let model = model_from_row(row)?;
        transaction.commit().await?;
        Ok(model)
    }

    async fn delete(
        &self,
        input: DeleteAiModelInput,
    ) -> Result<DeleteAiModelOutcome, AiModelRepositoryError> {
        let mut transaction = self.pool.begin().await?;
        let current = get_for_update(&mut transaction, input.id).await?;
        ensure_version(&current, input.version)?;
        if current.status == AiModelStatus::Deleted {
            return Ok(DeleteAiModelOutcome::Logical(current));
        }
        lock_model_type(&mut transaction, current.model_type).await?;
        if current.is_default {
            replace_or_clear_default(
                &mut transaction,
                &current,
                input.replacement_model_id,
                input.allow_no_default,
            )
            .await?;
        }
        let reference_count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT
                (SELECT COUNT(*) FROM agent_runs WHERE model_id = $1) +
                (SELECT COUNT(*) FROM asset_generation_tasks WHERE model_id = $1)
            "#,
        )
        .bind(input.id)
        .fetch_one(&mut *transaction)
        .await?;

        let outcome = if reference_count == 0 {
            let result = sqlx::query("DELETE FROM ai_models WHERE id = $1 AND version = $2")
                .bind(input.id)
                .bind(input.version)
                .execute(&mut *transaction)
                .await?;
            if result.rows_affected() != 1 {
                return Err(AiModelRepositoryError::VersionConflict(input.id));
            }
            DeleteAiModelOutcome::Physical
        } else {
            let row = sqlx::query(&format!(
                r#"
                UPDATE ai_models
                SET status = 'deleted', is_default = FALSE, deleted_at = NOW(),
                    version = version + 1
                WHERE id = $1 AND version = $2
                RETURNING {MODEL_COLUMNS}
                "#
            ))
            .bind(input.id)
            .bind(input.version)
            .fetch_optional(&mut *transaction)
            .await?
            .ok_or(AiModelRepositoryError::VersionConflict(input.id))?;
            DeleteAiModelOutcome::Logical(model_from_row(row)?)
        };
        transaction.commit().await?;
        Ok(outcome)
    }

    async fn resolve_enabled(
        &self,
        id: Uuid,
        expected_type: ModelType,
    ) -> Result<ModelRuntimeConfig, AiModelRepositoryError> {
        let model = self.get(id).await?;
        if model.model_type != expected_type {
            return Err(AiModelRepositoryError::TypeMismatch {
                id,
                expected: expected_type,
                actual: model.model_type,
            });
        }
        ensure_enabled(&model)?;
        validate_configuration(ModelConfiguration {
            model_type: model.model_type,
            api_protocol: model.api_protocol,
            auth_scheme: model.auth_scheme,
            display_name: &model.display_name,
            provider_name: &model.provider_name,
            request_base_url: &model.request_base_url,
            upstream_model: &model.upstream_model,
            api_key: &model.api_key,
            api_secret: model.api_secret.as_deref(),
            timeout_seconds: model.timeout_seconds,
            reasoning_effort: model.reasoning_effort.as_deref(),
            max_output_tokens: model.max_output_tokens,
            settings: &model.settings,
        })?;
        Ok(ModelRuntimeConfig {
            snapshot: ModelExecutionSnapshot {
                model_id: model.id,
                display_name: model.display_name,
                model_type: model.model_type,
                provider_name: model.provider_name,
                api_protocol: model.api_protocol,
                protocol_version: model.protocol_version,
                request_base_url: model.request_base_url,
                upstream_model: model.upstream_model,
                reasoning_effort: model.reasoning_effort,
                timeout_seconds: model.timeout_seconds as u64,
                max_output_tokens: model.max_output_tokens.map(|value| value as u32),
                settings: model.settings,
            },
            auth_scheme: model.auth_scheme,
            api_key: model.api_key,
            api_secret: model.api_secret,
        })
    }

    async fn list_enabled_options(
        &self,
        model_type: ModelType,
    ) -> Result<Vec<AiModel>, AiModelRepositoryError> {
        let rows = sqlx::query(&format!(
            r#"
            SELECT {MODEL_COLUMNS}
            FROM ai_models
            WHERE model_type = $1 AND status = 'enabled' AND deleted_at IS NULL
            ORDER BY is_default DESC, sort_order ASC, display_name ASC, id ASC
            "#
        ))
        .bind(model_type.as_str())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(model_from_row).collect()
    }
}

const MODEL_COLUMNS: &str = r#"
    id, display_name, model_type, provider_name, api_protocol, protocol_version,
    auth_scheme, request_base_url, upstream_model, api_key, api_secret,
    timeout_seconds, reasoning_effort, max_output_tokens, settings, sort_order,
    remark, status, is_default, last_call_status, last_call_at, last_error_summary,
    source, source_key, version, deleted_at, created_at, updated_at
"#;

struct ModelConfiguration<'a> {
    model_type: ModelType,
    api_protocol: ApiProtocol,
    auth_scheme: AuthScheme,
    display_name: &'a str,
    provider_name: &'a str,
    request_base_url: &'a str,
    upstream_model: &'a str,
    api_key: &'a str,
    api_secret: Option<&'a str>,
    timeout_seconds: i32,
    reasoning_effort: Option<&'a str>,
    max_output_tokens: Option<i32>,
    settings: &'a Value,
}

fn validate_configuration(config: ModelConfiguration<'_>) -> Result<(), AiModelRepositoryError> {
    if !config.api_protocol.supports(config.model_type) {
        return Err(AiModelRepositoryError::InvalidConfig(
            "API protocol does not support model type".to_string(),
        ));
    }
    if config.api_protocol.required_auth() != config.auth_scheme {
        return Err(AiModelRepositoryError::InvalidConfig(
            "auth scheme does not match API protocol".to_string(),
        ));
    }
    for (name, value) in [
        ("display_name", config.display_name),
        ("provider_name", config.provider_name),
        ("request_base_url", config.request_base_url),
        ("upstream_model", config.upstream_model),
        ("api_key", config.api_key),
    ] {
        if value.trim().is_empty() {
            return Err(AiModelRepositoryError::InvalidConfig(format!(
                "{name} is required"
            )));
        }
    }
    if !matches!(config.request_base_url.trim(), value if value.starts_with("http://") || value.starts_with("https://")) {
        return Err(AiModelRepositoryError::InvalidConfig(
            "request_base_url must use HTTP or HTTPS".to_string(),
        ));
    }
    if config.auth_scheme == AuthScheme::AccessKeySecret
        && config.api_secret.unwrap_or_default().trim().is_empty()
    {
        return Err(AiModelRepositoryError::InvalidConfig(
            "API Secret is required for access_key_secret".to_string(),
        ));
    }
    if !(1..=3600).contains(&config.timeout_seconds) {
        return Err(AiModelRepositoryError::InvalidConfig(
            "timeout_seconds must be between 1 and 3600".to_string(),
        ));
    }
    if config.max_output_tokens.is_some_and(|value| value <= 0) {
        return Err(AiModelRepositoryError::InvalidConfig(
            "max_output_tokens must be positive".to_string(),
        ));
    }
    if config.model_type != ModelType::Text
        && (config.reasoning_effort.is_some() || config.max_output_tokens.is_some())
    {
        return Err(AiModelRepositoryError::InvalidConfig(
            "text inference fields are only valid for text models".to_string(),
        ));
    }
    ModelSettings::parse(config.model_type, config.settings.clone())
        .map_err(|error| AiModelRepositoryError::InvalidConfig(error.to_string()))?;
    Ok(())
}

async fn lock_model_type(
    transaction: &mut Transaction<'_, Postgres>,
    model_type: ModelType,
) -> Result<(), AiModelRepositoryError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtext('ai_models:' || $1))")
        .bind(model_type.as_str())
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn lock_model_types(
    transaction: &mut Transaction<'_, Postgres>,
    first: ModelType,
    second: ModelType,
) -> Result<(), AiModelRepositoryError> {
    if first == second {
        return lock_model_type(transaction, first).await;
    }
    let (first, second) = if first.as_str() < second.as_str() {
        (first, second)
    } else {
        (second, first)
    };
    lock_model_type(transaction, first).await?;
    lock_model_type(transaction, second).await
}

async fn get_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<AiModel, AiModelRepositoryError> {
    let row = sqlx::query(&format!(
        "SELECT {MODEL_COLUMNS} FROM ai_models WHERE id = $1 FOR UPDATE"
    ))
    .bind(id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(AiModelRepositoryError::NotFound(id))?;
    model_from_row(row)
}

fn ensure_version(model: &AiModel, version: i64) -> Result<(), AiModelRepositoryError> {
    if model.version != version {
        Err(AiModelRepositoryError::VersionConflict(model.id))
    } else {
        Ok(())
    }
}

fn ensure_enabled(model: &AiModel) -> Result<(), AiModelRepositoryError> {
    if model.status != AiModelStatus::Enabled || model.deleted_at.is_some() {
        Err(AiModelRepositoryError::Disabled(model.id))
    } else {
        Ok(())
    }
}

async fn clear_other_defaults(
    transaction: &mut Transaction<'_, Postgres>,
    model_type: ModelType,
    target_id: Uuid,
) -> Result<(), AiModelRepositoryError> {
    sqlx::query(
        r#"
        UPDATE ai_models
        SET is_default = FALSE, version = version + 1
        WHERE model_type = $1 AND is_default = TRUE AND id <> $2 AND deleted_at IS NULL
        "#,
    )
    .bind(model_type.as_str())
    .bind(target_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn replace_or_clear_default(
    transaction: &mut Transaction<'_, Postgres>,
    current: &AiModel,
    replacement_model_id: Option<Uuid>,
    allow_no_default: bool,
) -> Result<(), AiModelRepositoryError> {
    let other_enabled_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*) FROM ai_models
        WHERE model_type = $1 AND status = 'enabled' AND deleted_at IS NULL AND id <> $2
        "#,
    )
    .bind(current.model_type.as_str())
    .bind(current.id)
    .fetch_one(&mut **transaction)
    .await?;

    if other_enabled_count == 0 {
        if allow_no_default {
            return Ok(());
        }
        return Err(AiModelRepositoryError::NoDefaultConfirmation(current.id));
    }
    let replacement_id = replacement_model_id
        .ok_or(AiModelRepositoryError::ReplacementRequired(current.id))?;
    let replacement = get_for_update(transaction, replacement_id).await?;
    if replacement.id == current.id
        || replacement.model_type != current.model_type
        || replacement.status != AiModelStatus::Enabled
        || replacement.deleted_at.is_some()
    {
        return Err(AiModelRepositoryError::InvalidReplacement(replacement_id));
    }
    sqlx::query(
        r#"
        UPDATE ai_models
        SET is_default = FALSE
        WHERE model_type = $1 AND is_default = TRUE AND id <> $2 AND deleted_at IS NULL
        "#,
    )
    .bind(current.model_type.as_str())
    .bind(replacement.id)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE ai_models SET is_default = TRUE, version = version + 1 WHERE id = $1",
    )
    .bind(replacement.id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn model_from_row(row: sqlx::postgres::PgRow) -> Result<AiModel, AiModelRepositoryError> {
    let model_type_value: String = row.get("model_type");
    let api_protocol_value: String = row.get("api_protocol");
    let auth_scheme_value: String = row.get("auth_scheme");
    let status_value: String = row.get("status");
    Ok(AiModel {
        id: row.get("id"),
        display_name: row.get("display_name"),
        model_type: ModelType::from_str(&model_type_value)
            .map_err(|error| AiModelRepositoryError::Storage(error.to_string()))?,
        provider_name: row.get("provider_name"),
        api_protocol: ApiProtocol::from_str(&api_protocol_value)
            .map_err(|error| AiModelRepositoryError::Storage(error.to_string()))?,
        protocol_version: row.get("protocol_version"),
        auth_scheme: AuthScheme::from_str(&auth_scheme_value)
            .map_err(|error| AiModelRepositoryError::Storage(error.to_string()))?,
        request_base_url: row.get("request_base_url"),
        upstream_model: row.get("upstream_model"),
        api_key: row.get("api_key"),
        api_secret: row.get("api_secret"),
        timeout_seconds: row.get("timeout_seconds"),
        reasoning_effort: row.get("reasoning_effort"),
        max_output_tokens: row.get("max_output_tokens"),
        settings: row.get("settings"),
        sort_order: row.get("sort_order"),
        remark: row.get("remark"),
        status: AiModelStatus::try_from(status_value.as_str())?,
        is_default: row.get("is_default"),
        last_call_status: row.get("last_call_status"),
        last_call_at: row.get("last_call_at"),
        last_error_summary: row.get("last_error_summary"),
        source: row.get("source"),
        source_key: row.get("source_key"),
        version: row.get("version"),
        deleted_at: row.get("deleted_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn normalize_base_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn non_empty(value: Option<String>) -> Option<String> {
    normalize_optional(value)
}

#[derive(Debug, PartialEq)]
pub enum AiModelRepositoryError {
    NotFound(Uuid),
    Disabled(Uuid),
    TypeMismatch {
        id: Uuid,
        expected: ModelType,
        actual: ModelType,
    },
    VersionConflict(Uuid),
    ReplacementRequired(Uuid),
    InvalidReplacement(Uuid),
    NoDefaultConfirmation(Uuid),
    InvalidConfig(String),
    Storage(String),
}

impl From<sqlx::Error> for AiModelRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for AiModelRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(formatter, "model not found: {id}"),
            Self::Disabled(id) => write!(formatter, "model is not enabled: {id}"),
            Self::TypeMismatch {
                id,
                expected,
                actual,
            } => write!(
                formatter,
                "model type mismatch for {id}: expected {expected}, got {actual}"
            ),
            Self::VersionConflict(id) => write!(formatter, "model version conflict: {id}"),
            Self::ReplacementRequired(id) => {
                write!(formatter, "replacement model is required for {id}")
            }
            Self::InvalidReplacement(id) => write!(formatter, "invalid replacement model: {id}"),
            Self::NoDefaultConfirmation(id) => {
                write!(formatter, "explicit no-default confirmation is required for {id}")
            }
            Self::InvalidConfig(message) => write!(formatter, "invalid model config: {message}"),
            Self::Storage(message) => write!(formatter, "model storage error: {message}"),
        }
    }
}

impl std::error::Error for AiModelRepositoryError {}
