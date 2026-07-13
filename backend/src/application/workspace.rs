//! 工作台菜单查询用例，不依赖 HTTP 状态或应用组装容器。

use crate::repositories::{
    PostgresWorkspaceMenuRepository, WorkspaceMenuRepositoryError, WorkspaceMenuTreeNode,
};
use std::fmt;

#[derive(Clone)]
/// 读取数据库持久化的可见工作台菜单树。
pub struct WorkspaceService {
    repository: PostgresWorkspaceMenuRepository,
}

impl WorkspaceService {
    pub fn new(repository: PostgresWorkspaceMenuRepository) -> Self {
        Self { repository }
    }

    pub async fn list_visible_menu_tree(
        &self,
    ) -> Result<Vec<WorkspaceMenuTreeNode>, WorkspaceApplicationError> {
        self.repository
            .list_visible_menu_tree()
            .await
            .map_err(Into::into)
    }
}

#[derive(Debug)]
pub enum WorkspaceApplicationError {
    Repository(WorkspaceMenuRepositoryError),
}

impl From<WorkspaceMenuRepositoryError> for WorkspaceApplicationError {
    fn from(error: WorkspaceMenuRepositoryError) -> Self {
        Self::Repository(error)
    }
}

impl fmt::Display for WorkspaceApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Repository(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for WorkspaceApplicationError {}
