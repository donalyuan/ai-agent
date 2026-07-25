use async_trait::async_trait;
use novex_ai_core::{
    validate_model_capabilities, DefinitionRegistry, ModelCapabilities, PromptCompileInput,
    PromptCompiler, PromptSnapshot,
};
use novex_model::{LLMClient, LLMError, LLMJsonSchema, LLMPrompt};
use serde_json::{json, Value};
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;

use crate::BoxError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditedCallOwner {
    Conversation(Uuid),
    AgentRun(Uuid),
    EvalRun(Uuid),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FixedModelBinding {
    pub model_id: Uuid,
    pub behavior_fingerprint: String,
}

#[derive(Clone)]
pub struct ResolvedBoundModel {
    pub client: Arc<dyn LLMClient>,
    pub model_id: Uuid,
    pub behavior_fingerprint: String,
    pub capabilities: ModelCapabilities,
    pub model_snapshot: Value,
    pub known_secrets: Vec<String>,
}

#[async_trait]
pub trait BoundModelResolver: Send + Sync {
    async fn resolve(&self, model_id: Uuid) -> Result<ResolvedBoundModel, BoxError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrepareAuditedCall {
    pub owner: AuditedCallOwner,
    pub root_call_id: Option<Uuid>,
    pub parent_call_id: Option<Uuid>,
    pub attempt: i32,
    pub snapshot: PromptSnapshot,
    pub model_id: Uuid,
    pub behavior_fingerprint: String,
    pub model_snapshot: Value,
    pub context_sources: Value,
    pub memory_sources: Value,
    pub parameters: Value,
    pub asset_references: Value,
    pub known_secrets: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditedTerminalStatus {
    Succeeded,
    Failed,
    Aborted,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FinishAuditedCall {
    pub id: Uuid,
    pub status: AuditedTerminalStatus,
    pub output_snapshot: Option<Value>,
    pub usage_snapshot: Option<Value>,
    pub error_snapshot: Option<Value>,
    pub structured_parse_status: Option<String>,
    pub known_secrets: Vec<String>,
}

#[async_trait]
pub trait ModelCallAuditStore: Send + Sync {
    async fn prepare(&self, input: PrepareAuditedCall) -> Result<Uuid, BoxError>;
    async fn associate_step(&self, model_call_id: Uuid, step_id: Uuid) -> Result<(), BoxError>;
    async fn finish(&self, input: FinishAuditedCall) -> Result<(), BoxError>;
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuditedModelRequest {
    pub owner: AuditedCallOwner,
    pub step_id: Option<Uuid>,
    pub root_call_id: Option<Uuid>,
    pub parent_call_id: Option<Uuid>,
    pub attempt: i32,
    pub agent_key: String,
    pub agent_version: String,
    pub node_key: String,
    pub compile_input: PromptCompileInput,
    pub tool_profile: String,
    pub tool_schema: Option<Value>,
    pub binding: FixedModelBinding,
    pub context_sources: Value,
    pub memory_sources: Value,
    pub parameters: Value,
    pub asset_references: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuditedModelResponse {
    pub model_call_id: Uuid,
    pub output: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuditedParsedModelResponse<T> {
    pub model_call_id: Uuid,
    pub output: T,
}

struct ProviderOutput {
    model_call_id: Uuid,
    output: String,
    known_secrets: Vec<String>,
}

pub struct AuditedModelExecutor {
    registry: Arc<DefinitionRegistry>,
    models: Arc<dyn BoundModelResolver>,
    audit: Arc<dyn ModelCallAuditStore>,
}

impl AuditedModelExecutor {
    pub fn new(
        registry: Arc<DefinitionRegistry>,
        models: Arc<dyn BoundModelResolver>,
        audit: Arc<dyn ModelCallAuditStore>,
    ) -> Self {
        Self {
            registry,
            models,
            audit,
        }
    }

    pub async fn associate_step(
        &self,
        model_call_id: Uuid,
        step_id: Uuid,
    ) -> Result<(), AuditedModelError> {
        self.audit
            .associate_step(model_call_id, step_id)
            .await
            .map_err(AuditedModelError::FinishAudit)
    }

    pub async fn execute(
        &self,
        request: AuditedModelRequest,
    ) -> Result<AuditedModelResponse, AuditedModelError> {
        let provider = self.execute_provider(request).await?;
        self.audit
            .finish(FinishAuditedCall {
                id: provider.model_call_id,
                status: AuditedTerminalStatus::Succeeded,
                output_snapshot: Some(json!({ "text": provider.output })),
                usage_snapshot: None,
                error_snapshot: None,
                structured_parse_status: None,
                known_secrets: provider.known_secrets,
            })
            .await
            .map_err(AuditedModelError::FinishAudit)?;
        Ok(AuditedModelResponse {
            model_call_id: provider.model_call_id,
            output: provider.output,
        })
    }

    pub async fn execute_parsed<T, Parse>(
        &self,
        request: AuditedModelRequest,
        parse: Parse,
    ) -> Result<AuditedParsedModelResponse<T>, AuditedModelError>
    where
        Parse: FnOnce(&str) -> Result<T, String>,
    {
        let provider = self.execute_provider(request).await?;
        match parse(&provider.output) {
            Ok(output) => {
                self.audit
                    .finish(FinishAuditedCall {
                        id: provider.model_call_id,
                        status: AuditedTerminalStatus::Succeeded,
                        output_snapshot: Some(json!({ "text": provider.output })),
                        usage_snapshot: None,
                        error_snapshot: None,
                        structured_parse_status: Some("succeeded".into()),
                        known_secrets: provider.known_secrets,
                    })
                    .await
                    .map_err(AuditedModelError::FinishAudit)?;
                Ok(AuditedParsedModelResponse {
                    model_call_id: provider.model_call_id,
                    output,
                })
            }
            Err(message) => {
                self.audit
                    .finish(FinishAuditedCall {
                        id: provider.model_call_id,
                        status: AuditedTerminalStatus::Failed,
                        output_snapshot: Some(json!({ "text": provider.output })),
                        usage_snapshot: None,
                        error_snapshot: Some(json!({
                            "kind": "structured_parse",
                            "message": message,
                        })),
                        structured_parse_status: Some("failed".into()),
                        known_secrets: provider.known_secrets,
                    })
                    .await
                    .map_err(AuditedModelError::FinishAudit)?;
                Err(AuditedModelError::StructuredParse {
                    model_call_id: provider.model_call_id,
                    message,
                })
            }
        }
    }

    async fn execute_provider(
        &self,
        request: AuditedModelRequest,
    ) -> Result<ProviderOutput, AuditedModelError> {
        let snapshot = PromptCompiler::new(&self.registry)
            .compile(
                &request.agent_key,
                &request.agent_version,
                &request.node_key,
                request.compile_input,
                &request.tool_profile,
                request.tool_schema,
            )
            .map_err(|error| AuditedModelError::Compile(error.to_string()))?;
        let resolved = self
            .models
            .resolve(request.binding.model_id)
            .await
            .map_err(AuditedModelError::ModelResolution)?;
        if resolved.model_id != request.binding.model_id
            || resolved.behavior_fingerprint != request.binding.behavior_fingerprint
        {
            return Err(AuditedModelError::ModelRebindRequired);
        }
        let agent = self
            .registry
            .agent(&request.agent_key, &request.agent_version)
            .map_err(|error| AuditedModelError::Compile(error.to_string()))?;
        validate_model_capabilities(&agent.model_requirements, &resolved.capabilities)
            .map_err(|_| AuditedModelError::ModelCapabilityMismatch)?;
        let prompt = prompt_from_snapshot(&snapshot)?;
        let call_id = self
            .audit
            .prepare(PrepareAuditedCall {
                owner: request.owner,
                root_call_id: request.root_call_id,
                parent_call_id: request.parent_call_id,
                attempt: request.attempt,
                snapshot,
                model_id: resolved.model_id,
                behavior_fingerprint: resolved.behavior_fingerprint.clone(),
                model_snapshot: resolved.model_snapshot.clone(),
                context_sources: request.context_sources,
                memory_sources: request.memory_sources,
                parameters: request.parameters,
                asset_references: request.asset_references,
                known_secrets: resolved.known_secrets.clone(),
            })
            .await
            .map_err(AuditedModelError::PrepareAudit)?;
        if let Some(step_id) = request.step_id {
            self.audit
                .associate_step(call_id, step_id)
                .await
                .map_err(AuditedModelError::PrepareAudit)?;
        }

        match resolved.client.generate_script(prompt).await {
            Ok(output) => Ok(ProviderOutput {
                model_call_id: call_id,
                output,
                known_secrets: resolved.known_secrets,
            }),
            Err(error) => {
                self.audit
                    .finish(FinishAuditedCall {
                        id: call_id,
                        status: AuditedTerminalStatus::Failed,
                        output_snapshot: None,
                        usage_snapshot: None,
                        error_snapshot: Some(json!({
                            "kind": llm_error_kind(&error),
                            "message": error.to_string(),
                        })),
                        structured_parse_status: None,
                        known_secrets: resolved.known_secrets,
                    })
                    .await
                    .map_err(AuditedModelError::FinishAudit)?;
                Err(AuditedModelError::Provider {
                    model_call_id: call_id,
                    source: error,
                })
            }
        }
    }
}

fn prompt_from_snapshot(snapshot: &PromptSnapshot) -> Result<LLMPrompt, AuditedModelError> {
    let output_schema = snapshot
        .output_schema
        .as_ref()
        .map(|value| {
            let name = value.get("name").and_then(Value::as_str).ok_or_else(|| {
                AuditedModelError::Compile("output schema name is missing".into())
            })?;
            let strict = value
                .get("strict")
                .and_then(Value::as_bool)
                .ok_or_else(|| {
                    AuditedModelError::Compile("output schema strict is missing".into())
                })?;
            let schema = value.get("schema").cloned().ok_or_else(|| {
                AuditedModelError::Compile("output schema body is missing".into())
            })?;
            Ok(LLMJsonSchema {
                name: name.into(),
                strict,
                schema,
            })
        })
        .transpose()?;
    Ok(LLMPrompt {
        system: snapshot.system.clone(),
        user: snapshot.user.clone(),
        max_output_tokens: snapshot.max_output_tokens,
        output_schema,
    })
}

fn llm_error_kind(error: &LLMError) -> &'static str {
    match error {
        LLMError::Config(_) => "config",
        LLMError::Timeout => "timeout",
        LLMError::Provider(_) => "provider",
        LLMError::Transport(_) => "transport",
    }
}

#[derive(Debug)]
pub enum AuditedModelError {
    Compile(String),
    ModelResolution(BoxError),
    ModelRebindRequired,
    ModelCapabilityMismatch,
    PrepareAudit(BoxError),
    Provider {
        model_call_id: Uuid,
        source: LLMError,
    },
    StructuredParse {
        model_call_id: Uuid,
        message: String,
    },
    FinishAudit(BoxError),
}

impl fmt::Display for AuditedModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compile(message) => write!(formatter, "prompt compile failed: {message}"),
            Self::ModelResolution(error) => write!(formatter, "model resolution failed: {error}"),
            Self::ModelRebindRequired => formatter.write_str("model_rebind_required"),
            Self::ModelCapabilityMismatch => formatter.write_str("model_capability_mismatch"),
            Self::PrepareAudit(error) => write!(formatter, "audit_persistence_failed: {error}"),
            Self::Provider { source, .. } => write!(formatter, "{source}"),
            Self::StructuredParse { message, .. } => {
                write!(formatter, "structured output parse failed: {message}")
            }
            Self::FinishAudit(error) => write!(formatter, "audit finalization failed: {error}"),
        }
    }
}

impl std::error::Error for AuditedModelError {}

impl AuditedModelError {
    pub fn model_call_id(&self) -> Option<Uuid> {
        match self {
            Self::Provider { model_call_id, .. } | Self::StructuredParse { model_call_id, .. } => {
                Some(*model_call_id)
            }
            _ => None,
        }
    }

    pub fn provider_error(&self) -> Option<&LLMError> {
        match self {
            Self::Provider { source, .. } => Some(source),
            _ => None,
        }
    }
}
