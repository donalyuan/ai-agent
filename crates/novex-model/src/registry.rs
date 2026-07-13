use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    Text,
    Image,
    Video,
}

impl ModelType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Video => "video",
        }
    }
}

impl fmt::Display for ModelType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ModelType {
    type Err = ModelSettingsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "text" => Ok(Self::Text),
            "image" => Ok(Self::Image),
            "video" => Ok(Self::Video),
            _ => Err(ModelSettingsError::InvalidModelType(value.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthScheme {
    Bearer,
    AccessKeySecret,
}

impl AuthScheme {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bearer => "bearer",
            Self::AccessKeySecret => "access_key_secret",
        }
    }
}

impl FromStr for AuthScheme {
    type Err = ModelSettingsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "bearer" => Ok(Self::Bearer),
            "access_key_secret" => Ok(Self::AccessKeySecret),
            _ => Err(ModelSettingsError::InvalidAuthScheme(value.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiProtocol {
    #[serde(rename = "openai_responses")]
    OpenAiResponses,
    #[serde(rename = "openai_chat_completions")]
    OpenAiChatCompletions,
    #[serde(rename = "openai_images")]
    OpenAiImages,
    JimengVisual,
    RunwayApi,
    KlingApi,
}

impl ApiProtocol {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai_responses",
            Self::OpenAiChatCompletions => "openai_chat_completions",
            Self::OpenAiImages => "openai_images",
            Self::JimengVisual => "jimeng_visual",
            Self::RunwayApi => "runway_api",
            Self::KlingApi => "kling_api",
        }
    }

    pub const fn supports(self, model_type: ModelType) -> bool {
        matches!(
            (self, model_type),
            (
                Self::OpenAiResponses | Self::OpenAiChatCompletions,
                ModelType::Text
            ) | (Self::OpenAiImages | Self::JimengVisual, ModelType::Image)
                | (Self::RunwayApi | Self::KlingApi, ModelType::Video)
        )
    }

    pub const fn required_auth(self) -> AuthScheme {
        match self {
            Self::JimengVisual | Self::KlingApi => AuthScheme::AccessKeySecret,
            Self::OpenAiResponses
            | Self::OpenAiChatCompletions
            | Self::OpenAiImages
            | Self::RunwayApi => AuthScheme::Bearer,
        }
    }
}

impl fmt::Display for ApiProtocol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ApiProtocol {
    type Err = ModelSettingsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "openai_responses" => Ok(Self::OpenAiResponses),
            "openai_chat_completions" => Ok(Self::OpenAiChatCompletions),
            "openai_images" => Ok(Self::OpenAiImages),
            "jimeng_visual" => Ok(Self::JimengVisual),
            "runway_api" => Ok(Self::RunwayApi),
            "kling_api" => Ok(Self::KlingApi),
            _ => Err(ModelSettingsError::InvalidProtocol(value.to_string())),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextModelSettings {}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImageModelSettings {
    #[serde(default)]
    pub supported_sizes: Vec<String>,
    #[serde(default)]
    pub default_size: Option<String>,
    #[serde(default)]
    pub max_images_per_request: Option<u32>,
    #[serde(default)]
    pub request_key: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VideoModelSettings {
    #[serde(default)]
    pub resolutions: Vec<String>,
    #[serde(default)]
    pub aspect_ratios: Vec<String>,
    #[serde(default)]
    pub min_duration_seconds: Option<u32>,
    #[serde(default)]
    pub max_duration_seconds: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "model_type", content = "settings", rename_all = "snake_case")]
pub enum ModelSettings {
    Text(TextModelSettings),
    Image(ImageModelSettings),
    Video(VideoModelSettings),
}

impl ModelSettings {
    pub fn parse(model_type: ModelType, value: Value) -> Result<Self, ModelSettingsError> {
        let settings = match model_type {
            ModelType::Text => Self::Text(serde_json::from_value(value)?),
            ModelType::Image => Self::Image(serde_json::from_value(value)?),
            ModelType::Video => Self::Video(serde_json::from_value(value)?),
        };
        settings.validate()?;
        Ok(settings)
    }

    pub fn default_image_size(&self) -> Option<&str> {
        match self {
            Self::Image(settings) => settings.default_size.as_deref(),
            _ => None,
        }
    }

    pub fn video_duration_range(&self) -> Option<(u32, u32)> {
        match self {
            Self::Video(VideoModelSettings {
                min_duration_seconds: Some(minimum),
                max_duration_seconds: Some(maximum),
                ..
            }) => Some((*minimum, *maximum)),
            _ => None,
        }
    }

    fn validate(&self) -> Result<(), ModelSettingsError> {
        match self {
            Self::Text(_) => Ok(()),
            Self::Image(settings) => {
                if let Some(maximum) = settings.max_images_per_request {
                    if !(1..=48).contains(&maximum) {
                        return Err(ModelSettingsError::InvalidSettings(
                            "max_images_per_request must be between 1 and 48".to_string(),
                        ));
                    }
                }
                if let Some(default_size) = &settings.default_size {
                    if !settings.supported_sizes.is_empty()
                        && !settings.supported_sizes.contains(default_size)
                    {
                        return Err(ModelSettingsError::InvalidSettings(
                            "default_size must be included in supported_sizes".to_string(),
                        ));
                    }
                }
                Ok(())
            }
            Self::Video(settings) => {
                match (settings.min_duration_seconds, settings.max_duration_seconds) {
                    (Some(minimum), Some(maximum)) if minimum == 0 || minimum > maximum => {
                        Err(ModelSettingsError::InvalidSettings(
                            "video duration range is invalid".to_string(),
                        ))
                    }
                    (Some(_), None) | (None, Some(_)) => Err(ModelSettingsError::InvalidSettings(
                        "video duration range requires both minimum and maximum".to_string(),
                    )),
                    _ => Ok(()),
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ModelExecutionSnapshot {
    pub model_id: Uuid,
    pub display_name: String,
    pub model_type: ModelType,
    pub provider_name: String,
    pub api_protocol: ApiProtocol,
    pub protocol_version: String,
    pub request_base_url: String,
    pub upstream_model: String,
    pub reasoning_effort: Option<String>,
    pub timeout_seconds: u64,
    pub max_output_tokens: Option<u32>,
    pub settings: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelRuntimeConfig {
    pub snapshot: ModelExecutionSnapshot,
    pub auth_scheme: AuthScheme,
    pub api_key: String,
    pub api_secret: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelSettingsError {
    InvalidModelType(String),
    InvalidProtocol(String),
    InvalidAuthScheme(String),
    InvalidSettings(String),
}

impl fmt::Display for ModelSettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModelType(value) => write!(formatter, "invalid model type: {value}"),
            Self::InvalidProtocol(value) => write!(formatter, "invalid API protocol: {value}"),
            Self::InvalidAuthScheme(value) => write!(formatter, "invalid auth scheme: {value}"),
            Self::InvalidSettings(value) => write!(formatter, "invalid model settings: {value}"),
        }
    }
}

impl std::error::Error for ModelSettingsError {}

impl From<serde_json::Error> for ModelSettingsError {
    fn from(error: serde_json::Error) -> Self {
        Self::InvalidSettings(error.to_string())
    }
}
