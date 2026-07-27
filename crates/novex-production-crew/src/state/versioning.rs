//! 产物版本管理：approve 操作的 supersede 逻辑

use crate::error::ProductionResult;
use crate::state::repository::ProductionStateRepository;
use std::sync::Arc;
use uuid::Uuid;

/// 产物版本管理器：封装版本状态流转逻辑
pub struct ArtifactVersionManager {
    repo: Arc<ProductionStateRepository>,
}

impl ArtifactVersionManager {
    pub fn new(repo: Arc<ProductionStateRepository>) -> Self {
        Self { repo }
    }

    /// 批准产物：自动将同类型旧 approved 版本改为 superseded
    pub async fn approve_artifact(
        &self,
        project_id: Uuid,
        artifact_type: &str,
        artifact_id: Uuid,
        user_id: Uuid,
    ) -> ProductionResult<()> {
        self.repo
            .approve_artifact(project_id, artifact_type, artifact_id, user_id)
            .await
    }
}
