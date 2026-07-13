use crate::repositories::WorkspaceMenuTreeNode;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkspaceMenuListResponse {
    pub menus: Vec<WorkspaceMenuNodeResponse>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkspaceMenuNodeResponse {
    pub menu_id: Uuid,
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
    pub children: Vec<WorkspaceMenuNodeResponse>,
}

impl From<WorkspaceMenuTreeNode> for WorkspaceMenuNodeResponse {
    fn from(node: WorkspaceMenuTreeNode) -> Self {
        Self {
            menu_id: node.menu.id,
            menu_key: node.menu.menu_key,
            label: node.menu.label,
            description: node.menu.description,
            route_path: node.menu.route_path,
            icon: node.menu.icon,
            menu_type: node.menu.menu_type,
            module_key: node.menu.module_key,
            agent_key: node.menu.agent_key,
            sort_order: node.menu.sort_order,
            is_enabled: node.menu.is_enabled,
            status: node.menu.status,
            metadata: node.menu.metadata,
            children: node
                .children
                .into_iter()
                .map(WorkspaceMenuNodeResponse::from)
                .collect(),
        }
    }
}
