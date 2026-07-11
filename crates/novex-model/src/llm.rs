use async_trait::async_trait;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::time::Duration;

use crate::ApiProtocol;

#[derive(Clone, Debug, PartialEq)]
pub struct LLMPrompt {
    pub system: String,
    pub user: String,
    pub max_output_tokens: Option<u32>,
    pub output_schema: Option<LLMJsonSchema>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LLMJsonSchema {
    pub name: String,
    pub strict: bool,
    pub schema: Value,
}

const OPENAI_COMPATIBLE_USER_AGENT: &str = "codex-cli/0.142.5";

#[async_trait]
pub trait LLMClient: Send + Sync {
    async fn generate_script(&self, prompt: LLMPrompt) -> Result<String, LLMError>;
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
    pub api_protocol: ApiProtocol,
    pub api_key: String,
    pub request_base_url: String,
    pub upstream_model: String,
    pub timeout_seconds: u64,
    pub responses_reasoning_effort: Option<String>,
    pub responses_max_output_tokens: u32,
}

#[derive(Clone)]
pub struct OpenAIClient {
    config: OpenAIConfig,
    http_client: reqwest::Client,
}

impl OpenAIClient {
    pub fn new(config: OpenAIConfig) -> Result<Self, LLMError> {
        if !matches!(
            config.api_protocol,
            ApiProtocol::OpenAiResponses | ApiProtocol::OpenAiChatCompletions
        ) {
            return Err(LLMError::Config(format!(
                "unsupported text API protocol: {}",
                config.api_protocol
            )));
        }
        if config.api_key.trim().is_empty() {
            return Err(LLMError::Config("OPENAI_API_KEY is required".to_string()));
        }
        if config.request_base_url.trim().is_empty() {
            return Err(LLMError::Config(
                "request_base_url is required".to_string(),
            ));
        }
        if config.upstream_model.trim().is_empty() {
            return Err(LLMError::Config("upstream_model is required".to_string()));
        }

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .user_agent(OPENAI_COMPATIBLE_USER_AGENT)
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
    async fn generate_script(&self, prompt: LLMPrompt) -> Result<String, LLMError> {
        match self.config.api_protocol {
            ApiProtocol::OpenAiResponses => self.generate_script_with_responses(prompt).await,
            ApiProtocol::OpenAiChatCompletions => {
                self.generate_script_with_chat_completions(prompt).await
            }
            _ => Err(LLMError::Config(format!(
                "unsupported text API protocol: {}",
                self.config.api_protocol
            ))),
        }
    }
}

impl OpenAIClient {
    async fn generate_script_with_chat_completions(
        &self,
        prompt: LLMPrompt,
    ) -> Result<String, LLMError> {
        let endpoint = format!(
            "{}/chat/completions",
            self.config.request_base_url.trim_end_matches('/')
        );
        let response_format = OpenAIResponseFormat::for_chat(prompt.output_schema.as_ref());
        let payload = OpenAIChatCompletionRequest {
            model: self.config.upstream_model.clone(),
            temperature: 0.8,
            response_format,
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

    async fn generate_script_with_responses(&self, prompt: LLMPrompt) -> Result<String, LLMError> {
        let endpoint = format!(
            "{}/responses",
            self.config.request_base_url.trim_end_matches('/')
        );
        let response_format = OpenAIResponseFormat::for_responses(prompt.output_schema.as_ref());
        let payload = OpenAIResponsesRequest {
            model: self.config.upstream_model.clone(),
            temperature: 0.8,
            max_output_tokens: prompt
                .max_output_tokens
                .unwrap_or(self.config.responses_max_output_tokens),
            stream: true,
            input: vec![
                OpenAIResponsesMessage {
                    role: "system".to_string(),
                    content: vec![OpenAIResponsesContent {
                        content_type: "input_text".to_string(),
                        text: prompt.system,
                    }],
                },
                OpenAIResponsesMessage {
                    role: "user".to_string(),
                    content: vec![OpenAIResponsesContent {
                        content_type: "input_text".to_string(),
                        text: prompt.user,
                    }],
                },
            ],
            text: OpenAIResponsesTextConfig {
                format: response_format,
            },
            reasoning: self
                .config
                .responses_reasoning_effort
                .clone()
                .map(|effort| OpenAIResponsesReasoningConfig { effort }),
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

        let body = response
            .text()
            .await
            .map_err(|error| LLMError::Provider(error.to_string()))?;
        parse_responses_stream(&body)
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
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    json_schema: Option<OpenAIChatJsonSchema>,
}

impl OpenAIResponseFormat {
    fn for_chat(schema: Option<&LLMJsonSchema>) -> Self {
        match schema {
            Some(schema) => Self {
                response_type: "json_schema".to_string(),
                name: None,
                strict: None,
                schema: None,
                json_schema: Some(OpenAIChatJsonSchema {
                    name: schema.name.clone(),
                    strict: schema.strict,
                    schema: schema.schema.clone(),
                }),
            },
            None => Self {
                response_type: "json_object".to_string(),
                name: None,
                strict: None,
                schema: None,
                json_schema: None,
            },
        }
    }

    fn for_responses(schema: Option<&LLMJsonSchema>) -> Self {
        match schema {
            Some(schema) => Self {
                response_type: "json_schema".to_string(),
                name: Some(schema.name.clone()),
                strict: Some(schema.strict),
                schema: Some(schema.schema.clone()),
                json_schema: None,
            },
            None => Self {
                response_type: "json_object".to_string(),
                name: None,
                strict: None,
                schema: None,
                json_schema: None,
            },
        }
    }
}

#[derive(Serialize)]
struct OpenAIChatJsonSchema {
    name: String,
    strict: bool,
    schema: Value,
}

#[derive(Serialize)]
struct OpenAIResponsesRequest {
    model: String,
    input: Vec<OpenAIResponsesMessage>,
    temperature: f32,
    max_output_tokens: u32,
    stream: bool,
    text: OpenAIResponsesTextConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<OpenAIResponsesReasoningConfig>,
}

#[derive(Serialize)]
struct OpenAIResponsesMessage {
    role: String,
    content: Vec<OpenAIResponsesContent>,
}

#[derive(Serialize)]
struct OpenAIResponsesContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Serialize)]
struct OpenAIResponsesTextConfig {
    format: OpenAIResponseFormat,
}

#[derive(Serialize)]
struct OpenAIResponsesReasoningConfig {
    effort: String,
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

fn parse_responses_stream(body: &str) -> Result<String, LLMError> {
    let mut output = String::new();

    for line in body.lines() {
        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };
        if data == "[DONE]" {
            break;
        }

        let event: OpenAIResponsesStreamEvent =
            serde_json::from_str(data).map_err(|error| LLMError::Provider(error.to_string()))?;
        if event.event_type == "response.output_text.delta" {
            if let Some(delta) = event.delta {
                output.push_str(&delta);
            }
        }
    }

    if output.trim().is_empty() {
        Err(LLMError::Provider(
            "missing response output text".to_string(),
        ))
    } else {
        Ok(output)
    }
}

#[derive(Deserialize)]
struct OpenAIResponsesStreamEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<String>,
}
