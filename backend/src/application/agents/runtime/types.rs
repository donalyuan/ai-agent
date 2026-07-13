//! 定义统一 Runtime 的单轮请求和成功响应，避免能力模块各自声明传输结构。

use crate::domain::conversation::{AgentMessage, AgentRunRecord};
use uuid::Uuid;

/// 单次 Agent 轮次输入；补充批次仅对选题生成能力有意义。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentTurnRequest {
    pub conversation_id: Uuid,
    pub user_message: String,
    pub supplement_of_batch_id: Option<Uuid>,
}

/// 单次 Agent 轮次完成后持久化的用户消息、回复消息和运行记录。
#[derive(Clone, Debug, PartialEq)]
pub struct AgentTurnResponse {
    pub user_message: AgentMessage,
    pub agent_message: AgentMessage,
    pub run: AgentRunRecord,
}
