use async_trait::async_trait;
use novex_ai_core::{ContextCompileAttempt, ContextSnapshot};
use uuid::Uuid;

use crate::{AuditedCallOwner, BoxError};

#[derive(Clone, Debug, PartialEq)]
pub struct PersistContextSnapshot {
    pub owner: AuditedCallOwner,
    pub snapshot: ContextSnapshot,
    pub known_secrets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PersistContextCompileAttempt {
    pub owner: AuditedCallOwner,
    pub attempt: ContextCompileAttempt,
    pub known_secrets: Vec<String>,
}

#[async_trait]
pub trait ContextAuditStore: Send + Sync {
    async fn binding_is_executable(&self, owner: AuditedCallOwner) -> Result<bool, BoxError>;

    async fn block_tokenizer_profile_binding(
        &self,
        owner: AuditedCallOwner,
    ) -> Result<(), BoxError>;

    async fn persist_snapshot(&self, input: PersistContextSnapshot) -> Result<Uuid, BoxError>;

    async fn persist_attempt(&self, input: PersistContextCompileAttempt) -> Result<Uuid, BoxError>;

    async fn link_failure(
        &self,
        owner: AuditedCallOwner,
        attempt_id: Uuid,
        step_id: Option<Uuid>,
    ) -> Result<(), BoxError>;
}
