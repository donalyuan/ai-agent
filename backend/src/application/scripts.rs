//! 脚本用例，负责模型选择、Agent 组装和直接调用的 run 生命周期。

use crate::agents::{
    AuditedScriptModelExecutor, ScriptAgentError, ScriptAgentService, ScriptGenerationMode,
    ScriptListResult,
};
use crate::application::agents::adapters::AgentRuntimeError;
use crate::application::agents::kernel::{
    active_rust_definition_binding, run_lifecycle, run_lifecycle_error,
};
use crate::domain::conversation::ModelBindingEvidence;
use crate::domain::script::{Script, ScriptGenerationInput, ScriptListFilter, ScriptStatus};
use crate::model_routing::{model_behavior_evidence, ModelClientResolver, ModelResolveError};
use crate::repositories::{
    AgentBindingError, ConversationRepositoryError, PostgresConversationRepository,
    PostgresProjectRepository, PostgresScriptRepository, PostgresTopicRepository,
};
use novex_agent::{AuditedCallOwner, AuditedModelExecutor, FixedModelBinding, StartRun};
use novex_ai_core::{validate_model_capabilities, AgentKey, DefinitionRegistry};
use serde_json::json;
use std::{fmt, sync::Arc};
use uuid::Uuid;

#[derive(Clone)]
/// 组装脚本 Agent，并维护直接模型调用对应的 run 生命周期。
pub struct ScriptService {
    script_repository: PostgresScriptRepository,
    project_repository: PostgresProjectRepository,
    topic_repository: PostgresTopicRepository,
    conversation_repository: PostgresConversationRepository,
    model_resolver: Arc<dyn ModelClientResolver>,
    definition_registry: Arc<DefinitionRegistry>,
    audited_model_executor: Arc<AuditedModelExecutor>,
}

impl ScriptService {
    pub fn new(
        script_repository: PostgresScriptRepository,
        project_repository: PostgresProjectRepository,
        topic_repository: PostgresTopicRepository,
        conversation_repository: PostgresConversationRepository,
        model_resolver: Arc<dyn ModelClientResolver>,
        definition_registry: Arc<DefinitionRegistry>,
        audited_model_executor: Arc<AuditedModelExecutor>,
    ) -> Self {
        Self {
            script_repository,
            project_repository,
            topic_repository,
            conversation_repository,
            model_resolver,
            definition_registry,
            audited_model_executor,
        }
    }

    pub async fn generate(
        &self,
        model_id: Uuid,
        input: ScriptGenerationInput,
    ) -> Result<Script, ScriptApplicationError> {
        let project_id = input.project_id;
        let resolved = self.model_resolver.text_client(model_id).await?;
        let evidence = model_behavior_evidence(&resolved.snapshot)?;
        let agent = self
            .definition_registry
            .active_agent("video.script")
            .map_err(|error| ScriptApplicationError::Definition(error.to_string()))?;
        validate_model_capabilities(&agent.model_requirements, &evidence.capabilities)
            .map_err(|_| ScriptApplicationError::ModelCapabilityMismatch)?;
        let definition = active_rust_definition_binding(&self.definition_registry, "video.script")
            .map_err(ScriptApplicationError::Definition)?;
        let model_binding = ModelBindingEvidence {
            model_id,
            behavior_fingerprint: evidence.behavior_fingerprint.clone(),
            model_capabilities: serde_json::to_value(&evidence.capabilities)
                .map_err(|error| ScriptApplicationError::Serialization(error.to_string()))?,
        };
        let fixed_binding = FixedModelBinding {
            model_id,
            behavior_fingerprint: evidence.behavior_fingerprint,
        };
        let model_snapshot = serde_json::to_value(&resolved.snapshot)
            .map_err(|error| ScriptApplicationError::Serialization(error.to_string()))?;
        let generation_mode = script_generation_mode(resolved.snapshot.reasoning_effort.as_deref());
        let repository = self.conversation_repository.clone();
        let executor = self.audited_model_executor.clone();
        let script_repository = self.script_repository.clone();
        let project_repository = self.project_repository.clone();
        let topic_repository = self.topic_repository.clone();
        run_lifecycle(self.conversation_repository.clone())
            .execute(
                StartRun {
                    session_id: project_id,
                    project_id: Some(project_id),
                    agent_key: AgentKey::new("script").expect("script is a valid static AgentKey"),
                    input: json!({ "intent": "generate_script" }),
                    model_id: Some(model_id),
                    model_snapshot: Some(model_snapshot),
                },
                |run_id| async move {
                    repository
                        .create_run_binding(run_id, definition.clone(), model_binding, false)
                        .await?;
                    let model_executor = Arc::new(AuditedScriptModelExecutor::new(
                        executor,
                        AuditedCallOwner::AgentRun(run_id),
                        definition.agent_key,
                        definition.agent_version,
                        fixed_binding,
                    ));
                    let agent_service = ScriptAgentService::new(
                        model_executor,
                        Arc::new(script_repository),
                        Arc::new(project_repository),
                    )
                    .with_generation_mode(generation_mode)
                    .with_topic_repository(Arc::new(topic_repository));
                    agent_service
                        .generate(input)
                        .await
                        .map_err(ScriptApplicationError::from)
                },
                |script| Some(json!({ "script_id": script.id })),
                |error| error.to_string(),
            )
            .await
            .map_err(run_lifecycle_error)
    }

