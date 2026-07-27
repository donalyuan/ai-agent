//! Gate trait：质量闸门抽象，在角色执行的关键节点检查产物状态

use crate::error::ProductionResult;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// 闸门决策结果
#[derive(Debug, Clone)]
pub enum GateDecision {
    /// 通过，继续执行下一步
    Pass,
    /// 拒绝，阻断流程并返回原因
    Reject { reason: String },
    /// 等待人工审批
    WaitApproval { artifact_id: Uuid },
}

/// 闸门执行上下文：包含项目信息和当前所有产物快照
pub struct GateContext {
    /// 项目 UUID
    pub project_id: Uuid,
    /// 用户 UUID（用于权限校验和预算查询）
    pub user_id: Uuid,
    /// 当前所有产物数据（artifact_type -> JSONB 数组）
    pub artifacts: HashMap<String, Vec<Value>>,
    /// 项目元数据（budget、platform 等）
    pub project_metadata: Value,
}

/// Gate trait：所有质量闸门必须实现此接口
#[async_trait]
pub trait Gate: Send + Sync {
    /// 闸门名称，用于注册和日志记录
    fn name(&self) -> &str;

    /// 执行闸门检查，返回通过/拒绝/等待审批三种决策之一
    async fn check(&self, context: &GateContext) -> ProductionResult<GateDecision>;
}
