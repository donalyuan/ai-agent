//! 定义统一 Runtime 的单轮请求和成功响应，避免能力模块各自声明传输结构。

use crate::domain::conversation::{AgentMessage, AgentRunRecord};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// 声音 Agent 在发送瞬间读取的编辑区快照，避免依赖“旁边文本”等界面隐式引用。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SoundAgentContext {
    pub speech_model_id: Uuid,
    pub tts_text: String,
    pub voice_type: String,
    pub language: String,
    pub parameters: Value,
    pub subtitle_segments: Vec<String>,
}

impl SoundAgentContext {
    pub fn validate(&self) -> Result<(), String> {
        if self.speech_model_id.is_nil() {
            return Err("声音消息必须携带有效 TTS 模型".to_string());
        }
        if self.voice_type.trim().is_empty() || self.language.trim().is_empty() {
            return Err("声音消息必须携带当前音色和语言".to_string());
        }
        if !self.parameters.is_object() {
            return Err("声音消息 parameters 必须是 object".to_string());
        }
        if self
            .subtitle_segments
            .iter()
            .any(|segment| segment.trim().is_empty())
        {
            return Err("声音消息字幕断句不能包含空项".to_string());
        }
        Ok(())
    }

    pub fn normalized(mut self) -> Self {
        self.tts_text = self.tts_text.trim().to_string();
        self.voice_type = self.voice_type.trim().to_string();
        self.language = self.language.trim().to_string();
        self.subtitle_segments = self
            .subtitle_segments
            .into_iter()
            .map(|segment| segment.trim().to_string())
            .filter(|segment| !segment.is_empty())
            .collect();
        self
    }
}

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
