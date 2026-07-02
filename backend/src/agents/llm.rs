use crate::agents::models::GenerateScriptRequest;
use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptPrompt {
    pub system: String,
    pub user: String,
}

pub struct ScriptPromptBuilder;

impl ScriptPromptBuilder {
    pub fn build(request: &GenerateScriptRequest) -> ScriptPrompt {
        let style = request.style_or_default();
        let scene_count = request.scene_count_or_default();
        let variant_instruction = if request.parent_id.is_some() {
            "\n6. 这是 A/B 测试的差异化版本，必须避免复用相同表达、相同开场结构和相同分镜节奏。"
        } else {
            ""
        };

        ScriptPrompt {
            system: "你是专业的短视频脚本创作者，擅长创作15-60秒的抖音/小红书短视频脚本。你必须只输出合法 JSON，不要输出解释、Markdown 或额外文本。".to_string(),
            user: format!(
                r#"请根据以下选题生成{scene_count}个分镜的中文短视频脚本。

选题：{topic}
风格：{style_label}（{style_code}）

输出要求：
1. 标题不超过30个中文字符。
2. hook 必须能在前3秒抓住观众注意力。
3. 必须严格输出 {scene_count} 个分镜，sequence 从 1 连续递增。
4. 每个分镜包含 narration、visual_description、emotion、duration_sec。
5. 每个分镜 duration_sec 为 1-30 秒，总时长建议 45-60 秒。{variant_instruction}

JSON Schema：
{{
  "title": "标题",
  "hook": "前3秒吸引点",
  "scenes": [
    {{
      "sequence": 1,
      "narration": "旁白文本",
      "visual_description": "视觉描述",
      "emotion": "情绪标签",
      "duration_sec": 8
    }}
  ]
}}"#,
                scene_count = scene_count,
                topic = request.topic,
                style_label = style.label(),
                style_code = style.as_str(),
                variant_instruction = variant_instruction,
            ),
        }
    }
}

#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn generate_script(&self, prompt: ScriptPrompt) -> Result<String, LLMError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LLMError {
    Config(String),
    Timeout,
    Provider(String),
    Transport(String),
}

impl fmt::Display for LLMError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(message) => write!(formatter, "llm config error: {message}"),
            Self::Timeout => write!(formatter, "llm request timeout"),
            Self::Provider(message) => write!(formatter, "llm provider error: {message}"),
            Self::Transport(message) => write!(formatter, "llm transport error: {message}"),
        }
    }
}

impl std::error::Error for LLMError {}

#[derive(Clone, Debug)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub timeout_seconds: u64,
}

impl OpenAIConfig {
    pub fn from_env() -> Self {
        Self {
            api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
            base_url: std::env::var("OPENAI_BASE_URL")
                .unwrap_or_else(|_| "https://api.openai.com/v1".to_string()),
            model: std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4-turbo".to_string()),
            timeout_seconds: std::env::var("OPENAI_TIMEOUT_SECONDS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(30),
        }
    }
}

#[derive(Clone)]
pub struct OpenAIClient {
    config: OpenAIConfig,
    http_client: reqwest::Client,
}

impl OpenAIClient {
    pub fn new(config: OpenAIConfig) -> Result<Self, LLMError> {
        if config.api_key.trim().is_empty() {
            return Err(LLMError::Config("OPENAI_API_KEY is required".to_string()));
        }

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .map_err(|error| LLMError::Config(error.to_string()))?;

        Ok(Self {
            config,
            http_client,
        })
    }
}

#[async_trait]
impl LLMClient for OpenAIClient {
    async fn generate_script(&self, prompt: ScriptPrompt) -> Result<String, LLMError> {
        let endpoint = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );
        let payload = OpenAIChatCompletionRequest {
            model: self.config.model.clone(),
            temperature: 0.8,
            response_format: OpenAIResponseFormat {
                response_type: "json_object".to_string(),
            },
            messages: vec![
                OpenAIMessage {
                    role: "system".to_string(),
                    content: prompt.system,
                },
                OpenAIMessage {
                    role: "user".to_string(),
                    content: prompt.user,
                },
            ],
        };

        let response = self
            .http_client
            .post(endpoint)
            .bearer_auth(&self.config.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    LLMError::Timeout
                } else {
                    LLMError::Transport(error.to_string())
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(LLMError::Provider(format_provider_error(status, &body)));
        }

        let body: OpenAIChatCompletionResponse = response
            .json()
            .await
            .map_err(|error| LLMError::Provider(error.to_string()))?;
        body.choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .filter(|content| !content.trim().is_empty())
            .ok_or_else(|| LLMError::Provider("missing assistant content".to_string()))
    }
}

