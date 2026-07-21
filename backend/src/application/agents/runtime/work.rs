//! 作品 Agent adapter：展示读取主画面、规划和校验步骤；Agent 只推荐模型和提示词。

use super::{AgentRuntime, AgentRuntimeError};
use crate::domain::conversation::{
    AgentConversation, AgentMessage, AgentMessageRole, AgentRunRecord, CreateAgentMessageInput,
    CreateAgentStepInput,
};
use novex_model::LLMPrompt;
use serde_json::json;

impl AgentRuntime {
    pub(super) async fn handle_work_turn(
        &self,
        conversation: &AgentConversation,
        user_message: &AgentMessage,
        run: &AgentRunRecord,
    ) -> Result<AgentMessage, AgentRuntimeError> {
        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 1,
                step_type: "read_work_manifest".into(),
                status: "succeeded".into(),
                input: json!({"subject_id": conversation.subject_id}),
                output: Some(json!({"visible": true})),
                error_message: None,
            })
            .await?;
        let prompt = LLMPrompt { system: "你是作品生成 Agent。只提供可见的全片方案、分段提示词和模型建议，不调用视频模型；必须等待用户确认后才生成。".into(), user: user_message.content.clone(), max_output_tokens: None, output_schema: None };
        let reply = self.llm_client.generate_script(prompt).await?;
        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 2,
                step_type: "recommend_work_plan".into(),
                status: "succeeded".into(),
                input: json!({"message_id": user_message.id}),
                output: Some(json!({"requires_confirmation": true, "seedance_called": false})),
                error_message: None,
            })
            .await?;
        self.conversation_repository.save_message(CreateAgentMessageInput { conversation_id: conversation.id, role: AgentMessageRole::Assistant, content: reply.trim().to_string(), metadata: json!({"intent": "recommend_work_plan", "requires_confirmation": true, "seedance_called": false, "tool_execution": false}) }).await.map_err(AgentRuntimeError::from)
    }
}
