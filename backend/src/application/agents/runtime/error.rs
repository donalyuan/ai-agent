//! 统一 Runtime 的业务校验、领域仓储和模型调用错误，交由 API 层转换协议。

use crate::agents::ScriptAgentError;
use crate::repositories::{
    ConversationRepositoryError, ProjectRepositoryError, ScriptRepositoryError,
    TopicRepositoryError,
};
use novex_model::LLMError;
use std::fmt;
use uuid::Uuid;

#[derive(Debug)]
pub enum AgentRuntimeError {
    Validation(String),
    UnsupportedAgent(String),
    InvalidLlmOutput(String),
    SceneNotFound { script_id: Uuid, sequence: i32 },
    ConversationRepository(ConversationRepositoryError),
    ScriptRepository(ScriptRepositoryError),
    ProjectRepository(ProjectRepositoryError),
    TopicRepository(TopicRepositoryError),
    ScriptAgent(ScriptAgentError),
    Llm(LLMError),
}

impl From<ConversationRepositoryError> for AgentRuntimeError {
    fn from(error: ConversationRepositoryError) -> Self {
        Self::ConversationRepository(error)
    }
}

impl From<ScriptRepositoryError> for AgentRuntimeError {
    fn from(error: ScriptRepositoryError) -> Self {
        Self::ScriptRepository(error)
    }
}

impl From<ProjectRepositoryError> for AgentRuntimeError {
    fn from(error: ProjectRepositoryError) -> Self {
        Self::ProjectRepository(error)
    }
}

impl From<TopicRepositoryError> for AgentRuntimeError {
    fn from(error: TopicRepositoryError) -> Self {
        Self::TopicRepository(error)
    }
}

impl From<LLMError> for AgentRuntimeError {
    fn from(error: LLMError) -> Self {
        Self::Llm(error)
    }
}

impl From<ScriptAgentError> for AgentRuntimeError {
    fn from(error: ScriptAgentError) -> Self {
        Self::ScriptAgent(error)
    }
}

impl fmt::Display for AgentRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(message) => {
                write!(formatter, "agent turn validation error: {message}")
            }
            Self::UnsupportedAgent(agent_type) => {
                write!(formatter, "unsupported agent type: {agent_type}")
            }
            Self::InvalidLlmOutput(message) => {
                write!(formatter, "invalid agent LLM output: {message}")
            }
            Self::SceneNotFound {
                script_id,
                sequence,
            } => write!(
                formatter,
                "scene not found for script {script_id}: sequence {sequence}"
            ),
            Self::ConversationRepository(error) => write!(formatter, "{error}"),
            Self::ScriptRepository(error) => write!(formatter, "{error}"),
            Self::ProjectRepository(error) => write!(formatter, "{error}"),
            Self::TopicRepository(error) => write!(formatter, "{error}"),
            Self::ScriptAgent(error) => write!(formatter, "{error}"),
            Self::Llm(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AgentRuntimeError {}
