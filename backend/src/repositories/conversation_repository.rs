use crate::agents::conversation::{
    AgentConversation, AgentConversationStatus, AgentMessage, AgentMessageRole, AgentRunRecord,
    BindAgentConversationSubjectInput, CreateAgentConversationInput, CreateAgentMessageInput,
    CreateAgentRunInput, CreateAgentStepInput, FinishAgentRunInput,
};
use async_trait::async_trait;
use sqlx::{postgres::PgRow, PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone)]
pub struct PostgresConversationRepository {
    pool: PgPool,
}

impl PostgresConversationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
pub trait ConversationRepository: Send + Sync {
    async fn create_conversation(
        &self,
        input: CreateAgentConversationInput,
    ) -> Result<AgentConversation, ConversationRepositoryError>;

    async fn get_conversation(
        &self,
        conversation_id: Uuid,
    ) -> Result<AgentConversation, ConversationRepositoryError>;

    async fn save_message(
        &self,
        input: CreateAgentMessageInput,
    ) -> Result<AgentMessage, ConversationRepositoryError>;

    async fn list_messages(
        &self,
        conversation_id: Uuid,
    ) -> Result<Vec<AgentMessage>, ConversationRepositoryError>;

    async fn create_run(
        &self,
        input: CreateAgentRunInput,
    ) -> Result<AgentRunRecord, ConversationRepositoryError>;

    async fn add_step(
        &self,
        input: CreateAgentStepInput,
    ) -> Result<(), ConversationRepositoryError>;

    async fn finish_run(
        &self,
        input: FinishAgentRunInput,
    ) -> Result<AgentRunRecord, ConversationRepositoryError>;

    async fn bind_conversation_subject(
        &self,
        input: BindAgentConversationSubjectInput,
    ) -> Result<AgentConversation, ConversationRepositoryError>;
}

