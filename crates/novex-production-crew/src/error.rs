/// 虚拟制作团队统一 Result 类型别名
pub type ProductionResult<T> = std::result::Result<T, ProductionError>;

/// 虚拟制作团队统一错误类型
#[derive(Debug, thiserror::Error)]
pub enum ProductionError {
    #[error("来源无效: {reason}")]
    SourceInvalid { reason: String },

    #[error("来源已被活跃 Full Crew 锁定")]
    SourceLocked,

    #[error("同一 Topic 已存在活跃制作意图")]
    ActiveIntentConflict,

    #[error("制作意图已存在 ProductionRun")]
    RunAlreadyExists,

    #[error("幂等键对应的请求内容不同")]
    IdempotencyConflict,

    #[error("状态转换冲突: {reason}")]
    TransitionConflict { reason: String },

    #[error("提交的 package digest 已过期")]
    StalePackage,

    #[error("资源限制: {resource}, current={current}, requested={requested}, limit={limit}")]
    ResourceLimit {
        resource: String,
        current: u64,
        requested: u64,
        limit: u64,
    },

    #[error("媒体或模型能力不满足: {reason}")]
    CapabilityMismatch { reason: String },

    #[error("媒体证据不完整: {reason}")]
    EvidenceBlocker {
        reason: String,
        details: serde_json::Value,
    },

    #[error("外部副作用结果不确定，需要人工处理")]
    AttentionRequired,

    #[error("等待外部条件: {reason}")]
    ExternalWait {
        reason: String,
        details: serde_json::Value,
    },
    #[error("缺少必需的输入产物: {artifact_type}")]
    MissingInputArtifact { artifact_type: String },

    #[error("产物 schema 无效: {details}")]
    InvalidArtifactSchema { details: String },

    #[error("质量闸门拦截: {gate_name} - {reason}")]
    GateRejected { gate_name: String, reason: String },

    #[error("等待人工审批: artifact_id = {artifact_id}")]
    GateWaitApproval { artifact_id: uuid::Uuid },

    #[error("角色不存在: {role_key}")]
    RoleNotFound { role_key: String },

    #[error("角色执行顺序非法: {message}")]
    InvalidRoleSequence { message: String },

    #[error("预算超限: 需要 {requested}，可用 {available}")]
    BudgetExceeded { requested: u64, available: u64 },

    #[error("项目不存在: {project_id}")]
    ProjectNotFound { project_id: uuid::Uuid },

    #[error("产物不存在: {artifact_type}/{artifact_id}")]
    ArtifactNotFound {
        artifact_type: String,
        artifact_id: uuid::Uuid,
    },

    #[error("协作建议不存在: {suggestion_id}")]
    SuggestionNotFound { suggestion_id: uuid::Uuid },

    #[error("数据库错误: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Agent 执行错误: {0}")]
    AgentExecution(String),

    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("YAML 解析错误: {0}")]
    YamlParse(String),

    #[error("权限不足: {message}")]
    Unauthorized { message: String },
}

impl ProductionError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SourceInvalid { .. } => "source_invalid",
            Self::SourceLocked => "source_locked",
            Self::ActiveIntentConflict => "active_intent_conflict",
            Self::RunAlreadyExists => "run_already_exists",
            Self::IdempotencyConflict => "idempotency_conflict",
            Self::TransitionConflict { .. } => "transition_conflict",
            Self::StalePackage => "stale_package",
            Self::ResourceLimit { .. } => "resource_limit",
            Self::CapabilityMismatch { .. } => "capability_mismatch",
            Self::EvidenceBlocker { .. } => "evidence_blocker",
            Self::AttentionRequired => "attention_required",
            Self::ExternalWait { .. } => "external_wait",
            Self::MissingInputArtifact { .. } => "missing_input_artifact",
            Self::InvalidArtifactSchema { .. } => "invalid_artifact_schema",
            Self::GateRejected { .. } => "gate_rejected",
            Self::GateWaitApproval { .. } => "waiting_approval",
            Self::RoleNotFound { .. } => "role_not_found",
            Self::InvalidRoleSequence { .. } => "invalid_role_sequence",
            Self::BudgetExceeded { .. } => "budget_exceeded",
            Self::ProjectNotFound { .. } => "project_not_found",
            Self::ArtifactNotFound { .. } => "artifact_not_found",
            Self::SuggestionNotFound { .. } => "suggestion_not_found",
            Self::Database(_) => "database_error",
            Self::AgentExecution(_) => "agent_execution_failed",
            Self::Serialization(_) => "serialization_error",
            Self::YamlParse(_) => "yaml_parse_error",
            Self::Unauthorized { .. } => "unauthorized",
        }
    }

    pub fn details(&self) -> Option<&serde_json::Value> {
        match self {
            Self::ExternalWait { details, .. } | Self::EvidenceBlocker { details, .. } => {
                Some(details)
            }
            _ => None,
        }
    }
}
