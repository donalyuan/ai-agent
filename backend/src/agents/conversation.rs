use chrono::{DateTime, Utc};
use serde_json::Value;
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentConversationStatus {
    Active,
    Archived,
}

impl AgentConversationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

impl TryFrom<&str> for AgentConversationStatus {
    type Error = AgentConversationParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            _ => Err(AgentConversationParseError {
                field: "status",
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentMessageRole {
    System,
    User,
    Assistant,
    Tool,
}

impl AgentMessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

impl TryFrom<&str> for AgentMessageRole {
    type Error = AgentConversationParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "system" => Ok(Self::System),
            "user" => Ok(Self::User),
            "assistant" => Ok(Self::Assistant),
            "tool" => Ok(Self::Tool),
            _ => Err(AgentConversationParseError {
                field: "role",
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentConversation {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_type: String,
    pub subject_type: Option<String>,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub status: AgentConversationStatus,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: AgentMessageRole,
    pub content: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentRunRecord {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_type: String,
    pub status: String,
    pub input: Value,
    pub output: Option<Value>,
    pub error_message: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateAgentConversationInput {
    pub project_id: Option<Uuid>,
    pub agent_type: String,
    pub subject_type: Option<String>,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateAgentMessageInput {
    pub conversation_id: Uuid,
    pub role: AgentMessageRole,
    pub content: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateAgentRunInput {
    pub conversation_id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_type: String,
    pub input: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateAgentStepInput {
    pub agent_run_id: Uuid,
    pub step_order: i32,
    pub step_type: String,
    pub status: String,
    pub input: Value,
    pub output: Option<Value>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FinishAgentRunInput {
    pub agent_run_id: Uuid,
    pub status: String,
    pub output: Option<Value>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConversationParseError {
    field: &'static str,
    value: String,
}

impl fmt::Display for AgentConversationParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unknown agent conversation {}: {}",
            self.field, self.value
        )
    }
}

impl std::error::Error for AgentConversationParseError {}
