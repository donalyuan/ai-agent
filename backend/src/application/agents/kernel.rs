//! Backend implementations of the reusable Agent Kernel persistence ports.

use super::adapters::AgentRuntimeError;
use crate::domain::conversation::{
    AgentMessage, AgentMessageRole, AgentRunRecord, CreateAgentMessageInput, CreateAgentRunInput,
    CreateAgentStepInput, FinishAgentRunInput,
};
use crate::repositories::{ConversationRepository, PostgresConversationRepository};
use async_trait::async_trait;
use novex_agent::{
    AgentSession, AgentStep, AgentTurn, BoxError, FinishRun, FixedDefinitionBinding,
    FixedModelBinding, MessageRole, NewMessage, RunRecord, RunRecorder, SessionStore, StartRun,
    StepRecorder, StoredMessage,
};
use novex_ai_core::AgentKey;
use std::sync::Arc;
use uuid::Uuid;

/// Resolves the active Rust-owned Definition into the immutable binding stored with a Run/Conversation.
pub fn active_rust_definition_binding(
    registry: &novex_ai_core::DefinitionRegistry,
    agent_key: &str,
) -> Result<crate::domain::conversation::AgentConversationDefinitionBindingInput, String> {
    let agent = registry
        .active_agent(agent_key)
        .map_err(|error| error.to_string())?;
    if agent.executor_owner != novex_ai_core::ExecutorOwner::Rust {
        return Err("definition owner must be rust".into());
    }
    let mut prompts = serde_json::Map::new();
    let mut policies = serde_json::Map::new();
    for (node_key, reference) in &agent.nodes {
        let prompt = registry
            .prompts()
            .iter()
            .find(|prompt| {
                prompt.prompt_key == reference.key && prompt.version == reference.version
            })
            .ok_or_else(|| {
                format!(
                    "prompt definition not found: {}@{}",
                    reference.key, reference.version
                )
            })?;
        prompts.insert(
            node_key.clone(),
            serde_json::json!({
                "key": reference.key,
                "version": reference.version,
                "digest": novex_ai_core::definition_digest(prompt)
                    .map_err(|error| error.to_string())?
            }),
        );
        let policy_reference = reference.context_policy.as_ref().ok_or_else(|| {
            format!("governed Context Policy binding is missing at node {node_key}")
        })?;
        let policy = registry
            .context_policy(&policy_reference.key, &policy_reference.version)
            .map_err(|error| error.to_string())?;
        policies.insert(
            node_key.clone(),
            serde_json::json!({
                "key": policy_reference.key,
                "version": policy_reference.version,
                "digest": novex_ai_core::definition_digest(policy)
                    .map_err(|error| error.to_string())?
            }),
        );
    }
    Ok(
        crate::domain::conversation::AgentConversationDefinitionBindingInput {
            agent_key: agent.agent_key.clone(),
            agent_version: agent.version.clone(),
            agent_digest: novex_ai_core::definition_digest(agent)
                .map_err(|error| error.to_string())?,
            prompt_bindings: serde_json::Value::Object(prompts),
            context_policy_bindings: serde_json::Value::Object(policies),
            registry_digest: registry.digest().into(),
            migration_source: None,
            parent_conversation_id: None,
        },
    )
}

