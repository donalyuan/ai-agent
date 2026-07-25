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
    Speech,
}

impl ModelType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Video => "video",
            Self::Speech => "speech",
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
            "speech" => Ok(Self::Speech),
            _ => Err(ModelSettingsError::InvalidModelType(value.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthScheme {
    Bearer,
    AccessKeySecret,
    ApiKey,
}

impl AuthScheme {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bearer => "bearer",
            Self::AccessKeySecret => "access_key_secret",
            Self::ApiKey => "api_key",
        }
    }
}

impl FromStr for AuthScheme {
    type Err = ModelSettingsError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "bearer" => Ok(Self::Bearer),
            "access_key_secret" => Ok(Self::AccessKeySecret),
            "api_key" => Ok(Self::ApiKey),
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
    VolcengineArkImages,
    VolcengineArkVideo,
    RunwayApi,
    KlingApi,
    VolcengineTtsV3,
    #[serde(rename = "openai_audio_speech")]
    OpenAiAudioSpeech,
    VolcengineAsrV3,
}

impl ApiProtocol {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai_responses",
            Self::OpenAiChatCompletions => "openai_chat_completions",
            Self::OpenAiImages => "openai_images",
            Self::VolcengineArkImages => "volcengine_ark_images",
            Self::VolcengineArkVideo => "volcengine_ark_video",
            Self::RunwayApi => "runway_api",
            Self::KlingApi => "kling_api",
            Self::VolcengineTtsV3 => "volcengine_tts_v3",
            Self::OpenAiAudioSpeech => "openai_audio_speech",
            Self::VolcengineAsrV3 => "volcengine_asr_v3",
        }
    }

    pub const fn supports(self, model_type: ModelType) -> bool {
        matches!(
            (self, model_type),
            (
                Self::OpenAiResponses | Self::OpenAiChatCompletions,
                ModelType::Text
            ) | (
                Self::OpenAiImages | Self::VolcengineArkImages,
                ModelType::Image
            ) | (
                Self::VolcengineArkVideo | Self::RunwayApi | Self::KlingApi,
                ModelType::Video
            ) | (
                Self::VolcengineTtsV3 | Self::OpenAiAudioSpeech | Self::VolcengineAsrV3,
                ModelType::Speech
            )
        )
    }

    pub const fn required_auth(self) -> AuthScheme {
        match self {
            Self::KlingApi => AuthScheme::AccessKeySecret,
            Self::VolcengineTtsV3 | Self::VolcengineAsrV3 => AuthScheme::ApiKey,
            Self::OpenAiResponses
            | Self::OpenAiChatCompletions
            | Self::OpenAiImages
            | Self::OpenAiAudioSpeech
            | Self::VolcengineArkImages
            | Self::VolcengineArkVideo
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
            "volcengine_ark_images" => Ok(Self::VolcengineArkImages),
            "volcengine_ark_video" => Ok(Self::VolcengineArkVideo),
            "runway_api" => Ok(Self::RunwayApi),
            "kling_api" => Ok(Self::KlingApi),
            "volcengine_tts_v3" => Ok(Self::VolcengineTtsV3),
            "openai_audio_speech" => Ok(Self::OpenAiAudioSpeech),
            "volcengine_asr_v3" => Ok(Self::VolcengineAsrV3),
            _ => Err(ModelSettingsError::InvalidProtocol(value.to_string())),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TextModelSettings {
    pub context_window: u64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImageModelSettings {
    #[serde(default)]
    pub supported_sizes: Vec<String>,
    #[serde(default)]
    pub default_size: Option<String>,
    #[serde(default)]
    pub max_images_per_request: Option<u32>,
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
    #[serde(default)]
    pub max_reference_images: Option<u32>,
    #[serde(default)]
    pub reference_image_mode: Option<String>,
    #[serde(default)]
    pub max_prompt_chars: Option<u32>,
    #[serde(default)]
    pub generate_audio: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechModelSettings {
    #[serde(default)]
    pub resource_id: String,
    #[serde(default)]
    pub supported_audio_formats: Vec<String>,
    #[serde(default)]
    pub default_audio_format: Option<String>,
    #[serde(default)]
    pub supported_sample_rates: Vec<u32>,
    #[serde(default)]
    pub default_sample_rate: Option<u32>,
    #[serde(default)]
    pub max_input_characters: Option<u32>,
    #[serde(default)]
    pub max_audio_duration_seconds: Option<u32>,
    #[serde(default)]
    pub supports_word_timestamps: bool,
    #[serde(default)]
    pub word_timestamp_languages: Vec<String>,
    #[serde(default)]
    pub catalog_sync_interval_minutes: Option<u32>,
    #[serde(default = "empty_object")]
    pub parameters: Value,
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "model_type", content = "settings", rename_all = "snake_case")]
pub enum ModelSettings {
    Text(TextModelSettings),
    Image(ImageModelSettings),
    Video(VideoModelSettings),
    Speech(SpeechModelSettings),
}

impl ModelSettings {
    pub fn parse(model_type: ModelType, value: Value) -> Result<Self, ModelSettingsError> {
        let settings = match model_type {
            ModelType::Text => Self::Text(serde_json::from_value(value)?),
            ModelType::Image => Self::Image(serde_json::from_value(value)?),
            ModelType::Video => Self::Video(serde_json::from_value(value)?),
            ModelType::Speech => Self::Speech(serde_json::from_value(value)?),
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

    pub fn speech_resource_id(&self) -> Option<&str> {
        match self {
            Self::Speech(settings) => Some(settings.resource_id.as_str()),
            _ => None,
        }
    }

    fn validate(&self) -> Result<(), ModelSettingsError> {
        match self {
            Self::Text(settings) => {
                if settings.context_window == 0 {
                    return Err(ModelSettingsError::InvalidSettings(
                        "text context_window must be positive".to_string(),
                    ));
                }
                Ok(())
            }
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
                if settings
                    .reference_image_mode
                    .as_deref()
                    .is_some_and(|value| !matches!(value, "first_last_frames" | "multi_reference"))
                {
                    return Err(ModelSettingsError::InvalidSettings(
                        "video reference_image_mode must be first_last_frames or multi_reference"
                            .to_string(),
                    ));
                }
                if settings
                    .max_reference_images
                    .is_some_and(|value| !(1..=9).contains(&value))
                {
                    return Err(ModelSettingsError::InvalidSettings(
                        "video max_reference_images must be between 1 and 9".to_string(),
                    ));
                }
                if settings
                    .max_prompt_chars
                    .is_some_and(|value| value == 0 || value > 500)
                {
                    return Err(ModelSettingsError::InvalidSettings(
                        "video max_prompt_chars must be between 1 and 500".to_string(),
                    ));
                }
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
            Self::Speech(settings) => {
                if settings.resource_id.trim().is_empty() {
                    return Err(ModelSettingsError::InvalidSettings(
                        "speech resource_id is required".to_string(),
                    ));
                }
                if settings.supported_audio_formats.is_empty() {
                    return Err(ModelSettingsError::InvalidSettings(
                        "speech supported_audio_formats is required".to_string(),
                    ));
                }
                if let Some(default_format) = &settings.default_audio_format {
                    if !settings.supported_audio_formats.contains(default_format) {
                        return Err(ModelSettingsError::InvalidSettings(
                            "speech default_audio_format must be supported".to_string(),
                        ));
                    }
                }
                if settings.supported_sample_rates.contains(&0) {
                    return Err(ModelSettingsError::InvalidSettings(
                        "speech sample rates must be positive".to_string(),
                    ));
                }
                if let Some(default_rate) = settings.default_sample_rate {
                    if !settings.supported_sample_rates.contains(&default_rate) {
                        return Err(ModelSettingsError::InvalidSettings(
                            "speech default_sample_rate must be supported".to_string(),
                        ));
                    }
                }
                if settings.max_input_characters == Some(0)
                    || settings.max_audio_duration_seconds == Some(0)
                    || settings.catalog_sync_interval_minutes == Some(0)
                {
                    return Err(ModelSettingsError::InvalidSettings(
                        "speech limits must be positive".to_string(),
                    ));
                }
                if settings.supports_word_timestamps
                    && (settings.word_timestamp_languages.is_empty()
                        || settings
                            .word_timestamp_languages
                            .iter()
                            .any(|language| language.trim().is_empty()))
                {
                    return Err(ModelSettingsError::InvalidSettings(
                        "speech word_timestamp_languages must be explicit when timestamps are supported"
                            .to_string(),
                    ));
                }
                if !settings.supports_word_timestamps
                    && !settings.word_timestamp_languages.is_empty()
                {
                    return Err(ModelSettingsError::InvalidSettings(
                        "speech word_timestamp_languages require timestamp support".to_string(),
                    ));
                }
                if !settings.parameters.is_object() {
                    return Err(ModelSettingsError::InvalidSettings(
                        "speech parameters must be an object".to_string(),
                    ));
                }
                Ok(())
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
