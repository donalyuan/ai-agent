//! Reusable Agent execution kernel. Business payloads and repositories stay in adapters.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use novex_ai_core::AgentKey;
use novex_model::ModelExecutionSnapshot;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use uuid::Uuid;

mod audited_model;
mod context_audit;

pub use audited_model::{
    text_context_candidate, AuditedCallOwner, AuditedModelError, AuditedModelExecutor,
    AuditedModelRequest, AuditedModelResponse, AuditedParsedModelResponse, AuditedTerminalStatus,
    BoundModelResolver, FinishAuditedCall, FixedDefinitionBinding, FixedModelBinding,
    ModelCallAuditStore, PrepareAuditedCall, PrepareAuditedCallWithContext,
    PreparedAuditedModelCall, PreparedAuditedModelFailure, PreparedAuditedModelOutcome,
    PreparedAuditedModelSuccess, ResolvedBindingEvidence, ResolvedBoundModel,
    TextContextCandidateInput,
};
pub use context_audit::{ContextAuditStore, PersistContextCompileAttempt, PersistContextSnapshot};

pub const CRATE_PURPOSE: &str = "novex-agent";
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Clone, Debug, PartialEq)]
pub struct AgentSession {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_key: AgentKey,
    pub subject_type: Option<String>,
    pub subject_id: Option<Uuid>,
    pub metadata: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NewMessage {
    pub session_id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredMessage {
    pub id: Uuid,
    pub session_id: Uuid,
    pub role: MessageRole,
    pub content: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StartRun {
    pub session_id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_key: AgentKey,
    pub input: Value,
    pub model_id: Option<Uuid>,
    pub model_snapshot: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunRecord {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_key: AgentKey,
    pub status: String,
    pub input: Value,
    pub output: Option<Value>,
    pub error_message: Option<String>,
    pub context_compile_attempt_id: Option<Uuid>,
    pub model_id: Option<Uuid>,
    pub model_snapshot: Option<Value>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunStatus {
    Succeeded,
    Failed,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FinishRun {
    pub run_id: Uuid,
    pub status: RunStatus,
    pub output: Option<Value>,
    pub error_message: Option<String>,
    pub context_compile_attempt_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentStep {
    pub run_id: Uuid,
    pub order: i32,
    pub step_type: String,
    pub status: String,
    pub input: Value,
    pub output: Option<Value>,
    pub error_message: Option<String>,
}

/// Generic envelope. Adapter-specific values are isolated in `payload`.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentInvocation {
    pub session_id: Uuid,
    pub user_message: String,
    pub user_metadata: Value,
    pub run_input: Value,
    pub payload: Value,
}

#[derive(Clone)]
pub struct ModelExecutionRef {
    pub snapshot: Option<ModelExecutionSnapshot>,
    pub audited: Option<AuditedExecutionBinding>,
}

#[derive(Clone)]
pub struct AuditedExecutionBinding {
    pub executor: Arc<AuditedModelExecutor>,
    pub agent_key: String,
    pub agent_version: String,
    pub binding: FixedModelBinding,
}

#[derive(Clone)]
pub struct AgentExecutionContext {
    pub session: AgentSession,
    pub user_message: StoredMessage,
    pub run_id: Uuid,
    pub model: ModelExecutionRef,
    pub steps: Arc<dyn StepRecorder>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentOutcome {
    pub assistant_content: String,
    pub assistant_metadata: Value,
}

impl AgentOutcome {
    pub fn new(content: impl Into<String>, metadata: Value) -> Self {
        Self {
            assistant_content: content.into(),
            assistant_metadata: metadata,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentTurn {
    pub user_message: StoredMessage,
    pub assistant_message: StoredMessage,
    pub run: RunRecord,
}

#[async_trait]
pub trait AgentAdapter: Send + Sync {
    fn key(&self) -> &AgentKey;

    async fn execute(
        &self,
        invocation: &AgentInvocation,
        context: &AgentExecutionContext,
    ) -> Result<AgentOutcome, BoxError>;
}

#[derive(Default)]
pub struct AgentRegistry {
    adapters: HashMap<AgentKey, Arc<dyn AgentAdapter>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, adapter: Arc<dyn AgentAdapter>) -> Result<(), RegistryError> {
        self.register_as(adapter.key().clone(), adapter)
    }

    pub fn register_as(
        &mut self,
        key: AgentKey,
        adapter: Arc<dyn AgentAdapter>,
    ) -> Result<(), RegistryError> {
        if &key != adapter.key() {
            return Err(RegistryError::KeyMismatch {
                registered: key,
                declared: adapter.key().clone(),
            });
        }
        if self.adapters.contains_key(&key) {
            return Err(RegistryError::DuplicateKey(key));
        }
        self.adapters.insert(key, adapter);
        Ok(())
    }

    pub fn resolve(&self, key: &AgentKey) -> Option<Arc<dyn AgentAdapter>> {
        self.adapters.get(key).cloned()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryError {
    DuplicateKey(AgentKey),
    KeyMismatch {
        registered: AgentKey,
        declared: AgentKey,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateKey(key) => write!(formatter, "duplicate agent key: {key}"),
            Self::KeyMismatch {
                registered,
                declared,
            } => write!(
                formatter,
                "registered agent key {registered} does not match adapter key {declared}"
            ),
        }
    }
}

impl std::error::Error for RegistryError {}

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn load_session(&self, id: Uuid) -> Result<AgentSession, BoxError>;
    async fn save_message(&self, input: NewMessage) -> Result<StoredMessage, BoxError>;
}

#[async_trait]
pub trait RunRecorder: Send + Sync {
    async fn start_run(&self, input: StartRun) -> Result<RunRecord, BoxError>;
    async fn finish_run(&self, input: FinishRun) -> Result<RunRecord, BoxError>;
}

#[async_trait]
pub trait StepRecorder: Send + Sync {
    async fn record_step(&self, step: AgentStep) -> Result<Uuid, BoxError>;
}

#[derive(Debug)]
pub enum KernelError {
    Validation(String),
    UnsupportedAgent(AgentKey),
    Adapter(BoxError),
    Store(BoxError),
}

impl fmt::Display for KernelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(message) => {
                write!(formatter, "agent turn validation error: {message}")
            }
            Self::UnsupportedAgent(key) => write!(formatter, "unsupported agent type: {key}"),
            Self::Adapter(error) | Self::Store(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for KernelError {}

#[derive(Debug)]
pub enum RunLifecycleError<E> {
    Operation(E),
    Store(BoxError),
}

impl<E: fmt::Display> fmt::Display for RunLifecycleError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Operation(error) => write!(formatter, "{error}"),
            Self::Store(error) => write!(formatter, "{error}"),
        }
    }
}

impl<E> std::error::Error for RunLifecycleError<E>
where
    E: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Operation(error) => Some(error),
            Self::Store(error) => Some(error.as_ref()),
        }
    }
}

#[derive(Clone)]
pub struct RunLifecycleCoordinator {
    runs: Arc<dyn RunRecorder>,
}

impl RunLifecycleCoordinator {
    pub fn new(runs: Arc<dyn RunRecorder>) -> Self {
        Self { runs }
    }

    /// Owns a non-conversational task Run from creation through one terminal transition.
    pub async fn execute<T, E, Operation, OperationFuture, SuccessOutput, FailureMessage>(
        &self,
        start: StartRun,
        operation: Operation,
        success_output: SuccessOutput,
        failure_message: FailureMessage,
    ) -> Result<T, RunLifecycleError<E>>
    where
        E: fmt::Display,
        Operation: FnOnce(Uuid) -> OperationFuture,
        OperationFuture: Future<Output = Result<T, E>>,
        SuccessOutput: FnOnce(&T) -> Option<Value>,
        FailureMessage: FnOnce(&E) -> String,
    {
        let running = self
            .runs
            .start_run(start)
            .await
            .map_err(RunLifecycleError::Store)?;

        match operation(running.id).await {
            Ok(value) => {
                self.runs
                    .finish_run(FinishRun {
                        run_id: running.id,
                        status: RunStatus::Succeeded,
                        output: success_output(&value),
                        error_message: None,
                        context_compile_attempt_id: None,
                    })
                    .await
                    .map_err(RunLifecycleError::Store)?;
                Ok(value)
            }
            Err(error) => {
                finish_failed_run(self.runs.as_ref(), running.id, failure_message(&error)).await;
                Err(RunLifecycleError::Operation(error))
            }
        }
    }
}

#[derive(Clone)]
pub struct AgentRunCoordinator {
    registry: Arc<AgentRegistry>,
    sessions: Arc<dyn SessionStore>,
    runs: Arc<dyn RunRecorder>,
    steps: Arc<dyn StepRecorder>,
}

impl AgentRunCoordinator {
    pub fn new(
        registry: Arc<AgentRegistry>,
        sessions: Arc<dyn SessionStore>,
        runs: Arc<dyn RunRecorder>,
        steps: Arc<dyn StepRecorder>,
    ) -> Self {
        Self {
            registry,
            sessions,
            runs,
            steps,
        }
    }

    /// Owns the complete turn lifecycle and makes exactly one terminal Run transition.
    pub async fn execute(
        &self,
        invocation: AgentInvocation,
        model: ModelExecutionRef,
    ) -> Result<AgentTurn, KernelError> {
        let content = invocation.user_message.trim();
        if content.is_empty() {
            return Err(KernelError::Validation("消息不能为空".into()));
        }
        let session = self
            .sessions
            .load_session(invocation.session_id)
            .await
            .map_err(KernelError::Store)?;
        let adapter = self
            .registry
            .resolve(&session.agent_key)
            .ok_or_else(|| KernelError::UnsupportedAgent(session.agent_key.clone()))?;
        let user_message = self
            .sessions
            .save_message(NewMessage {
                session_id: session.id,
                role: MessageRole::User,
                content: content.to_string(),
                metadata: invocation.user_metadata.clone(),
            })
            .await
            .map_err(KernelError::Store)?;

        let mut run_input = json!({ "user_message_id": user_message.id });
        merge_object(&mut run_input, &invocation.run_input);
        let model_id = model.snapshot.as_ref().map(|snapshot| snapshot.model_id);
        let model_snapshot = model
            .snapshot
            .as_ref()
            .and_then(|snapshot| serde_json::to_value(snapshot).ok());
        let running = self
            .runs
            .start_run(StartRun {
                session_id: session.id,
                project_id: session.project_id,
                agent_key: session.agent_key.clone(),
                input: run_input,
                model_id,
                model_snapshot,
            })
            .await
            .map_err(KernelError::Store)?;
        let context = AgentExecutionContext {
            session,
            user_message: user_message.clone(),
            run_id: running.id,
            model,
            steps: self.steps.clone(),
        };

        let outcome = match adapter.execute(&invocation, &context).await {
            Ok(outcome) => outcome,
            Err(error) => {
                let message = error.to_string();
                finish_failed_run(self.runs.as_ref(), running.id, message).await;
                return Err(KernelError::Adapter(error));
            }
        };

        let assistant_message = match self
            .sessions
            .save_message(NewMessage {
                session_id: context.session.id,
                role: MessageRole::Assistant,
                content: outcome.assistant_content,
                metadata: outcome.assistant_metadata,
            })
            .await
        {
            Ok(message) => message,
            Err(error) => {
                let message = error.to_string();
                finish_failed_run(self.runs.as_ref(), running.id, message).await;
                return Err(KernelError::Store(error));
            }
        };
        let run = self
            .runs
            .finish_run(FinishRun {
                run_id: running.id,
                status: RunStatus::Succeeded,
                output: Some(json!({ "assistant_message_id": assistant_message.id })),
                error_message: None,
                context_compile_attempt_id: None,
            })
            .await
            .map_err(KernelError::Store)?;
        Ok(AgentTurn {
            user_message,
            assistant_message,
            run,
        })
    }
}

async fn finish_failed_run(runs: &dyn RunRecorder, run_id: Uuid, message: String) {
    let _ = runs
        .finish_run(FinishRun {
            run_id,
            status: RunStatus::Failed,
            output: None,
            error_message: Some(message),
            context_compile_attempt_id: None,
        })
        .await;
}

fn merge_object(target: &mut Value, additions: &Value) {
    if let (Some(target), Some(additions)) = (target.as_object_mut(), additions.as_object()) {
        target.extend(additions.clone());
    }
}
