use crate::repositories::{
    AiModelRepository, AiModelRepositoryError, AiModelStatus, CreateAiModelInput,
    PostgresAiModelRepository,
};
use novex_ai_core::{DefinitionRegistry, DefinitionStatus};
use novex_model::{ApiProtocol, AuthScheme, ModelType};
use serde_json::json;
use sqlx::PgPool;
use url::Url;
use uuid::Uuid;

#[derive(Clone, Debug, Default)]
pub struct LegacyModelImportConfig {
    pub text_api_key: Option<String>,
    pub text_base_url: Option<String>,
    pub text_model: Option<String>,
    pub text_timeout_seconds: i32,
    pub text_reasoning_effort: Option<String>,
    pub text_max_output_tokens: Option<i32>,
    pub text_context_window: Option<u64>,
    pub text_tokenizer_profile_key: Option<String>,
    pub text_tokenizer_profile_version: Option<String>,
    pub image_api_key: Option<String>,
    pub image_base_url: Option<String>,
    pub image_model: Option<String>,
}

impl LegacyModelImportConfig {
    pub fn from_env() -> Self {
        Self {
            text_api_key: env_non_empty("OPENAI_API_KEY"),
            text_base_url: env_non_empty("OPENAI_BASE_URL"),
            text_model: env_non_empty("OPENAI_MODEL"),
            text_timeout_seconds: env_i32("OPENAI_TIMEOUT_SECONDS", 120),
            text_reasoning_effort: env_non_empty("OPENAI_REASONING_EFFORT"),
            text_max_output_tokens: Some(env_i32("OPENAI_MAX_OUTPUT_TOKENS", 3000)),
            text_context_window: env_u64("OPENAI_CONTEXT_WINDOW"),
            text_tokenizer_profile_key: env_non_empty("OPENAI_TOKENIZER_PROFILE_KEY"),
            text_tokenizer_profile_version: env_non_empty("OPENAI_TOKENIZER_PROFILE_VERSION"),
            image_api_key: env_non_empty("OPENAI_IMAGE_KEY"),
            image_base_url: env_non_empty("OPENAI_IMAGE_BASE_URL"),
            image_model: env_non_empty("OPENAI_IMAGE_MODEL"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImportedModelSummary {
    pub model_id: Uuid,
    pub display_name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModelImportOutcome {
    pub created: Vec<ImportedModelSummary>,
    pub skipped: Vec<String>,
}

pub async fn import_legacy_model_config(
    pool: &PgPool,
    config: LegacyModelImportConfig,
) -> Result<ModelImportOutcome, AiModelRepositoryError> {
    let repository = PostgresAiModelRepository::new(pool.clone());
    let mut outcome = ModelImportOutcome::default();

    if let (
        Some(api_key),
        Some(base_url),
        Some(upstream_model),
        Some(context_window),
        Some(tokenizer_profile_key),
        Some(tokenizer_profile_version),
    ) = (
        non_empty(config.text_api_key),
        non_empty(config.text_base_url),
        non_empty(config.text_model),
        config.text_context_window,
        non_empty(config.text_tokenizer_profile_key),
        non_empty(config.text_tokenizer_profile_version),
    ) {
        let (api_protocol, request_base_url) = normalize_text_url(&base_url)?;
        validate_import_profile(
            api_protocol,
            &tokenizer_profile_key,
            &tokenizer_profile_version,
        )?;
        create_or_skip(
            pool,
            &repository,
            &mut outcome,
            "legacy:text-openai",
            CreateAiModelInput {
                display_name: "环境导入 / 文本 OpenAI".to_string(),
                model_type: ModelType::Text,
                provider_name: "OpenAI Compatible".to_string(),
                api_protocol,
                protocol_version: "v1".to_string(),
                auth_scheme: AuthScheme::Bearer,
                request_base_url,
                upstream_model,
                api_key,
                api_secret: None,
                catalog_access_key: None,
                catalog_secret_key: None,
                voice_catalog_source_model_id: None,
                timeout_seconds: config.text_timeout_seconds,
                reasoning_effort: non_empty(config.text_reasoning_effort),
                max_output_tokens: config.text_max_output_tokens,
                context_window: i64::try_from(context_window).ok(),
                tokenizer_profile_key: Some(tokenizer_profile_key),
                tokenizer_profile_version: Some(tokenizer_profile_version),
                settings: json!({}),
                sort_order: 0,
                remark: "由一次性环境配置导入命令创建".to_string(),
                status: AiModelStatus::Enabled,
                source: "environment_import".to_string(),
                source_key: Some("legacy:text-openai".to_string()),
            },
        )
        .await?;
    }

    if let (Some(api_key), Some(base_url), Some(upstream_model)) = (
        non_empty(config.image_api_key),
        non_empty(config.image_base_url),
        non_empty(config.image_model),
    ) {
        let request_base_url = normalize_image_url(&base_url)?;
        create_or_skip(
            pool,
            &repository,
            &mut outcome,
            "legacy:image-openai",
            CreateAiModelInput {
                display_name: "环境导入 / OpenAI 图片".to_string(),
                model_type: ModelType::Image,
                provider_name: "OpenAI".to_string(),
                api_protocol: ApiProtocol::OpenAiImages,
                protocol_version: "v1".to_string(),
                auth_scheme: AuthScheme::Bearer,
                request_base_url,
                upstream_model,
                api_key,
                api_secret: None,
                catalog_access_key: None,
                catalog_secret_key: None,
                voice_catalog_source_model_id: None,
                timeout_seconds: 120,
                reasoning_effort: None,
                max_output_tokens: None,
                context_window: None,
                tokenizer_profile_key: None,
                tokenizer_profile_version: None,
                settings: json!({
                    "supported_sizes": ["1024x1024"],
                    "default_size": "1024x1024",
                    "max_images_per_request": 4
                }),
                sort_order: 0,
                remark: "由一次性环境配置导入命令创建".to_string(),
                status: AiModelStatus::Enabled,
                source: "environment_import".to_string(),
                source_key: Some("legacy:image-openai".to_string()),
            },
        )
        .await?;
    }

    Ok(outcome)
}

fn validate_import_profile(
    protocol: ApiProtocol,
    key: &str,
    version: &str,
) -> Result<(), AiModelRepositoryError> {
    let definitions_dir = std::env::var("NOVEX_AGENT_DEFINITIONS_DIR")
        .unwrap_or_else(|_| "/app/agent-definitions".to_string());
    let definitions = DefinitionRegistry::load(definitions_dir).map_err(|error| {
        AiModelRepositoryError::InvalidConfig(format!(
            "cannot load Definition Registry for model import: {error}"
        ))
    })?;
    let profile = definitions.tokenizer_profile(key, version).map_err(|_| {
        AiModelRepositoryError::InvalidConfig(format!(
            "unknown Tokenizer Profile for model import: {key}@{version}"
        ))
    })?;
    if matches!(
        profile.status,
        DefinitionStatus::Candidate | DefinitionStatus::Revoked
    ) || !profile
        .applicable_protocols
        .iter()
        .any(|item| item == protocol.as_str())
    {
        return Err(AiModelRepositoryError::InvalidConfig(format!(
            "Tokenizer Profile is unavailable or incompatible for model import: {key}@{version}"
        )));
    }
    Ok(())
}

async fn create_or_skip(
    pool: &PgPool,
    repository: &PostgresAiModelRepository,
    outcome: &mut ModelImportOutcome,
    source_key: &str,
    input: CreateAiModelInput,
) -> Result<(), AiModelRepositoryError> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM ai_models WHERE source_key = $1)",
    )
    .bind(source_key)
    .fetch_one(pool)
    .await
    .map_err(AiModelRepositoryError::from)?;
    if exists {
        outcome.skipped.push(source_key.to_string());
        return Ok(());
    }
    let model = repository.create(input).await?;
    outcome.created.push(ImportedModelSummary {
        model_id: model.id,
        display_name: model.display_name,
    });
    Ok(())
}

fn normalize_text_url(value: &str) -> Result<(ApiProtocol, String), AiModelRepositoryError> {
    let mut url = parse_url(value)?;
    let path = url.path().trim_end_matches('/').to_string();
    let (protocol, root_path) = if let Some(root) = path.strip_suffix("/responses") {
        (ApiProtocol::OpenAiResponses, root)
    } else if let Some(root) = path.strip_suffix("/chat/completions") {
        (ApiProtocol::OpenAiChatCompletions, root)
    } else {
        (ApiProtocol::OpenAiChatCompletions, path.as_str())
    };
    set_versioned_root(&mut url, root_path);
    Ok((protocol, url.to_string().trim_end_matches('/').to_string()))
}

fn normalize_image_url(value: &str) -> Result<String, AiModelRepositoryError> {
    let mut url = parse_url(value)?;
    let path = url.path().trim_end_matches('/').to_string();
    let root_path = path
        .strip_suffix("/images/generations")
        .unwrap_or(path.as_str());
    set_versioned_root(&mut url, root_path);
    Ok(url.to_string().trim_end_matches('/').to_string())
}

fn parse_url(value: &str) -> Result<Url, AiModelRepositoryError> {
    Url::parse(value.trim()).map_err(|error| {
        AiModelRepositoryError::InvalidConfig(format!("invalid legacy model URL: {error}"))
    })
}

fn set_versioned_root(url: &mut Url, root_path: &str) {
    let root_path = root_path.trim_end_matches('/');
    let versioned = root_path.rsplit('/').next().is_some_and(|segment| {
        segment.strip_prefix('v').is_some_and(|version| {
            !version.is_empty() && version.chars().all(|ch| ch.is_ascii_digit())
        })
    });
    let path = if versioned {
        root_path.to_string()
    } else if root_path.is_empty() || root_path == "/" {
        "/v1".to_string()
    } else {
        format!("{root_path}/v1")
    };
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
}

fn env_non_empty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .and_then(|value| non_empty(Some(value)))
}

fn env_i32(name: &str, fallback: i32) -> i32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}
