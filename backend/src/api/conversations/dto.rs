use crate::application::agents::adapters::SoundAgentContext;
use crate::application::conversations::CreateConversationCommand;
use crate::domain::conversation::{
    AgentConversation, AgentConversationStatus, AgentMessage, AgentMessageRole, AgentRunRecord,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct CreateAgentConversationRequest {
    pub agent_type: String,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub subject_type: Option<String>,
    #[serde(default)]
    pub subject_id: Option<Uuid>,
    #[validate(length(min = 1, max = 160))]
    pub title: String,
    #[serde(default)]
    pub metadata: Value,
}

impl CreateAgentConversationRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("会话标题不能为空".to_string());
        }
        let agent_type = self.agent_type.trim();
        if !matches!(agent_type, "script" | "topic" | "sound" | "work") {
            return Err("暂不支持该 Agent 类型".to_string());
        }
        if self.project_id.is_none() {
            return Err("Agent 会话必须绑定项目".to_string());
        }
        let subject_type = self.subject_type.as_deref().map(str::trim);
        if agent_type == "topic" && (self.subject_id.is_some() || subject_type.is_some()) {
            return Err("选题会话暂不绑定 subject".to_string());
        }
        if agent_type == "script" && self.subject_id.is_some() && subject_type != Some("script") {
            return Err("脚本会话 subject_type 必须为 script".to_string());
        }
        if agent_type == "script"
            && self.subject_id.is_none()
            && subject_type.is_some_and(|value| !value.is_empty())
        {
            return Err("未绑定脚本会话不能传 subject_type".to_string());
        }
        if agent_type == "work"
            && self.subject_type.as_deref() != Some("work")
            && self.subject_id.is_some()
        {
            return Err("作品会话绑定 subject 时 subject_type 必须为 work".to_string());
        }
        if agent_type == "sound" {
            if self.subject_id.is_some() || subject_type.is_some() {
                return Err("声音会话暂不绑定 subject".to_string());
            }
            let metadata = self
                .metadata
                .as_object()
                .ok_or_else(|| "声音会话 metadata 必须是 object".to_string())?;
            if metadata.len() != 1
                || metadata
                    .get("speech_model_id")
                    .and_then(Value::as_str)
                    .and_then(|value| Uuid::parse_str(value).ok())
                    .is_none()
            {
                return Err("声音会话 metadata 只能包含有效 speech_model_id".to_string());
            }
        }
        self.validate()
            .map_err(|error| format!("会话参数无效: {error}"))
    }

    pub fn into_command(self) -> CreateConversationCommand {
        CreateConversationCommand {
            project_id: self.project_id,
            agent_type: self.agent_type.trim().to_string(),
            subject_type: self.subject_type.map(|value| value.trim().to_string()),
            subject_id: self.subject_id,
            title: self.title.trim().to_string(),
            metadata: self.metadata,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct SendAgentMessageRequest {
    pub model_id: Uuid,
    #[validate(length(min = 1, max = 4000))]
    pub content: String,
    #[serde(default)]
    pub supplement_of_batch_id: Option<Uuid>,
    #[serde(default)]
    pub sound_context: Option<SoundAgentContext>,
}

impl SendAgentMessageRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.model_id.is_nil() {
            return Err("必须选择文本模型".to_string());
        }
        if self.content.trim().is_empty() {
            return Err("消息不能为空".to_string());
        }
        if let Some(context) = &self.sound_context {
            context.validate()?;
        }
        self.validate()
            .map_err(|error| format!("消息参数无效: {error}"))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentConversationResponse {
    pub conversation_id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_type: String,
    pub subject_type: Option<String>,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub status: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AgentConversation> for AgentConversationResponse {
    fn from(conversation: AgentConversation) -> Self {
        Self {
            conversation_id: conversation.id,
            project_id: conversation.project_id,
            agent_type: conversation.agent_type,
            subject_type: conversation.subject_type,
            subject_id: conversation.subject_id,
            title: conversation.title,
            status: conversation_status_value(&conversation.status).to_string(),
            metadata: conversation.metadata,
            created_at: conversation.created_at,
            updated_at: conversation.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentMessageResponse {
    pub message_id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

impl From<AgentMessage> for AgentMessageResponse {
    fn from(message: AgentMessage) -> Self {
        Self {
            message_id: message.id,
            conversation_id: message.conversation_id,
            role: message_role_value(&message.role).to_string(),
            content: message.content,
            metadata: message.metadata,
            created_at: message.created_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentRunResponse {
    pub run_id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_type: String,
    pub status: String,
    pub input: Value,
    pub output: Option<Value>,
    pub error_message: Option<String>,
    pub model_id: Option<Uuid>,
    pub model_snapshot: Option<Value>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

impl From<AgentRunRecord> for AgentRunResponse {
    fn from(run: AgentRunRecord) -> Self {
        Self {
            run_id: run.id,
            project_id: run.project_id,
            agent_type: run.agent_type,
            status: run.status,
            input: run.input,
            output: run.output,
            error_message: run.error_message,
            model_id: run.model_id,
            model_snapshot: run.model_snapshot,
            started_at: run.started_at,
            ended_at: run.ended_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentMessageListResponse {
    pub messages: Vec<AgentMessageResponse>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentTurnResponseBody {
    pub user_message: AgentMessageResponse,
    pub assistant_message: AgentMessageResponse,
    pub run: AgentRunResponse,
}

fn conversation_status_value(status: &AgentConversationStatus) -> &'static str {
    match status {
        AgentConversationStatus::Active => "active",
        AgentConversationStatus::Archived => "archived",
    }
}

fn message_role_value(role: &AgentMessageRole) -> &'static str {
    match role {
        AgentMessageRole::System => "system",
        AgentMessageRole::User => "user",
        AgentMessageRole::Assistant => "assistant",
        AgentMessageRole::Tool => "tool",
    }
}