fn format_provider_error(status: StatusCode, body: &str) -> String {
    if body.trim().is_empty() {
        status.to_string()
    } else {
        format!("{status}: {body}")
    }
}

#[derive(Serialize)]
struct OpenAIChatCompletionRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    temperature: f32,
    #[serde(rename = "response_format")]
    response_format: OpenAIResponseFormat,
}

#[derive(Serialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct OpenAIResponseFormat {
    #[serde(rename = "type")]
    response_type: String,
}

#[derive(Deserialize)]
struct OpenAIChatCompletionResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: OpenAIChoiceMessage,
}

#[derive(Deserialize)]
struct OpenAIChoiceMessage {
    content: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ScriptLLMOutput {
    pub title: String,
    pub hook: String,
    pub scenes: Vec<ScriptLLMScene>,
}

impl ScriptLLMOutput {
    pub fn parse_and_validate(raw: &str, expected_scene_count: u8) -> Result<Self, LLMOutputError> {
        let json_text = extract_json_object(raw)?;
        let mut output: Self =
            serde_json::from_str(json_text).map_err(|error| LLMOutputError::InvalidJson {
                message: error.to_string(),
            })?;

        output.title = output.title.trim().to_string();
        output.hook = output.hook.trim().to_string();
        if output.title.is_empty() {
            return Err(LLMOutputError::Validation(
                "title must not be empty".to_string(),
            ));
        }
        if output.title.chars().count() > 30 {
            return Err(LLMOutputError::Validation(
                "title must be 30 characters or fewer".to_string(),
            ));
        }
        if output.hook.is_empty() {
            return Err(LLMOutputError::Validation(
                "hook must not be empty".to_string(),
            ));
        }
        if output.scenes.len() != usize::from(expected_scene_count) {
            return Err(LLMOutputError::Validation(format!(
                "expected {expected_scene_count} scenes, got {}",
                output.scenes.len()
            )));
        }

        output.scenes.sort_by_key(|scene| scene.sequence);
        for (index, scene) in output.scenes.iter().enumerate() {
            let expected_sequence = i32::try_from(index + 1).unwrap_or(i32::MAX);
            if scene.sequence != expected_sequence {
                return Err(LLMOutputError::Validation(format!(
                    "scene sequence must be contiguous from 1; expected {expected_sequence}, got {}",
                    scene.sequence
                )));
            }
            scene.validate()?;
        }

        Ok(output)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ScriptLLMScene {
    pub sequence: i32,
    pub narration: String,
    pub visual_description: String,
    pub emotion: String,
    pub duration_sec: i32,
}

impl ScriptLLMScene {
    fn validate(&self) -> Result<(), LLMOutputError> {
        if self.narration.trim().is_empty() {
            return Err(LLMOutputError::Validation(format!(
                "scene {} narration must not be empty",
                self.sequence
            )));
        }
        let narration_length = self.narration.trim().chars().count();
        if !(50..=150).contains(&narration_length) {
            return Err(LLMOutputError::Validation(format!(
                "scene {} narration must be between 50 and 150 characters",
                self.sequence
            )));
        }
        if self.visual_description.trim().is_empty() {
            return Err(LLMOutputError::Validation(format!(
                "scene {} visual_description must not be empty",
                self.sequence
            )));
        }
        if self.emotion.trim().is_empty() {
            return Err(LLMOutputError::Validation(format!(
                "scene {} emotion must not be empty",
                self.sequence
            )));
        }
        if !(1..=30).contains(&self.duration_sec) {
            return Err(LLMOutputError::Validation(format!(
                "scene {} duration_sec must be between 1 and 30",
                self.sequence
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LLMOutputError {
    InvalidJson { message: String },
    Validation(String),
}

impl fmt::Display for LLMOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { message } => write!(formatter, "invalid JSON: {message}"),
            Self::Validation(message) => write!(formatter, "invalid script output: {message}"),
        }
    }
}

impl std::error::Error for LLMOutputError {}

fn extract_json_object(raw: &str) -> Result<&str, LLMOutputError> {
    let start = raw.find('{').ok_or_else(|| LLMOutputError::InvalidJson {
        message: "missing JSON object start".to_string(),
    })?;
    let end = raw.rfind('}').ok_or_else(|| LLMOutputError::InvalidJson {
        message: "missing JSON object end".to_string(),
    })?;
    if start > end {
        return Err(LLMOutputError::InvalidJson {
            message: "invalid JSON object bounds".to_string(),
        });
    }
    Ok(&raw[start..=end])
}