#[async_trait]
impl ConversationRepository for PostgresConversationRepository {
    async fn create_conversation(
        &self,
        input: CreateAgentConversationInput,
    ) -> Result<AgentConversation, ConversationRepositoryError> {
        let row = sqlx::query(
            r#"
            INSERT INTO agent_conversations (
                project_id, agent_type, subject_type, subject_id, title, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, project_id, agent_type, subject_type, subject_id, title,
                      status, metadata, created_at, updated_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.agent_type)
        .bind(input.subject_type)
        .bind(input.subject_id)
        .bind(input.title)
        .bind(input.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(ConversationRepositoryError::from)?;

        conversation_from_row(row)
    }

    async fn get_conversation(
        &self,
        conversation_id: Uuid,
    ) -> Result<AgentConversation, ConversationRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, project_id, agent_type, subject_type, subject_id, title,
                   status, metadata, created_at, updated_at
            FROM agent_conversations
            WHERE id = $1
            "#,
        )
        .bind(conversation_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(ConversationRepositoryError::from)?
        .ok_or(ConversationRepositoryError::ConversationNotFound(
            conversation_id,
        ))?;

        conversation_from_row(row)
    }

    async fn save_message(
        &self,
        input: CreateAgentMessageInput,
    ) -> Result<AgentMessage, ConversationRepositoryError> {
        let row = sqlx::query(
            r#"
            INSERT INTO agent_messages (conversation_id, role, content, metadata)
            VALUES ($1, $2, $3, $4)
            RETURNING id, conversation_id, role, content, metadata, created_at
            "#,
        )
        .bind(input.conversation_id)
        .bind(input.role.as_str())
        .bind(input.content)
        .bind(input.metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(ConversationRepositoryError::from)?;

        message_from_row(row)
    }

    async fn list_messages(
        &self,
        conversation_id: Uuid,
    ) -> Result<Vec<AgentMessage>, ConversationRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT id, conversation_id, role, content, metadata, created_at
            FROM agent_messages
            WHERE conversation_id = $1
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(conversation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(ConversationRepositoryError::from)?;

        rows.into_iter().map(message_from_row).collect()
    }

    async fn create_run(
        &self,
        input: CreateAgentRunInput,
    ) -> Result<AgentRunRecord, ConversationRepositoryError> {
        let run_input = with_conversation_id(input.input, input.conversation_id);
        let row = sqlx::query(
            r#"
            INSERT INTO agent_runs (project_id, agent_type, status, input)
            VALUES ($1, $2, 'running', $3)
            RETURNING id, project_id, agent_type, status, input, output,
                      error_message, started_at, ended_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.agent_type)
        .bind(run_input)
        .fetch_one(&self.pool)
        .await
        .map_err(ConversationRepositoryError::from)?;

        Ok(run_from_row(row))
    }

    async fn add_step(
        &self,
        input: CreateAgentStepInput,
    ) -> Result<(), ConversationRepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO agent_steps (
                agent_run_id, step_order, step_type, status, input, output, error_message
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(input.agent_run_id)
        .bind(input.step_order)
        .bind(input.step_type)
        .bind(input.status)
        .bind(input.input)
        .bind(input.output)
        .bind(input.error_message)
        .execute(&self.pool)
        .await
        .map_err(ConversationRepositoryError::from)?;

        Ok(())
    }

    async fn finish_run(
        &self,
        input: FinishAgentRunInput,
    ) -> Result<AgentRunRecord, ConversationRepositoryError> {
        let row = sqlx::query(
            r#"
            UPDATE agent_runs
            SET status = $2,
                output = $3,
                error_message = $4,
                ended_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, agent_type, status, input, output,
                      error_message, started_at, ended_at
            "#,
        )
        .bind(input.agent_run_id)
        .bind(input.status)
        .bind(input.output)
        .bind(input.error_message)
        .fetch_optional(&self.pool)
        .await
        .map_err(ConversationRepositoryError::from)?
        .ok_or(ConversationRepositoryError::RunNotFound(input.agent_run_id))?;

        Ok(run_from_row(row))
    }

    async fn bind_conversation_subject(
        &self,
        input: BindAgentConversationSubjectInput,
    ) -> Result<AgentConversation, ConversationRepositoryError> {
        let row = sqlx::query(
            r#"
            UPDATE agent_conversations
            SET subject_type = $2,
                subject_id = $3,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, agent_type, subject_type, subject_id, title,
                      status, metadata, created_at, updated_at
            "#,
        )
        .bind(input.conversation_id)
        .bind(input.subject_type)
        .bind(input.subject_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(ConversationRepositoryError::from)?
        .ok_or(ConversationRepositoryError::ConversationNotFound(
            input.conversation_id,
        ))?;

        conversation_from_row(row)
    }
}

fn with_conversation_id(mut input: serde_json::Value, conversation_id: Uuid) -> serde_json::Value {
    if let serde_json::Value::Object(ref mut object) = input {
        object.insert(
            "conversation_id".to_string(),
            serde_json::Value::String(conversation_id.to_string()),
        );
    }
    input
}

fn conversation_from_row(row: PgRow) -> Result<AgentConversation, ConversationRepositoryError> {
    let status_value: String = row.get("status");
    let status = AgentConversationStatus::try_from(status_value.as_str())
        .map_err(|error| ConversationRepositoryError::Storage(error.to_string()))?;

    Ok(AgentConversation {
        id: row.get("id"),
        project_id: row.get("project_id"),
        agent_type: row.get("agent_type"),
        subject_type: row.get("subject_type"),
        subject_id: row.get("subject_id"),
        title: row.get("title"),
        status,
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn message_from_row(row: PgRow) -> Result<AgentMessage, ConversationRepositoryError> {
    let role_value: String = row.get("role");
    let role = AgentMessageRole::try_from(role_value.as_str())
        .map_err(|error| ConversationRepositoryError::Storage(error.to_string()))?;

    Ok(AgentMessage {
        id: row.get("id"),
        conversation_id: row.get("conversation_id"),
        role,
        content: row.get("content"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
    })
}

fn run_from_row(row: PgRow) -> AgentRunRecord {
    AgentRunRecord {
        id: row.get("id"),
        project_id: row.get("project_id"),
        agent_type: row.get("agent_type"),
        status: row.get("status"),
        input: row.get("input"),
        output: row.get("output"),
        error_message: row.get("error_message"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
    }
}

#[derive(Debug)]
pub enum ConversationRepositoryError {
    ConversationNotFound(Uuid),
    RunNotFound(Uuid),
    Storage(String),
}

impl From<sqlx::Error> for ConversationRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for ConversationRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConversationNotFound(conversation_id) => {
                write!(formatter, "conversation not found: {conversation_id}")
            }
            Self::RunNotFound(run_id) => write!(formatter, "agent run not found: {run_id}"),
            Self::Storage(message) => write!(formatter, "conversation storage error: {message}"),
        }
    }
}

impl std::error::Error for ConversationRepositoryError {}
