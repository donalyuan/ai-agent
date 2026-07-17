use crate::repositories::{AiModelRepository, AiModelRepositoryError, PostgresAiModelRepository};
use async_trait::async_trait;
use novex_model::{
    ApiProtocol, LLMClient, ModelExecutionSnapshot, ModelType, OpenAIClient, OpenAIConfig,
};
use serde_json::json;
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct ResolvedTextClient {
    pub client: Arc<dyn LLMClient>,
    pub snapshot: ModelExecutionSnapshot,
}

#[async_trait]
pub trait ModelClientResolver: Send + Sync {
    async fn text_client(&self, model_id: Uuid) -> Result<ResolvedTextClient, ModelResolveError>;
}

#[derive(Clone)]
pub struct PostgresModelClientResolver {
    repository: PostgresAiModelRepository,
}

impl PostgresModelClientResolver {
    pub fn new(repository: PostgresAiModelRepository) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl ModelClientResolver for PostgresModelClientResolver {
    async fn text_client(&self, model_id: Uuid) -> Result<ResolvedTextClient, ModelResolveError> {
        let runtime = self
            .repository
            .resolve_enabled(model_id, ModelType::Text)
            .await
            .map_err(|error| ModelResolveError::from_repository(error, model_id))?;
        let client = OpenAIClient::new(OpenAIConfig {
            api_protocol: runtime.snapshot.api_protocol,
            api_key: runtime.api_key,
            request_base_url: runtime.snapshot.request_base_url.clone(),
            upstream_model: runtime.snapshot.upstream_model.clone(),
            timeout_seconds: runtime.snapshot.timeout_seconds,
            responses_reasoning_effort: runtime.snapshot.reasoning_effort.clone(),
            responses_max_output_tokens: runtime.snapshot.max_output_tokens.unwrap_or(3000),
        })
        .map_err(|_| ModelResolveError::InvalidConfig(model_id))?;
        Ok(ResolvedTextClient {
            client: Arc::new(client),
            snapshot: runtime.snapshot,
        })
    }
}

/// Keeps existing unit tests injectable without providing a production runtime fallback.
pub(crate) struct StaticModelClientResolver {
    client: Arc<dyn LLMClient>,
}

impl StaticModelClientResolver {
    pub(crate) fn new(client: Arc<dyn LLMClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl ModelClientResolver for StaticModelClientResolver {
    async fn text_client(&self, model_id: Uuid) -> Result<ResolvedTextClient, ModelResolveError> {
        Ok(ResolvedTextClient {
            client: self.client.clone(),
            snapshot: ModelExecutionSnapshot {
                model_id,
                display_name: "Injected test model".to_string(),
                model_type: ModelType::Text,
                provider_name: "test".to_string(),
                api_protocol: ApiProtocol::OpenAiResponses,
                protocol_version: "test".to_string(),
                request_base_url: "https://example.invalid/v1".to_string(),
                upstream_model: "test-model".to_string(),
                reasoning_effort: None,
                timeout_seconds: 5,
                max_output_tokens: Some(3000),
                settings: json!({}),
            },
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ModelResolveError {
    NotFound(Uuid),
    Disabled(Uuid),
    TypeMismatch {
        id: Uuid,
        expected: ModelType,
        actual: ModelType,
    },
    InvalidConfig(Uuid),
    Storage,
}

impl ModelResolveError {
    pub(crate) fn from_repository(error: AiModelRepositoryError, requested_id: Uuid) -> Self {
        match error {
            AiModelRepositoryError::NotFound(id) => Self::NotFound(id),
            AiModelRepositoryError::Disabled(id) => Self::Disabled(id),
            AiModelRepositoryError::TypeMismatch {
                id,
                expected,
                actual,
            } => Self::TypeMismatch {
                id,
                expected,
                actual,
            },
            AiModelRepositoryError::InvalidConfig(_) => Self::InvalidConfig(requested_id),
            AiModelRepositoryError::VersionConflict(_)
            | AiModelRepositoryError::ReplacementRequired(_)
            | AiModelRepositoryError::InvalidReplacement(_)
            | AiModelRepositoryError::NoDefaultConfirmation(_)
            | AiModelRepositoryError::VoiceCatalogSourceInUse(_)
            | AiModelRepositoryError::Storage(_) => Self::Storage,
        }
    }
}

impl fmt::Display for ModelResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(id) => write!(formatter, "model not found: {id}"),
            Self::Disabled(id) => write!(formatter, "model disabled: {id}"),
            Self::TypeMismatch {
                id,
                expected,
                actual,
            } => write!(
                formatter,
                "model type mismatch for {id}: expected {expected}, got {actual}"
            ),
            Self::InvalidConfig(id) => write!(formatter, "invalid model config: {id}"),
            Self::Storage => formatter.write_str("model storage error"),
        }
    }
}

impl std::error::Error for ModelResolveError {}