/// 将持久化 Definition/模型证据转换为一次调用不可漂移的 Executor binding。
pub fn fixed_model_binding(
    context_policy_bindings: &serde_json::Value,
    model: &crate::domain::conversation::ModelBindingEvidence,
) -> Result<FixedModelBinding, String> {
    let policies = context_policy_bindings
        .as_object()
        .ok_or_else(|| "Context Policy bindings must be an object".to_string())?
        .iter()
        .map(|(node_key, value)| {
            let value = value
                .as_object()
                .ok_or_else(|| format!("Context Policy binding at {node_key} must be an object"))?;
            let field = |name: &str| {
                value
                    .get(name)
                    .and_then(serde_json::Value::as_str)
                    .filter(|item| !item.trim().is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| {
                        format!("Context Policy binding at {node_key} is missing {name}")
                    })
            };
            Ok((
                node_key.clone(),
                FixedDefinitionBinding {
                    key: field("key")?,
                    version: field("version")?,
                    digest: field("digest")?,
                },
            ))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, String>>()?;
    if policies.is_empty() {
        return Err("Context Policy bindings are empty".into());
    }
    Ok(FixedModelBinding {
        model_id: model.model_id,
        behavior_fingerprint: model.behavior_fingerprint.clone(),
        context_policy_bindings: policies,
        tokenizer_profile: FixedDefinitionBinding {
            key: model.tokenizer_profile_key.clone(),
            version: model.tokenizer_profile_version.clone(),
            digest: model.tokenizer_profile_digest.clone(),
        },
    })
}

pub fn build_registry(
    pool: &sqlx::PgPool,
) -> Result<novex_agent::AgentRegistry, AgentBootstrapError> {
    use super::adapters::{
        ScriptAgentAdapter, SoundAgentAdapter, TopicAgentAdapter, WorkAgentAdapter,
    };
    use crate::repositories::{
        PostgresProjectRepository, PostgresScriptRepository, PostgresTopicRepository,
        PostgresVoiceCatalogRepository, PostgresWorkLibraryRepository,
    };

    let conversations: Arc<dyn ConversationRepository> =
        Arc::new(PostgresConversationRepository::new(pool.clone()));
    let projects: Arc<dyn crate::repositories::ProjectRepository> =
        Arc::new(PostgresProjectRepository::new(pool.clone()));
    assemble_registry(vec![
        (
            "script adapter",
            Some(Arc::new(ScriptAgentAdapter::new(
                conversations.clone(),
                Arc::new(PostgresScriptRepository::new(pool.clone())),
                projects.clone(),
            )) as Arc<dyn novex_agent::AgentAdapter>),
        ),
        (
            "topic adapter",
            Some(Arc::new(TopicAgentAdapter::new(
                conversations,
                projects.clone(),
                Arc::new(PostgresTopicRepository::new(pool.clone())),
            )) as Arc<dyn novex_agent::AgentAdapter>),
        ),
        (
            "sound adapter",
            Some(Arc::new(SoundAgentAdapter::new(Arc::new(
                PostgresVoiceCatalogRepository::new(pool.clone()),
            ))) as Arc<dyn novex_agent::AgentAdapter>),
        ),
        (
            "work adapter",
            Some(Arc::new(WorkAgentAdapter::new(
                projects,
                Arc::new(PostgresWorkLibraryRepository::new(pool.clone())),
            )) as Arc<dyn novex_agent::AgentAdapter>),
        ),
    ])
}

pub fn assemble_registry(
    adapters: Vec<(&'static str, Option<Arc<dyn novex_agent::AgentAdapter>>)>,
) -> Result<novex_agent::AgentRegistry, AgentBootstrapError> {
    let mut registry = novex_agent::AgentRegistry::new();
    for (dependency, adapter) in adapters {
        let adapter = adapter.ok_or(AgentBootstrapError::MissingDependency(dependency))?;
        registry
            .register(adapter)
            .map_err(AgentBootstrapError::Registry)?;
    }
    Ok(registry)
}

#[derive(Debug)]
pub enum AgentBootstrapError {
    MissingDependency(&'static str),
    Registry(novex_agent::RegistryError),
}

impl std::fmt::Display for AgentBootstrapError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingDependency(name) => {
                write!(formatter, "missing required dependency: {name}")
            }
            Self::Registry(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AgentBootstrapError {}

#[derive(Clone)]
pub struct PostgresAgentKernelStore {
    repository: PostgresConversationRepository,
}

impl PostgresAgentKernelStore {
    pub fn new(repository: PostgresConversationRepository) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl SessionStore for PostgresAgentKernelStore {
    async fn load_session(&self, id: uuid::Uuid) -> Result<AgentSession, BoxError> {
        let conversation = self
            .repository
            .get_conversation(id)
            .await
            .map_err(AgentRuntimeError::from)?;
        let agent_key = AgentKey::new(conversation.agent_type.clone())
            .map_err(|_| AgentRuntimeError::UnsupportedAgent(conversation.agent_type.clone()))?;
        Ok(AgentSession {
            id: conversation.id,
            project_id: conversation.project_id,
            agent_key,
            subject_type: conversation.subject_type,
            subject_id: conversation.subject_id,
            metadata: conversation.metadata,
        })
    }

    async fn save_message(&self, input: NewMessage) -> Result<StoredMessage, BoxError> {
        let message = self
            .repository
            .save_message(CreateAgentMessageInput {
                conversation_id: input.session_id,
                role: match input.role {
                    MessageRole::User => AgentMessageRole::User,
                    MessageRole::Assistant => AgentMessageRole::Assistant,
                },
                content: input.content,
                metadata: input.metadata,
            })
            .await
            .map_err(AgentRuntimeError::from)?;
        Ok(stored_message(message))
    }
}

#[async_trait]
impl RunRecorder for PostgresAgentKernelStore {
    async fn start_run(&self, input: StartRun) -> Result<RunRecord, BoxError> {
        let run = self
            .repository
            .create_run(CreateAgentRunInput {
                conversation_id: input.session_id,
                project_id: input.project_id,
                agent_type: input.agent_key.to_string(),
                input: input.input,
                model_id: input.model_id,
                model_snapshot: input.model_snapshot,
            })
            .await
            .map_err(AgentRuntimeError::from)?;
        run_record(run)
    }

    async fn finish_run(&self, input: FinishRun) -> Result<RunRecord, BoxError> {
        let run = self
            .repository
            .finish_run(FinishAgentRunInput {
                agent_run_id: input.run_id,
                status: input.status.as_str().to_string(),
                output: input.output,
                error_message: input.error_message,
                context_compile_attempt_id: input.context_compile_attempt_id,
            })
            .await
            .map_err(AgentRuntimeError::from)?;
        run_record(run)
    }
}

#[async_trait]
impl StepRecorder for PostgresAgentKernelStore {
    async fn record_step(&self, step: AgentStep) -> Result<Uuid, BoxError> {
        self.repository
            .add_step(CreateAgentStepInput {
                agent_run_id: step.run_id,
                step_order: step.order,
                step_type: step.step_type,
                status: step.status,
                input: step.input,
                output: step.output,
                error_message: step.error_message,
            })
            .await
            .map_err(|error| Box::new(AgentRuntimeError::from(error)) as BoxError)
    }
}

pub fn kernel_error(error: novex_agent::KernelError) -> AgentRuntimeError {
    match error {
        novex_agent::KernelError::Validation(message) => AgentRuntimeError::Validation(message),
        novex_agent::KernelError::UnsupportedAgent(key) => {
            AgentRuntimeError::UnsupportedAgent(key.to_string())
        }
        novex_agent::KernelError::Adapter(error) | novex_agent::KernelError::Store(error) => {
            AgentRuntimeError::from_boxed(error)
        }
    }
}

pub fn run_lifecycle_error<E>(error: novex_agent::RunLifecycleError<E>) -> E
where
    E: From<AgentRuntimeError>,
{
    match error {
        novex_agent::RunLifecycleError::Operation(error) => error,
        novex_agent::RunLifecycleError::Store(error) => AgentRuntimeError::from_boxed(error).into(),
    }
}

pub fn domain_turn(
    turn: AgentTurn,
) -> Result<super::adapters::AgentTurnResponse, AgentRuntimeError> {
    Ok(super::adapters::AgentTurnResponse {
        user_message: domain_message(turn.user_message),
        agent_message: domain_message(turn.assistant_message),
        run: domain_run(turn.run)?,
    })
}

fn stored_message(message: AgentMessage) -> StoredMessage {
    StoredMessage {
        id: message.id,
        session_id: message.conversation_id,
        role: match message.role {
            AgentMessageRole::Assistant => MessageRole::Assistant,
            _ => MessageRole::User,
        },
        content: message.content,
        metadata: message.metadata,
        created_at: message.created_at,
    }
}

fn domain_message(message: StoredMessage) -> AgentMessage {
    AgentMessage {
        id: message.id,
        conversation_id: message.session_id,
        role: match message.role {
            MessageRole::User => AgentMessageRole::User,
            MessageRole::Assistant => AgentMessageRole::Assistant,
        },
        content: message.content,
        metadata: message.metadata,
        created_at: message.created_at,
    }
}

fn run_record(run: AgentRunRecord) -> Result<RunRecord, BoxError> {
    let agent_key = AgentKey::new(run.agent_type.clone())
        .map_err(|_| AgentRuntimeError::UnsupportedAgent(run.agent_type.clone()))?;
    Ok(RunRecord {
        id: run.id,
        project_id: run.project_id,
        agent_key,
        status: run.status,
        input: run.input,
        output: run.output,
        error_message: run.error_message,
        context_compile_attempt_id: run.context_compile_attempt_id,
        model_id: run.model_id,
        model_snapshot: run.model_snapshot,
        started_at: run.started_at,
        ended_at: run.ended_at,
    })
}

fn domain_run(run: RunRecord) -> Result<AgentRunRecord, AgentRuntimeError> {
    Ok(AgentRunRecord {
        id: run.id,
        project_id: run.project_id,
        agent_type: run.agent_key.to_string(),
        status: run.status,
        input: run.input,
        output: run.output,
        error_message: run.error_message,
        context_compile_attempt_id: run.context_compile_attempt_id,
        model_id: run.model_id,
        model_snapshot: run.model_snapshot,
        started_at: run.started_at,
        ended_at: run.ended_at,
    })
}

pub fn coordinator(
    registry: Arc<novex_agent::AgentRegistry>,
    store: PostgresAgentKernelStore,
) -> novex_agent::AgentRunCoordinator {
    let store = Arc::new(store);
    novex_agent::AgentRunCoordinator::new(registry, store.clone(), store.clone(), store)
}

pub fn run_lifecycle(
    repository: PostgresConversationRepository,
) -> novex_agent::RunLifecycleCoordinator {
    novex_agent::RunLifecycleCoordinator::new(Arc::new(PostgresAgentKernelStore::new(repository)))
}

#[derive(Clone)]
pub struct AgentExecutor {
    coordinator: novex_agent::AgentRunCoordinator,
}

impl AgentExecutor {
    pub fn new(
        registry: impl Into<Arc<novex_agent::AgentRegistry>>,
        repository: PostgresConversationRepository,
    ) -> Self {
        Self {
            coordinator: coordinator(registry.into(), PostgresAgentKernelStore::new(repository)),
        }
    }

    pub async fn execute(
        &self,
        invocation: novex_agent::AgentInvocation,
        model: novex_agent::ModelExecutionRef,
    ) -> Result<super::adapters::AgentTurnResponse, AgentRuntimeError> {
        let turn = self
            .coordinator
            .execute(invocation, model)
            .await
            .map_err(kernel_error)?;
        domain_turn(turn)
    }
}
