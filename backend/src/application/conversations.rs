//! 连续对话用例，负责会话绑定规则、模型选择和统一 Runtime 组装。

use crate::agents::ScriptAgentError;
use crate::application::agents::runtime::{
    AgentRuntime, AgentRuntimeError, AgentTurnRequest, AgentTurnResponse,
};
use crate::domain::conversation::{AgentConversation, AgentMessage, CreateAgentConversationInput};
use crate::model_routing::{ModelClientResolver, ModelResolveError};
use crate::repositories::{
    ConversationRepository, ConversationRepositoryError, PostgresConversationRepository,
    PostgresProjectRepository, PostgresScriptRepository, PostgresTopicRepository,
    ProjectRepository, ProjectRepositoryError, ScriptRepository,
};
use serde_json::Value;
use std::{fmt, sync::Arc};
use uuid::Uuid;

#[derive(Clone)]
/// 管理 Agent 会话绑定，并将消息交给统一 Runtime 执行。
pub struct ConversationService {
    conversation_repository: PostgresConversationRepository,
    script_repository: PostgresScriptRepository,
    project_repository: PostgresProjectRepository,
    topic_repository: PostgresTopicRepository,
    model_resolver: Arc<dyn ModelClientResolver>,
}

impl ConversationService {
    pub fn new(
        conversation_repository: PostgresConversationRepository,
        script_repository: PostgresScriptRepository,
        project_repository: PostgresProjectRepository,
        topic_repository: PostgresTopicRepository,
        model_resolver: Arc<dyn ModelClientResolver>,
    ) -> Self {
        Self {
            conversation_repository,
            script_repository,
            project_repository,
            topic_repository,
            model_resolver,
        }
    }

    pub async fn create(
        &self,
        command: CreateConversationCommand,
    ) -> Result<AgentConversation, ConversationApplicationError> {
        if matches!(command.agent_type.as_str(), "script" | "topic") {
            let project_id = command.project_id.ok_or_else(|| {
                ConversationApplicationError::Validation("Agent 会话必须绑定项目".to_string())
            })?;
            if command.agent_type == "script" {
                if let Some(script_id) = command.subject_id {
                    let script = self
                        .script_repository
                        .get_script(script_id)
                        .await
                        .map_err(ScriptAgentError::from)?;
                    if script.project_id != project_id {
                        return Err(ConversationApplicationError::Validation(
                            "脚本不属于当前项目".to_string(),
                        ));
                    }
                } else {
                    self.ensure_project_exists(project_id).await?;
                }
            } else {
                self.ensure_project_exists(project_id).await?;
            }
        }

        self.conversation_repository
            .create_conversation(CreateAgentConversationInput {
                project_id: command.project_id,
                agent_type: command.agent_type,
                subject_type: command.subject_type,
                subject_id: command.subject_id,
                title: command.title,
                metadata: command.metadata,
            })
            .await
            .map_err(Into::into)
    }

    pub async fn list_messages(
        &self,
        conversation_id: Uuid,
    ) -> Result<Vec<AgentMessage>, ConversationApplicationError> {
        self.conversation_repository
            .get_conversation(conversation_id)
            .await?;
        self.conversation_repository
            .list_messages(conversation_id)
            .await
            .map_err(Into::into)
    }

    pub async fn send_message(
        &self,
        conversation_id: Uuid,
        model_id: Uuid,
        content: String,
        supplement_of_batch_id: Option<Uuid>,
    ) -> Result<AgentTurnResponse, ConversationApplicationError> {
        let resolved = self.model_resolver.text_client(model_id).await?;
        self.runtime(resolved.client, resolved.snapshot)
            .handle_turn(AgentTurnRequest {
                conversation_id,
                user_message: content,
                supplement_of_batch_id,
            })
            .await
            .map_err(Into::into)
    }

    async fn ensure_project_exists(
        &self,
        project_id: Uuid,
    ) -> Result<(), ConversationApplicationError> {
        if self.project_repository.project_exists(project_id).await? {
            Ok(())
        } else {
            Err(ScriptAgentError::ProjectNotFound(project_id).into())
        }
    }

    fn runtime(
        &self,
        llm_client: Arc<dyn novex_model::LLMClient>,
        model_execution: novex_model::ModelExecutionSnapshot,
    ) -> AgentRuntime {
        AgentRuntime::new(
            Arc::new(self.conversation_repository.clone()),
            Arc::new(self.script_repository.clone()),
            Arc::new(self.project_repository.clone()),
            llm_client,
        )
        .with_model_execution(model_execution)
        .with_topic_repository(Arc::new(self.topic_repository.clone()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateConversationCommand {
    pub project_id: Option<Uuid>,
    pub agent_type: String,
    pub subject_type: Option<String>,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub metadata: Value,
}

#[derive(Debug)]
pub enum ConversationApplicationError {
    ConversationRepository(ConversationRepositoryError),
    ProjectRepository(ProjectRepositoryError),
    Agent(ScriptAgentError),
    Runtime(AgentRuntimeError),
    ModelResolve(ModelResolveError),
    Validation(String),
}

impl From<ConversationRepositoryError> for ConversationApplicationError {
    fn from(error: ConversationRepositoryError) -> Self {
        Self::ConversationRepository(error)
    }
}

impl From<ProjectRepositoryError> for ConversationApplicationError {
    fn from(error: ProjectRepositoryError) -> Self {
        Self::ProjectRepository(error)
    }
}

impl From<ScriptAgentError> for ConversationApplicationError {
    fn from(error: ScriptAgentError) -> Self {
        Self::Agent(error)
    }
}

impl From<AgentRuntimeError> for ConversationApplicationError {
    fn from(error: AgentRuntimeError) -> Self {
        Self::Runtime(error)
    }
}

impl From<ModelResolveError> for ConversationApplicationError {
    fn from(error: ModelResolveError) -> Self {
        Self::ModelResolve(error)
    }
}

impl fmt::Display for ConversationApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConversationRepository(error) => write!(formatter, "{error}"),
            Self::ProjectRepository(error) => write!(formatter, "{error}"),
            Self::Agent(error) => write!(formatter, "{error}"),
            Self::Runtime(error) => write!(formatter, "{error}"),
            Self::ModelResolve(error) => write!(formatter, "{error}"),
            Self::Validation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ConversationApplicationError {}
