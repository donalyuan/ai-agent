//! Production API 请求/响应 DTO

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 创建制作项目请求
#[derive(Debug, Deserialize)]
pub struct CreateProductionRequest {
    pub title: String,
    pub description: Option<String>,
    pub project_type: String, // "fast_lane" | "full_crew"
    pub initial_input: serde_json::Value,
}

/// 项目响应
#[derive(Debug, Serialize)]
pub struct ProductionResponse {
    pub id: Uuid,
    pub title: String,
    pub project_type: String,
    pub status: String,
    pub user_id: Uuid,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_role: Option<String>,
}

/// 列表响应（分页）
#[derive(Debug, Serialize)]
pub struct ProductionListResponse {
    pub items: Vec<ProductionSummary>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

/// 项目摘要
#[derive(Debug, Serialize)]
pub struct ProductionSummary {
    pub id: Uuid,
    pub title: String,
    pub project_type: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 执行角色请求
#[derive(Debug, Deserialize)]
pub struct ExecuteRoleRequest {
    #[serde(default)]
    pub user_input: Option<String>,
    #[serde(default)]
    pub context: Option<serde_json::Value>,
}

/// 角色执行结果响应
#[derive(Debug, Serialize)]
pub struct RoleExecutionResponse {
    pub role: String,
    pub status: String,
    pub execution_time_ms: u64,
    pub output_artifacts: Vec<ArtifactSummaryDto>,
    pub model_call_id: Option<Uuid>,
    pub next_role: Option<String>,
}

/// 产物摘要DTO
#[derive(Debug, Serialize)]
pub struct ArtifactSummaryDto {
    #[serde(rename = "type")]
    pub artifact_type: String,
    pub id: Uuid,
    pub version: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shot_id: Option<String>,
}

/// 执行流程请求
#[derive(Debug, Deserialize)]
pub struct ExecuteFlowRequest {
    pub roles: Vec<String>,
    #[serde(default)]
    pub auto_approve: bool,
    #[serde(default)]
    pub user_input: Option<String>,
}

/// 流程状态响应
#[derive(Debug, Serialize)]
pub struct FlowStatusResponse {
    pub flow_id: Uuid,
    pub status: String,
    pub completed_roles: Vec<String>,
    pub current_role: Option<String>,
    pub pending_roles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_for: Option<GateWaitingDto>,
}

/// Gate等待信息DTO
#[derive(Debug, Serialize)]
pub struct GateWaitingDto {
    #[serde(rename = "type")]
    pub gate_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<Uuid>,
}

/// Fast Lane请求
#[derive(Debug, Deserialize)]
pub struct FastLaneRequest {
    pub prompt: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub duration_seconds: Option<u32>,
}

/// Fast Lane响应
#[derive(Debug, Serialize)]
pub struct FastLaneResponse {
    pub job_id: Uuid,
    pub status: String,
    pub estimated_time_seconds: u64,
}
