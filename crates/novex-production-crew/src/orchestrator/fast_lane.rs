//! Fast Lane 执行器：简化流程，直接生成视频，不走完整团队

use crate::error::ProductionResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Fast Lane 生成任务结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastLaneResult {
    /// 关联项目 UUID
    pub project_id: Uuid,
    /// 异步 job ID（提交到 Worker 队列后返回）
    pub job_id: Uuid,
    /// 初始状态
    pub status: FastLaneStatus,
    /// 预计完成秒数
    pub estimated_time_seconds: u64,
}

/// Fast Lane 任务状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FastLaneStatus {
    /// 已入队，等待 Worker 处理
    Queued,
    /// Worker 正在处理
    Processing,
    /// 完成，video_url 可用
    Completed,
    /// 生成失败
    Failed,
}

/// Fast Lane 执行请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastLaneRequest {
    /// 用户原始 Prompt
    pub prompt: String,
    /// 目标平台
    pub platform: Option<String>,
    /// 目标时长（秒）
    pub duration_seconds: Option<u32>,
}

/// 执行 Fast Lane：将请求入队后立即返回 job_id
///
/// 实际视频生成通过 Redis 队列异步完成，不在此阻塞等待。
pub async fn execute_fast_lane(
    project_id: Uuid,
    request: FastLaneRequest,
) -> ProductionResult<FastLaneResult> {
    // TODO: 实际实现需注入 Redis client 并将任务推入生成队列
    // 当前为 stub，用于建立接口约定
    let job_id = Uuid::new_v4();
    tracing::info!(
        project_id = %project_id,
        job_id = %job_id,
        prompt = %request.prompt,
        "Fast Lane 任务已入队"
    );
    Ok(FastLaneResult {
        project_id,
        job_id,
        status: FastLaneStatus::Queued,
        estimated_time_seconds: 180,
    })
}
