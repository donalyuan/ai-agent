use crate::repositories::{AiModelRepository, AiModelRepositoryError, PostgresAiModelRepository};
use async_trait::async_trait;
use novex_agent::{BoundModelResolver, ResolvedBoundModel};
use novex_ai_core::{behavior_fingerprint, ModelBehavior, ModelCapabilities};
use novex_model::{
    ApiProtocol, LLMClient, ModelExecutionSnapshot, ModelRuntimeConfig, ModelType, OpenAIClient,
    OpenAIConfig,
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

#[derive(Clone, Debug, PartialEq)]
pub struct ModelBehaviorEvidence {
    pub behavior_fingerprint: String,
    pub capabilities: ModelCapabilities,
}

pub fn model_behavior_evidence(
    snapshot: &ModelExecutionSnapshot,
) -> Result<ModelBehaviorEvidence, ModelResolveError> {
    if snapshot.model_type != ModelType::Text {
        return Err(ModelResolveError::TypeMismatch {
            id: snapshot.model_id,
            expected: ModelType::Text,
            actual: snapshot.model_type,
        });
    }
    let context_window = snapshot
        .settings
        .get("context_window")
        .and_then(serde_json::Value::as_u64)
        .filter(|value| *value > 0)
        .ok_or(ModelResolveError::InvalidConfig(snapshot.model_id))?;
    let max_output_tokens = snapshot
        .max_output_tokens
        .filter(|value| *value > 0)
        .ok_or(ModelResolveError::InvalidConfig(snapshot.model_id))?;
    let behavior = ModelBehavior {
        protocol: snapshot.api_protocol.as_str().into(),
        request_base_url: snapshot.request_base_url.clone(),
        upstream_model: snapshot.upstream_model.clone(),
        reasoning_effort: snapshot.reasoning_effort.clone(),
        max_output_tokens,
        context_window,
        settings: snapshot.settings.clone(),
    };
    let (behavior_fingerprint, _) = behavior_fingerprint(&behavior)
        .map_err(|_| ModelResolveError::InvalidConfig(snapshot.model_id))?;
    Ok(ModelBehaviorEvidence {
        behavior_fingerprint,
        capabilities: ModelCapabilities {
            text: true,
            tool_calling: false,
            structured_output: matches!(
                snapshot.api_protocol,
                ApiProtocol::OpenAiResponses | ApiProtocol::OpenAiChatCompletions
            ),
            vision: false,
            reasoning: snapshot.reasoning_effort.is_some(),
            context_window,
        },
    })
}

#[async_trait]
pub trait ModelClientResolver: BoundModelResolver + Send + Sync {
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
        let client = build_text_client(&runtime)?;
        Ok(ResolvedTextClient {
            client,
            snapshot: runtime.snapshot,
        })
    }
}

#[async_trait]
impl BoundModelResolver for PostgresModelClientResolver {
    async fn resolve(&self, model_id: Uuid) -> Result<ResolvedBoundModel, novex_agent::BoxError> {
        // Resolve from ai_models for every call so status, behavior and rotated credentials are current.
        let runtime = self
            .repository
            .resolve_enabled(model_id, ModelType::Text)
            .await
            .map_err(|error| ModelResolveError::from_repository(error, model_id))?;
        let evidence = model_behavior_evidence(&runtime.snapshot)?;
        let client = build_text_client(&runtime)?;
        let model_snapshot = serde_json::to_value(&runtime.snapshot)
            .map_err(|_| ModelResolveError::InvalidConfig(model_id))?;
        let known_secrets = std::iter::once(runtime.api_key.clone())
            .chain(runtime.api_secret.clone())
            .filter(|value| !value.is_empty())
            .collect();
        Ok(ResolvedBoundModel {
            client,
            model_id: runtime.snapshot.model_id,
            behavior_fingerprint: evidence.behavior_fingerprint,
            capabilities: evidence.capabilities,
            model_snapshot,
            known_secrets,
        })
    }
}

fn build_text_client(
    runtime: &ModelRuntimeConfig,
) -> Result<Arc<dyn LLMClient>, ModelResolveError> {
    let model_id = runtime.snapshot.model_id;
    let client = OpenAIClient::new(OpenAIConfig {
        api_protocol: runtime.snapshot.api_protocol,
        api_key: runtime.api_key.clone(),
        request_base_url: runtime.snapshot.request_base_url.clone(),
        upstream_model: runtime.snapshot.upstream_model.clone(),
        timeout_seconds: runtime.snapshot.timeout_seconds,
        responses_reasoning_effort: runtime.snapshot.reasoning_effort.clone(),
        responses_max_output_tokens: runtime.snapshot.max_output_tokens.unwrap_or(3000),
    })
    .map_err(|_| ModelResolveError::InvalidConfig(model_id))?;
    Ok(Arc::new(client))
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
                settings: json!({"context_window": 128000}),
            },
        })
    }
}

#[async_trait]
impl BoundModelResolver for StaticModelClientResolver {
    async fn resolve(&self, model_id: Uuid) -> Result<ResolvedBoundModel, novex_agent::BoxError> {
        let resolved = self.text_client(model_id).await?;
        let evidence = model_behavior_evidence(&resolved.snapshot)?;
        let model_snapshot = serde_json::to_value(&resolved.snapshot)
            .map_err(|_| ModelResolveError::InvalidConfig(model_id))?;
        Ok(ResolvedBoundModel {
            client: resolved.client,
            model_id,
            behavior_fingerprint: evidence.behavior_fingerprint,
            capabilities: evidence.capabilities,
            model_snapshot,
            known_secrets: Vec::new(),
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
