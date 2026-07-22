//! 作品 Agent adapter：把自然语言意图转换为受约束快照补丁，并原子生成待确认差异。

use super::{AgentRuntime, AgentRuntimeError};
use crate::domain::conversation::{
    AgentConversation, AgentMessage, AgentMessageRole, AgentRunRecord, CreateAgentMessageInput,
    CreateAgentStepInput,
};
use novex_model::LLMPrompt;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkAgentOutput {
    assistant_message: String,
    #[serde(default)]
    input_snapshot_patch: Option<Value>,
    #[serde(default)]
    model_snapshot_patch: Option<Value>,
    #[serde(default)]
    parameter_snapshot_patch: Option<Value>,
    #[serde(default)]
    prompt_snapshot_patch: Option<Value>,
    #[serde(default)]
    timeline_snapshot_patch: Option<Value>,
}

impl WorkAgentOutput {
    fn validate(&self) -> Result<(), AgentRuntimeError> {
        if self.assistant_message.trim().is_empty() {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "assistant_message 不能为空".to_string(),
            ));
        }
        let patches = self.patches();
        if patches.iter().all(|patch| patch.is_none()) {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "至少需要一个非空 snapshot patch".to_string(),
            ));
        }
        if patches.into_iter().flatten().any(|patch| {
            !patch.is_object() || patch.as_object().is_some_and(|value| value.is_empty())
        }) {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "snapshot patch 必须是非空 object".to_string(),
            ));
        }
        Ok(())
    }

    fn patches(&self) -> [&Option<Value>; 5] {
        [
            &self.input_snapshot_patch,
            &self.model_snapshot_patch,
            &self.parameter_snapshot_patch,
            &self.prompt_snapshot_patch,
            &self.timeline_snapshot_patch,
        ]
    }
}

impl AgentRuntime {
    pub(super) async fn handle_work_turn(
        &self,
        conversation: &AgentConversation,
        user_message: &AgentMessage,
        run: &AgentRunRecord,
    ) -> Result<AgentMessage, AgentRuntimeError> {
        let project_id = conversation
            .project_id
            .ok_or_else(|| AgentRuntimeError::Validation("作品会话必须绑定项目".to_string()))?;
        let Some(work_id) = conversation.subject_id else {
            if !self.project_repository.project_exists(project_id).await? {
                return Err(AgentRuntimeError::Validation(
                    "作品会话绑定项目不存在".to_string(),
                ));
            }
            self.conversation_repository
                .add_step(CreateAgentStepInput {
                    agent_run_id: run.id,
                    step_order: 1,
                    step_type: "recommend_work_plan".into(),
                    status: "succeeded".into(),
                    input: json!({"message_id": user_message.id}),
                    output: Some(
                        json!({"requires_confirmation": true, "downstream_called": false}),
                    ),
                    error_message: None,
                })
                .await?;
            let reply = self.llm_client.generate_script(LLMPrompt {
                system: "你是作品生成 Agent。只提供可见的全片方案、分段提示词和模型建议，不调用视频模型；必须等待用户确认后才生成。".into(),
                user: user_message.content.clone(),
                max_output_tokens: Some(1200),
                output_schema: None,
            }).await?;
            return self.conversation_repository.save_message(CreateAgentMessageInput {
                conversation_id: conversation.id,
                role: AgentMessageRole::Assistant,
                content: reply.trim().to_string(),
                metadata: json!({"intent": "recommend_work_plan", "requires_confirmation": true, "downstream_called": false}),
            }).await.map_err(AgentRuntimeError::from);
        };
        let repository = self.work_library_repository.as_ref().ok_or_else(|| {
            AgentRuntimeError::Validation("作品 Agent 缺少作品库仓储".to_string())
        })?;
        if conversation.subject_type.as_deref() != Some("work") {
            return Err(AgentRuntimeError::Validation(
                "作品会话 subject_type 必须为 work".to_string(),
            ));
        }
        let context = repository
            .work_agent_context(work_id, project_id)
            .await
            .map_err(|error| AgentRuntimeError::Validation(error.to_string()))?;
        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 1,
                step_type: "read_work_manifest".into(),
                status: "succeeded".into(),
                input: json!({"subject_id": conversation.subject_id}),
                output: Some(json!({"work_id": work_id, "version_no": context["current_version"]["version_no"]})),
                error_message: None,
            })
            .await?;
        let prompt = LLMPrompt {
            system: "你是作品修改 Agent。只输出一个 JSON object，不要 Markdown。字段只能包含 assistant_message、input_snapshot_patch、model_snapshot_patch、parameter_snapshot_patch、prompt_snapshot_patch、timeline_snapshot_patch。assistant_message 用简体中文说明修改；至少提供一个非空 patch object。只修改用户明确要求的字段，不调用视频、TTS、ASR 或发布能力。".into(),
            user: format!(
                "当前作品和草稿：\n{}\n\n用户修改要求：\n{}",
                serde_json::to_string_pretty(&context).unwrap_or_else(|_| "{}".to_string()),
                user_message.content
            ),
            max_output_tokens: Some(1800),
            output_schema: None,
        };
        let raw = self.llm_client.generate_script(prompt).await?;
        let output: WorkAgentOutput = serde_json::from_str(raw.trim()).map_err(|error| {
            AgentRuntimeError::InvalidLlmOutput(format!("作品补丁 JSON 无效: {error}"))
        })?;
        output.validate()?;
        let (draft, diff) = repository
            .apply_agent_edit(work_id, project_id, output.patches())
            .await?;
        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 2,
                step_type: "apply_work_patch".into(),
                status: "succeeded".into(),
                input: json!({"message_id": user_message.id}),
                output: Some(json!({"draft_version_id": draft.id, "diff_plan_id": diff.id, "requires_confirmation": true, "downstream_called": false})),
                error_message: None,
            })
            .await?;
        self.conversation_repository
            .save_message(CreateAgentMessageInput {
                conversation_id: conversation.id,
                role: AgentMessageRole::Assistant,
                content: output.assistant_message.trim().to_string(),
                metadata: json!({
                    "intent": "edit_work_draft",
                    "draft_version_id": draft.id,
                    "version_no": draft.version_no,
                    "diff": diff,
                    "requires_confirmation": true,
                    "downstream_called": false,
                }),
            })
            .await
            .map_err(AgentRuntimeError::from)
    }
}
