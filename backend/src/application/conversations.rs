//! 连续对话用例，负责会话绑定规则、模型选择和统一 Runtime 组装。

use crate::agents::ScriptAgentError;
use crate::application::agents::adapters::{
    AgentRuntimeError, AgentTurnResponse, SoundAgentContext,
};
use crate::application::agents::kernel::{
    active_rust_definition_binding, fixed_model_binding, AgentExecutor,
};
use crate::domain::conversation::{AgentConversation, AgentMessage, CreateAgentConversationInput};
use crate::model_routing::{
    model_behavior_evidence, model_binding_evidence, ModelClientResolver, ModelResolveError,
};
use crate::repositories::AiModelRepository;
use crate::repositories::{
    AgentBindingError, ConversationRepository, ConversationRepositoryError,
    PostgresAiModelRepository, PostgresConversationRepository, PostgresProjectRepository,
    PostgresScriptRepository, PostgresVoiceCatalogRepository, PostgresWorkLibraryRepository,
    ProjectRepository, ProjectRepositoryError, ScriptRepository,
};
use novex_agent::{
    AgentInvocation, AuditedExecutionBinding, AuditedModelExecutor, ModelExecutionRef,
};
use novex_ai_core::{validate_model_capabilities, DefinitionRegistry};
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
    definition_registry: Arc<DefinitionRegistry>,
    audited_model_executor: Arc<AuditedModelExecutor>,
}

impl ConversationService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        conversation_repository: PostgresConversationRepository,
        script_repository: PostgresScriptRepository,
        project_repository: PostgresProjectRepository,
        ai_model_repository: PostgresAiModelRepository,
        voice_catalog_repository: PostgresVoiceCatalogRepository,
        work_library_repository: PostgresWorkLibraryRepository,
        model_resolver: Arc<dyn ModelClientResolver>,
        agent_registry: Arc<novex_agent::AgentRegistry>,
        definition_registry: Arc<DefinitionRegistry>,
        audited_model_executor: Arc<AuditedModelExecutor>,
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
            definition_registry,
            audited_model_executor,
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

        let agent_key = conversation_agent_key(&command.agent_type).ok_or_else(|| {
            ConversationApplicationError::Validation(format!(
                "暂不支持该 Agent 类型: {}",
                command.agent_type
            ))
        })?;
        let definition_binding =
            active_rust_definition_binding(&self.definition_registry, agent_key)
                .map_err(ConversationApplicationError::Definition)?;
        self.conversation_repository
            .create_conversation_with_definition(
                CreateAgentConversationInput {
                    project_id: command.project_id,
                    agent_type: command.agent_type,
                    subject_type: command.subject_type,
                    subject_id: command.subject_id,
                    title: command.title,
                    metadata: command.metadata,
                },
                definition_binding,
            )
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
        let conversation = self
            .conversation_repository
            .get_conversation(conversation_id)
            .await?;
        let binding = self
            .conversation_repository
            .get_conversation_binding(conversation_id)
            .await?;
        let definition = self
            .definition_registry
            .agent(&binding.agent_key, &binding.agent_version)
            .map_err(|error| ConversationApplicationError::Definition(error.to_string()))?;
        let resolved = self.model_resolver.text_client(model_id).await?;
        let evidence = model_behavior_evidence(&resolved.snapshot)?;
        validate_model_capabilities(&definition.model_requirements, &evidence.capabilities)
            .map_err(|_| ConversationApplicationError::ModelCapabilityMismatch)?;
        let model_binding = model_binding_evidence(&self.definition_registry, &resolved.snapshot)?;
        self.conversation_repository
            .bind_or_validate_conversation_model(conversation_id, model_binding.clone())
            .await?;
        let fixed_binding = fixed_model_binding(
            binding.context_policy_bindings.as_ref().ok_or_else(|| {
                ConversationApplicationError::Definition(
                    "conversation Context Policy binding is missing".into(),
                )
            })?,
            &model_binding,
        )
        .map_err(ConversationApplicationError::Definition)?;
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
                    snapshot: Some(resolved.snapshot),
                    audited: Some(AuditedExecutionBinding {
                        executor: self.audited_model_executor.clone(),
                        agent_key: binding.agent_key,
                        agent_version: binding.agent_version,
                        binding: fixed_binding,
                    }),
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

fn conversation_agent_key(agent_type: &str) -> Option<&'static str> {
    match agent_type {
        "script" => Some("video.script"),
        "topic" => Some("video.topic"),
        "sound" => Some("video.sound"),
        "work" => Some("video.work"),
        _ => None,
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
    AgentBinding(AgentBindingError),
    ModelCapabilityMismatch,
    Definition(String),
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

impl From<AgentBindingError> for ConversationApplicationError {
    fn from(error: AgentBindingError) -> Self {
        Self::AgentBinding(error)
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
            Self::AgentBinding(error) => write!(formatter, "{error}"),
            Self::ModelCapabilityMismatch => formatter.write_str("model_capability_mismatch"),
            Self::Definition(message) => write!(formatter, "definition error: {message}"),
            Self::Validation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ConversationApplicationError {}
