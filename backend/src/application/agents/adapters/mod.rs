//! Backend business adapters for the reusable `novex-agent` execution kernel.

mod error;
mod prompt;
mod script;
mod sound;
mod topic_generation;
mod topic_quality;
mod topic_review;
mod types;
mod work;

pub use error::AgentRuntimeError;
pub use prompt::format_account_strategy_context;
pub use topic_review::AuditedTopicReviewExecution;
pub use types::{AgentTurnResponse, SoundAgentContext};

use crate::domain::conversation::CreateAgentStepInput;
use crate::repositories::{
    ConversationRepository, PostgresVoiceCatalogRepository, PostgresWorkLibraryRepository,
    ProjectRepository, ScriptRepository, TopicRepository,
};
use async_trait::async_trait;
use novex_agent::{
    AgentAdapter, AgentExecutionContext, AgentInvocation, AgentOutcome, AgentStep, BoxError,
    StepRecorder,
};
use novex_ai_core::AgentKey;
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

pub struct ScriptAgentAdapter {
    key: AgentKey,
    conversation_repository: Arc<dyn ConversationRepository>,
    script_repository: Arc<dyn ScriptRepository>,
    project_repository: Arc<dyn ProjectRepository>,
}

impl ScriptAgentAdapter {
    pub fn new(
        conversation_repository: Arc<dyn ConversationRepository>,
        script_repository: Arc<dyn ScriptRepository>,
        project_repository: Arc<dyn ProjectRepository>,
    ) -> Self {
        Self {
            key: AgentKey::new("script").expect("script is a valid static AgentKey"),
            conversation_repository,
            script_repository,
            project_repository,
        }
    }
}

pub struct TopicAgentAdapter {
    key: AgentKey,
    conversation_repository: Arc<dyn ConversationRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    topic_repository: Arc<dyn TopicRepository>,
}

impl TopicAgentAdapter {
    pub fn new(
        conversation_repository: Arc<dyn ConversationRepository>,
        project_repository: Arc<dyn ProjectRepository>,
        topic_repository: Arc<dyn TopicRepository>,
    ) -> Self {
        Self {
            key: AgentKey::new("topic").expect("topic is a valid static AgentKey"),
            conversation_repository,
            project_repository,
            topic_repository,
        }
    }
}

pub struct SoundAgentAdapter {
    key: AgentKey,
    voice_catalog_repository: Arc<PostgresVoiceCatalogRepository>,
}

impl SoundAgentAdapter {
    pub fn new(voice_catalog_repository: Arc<PostgresVoiceCatalogRepository>) -> Self {
        Self {
            key: AgentKey::new("sound").expect("sound is a valid static AgentKey"),
            voice_catalog_repository,
        }
    }
}

pub struct WorkAgentAdapter {
    key: AgentKey,
    project_repository: Arc<dyn ProjectRepository>,
    work_library_repository: Arc<PostgresWorkLibraryRepository>,
}

impl WorkAgentAdapter {
    pub fn new(
        project_repository: Arc<dyn ProjectRepository>,
        work_library_repository: Arc<PostgresWorkLibraryRepository>,
    ) -> Self {
        Self {
            key: AgentKey::new("work").expect("work is a valid static AgentKey"),
            project_repository,
            work_library_repository,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyPayload {}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct TopicPayload {
    supplement_of_batch_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SoundPayload {
    sound_context: SoundAgentContext,
}

fn decode_payload<T: for<'de> Deserialize<'de>>(payload: &Value) -> Result<T, AgentRuntimeError> {
    serde_json::from_value(payload.clone())
        .map_err(|error| AgentRuntimeError::Validation(format!("Agent payload 无效: {error}")))
}

#[async_trait]
impl AgentAdapter for ScriptAgentAdapter {
    fn key(&self) -> &AgentKey {
        &self.key
    }

    async fn execute(
        &self,
        invocation: &AgentInvocation,
        context: &AgentExecutionContext,
    ) -> Result<AgentOutcome, BoxError> {
        let _: EmptyPayload = decode_payload(&invocation.payload).map_err(boxed)?;
        self.handle_script_turn(
            &context.session,
            &context.user_message,
            context.run_id,
            context.model.clone(),
            context.steps.clone(),
        )
        .await
        .map_err(boxed)
    }
}

#[async_trait]
impl AgentAdapter for TopicAgentAdapter {
    fn key(&self) -> &AgentKey {
        &self.key
    }

    async fn execute(
        &self,
        invocation: &AgentInvocation,
        context: &AgentExecutionContext,
    ) -> Result<AgentOutcome, BoxError> {
        let payload: TopicPayload = decode_payload(&invocation.payload).map_err(boxed)?;
        self.handle_topic_turn(
            &context.session,
            &context.user_message,
            context.run_id,
            payload.supplement_of_batch_id,
            context.model.clone(),
            context.steps.clone(),
        )
        .await
        .map_err(boxed)
    }
}

#[async_trait]
impl AgentAdapter for SoundAgentAdapter {
    fn key(&self) -> &AgentKey {
        &self.key
    }

    async fn execute(
        &self,
        invocation: &AgentInvocation,
        context: &AgentExecutionContext,
    ) -> Result<AgentOutcome, BoxError> {
        let payload: SoundPayload = decode_payload(&invocation.payload).map_err(boxed)?;
        validate_sound_context(&context.session, &payload.sound_context).map_err(boxed)?;
        self.handle_sound_turn(
            &context.session,
            &context.user_message,
            context.run_id,
            &payload.sound_context,
            context.model.clone(),
            context.steps.clone(),
        )
        .await
        .map_err(boxed)
    }
}

#[async_trait]
impl AgentAdapter for WorkAgentAdapter {
    fn key(&self) -> &AgentKey {
        &self.key
    }

    async fn execute(
        &self,
        invocation: &AgentInvocation,
        context: &AgentExecutionContext,
    ) -> Result<AgentOutcome, BoxError> {
        let _: EmptyPayload = decode_payload(&invocation.payload).map_err(boxed)?;
        self.handle_work_turn(
            &context.session,
            &context.user_message,
            context.run_id,
            context.model.clone(),
            context.steps.clone(),
        )
        .await
        .map_err(boxed)
    }
}

fn boxed(error: AgentRuntimeError) -> BoxError {
    Box::new(error)
}

async fn record_step(
    recorder: &dyn StepRecorder,
    input: CreateAgentStepInput,
) -> Result<Uuid, AgentRuntimeError> {
    recorder
        .record_step(AgentStep {
            run_id: input.agent_run_id,
            order: input.step_order,
            step_type: input.step_type,
            status: input.status,
            input: input.input,
            output: input.output,
            error_message: input.error_message,
        })
        .await
        .map_err(AgentRuntimeError::from_boxed)
}

fn validate_sound_context(
    conversation: &novex_agent::AgentSession,
    context: &SoundAgentContext,
) -> Result<(), AgentRuntimeError> {
    context.validate().map_err(AgentRuntimeError::Validation)?;
    let conversation_model_id = conversation
        .metadata
        .get("speech_model_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| AgentRuntimeError::Validation("声音会话缺少有效 TTS 模型".to_string()))?;
    if context.speech_model_id != conversation_model_id {
        return Err(AgentRuntimeError::Validation(
            "声音消息上下文与会话 TTS 模型不一致".to_string(),
        ));
    }
    Ok(())
}
