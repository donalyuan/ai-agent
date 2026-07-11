use crate::repositories::{
    AiModelRepository, AiModelRepositoryError, AiModelStatus, CreateAiModelInput,
    PostgresAiModelRepository,
};
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
    pub image_api_key: Option<String>,
    pub image_base_url: Option<String>,
    pub image_model: Option<String>,
    pub jimeng_access_key: Option<String>,
    pub jimeng_secret_key: Option<String>,
    pub jimeng_request_key: Option<String>,
    pub jimeng_width: u32,
    pub jimeng_height: u32,
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
            image_api_key: env_non_empty("OPENAI_IMAGE_KEY"),
            image_base_url: env_non_empty("OPENAI_IMAGE_BASE_URL"),
            image_model: env_non_empty("OPENAI_IMAGE_MODEL"),
            jimeng_access_key: env_non_empty("JIMENG_ACCESS_KEY"),
            jimeng_secret_key: env_non_empty("JIMENG_SECRET_KEY"),
            jimeng_request_key: env_non_empty("JIMENG_REQ_KEY"),
            jimeng_width: env_u32("JIMENG_IMAGE_WIDTH", 1328),
            jimeng_height: env_u32("JIMENG_IMAGE_HEIGHT", 1328),
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

    if let (Some(api_key), Some(base_url), Some(upstream_model)) = (
        non_empty(config.text_api_key),
        non_empty(config.text_base_url),
        non_empty(config.text_model),
    ) {
        let (api_protocol, request_base_url) = normalize_text_url(&base_url)?;
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
                timeout_seconds: config.text_timeout_seconds,
                reasoning_effort: non_empty(config.text_reasoning_effort),
                max_output_tokens: config.text_max_output_tokens,
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
                timeout_seconds: 120,
                reasoning_effort: None,
                max_output_tokens: None,
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

    if let (Some(api_key), Some(api_secret)) = (
        non_empty(config.jimeng_access_key),
        non_empty(config.jimeng_secret_key),
    ) {
        let size = format!("{}x{}", config.jimeng_width, config.jimeng_height);
        create_or_skip(
            pool,
            &repository,
            &mut outcome,
            "legacy:image-jimeng",
            CreateAiModelInput {
                display_name: "环境导入 / 即梦图片".to_string(),
                model_type: ModelType::Image,
                provider_name: "火山引擎即梦".to_string(),
                api_protocol: ApiProtocol::JimengVisual,
                protocol_version: "v1".to_string(),
                auth_scheme: AuthScheme::AccessKeySecret,
                request_base_url: "https://visual.volcengineapi.com".to_string(),
                upstream_model: "jimeng-visual".to_string(),
                api_key,
                api_secret: Some(api_secret),
                timeout_seconds: 120,
                reasoning_effort: None,
                max_output_tokens: None,
                settings: json!({
                    "supported_sizes": [size],
                    "default_size": size,
                    "max_images_per_request": 4,
                    "request_key": non_empty(config.jimeng_request_key)
                        .unwrap_or_else(|| "high_aes_general_v30l_zt2i".to_string())
                }),
                sort_order: 10,
                remark: "由一次性环境配置导入命令创建".to_string(),
                status: AiModelStatus::Enabled,
                source: "environment_import".to_string(),
                source_key: Some("legacy:image-jimeng".to_string()),
            },
        )
        .await?;
    }

    Ok(outcome)
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
    let versioned = root_path
        .rsplit('/')
        .next()
        .is_some_and(|segment| {
            segment
                .strip_prefix('v')
                .is_some_and(|version| !version.is_empty() && version.chars().all(|ch| ch.is_ascii_digit()))
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
    std::env::var(name).ok().and_then(|value| non_empty(Some(value)))
}

fn env_i32(name: &str, fallback: i32) -> i32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn env_u32(name: &str, fallback: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.map(|item| item.trim().to_string()).filter(|item| !item.is_empty())
}
