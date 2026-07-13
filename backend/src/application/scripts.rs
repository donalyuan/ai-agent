//! 脚本用例，负责模型选择、Agent 组装和直接调用的 run 生命周期。

use crate::agents::{ScriptAgentError, ScriptAgentService, ScriptGenerationMode, ScriptListResult};
use crate::domain::conversation::{CreateAgentRunInput, FinishAgentRunInput};
use crate::domain::script::{Script, ScriptGenerationInput, ScriptListFilter, ScriptStatus};
use crate::model_routing::{ModelClientResolver, ModelResolveError};
use crate::repositories::{
    ConversationRepository, ConversationRepositoryError, PostgresConversationRepository,
    PostgresProjectRepository, PostgresScriptRepository, PostgresTopicRepository,
};
use novex_model::{LLMClient, LLMError, LLMPrompt};
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
}

impl ScriptService {
    pub fn new(
        script_repository: PostgresScriptRepository,
        project_repository: PostgresProjectRepository,
        topic_repository: PostgresTopicRepository,
        conversation_repository: PostgresConversationRepository,
        model_resolver: Arc<dyn ModelClientResolver>,
    ) -> Self {
        Self {
            script_repository,
            project_repository,
            topic_repository,
            conversation_repository,
            model_resolver,
        }
    }

    pub async fn generate(
        &self,
        model_id: Uuid,
        input: ScriptGenerationInput,
    ) -> Result<Script, ScriptApplicationError> {
        let project_id = input.project_id;
        let resolved = self.model_resolver.text_client(model_id).await?;
        let model_snapshot = serde_json::to_value(&resolved.snapshot)
            .map_err(|error| ScriptApplicationError::Serialization(error.to_string()))?;
        let run = self
            .conversation_repository
            .create_run(CreateAgentRunInput {
                conversation_id: project_id,
                project_id: Some(project_id),
                agent_type: "script".to_string(),
                input: json!({ "intent": "generate_script" }),
                model_id: Some(model_id),
                model_snapshot: Some(model_snapshot),
            })
            .await?;

        let generation_mode = script_generation_mode(resolved.snapshot.reasoning_effort.as_deref());
        let result = self
            .agent_service(resolved.client, generation_mode, true)
            .generate_script(input)
            .await;

        match result {
            Ok(script) => {
                self.finish_run(
                    run.id,
                    "succeeded",
                    Some(json!({ "script_id": script.id })),
                    None,
                )
                .await?;
                Ok(script)
            }
            Err(error) => {
                self.finish_run(run.id, "failed", None, Some(error.to_string()))
                    .await?;
                Err(error.into())
            }
        }
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
        self.agent_service(
            Arc::new(UnconfiguredLlmClient),
            ScriptGenerationMode::Complete,
            false,
        )
    }

    fn agent_service(
        &self,
        llm_client: Arc<dyn LLMClient>,
        generation_mode: ScriptGenerationMode,
        with_topic_repository: bool,
    ) -> ScriptAgentService {
        let service = ScriptAgentService::new(
            llm_client,
            Arc::new(self.script_repository.clone()),
            Arc::new(self.project_repository.clone()),
        )
        .with_generation_mode(generation_mode);

        if with_topic_repository {
            service.with_topic_repository(Arc::new(self.topic_repository.clone()))
        } else {
            service
        }
    }

    async fn finish_run(
        &self,
        run_id: Uuid,
        status: &str,
        output: Option<serde_json::Value>,
        error_message: Option<String>,
    ) -> Result<(), ScriptApplicationError> {
        self.conversation_repository
            .finish_run(FinishAgentRunInput {
                agent_run_id: run_id,
                status: status.to_string(),
                output,
                error_message,
            })
            .await
            .map(|_| ())
            .map_err(Into::into)
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

struct UnconfiguredLlmClient;

#[async_trait::async_trait]
impl LLMClient for UnconfiguredLlmClient {
    async fn generate_script(&self, _prompt: LLMPrompt) -> Result<String, LLMError> {
        Err(LLMError::Config(
            "LLM client is not configured for this route".to_string(),
        ))
    }
}

#[derive(Debug)]
pub enum ScriptApplicationError {
    Agent(ScriptAgentError),
    ConversationRepository(ConversationRepositoryError),
    ModelResolve(ModelResolveError),
    Serialization(String),
}

impl From<ScriptAgentError> for ScriptApplicationError {
    fn from(error: ScriptAgentError) -> Self {
        Self::Agent(error)
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

impl fmt::Display for ScriptApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Agent(error) => write!(formatter, "{error}"),
            Self::ConversationRepository(error) => write!(formatter, "{error}"),
            Self::ModelResolve(error) => write!(formatter, "{error}"),
            Self::Serialization(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ScriptApplicationError {}
