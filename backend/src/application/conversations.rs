//! 连续对话用例，负责会话绑定规则、模型选择和统一 Runtime 组装。

use crate::agents::ScriptAgentError;
use crate::application::agents::adapters::{
    AgentRuntimeError, AgentTurnResponse, SoundAgentContext,
};
use crate::application::agents::kernel::AgentExecutor;
use crate::domain::conversation::{AgentConversation, AgentMessage, CreateAgentConversationInput};
use crate::model_routing::{ModelClientResolver, ModelResolveError};
use crate::repositories::AiModelRepository;
use crate::repositories::{
    ConversationRepository, ConversationRepositoryError, PostgresAiModelRepository,
    PostgresConversationRepository, PostgresProjectRepository, PostgresScriptRepository,
    PostgresVoiceCatalogRepository, PostgresWorkLibraryRepository, ProjectRepository,
    ProjectRepositoryError, ScriptRepository,
};
use novex_agent::{AgentInvocation, ModelExecutionRef};
use novex_model::{ApiProtocol, ModelType};
use serde_json::{json, Value};
use std::{fmt, sync::Arc};
use uuid::Uuid;

#[derive(Clone)]
/// 管理 Agent 会话绑定，并将消息交给统一 Runtime 执行。
pub struct ConversationService {
    conversation_repository: PostgresConversationRepository,
    script_repository: PostgresScriptRepository,
    project_repository: PostgresProjectRepository,
    ai_model_repository: PostgresAiModelRepository,
    voice_catalog_repository: PostgresVoiceCatalogRepository,
    work_library_repository: PostgresWorkLibraryRepository,
    model_resolver: Arc<dyn ModelClientResolver>,
    agent_executor: AgentExecutor,
}

impl ConversationService {
    pub fn new(
        conversation_repository: PostgresConversationRepository,
        script_repository: PostgresScriptRepository,
        project_repository: PostgresProjectRepository,
        ai_model_repository: PostgresAiModelRepository,
        voice_catalog_repository: PostgresVoiceCatalogRepository,
        work_library_repository: PostgresWorkLibraryRepository,
        model_resolver: Arc<dyn ModelClientResolver>,
        agent_registry: Arc<novex_agent::AgentRegistry>,
    ) -> Self {
        let agent_executor = AgentExecutor::new(agent_registry, conversation_repository.clone());
        Self {
            conversation_repository,
            script_repository,
            project_repository,
            ai_model_repository,
            voice_catalog_repository,
            work_library_repository,
            model_resolver,
            agent_executor,
        }
    }

    pub async fn create(
        &self,
        command: CreateConversationCommand,
    ) -> Result<AgentConversation, ConversationApplicationError> {
        if matches!(
            command.agent_type.as_str(),
            "script" | "topic" | "sound" | "work"
        ) {
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
            } else if command.agent_type == "topic" {
                self.ensure_project_exists(project_id).await?;
            } else if command.agent_type == "work" {
                if let Some(work_id) = command.subject_id {
                    if command.subject_type.as_deref() != Some("work") {
                        return Err(ConversationApplicationError::Validation(
                            "作品会话 subject_type 必须为 work".to_string(),
                        ));
                    }
                    if !self
                        .work_library_repository
                        .work_belongs_to_project(work_id, project_id)
                        .await
                        .map_err(|error| {
                            ConversationApplicationError::Validation(error.to_string())
                        })?
                    {
                        return Err(ConversationApplicationError::Validation(
                            "作品不属于当前项目".to_string(),
                        ));
                    }
                } else {
                    self.ensure_project_exists(project_id).await?;
                }
            } else {
                self.ensure_project_exists(project_id).await?;
                let model_id = command
                    .metadata
                    .get("speech_model_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok())
                    .ok_or_else(|| {
                        ConversationApplicationError::Validation(
                            "声音会话必须绑定有效 speech_model_id".to_string(),
                        )
                    })?;
                let runtime = self
                    .ai_model_repository
                    .resolve_enabled(model_id, ModelType::Speech)
                    .await
                    .map_err(|error| ConversationApplicationError::Validation(error.to_string()))?;
                if !matches!(
                    runtime.snapshot.api_protocol,
                    ApiProtocol::VolcengineTtsV3 | ApiProtocol::OpenAiAudioSpeech
                ) {
                    return Err(ConversationApplicationError::Validation(
                        "声音会话只能绑定启用的 TTS 模型".to_string(),
                    ));
                }
                let catalog = self
                    .voice_catalog_repository
                    .catalog(model_id, false)
                    .await?;
                if catalog.voices.is_empty() {
                    return Err(ConversationApplicationError::Validation(
                        "声音会话绑定模型没有可用音色，请先同步目录".to_string(),
                    ));
                }
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
        sound_context: Option<SoundAgentContext>,
    ) -> Result<AgentTurnResponse, ConversationApplicationError> {
        let resolved = self.model_resolver.text_client(model_id).await?;
        let conversation = self
            .conversation_repository
            .get_conversation(conversation_id)
            .await?;
        let (user_metadata, run_input, payload) = invocation_payload(
            &conversation.agent_type,
            supplement_of_batch_id,
            sound_context,
        )?;
        self.agent_executor
            .execute(
                AgentInvocation {
                    session_id: conversation_id,
                    user_message: content,
                    user_metadata,
                    run_input,
                    payload,
                },
                ModelExecutionRef {
                    client: resolved.client,
                    snapshot: Some(resolved.snapshot),
                },
            )
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
}

fn invocation_payload(
    agent_type: &str,
    supplement_of_batch_id: Option<Uuid>,
    sound_context: Option<SoundAgentContext>,
) -> Result<(Value, Value, Value), ConversationApplicationError> {
    if agent_type != "sound" && sound_context.is_some() {
        return Err(ConversationApplicationError::Runtime(
            AgentRuntimeError::Validation("非声音会话不能携带声音上下文".to_string()),
        ));
    }
    if agent_type == "sound" {
        let context = sound_context.ok_or_else(|| {
            ConversationApplicationError::Runtime(AgentRuntimeError::Validation(
                "声音消息缺少当前编辑上下文".to_string(),
            ))
        })?;
        let metadata = json!({"sound_context": &context});
        return Ok((
            metadata.clone(),
            metadata.clone(),
            json!({"sound_context": context}),
        ));
    }
    let payload = if agent_type == "topic" {
        json!({"supplement_of_batch_id": supplement_of_batch_id})
    } else {
        json!({})
    };
    Ok((json!({}), json!({}), payload))
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
    VoiceCatalog(crate::repositories::VoiceCatalogRepositoryError),
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

impl From<crate::repositories::VoiceCatalogRepositoryError> for ConversationApplicationError {
    fn from(error: crate::repositories::VoiceCatalogRepositoryError) -> Self {
        Self::VoiceCatalog(error)
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
            Self::VoiceCatalog(error) => write!(formatter, "{error}"),
            Self::Agent(error) => write!(formatter, "{error}"),
            Self::Runtime(error) => write!(formatter, "{error}"),
            Self::ModelResolve(error) => write!(formatter, "{error}"),
            Self::Validation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ConversationApplicationError {}