    pub async fn get(&self, script_id: Uuid) -> Result<Script, ScriptApplicationError> {
        self.read_service()
            .get_script(script_id)
            .await
            .map_err(Into::into)
    }

    pub async fn list(
        &self,
        project_id: Uuid,
        filter: ScriptListFilter,
    ) -> Result<ScriptListResult, ScriptApplicationError> {
        self.read_service()
            .list_scripts(project_id, filter)
            .await
            .map_err(Into::into)
    }

    pub async fn update_status(
        &self,
        script_id: Uuid,
        status: ScriptStatus,
    ) -> Result<Script, ScriptApplicationError> {
        self.read_service()
            .update_status(script_id, status)
            .await
            .map_err(Into::into)
    }

    fn read_service(&self) -> ScriptAgentService {
        ScriptAgentService::new(
            Arc::new(UnavailableScriptModelExecutor),
            Arc::new(self.script_repository.clone()),
            Arc::new(self.project_repository.clone()),
        )
        .with_generation_mode(ScriptGenerationMode::Complete)
    }
}

fn script_generation_mode(reasoning_effort: Option<&str>) -> ScriptGenerationMode {
    match reasoning_effort {
        Some(effort) if effort.eq_ignore_ascii_case("xhigh") => {
            ScriptGenerationMode::StepwiseSingleScene
        }
        _ => ScriptGenerationMode::Complete,
    }
}

struct UnavailableScriptModelExecutor;

#[async_trait::async_trait]
impl crate::agents::ScriptModelExecutor for UnavailableScriptModelExecutor {
    async fn execute(
        &self,
        _call: crate::agents::ScriptModelCall,
    ) -> Result<crate::agents::ScriptModelResponse, crate::agents::ScriptModelExecutionError> {
        Err(crate::agents::ScriptModelExecutionError::Execution(
            "script model execution is unavailable for read operations".into(),
        ))
    }
}

#[derive(Debug)]
pub enum ScriptApplicationError {
    Agent(ScriptAgentError),
    Runtime(AgentRuntimeError),
    ConversationRepository(ConversationRepositoryError),
    ModelResolve(ModelResolveError),
    AgentBinding(AgentBindingError),
    ModelCapabilityMismatch,
    Definition(String),
    Serialization(String),
}

impl From<ScriptAgentError> for ScriptApplicationError {
    fn from(error: ScriptAgentError) -> Self {
        Self::Agent(error)
    }
}

impl From<AgentRuntimeError> for ScriptApplicationError {
    fn from(error: AgentRuntimeError) -> Self {
        Self::Runtime(error)
    }
}

impl From<ConversationRepositoryError> for ScriptApplicationError {
    fn from(error: ConversationRepositoryError) -> Self {
        Self::ConversationRepository(error)
    }
}

impl From<ModelResolveError> for ScriptApplicationError {
    fn from(error: ModelResolveError) -> Self {
        Self::ModelResolve(error)
    }
}

impl From<AgentBindingError> for ScriptApplicationError {
    fn from(error: AgentBindingError) -> Self {
        Self::AgentBinding(error)
    }
}

impl fmt::Display for ScriptApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Agent(error) => write!(formatter, "{error}"),
            Self::Runtime(error) => write!(formatter, "{error}"),
            Self::ConversationRepository(error) => write!(formatter, "{error}"),
            Self::ModelResolve(error) => write!(formatter, "{error}"),
            Self::AgentBinding(error) => write!(formatter, "{error}"),
            Self::ModelCapabilityMismatch => formatter.write_str("model_capability_mismatch"),
            Self::Definition(message) => write!(formatter, "definition error: {message}"),
            Self::Serialization(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ScriptApplicationError {}
