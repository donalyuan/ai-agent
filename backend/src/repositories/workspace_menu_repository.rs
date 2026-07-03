use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Row};
use std::{collections::HashMap, fmt};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceMenu {
    pub id: Uuid,
    pub parent_id: Option<Uuid>,
    pub menu_key: String,
    pub label: String,
    pub description: String,
    pub route_path: Option<String>,
    pub icon: String,
    pub menu_type: String,
    pub module_key: Option<String>,
    pub agent_key: Option<String>,
    pub sort_order: i32,
    pub is_enabled: bool,
    pub status: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceMenuTreeNode {
    pub menu: WorkspaceMenu,
    pub children: Vec<WorkspaceMenuTreeNode>,
}

#[derive(Clone)]
pub struct PostgresWorkspaceMenuRepository {
    pool: PgPool,
}

impl PostgresWorkspaceMenuRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list_visible_menu_tree(
        &self,
    ) -> Result<Vec<WorkspaceMenuTreeNode>, WorkspaceMenuRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT
                id,
                parent_id,
                menu_key,
                label,
                description,
                route_path,
                icon,
                menu_type,
                module_key,
                agent_key,
                sort_order,
                is_enabled,
                status,
                metadata
            FROM video_workspace_menus
            WHERE is_visible = true
            ORDER BY parent_id NULLS FIRST, sort_order ASC, menu_key ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(WorkspaceMenuRepositoryError::from)?;

        let mut grouped: HashMap<Option<Uuid>, Vec<WorkspaceMenu>> = HashMap::new();
        for row in rows {
            let menu = workspace_menu_from_row(row);
            grouped.entry(menu.parent_id).or_default().push(menu);
        }

        Ok(build_tree(None, &mut grouped))
    }
}

fn build_tree(
    parent_id: Option<Uuid>,
    grouped: &mut HashMap<Option<Uuid>, Vec<WorkspaceMenu>>,
) -> Vec<WorkspaceMenuTreeNode> {
    grouped
        .remove(&parent_id)
        .unwrap_or_default()
        .into_iter()
        .map(|menu| {
            let children = build_tree(Some(menu.id), grouped);
            WorkspaceMenuTreeNode { menu, children }
        })
        .collect()
}

fn workspace_menu_from_row(row: PgRow) -> WorkspaceMenu {
    WorkspaceMenu {
        id: row.get("id"),
        parent_id: row.get("parent_id"),
        menu_key: row.get("menu_key"),
        label: row.get("label"),
        description: row.get("description"),
        route_path: row.get("route_path"),
        icon: row.get("icon"),
        menu_type: row.get("menu_type"),
        module_key: row.get("module_key"),
        agent_key: row.get("agent_key"),
        sort_order: row.get("sort_order"),
        is_enabled: row.get("is_enabled"),
        status: row.get("status"),
        metadata: row.get("metadata"),
    }
}

#[derive(Debug)]
pub enum WorkspaceMenuRepositoryError {
    Storage(String),
}

impl From<sqlx::Error> for WorkspaceMenuRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for WorkspaceMenuRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(message) => write!(formatter, "workspace menu storage error: {message}"),
        }
    }
}

impl std::error::Error for WorkspaceMenuRepositoryError {}
