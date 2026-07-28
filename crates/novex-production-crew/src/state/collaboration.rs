//! 协作建议管理：角色间提出和响应修改意见

use crate::error::ProductionResult;
use crate::state::repository::ProductionStateRepository;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

/// 协作建议管理器
pub struct CollaborationManager {
    repo: Arc<ProductionStateRepository>,
}

impl CollaborationManager {
    pub fn new(repo: Arc<ProductionStateRepository>) -> Self {
        Self { repo }
    }

    /// 创建协作建议（角色 → 另一角色，针对特定产物）
    pub async fn create_suggestion(
        &self,
        project_id: Uuid,
        data: Value,
    ) -> ProductionResult<Value> {
        self.repo
            .create_collaboration_suggestion(project_id, data)
            .await
    }

    /// 列出项目协作建议（可按 to_role / status 过滤）
    pub async fn get_suggestions_by_project(
        &self,
        project_id: Uuid,
        to_role: Option<String>,
        status: Option<String>,
    ) -> ProductionResult<(Vec<Value>, i64)> {
        self.repo
            .list_collaboration_suggestions(project_id, to_role, status)
            .await
    }

    /// 列出某角色所有 pending 建议
    pub async fn get_pending_suggestions_for_role(
        &self,
        project_id: Uuid,
        role_key: &str,
    ) -> ProductionResult<Vec<Value>> {
        let (items, _) = self
            .repo
            .list_collaboration_suggestions(
                project_id,
                Some(role_key.to_string()),
                Some("pending".to_string()),
            )
            .await?;
        Ok(items)
    }

    /// 响应建议（接受或拒绝）
    pub async fn respond_to_suggestion(
        &self,
        project_id: Uuid,
        suggestion_id: Uuid,
        user_id: Uuid,
        status: String,
        note: Option<String>,
    ) -> ProductionResult<Value> {
        self.repo
            .respond_to_suggestion(project_id, suggestion_id, user_id, status, note)
            .await
    }
}
