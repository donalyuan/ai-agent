/// 虚拟制作团队统一 Result 类型别名
pub type ProductionResult<T> = std::result::Result<T, ProductionError>;

/// 虚拟制作团队统一错误类型
#[derive(Debug, thiserror::Error)]
pub enum ProductionError {
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
    ArtifactNotFound { artifact_type: String, artifact_id: uuid::Uuid },

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
