use crate::domain::conversation::{
    AgentConversation, AgentConversationBinding, AgentConversationDefinitionBindingInput,
    AgentConversationStatus, AgentMessage, AgentMessageRole, AgentRunBinding, AgentRunRecord,
    BindAgentConversationSubjectInput, CreateAgentConversationInput, CreateAgentMessageInput,
    CreateAgentRunInput, CreateAgentStepInput, FinishAgentRunInput, ModelBindingEvidence,
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

    /// Conversation 与代码 Definition 必须在同一事务中创建，不能留下可执行但未绑定的记录。
    pub async fn create_conversation_with_definition(
        &self,
        input: CreateAgentConversationInput,
        binding: AgentConversationDefinitionBindingInput,
    ) -> Result<AgentConversation, AgentBindingError> {
        validate_definition_binding(&binding)?;
        let mut transaction = self.pool.begin().await?;
        let row = sqlx::query(
            r#"
            INSERT INTO agent_conversations (
                project_id, agent_type, subject_type, subject_id, title, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, project_id, agent_type, subject_type, subject_id, title,
                      status, metadata, last_context_compile_attempt_id, created_at, updated_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.agent_type)
        .bind(input.subject_type)
        .bind(input.subject_id)
        .bind(input.title)
        .bind(input.metadata)
        .fetch_one(&mut *transaction)
        .await?;
        let conversation = conversation_from_row(row)
            .map_err(|error| AgentBindingError::Storage(error.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO agent_conversation_bindings (
                conversation_id, agent_key, agent_version, agent_digest, prompt_bindings,
                context_policy_bindings, registry_digest, migration_source, parent_conversation_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(conversation.id)
        .bind(binding.agent_key)
        .bind(binding.agent_version)
        .bind(binding.agent_digest)
        .bind(binding.prompt_bindings)
        .bind(binding.context_policy_bindings)
        .bind(binding.registry_digest)
        .bind(binding.migration_source)
        .bind(binding.parent_conversation_id)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(conversation)
    }

    pub async fn get_conversation_binding(
        &self,
        conversation_id: Uuid,
    ) -> Result<AgentConversationBinding, AgentBindingError> {
        let row = sqlx::query(
            r#"
            SELECT conversation_id, agent_key, agent_version, agent_digest, prompt_bindings,
                   context_policy_bindings, registry_digest, model_id, behavior_fingerprint, model_capabilities,
                   tokenizer_profile_key, tokenizer_profile_version, tokenizer_profile_digest,
                   binding_status, migration_source, parent_conversation_id, created_at
            FROM agent_conversation_bindings
            WHERE conversation_id = $1
            "#,
        )
        .bind(conversation_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AgentBindingError::BindingNotFound(conversation_id))?;
        Ok(binding_from_row(row))
    }

    /// 首次 UPDATE 依赖行锁串行化；并发失败方只能读取胜出的不可变 binding。
    pub async fn bind_or_validate_conversation_model(
        &self,
        conversation_id: Uuid,
        evidence: ModelBindingEvidence,
    ) -> Result<AgentConversationBinding, AgentBindingError> {
        validate_model_binding(&evidence)?;
        let updated = sqlx::query(
            r#"
            UPDATE agent_conversation_bindings
            SET model_id = $2,
                behavior_fingerprint = $3,
                model_capabilities = $4,
                tokenizer_profile_key = $5,
                tokenizer_profile_version = $6,
                tokenizer_profile_digest = $7,
                binding_status = 'executable'
            WHERE conversation_id = $1 AND model_id IS NULL
            RETURNING conversation_id, agent_key, agent_version, agent_digest, prompt_bindings,
                      context_policy_bindings, registry_digest, model_id, behavior_fingerprint, model_capabilities,
                      tokenizer_profile_key, tokenizer_profile_version, tokenizer_profile_digest,
                      binding_status, migration_source, parent_conversation_id, created_at
            "#,
        )
        .bind(conversation_id)
        .bind(evidence.model_id)
        .bind(&evidence.behavior_fingerprint)
        .bind(&evidence.model_capabilities)
        .bind(&evidence.tokenizer_profile_key)
        .bind(&evidence.tokenizer_profile_version)
        .bind(&evidence.tokenizer_profile_digest)
        .fetch_optional(&self.pool)
        .await?;
        if let Some(row) = updated {
            return Ok(binding_from_row(row));
        }

        let existing = self.get_conversation_binding(conversation_id).await?;
        if existing.model_id == Some(evidence.model_id)
            && existing.behavior_fingerprint.as_deref()
                == Some(evidence.behavior_fingerprint.as_str())
            && existing.model_capabilities.as_ref() == Some(&evidence.model_capabilities)
            && existing.tokenizer_profile_key.as_deref()
                == Some(evidence.tokenizer_profile_key.as_str())
            && existing.tokenizer_profile_version.as_deref()
                == Some(evidence.tokenizer_profile_version.as_str())
            && existing.tokenizer_profile_digest.as_deref()
                == Some(evidence.tokenizer_profile_digest.as_str())
            && existing.binding_status == "executable"
        {
            Ok(existing)
        } else {
            Err(AgentBindingError::ModelRebindRequired {
                conversation_id,
                bound_model_id: existing.model_id,
                requested_model_id: evidence.model_id,
            })
        }
    }

    pub async fn create_run_binding(
        &self,
        agent_run_id: Uuid,
        definition: AgentConversationDefinitionBindingInput,
        model: ModelBindingEvidence,
        legacy_partial_audit: bool,
    ) -> Result<AgentRunBinding, AgentBindingError> {
        validate_definition_binding(&definition)?;
        validate_model_binding(&model)?;
        sqlx::query(
            r#"
            INSERT INTO agent_run_bindings (
                agent_run_id, agent_key, agent_version, agent_digest, prompt_bindings,
                context_policy_bindings, registry_digest, model_id, behavior_fingerprint,
                model_capabilities, tokenizer_profile_key, tokenizer_profile_version,
                tokenizer_profile_digest, legacy_partial_audit
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            ON CONFLICT (agent_run_id) DO NOTHING
            "#,
        )
        .bind(agent_run_id)
        .bind(&definition.agent_key)
        .bind(&definition.agent_version)
        .bind(&definition.agent_digest)
        .bind(&definition.prompt_bindings)
        .bind(&definition.context_policy_bindings)
        .bind(&definition.registry_digest)
        .bind(model.model_id)
        .bind(&model.behavior_fingerprint)
        .bind(&model.model_capabilities)
        .bind(&model.tokenizer_profile_key)
        .bind(&model.tokenizer_profile_version)
        .bind(&model.tokenizer_profile_digest)
        .bind(legacy_partial_audit)
        .execute(&self.pool)
        .await?;

        let existing = self.get_run_binding(agent_run_id).await?;
        if existing.agent_key == definition.agent_key
            && existing.agent_version == definition.agent_version
            && existing.agent_digest == definition.agent_digest
            && existing.prompt_bindings == definition.prompt_bindings
            && existing.context_policy_bindings.as_ref()
                == Some(&definition.context_policy_bindings)
            && existing.registry_digest == definition.registry_digest
            && existing.model_id == model.model_id
            && existing.behavior_fingerprint == model.behavior_fingerprint
            && existing.model_capabilities == model.model_capabilities
            && existing.tokenizer_profile_key.as_deref()
                == Some(model.tokenizer_profile_key.as_str())
            && existing.tokenizer_profile_version.as_deref()
                == Some(model.tokenizer_profile_version.as_str())
            && existing.tokenizer_profile_digest.as_deref()
                == Some(model.tokenizer_profile_digest.as_str())
            && existing.context_binding_status == "executable"
            && existing.legacy_partial_audit == legacy_partial_audit
        {
            Ok(existing)
        } else {
            Err(AgentBindingError::RunBindingConflict(agent_run_id))
        }
    }

    pub async fn get_run_binding(
        &self,
        agent_run_id: Uuid,
    ) -> Result<AgentRunBinding, AgentBindingError> {
        let row = sqlx::query(
            r#"
            SELECT agent_run_id, agent_key, agent_version, agent_digest, prompt_bindings,
                   context_policy_bindings, registry_digest, model_id, behavior_fingerprint,
                   model_capabilities, tokenizer_profile_key, tokenizer_profile_version,
                   tokenizer_profile_digest, context_binding_status,
                   legacy_partial_audit, created_at
            FROM agent_run_bindings
            WHERE agent_run_id = $1
            "#,
        )
        .bind(agent_run_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AgentBindingError::RunBindingNotFound(agent_run_id))?;
        Ok(run_binding_from_row(row))
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
    ) -> Result<Uuid, ConversationRepositoryError>;

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
                      status, metadata, last_context_compile_attempt_id, created_at, updated_at
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
                   status, metadata, last_context_compile_attempt_id, created_at, updated_at
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
            INSERT INTO agent_runs (
                project_id, agent_type, status, input, model_id, model_snapshot
            )
            VALUES ($1, $2, 'running', $3, $4, $5)
            RETURNING id, project_id, agent_type, status, input, output,
                      error_message, context_compile_attempt_id, model_id, model_snapshot,
                      started_at, ended_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.agent_type)
        .bind(run_input)
        .bind(input.model_id)
        .bind(input.model_snapshot)
        .fetch_one(&self.pool)
        .await
        .map_err(ConversationRepositoryError::from)?;

        Ok(run_from_row(row))
    }

    async fn add_step(
        &self,
        input: CreateAgentStepInput,
    ) -> Result<Uuid, ConversationRepositoryError> {
        sqlx::query_scalar(
            r#"
            INSERT INTO agent_steps (
                agent_run_id, step_order, step_type, status, input, output, error_message
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(input.agent_run_id)
        .bind(input.step_order)
        .bind(input.step_type)
        .bind(input.status)
        .bind(input.input)
        .bind(input.output)
        .bind(input.error_message)
        .fetch_one(&self.pool)
        .await
        .map_err(ConversationRepositoryError::from)
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
                context_compile_attempt_id = $5,
                ended_at = NOW()
            WHERE id = $1
            RETURNING id, project_id, agent_type, status, input, output,
                      error_message, context_compile_attempt_id, model_id, model_snapshot,
                      started_at, ended_at
            "#,
        )
        .bind(input.agent_run_id)
        .bind(input.status)
        .bind(input.output)
        .bind(input.error_message)
        .bind(input.context_compile_attempt_id)
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
                      status, metadata, last_context_compile_attempt_id, created_at, updated_at
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
        last_context_compile_attempt_id: row.get("last_context_compile_attempt_id"),
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
        context_compile_attempt_id: row.get("context_compile_attempt_id"),
        model_id: row.get("model_id"),
        model_snapshot: row.get("model_snapshot"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
    }
}

fn binding_from_row(row: PgRow) -> AgentConversationBinding {
    AgentConversationBinding {
        conversation_id: row.get("conversation_id"),
        agent_key: row.get("agent_key"),
        agent_version: row.get("agent_version"),
        agent_digest: row.get("agent_digest"),
        prompt_bindings: row.get("prompt_bindings"),
        context_policy_bindings: row.get("context_policy_bindings"),
        registry_digest: row.get("registry_digest"),
        model_id: row.get("model_id"),
        behavior_fingerprint: row.get("behavior_fingerprint"),
        model_capabilities: row.get("model_capabilities"),
        tokenizer_profile_key: row.get("tokenizer_profile_key"),
        tokenizer_profile_version: row.get("tokenizer_profile_version"),
        tokenizer_profile_digest: row.get("tokenizer_profile_digest"),
        binding_status: row.get("binding_status"),
        migration_source: row.get("migration_source"),
        parent_conversation_id: row.get("parent_conversation_id"),
        created_at: row.get("created_at"),
    }
}

fn run_binding_from_row(row: PgRow) -> AgentRunBinding {
    AgentRunBinding {
        agent_run_id: row.get("agent_run_id"),
        agent_key: row.get("agent_key"),
        agent_version: row.get("agent_version"),
        agent_digest: row.get("agent_digest"),
        prompt_bindings: row.get("prompt_bindings"),
        context_policy_bindings: row.get("context_policy_bindings"),
        registry_digest: row.get("registry_digest"),
        model_id: row.get("model_id"),
        behavior_fingerprint: row.get("behavior_fingerprint"),
        model_capabilities: row.get("model_capabilities"),
        tokenizer_profile_key: row.get("tokenizer_profile_key"),
        tokenizer_profile_version: row.get("tokenizer_profile_version"),
        tokenizer_profile_digest: row.get("tokenizer_profile_digest"),
        context_binding_status: row.get("context_binding_status"),
        legacy_partial_audit: row.get("legacy_partial_audit"),
        created_at: row.get("created_at"),
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_definition_binding(
    binding: &AgentConversationDefinitionBindingInput,
) -> Result<(), AgentBindingError> {
    if binding.agent_key.trim().is_empty()
        || binding.agent_version.trim().is_empty()
        || !valid_digest(&binding.agent_digest)
        || !valid_digest(&binding.registry_digest)
        || !binding.prompt_bindings.is_object()
        || binding
            .prompt_bindings
            .as_object()
            .is_some_and(|value| value.is_empty())
        || !binding.context_policy_bindings.is_object()
        || binding
            .context_policy_bindings
            .as_object()
            .is_some_and(|value| value.is_empty())
    {
        return Err(AgentBindingError::InvalidEvidence(
            "definition binding is incomplete".into(),
        ));
    }
    Ok(())
}

fn validate_model_binding(evidence: &ModelBindingEvidence) -> Result<(), AgentBindingError> {
    if !valid_digest(&evidence.behavior_fingerprint)
        || !evidence.model_capabilities.is_object()
        || evidence.tokenizer_profile_key.trim().is_empty()
        || evidence.tokenizer_profile_version.trim().is_empty()
        || !valid_digest(&evidence.tokenizer_profile_digest)
    {
        return Err(AgentBindingError::InvalidEvidence(
            "model binding evidence is incomplete".into(),
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub enum AgentBindingError {
    BindingNotFound(Uuid),
    RunBindingNotFound(Uuid),
    RunBindingConflict(Uuid),
    ModelRebindRequired {
        conversation_id: Uuid,
        bound_model_id: Option<Uuid>,
        requested_model_id: Uuid,
    },
    InvalidEvidence(String),
    Storage(String),
}

impl From<sqlx::Error> for AgentBindingError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for AgentBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindingNotFound(id) => write!(formatter, "conversation binding not found: {id}"),
            Self::RunBindingNotFound(id) => write!(formatter, "agent run binding not found: {id}"),
            Self::RunBindingConflict(id) => write!(formatter, "agent run binding conflict: {id}"),
            Self::ModelRebindRequired { .. } => formatter.write_str("model_rebind_required"),
            Self::InvalidEvidence(message) => {
                write!(formatter, "invalid binding evidence: {message}")
            }
            Self::Storage(message) => write!(formatter, "agent binding storage error: {message}"),
        }
    }
}

impl std::error::Error for AgentBindingError {}

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
