//! 统一 Agent Runtime 门面；具体脚本、选题、质量和评审能力由子模块实现。

mod error;
mod prompt;
mod script;
mod sound;
mod topic_generation;
mod topic_quality;
mod topic_review;
mod types;
mod work;

pub use error::AgentRuntimeError;
pub use prompt::format_account_strategy_context;
pub use types::{AgentTurnRequest, AgentTurnResponse, SoundAgentContext};

use prompt::truncate_for_prompt;

use crate::domain::conversation::{
    AgentMessageRole, CreateAgentMessageInput, CreateAgentRunInput, FinishAgentRunInput,
};
use crate::repositories::{
    ConversationRepository, PostgresVoiceCatalogRepository, ProjectRepository, ScriptRepository,
    TopicRepository,
};
use novex_model::{LLMClient, ModelExecutionSnapshot};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
/// 统一加载对话上下文、创建 run，并把单轮消息分派给对应 Agent 能力。
pub struct AgentRuntime {
    conversation_repository: Arc<dyn ConversationRepository>,
    script_repository: Arc<dyn ScriptRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    topic_repository: Option<Arc<dyn TopicRepository>>,
    voice_catalog_repository: Option<Arc<PostgresVoiceCatalogRepository>>,
    llm_client: Arc<dyn LLMClient>,
    model_execution: Option<ModelExecutionSnapshot>,
}

impl AgentRuntime {
    pub fn new(
        conversation_repository: Arc<dyn ConversationRepository>,
        script_repository: Arc<dyn ScriptRepository>,
        project_repository: Arc<dyn ProjectRepository>,
        llm_client: Arc<dyn LLMClient>,
    ) -> Self {
        Self {
            conversation_repository,
            script_repository,
            project_repository,
            topic_repository: None,
            voice_catalog_repository: None,
            llm_client,
            model_execution: None,
        }
    }

    pub fn with_topic_repository(mut self, topic_repository: Arc<dyn TopicRepository>) -> Self {
        self.topic_repository = Some(topic_repository);
        self
    }

    pub fn with_model_execution(mut self, snapshot: ModelExecutionSnapshot) -> Self {
        self.model_execution = Some(snapshot);
        self
    }

    pub fn with_voice_catalog_repository(
        mut self,
        repository: Arc<PostgresVoiceCatalogRepository>,
    ) -> Self {
        self.voice_catalog_repository = Some(repository);
        self
    }

    /// 持久化用户消息与 run 后再分派；无论能力成功或失败，都在返回前收尾 run。
    pub async fn handle_turn(
        &self,
        request: AgentTurnRequest,
    ) -> Result<AgentTurnResponse, AgentRuntimeError> {
        self.handle_turn_with_sound_context(request, None).await
    }

    /// 声音上下文只通过声音消息入口传入；其他 Agent 继续使用通用单轮接口。
    pub async fn handle_turn_with_sound_context(
        &self,
        request: AgentTurnRequest,
        sound_context: Option<SoundAgentContext>,
    ) -> Result<AgentTurnResponse, AgentRuntimeError> {
        if request.user_message.trim().is_empty() {
            return Err(AgentRuntimeError::Validation("消息不能为空".to_string()));
        }

        let conversation = self
            .conversation_repository
            .get_conversation(request.conversation_id)
            .await?;
        validate_sound_context(&conversation, sound_context.as_ref())?;
        let user_metadata = sound_context
            .as_ref()
            .map(|context| json!({"sound_context": context}))
            .unwrap_or_else(|| json!({}));
        let user_message = self
            .conversation_repository
            .save_message(CreateAgentMessageInput {
                conversation_id: conversation.id,
                role: AgentMessageRole::User,
                content: request.user_message.trim().to_string(),
                metadata: user_metadata,
            })
            .await?;
        let mut run_input = json!({"user_message_id": user_message.id});
        if let Some(context) = sound_context.as_ref() {
            run_input
                .as_object_mut()
                .expect("Agent run input 固定为 object")
                .insert("sound_context".to_string(), json!(context));
        }
        let run = self
            .conversation_repository
            .create_run(CreateAgentRunInput {
                conversation_id: conversation.id,
                project_id: conversation.project_id,
                agent_type: conversation.agent_type.clone(),
                input: run_input,
                model_id: self
                    .model_execution
                    .as_ref()
                    .map(|snapshot| snapshot.model_id),
                model_snapshot: self
                    .model_execution
                    .as_ref()
                    .and_then(|snapshot| serde_json::to_value(snapshot).ok()),
            })
            .await?;

        let result = match conversation.agent_type.as_str() {
            "script" => {
                self.handle_script_turn(&conversation, &user_message, &run)
                    .await
            }
            "topic" => {
                self.handle_topic_turn(
                    &conversation,
                    &user_message,
                    &run,
                    request.supplement_of_batch_id,
                )
                .await
            }
            "sound" => {
                self.handle_sound_turn(
                    &conversation,
                    &user_message,
                    &run,
                    sound_context.as_ref().expect("声音上下文已在分派前校验"),
                )
                .await
            }
            "work" => {
                self.handle_work_turn(&conversation, &user_message, &run)
                    .await
            }
            agent_type => Err(AgentRuntimeError::UnsupportedAgent(agent_type.to_string())),
        };

        match result {
            Ok(agent_message) => {
                let finished_run = self
                    .conversation_repository
                    .finish_run(FinishAgentRunInput {
                        agent_run_id: run.id,
                        status: "succeeded".to_string(),
                        output: Some(json!({ "assistant_message_id": agent_message.id })),
                        error_message: None,
                    })
                    .await?;

                Ok(AgentTurnResponse {
                    user_message,
                    agent_message,
                    run: finished_run,
                })
            }
            Err(error) => {
                let _ = self
                    .conversation_repository
                    .finish_run(FinishAgentRunInput {
                        agent_run_id: run.id,
                        status: "failed".to_string(),
                        output: None,
                        error_message: Some(error.to_string()),
                    })
                    .await;
                Err(error)
            }
        }
    }
}

fn validate_sound_context(
    conversation: &crate::domain::conversation::AgentConversation,
    sound_context: Option<&SoundAgentContext>,
) -> Result<(), AgentRuntimeError> {
    if conversation.agent_type != "sound" {
        if sound_context.is_some() {
            return Err(AgentRuntimeError::Validation(
                "非声音会话不能携带声音上下文".to_string(),
            ));
        }
        return Ok(());
    }
    let context = sound_context
        .ok_or_else(|| AgentRuntimeError::Validation("声音消息缺少当前编辑上下文".to_string()))?;
    context.validate().map_err(AgentRuntimeError::Validation)?;
    let conversation_model_id = conversation
        .metadata
        .get("speech_model_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .ok_or_else(|| AgentRuntimeError::Validation("声音会话缺少有效 TTS 模型".to_string()))?;
    if context.speech_model_id != conversation_model_id {
        return Err(AgentRuntimeError::Validation(
            "声音消息上下文与会话 TTS 模型不一致".to_string(),
        ));
    }
    Ok(())
}
