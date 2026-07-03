use crate::agents::conversation::{
    AgentConversation, AgentMessage, AgentMessageRole, AgentRunRecord, CreateAgentMessageInput,
    CreateAgentRunInput, CreateAgentStepInput, FinishAgentRunInput,
};
use crate::agents::models::{Scene, Script};
use crate::repositories::{
    ConversationRepository, ConversationRepositoryError, ScriptRepository, ScriptRepositoryError,
};
use novex_model::{LLMClient, LLMError, LLMPrompt};
use serde::Deserialize;
use serde_json::json;
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AgentRuntime {
    conversation_repository: Arc<dyn ConversationRepository>,
    script_repository: Arc<dyn ScriptRepository>,
    llm_client: Arc<dyn LLMClient>,
}

impl AgentRuntime {
    pub fn new(
        conversation_repository: Arc<dyn ConversationRepository>,
        script_repository: Arc<dyn ScriptRepository>,
        llm_client: Arc<dyn LLMClient>,
    ) -> Self {
        Self {
            conversation_repository,
            script_repository,
            llm_client,
        }
    }

    pub async fn handle_turn(
        &self,
        request: AgentTurnRequest,
    ) -> Result<AgentTurnResponse, AgentRuntimeError> {
        if request.user_message.trim().is_empty() {
            return Err(AgentRuntimeError::Validation("消息不能为空".to_string()));
        }

        let conversation = self
            .conversation_repository
            .get_conversation(request.conversation_id)
            .await?;
        let user_message = self
            .conversation_repository
            .save_message(CreateAgentMessageInput {
                conversation_id: conversation.id,
                role: AgentMessageRole::User,
                content: request.user_message.trim().to_string(),
                metadata: json!({}),
            })
            .await?;
        let run = self
            .conversation_repository
            .create_run(CreateAgentRunInput {
                conversation_id: conversation.id,
                project_id: conversation.project_id,
                agent_type: conversation.agent_type.clone(),
                input: json!({ "user_message_id": user_message.id }),
            })
            .await?;

        let result = match conversation.agent_type.as_str() {
            "script" => {
                self.handle_script_turn(&conversation, &user_message, &run)
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

    async fn handle_script_turn(
        &self,
        conversation: &AgentConversation,
        user_message: &AgentMessage,
        run: &AgentRunRecord,
    ) -> Result<AgentMessage, AgentRuntimeError> {
        let script_id = conversation
            .subject_id
            .ok_or_else(|| AgentRuntimeError::Validation("脚本会话缺少 subject_id".to_string()))?;
        if conversation.subject_type.as_deref() != Some("script") {
            return Err(AgentRuntimeError::Validation(
                "脚本会话 subject_type 必须为 script".to_string(),
            ));
        }

        let script = self.script_repository.get_script(script_id).await?;
        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 1,
                step_type: "read_script".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "script_id": script_id }),
                output: Some(json!({ "scene_count": script.scenes.len() })),
                error_message: None,
            })
            .await?;

        let prompt = build_script_scene_patch_prompt(&script, &user_message.content);
        let raw = self.llm_client.generate_script(prompt).await?;
        let patch = ScriptScenePatch::parse(&raw)?;
        let existing_scene = script
            .scenes
            .iter()
            .find(|scene| scene.sequence == patch.scene_sequence)
            .cloned()
            .ok_or(AgentRuntimeError::SceneNotFound {
                script_id,
                sequence: patch.scene_sequence,
            })?;

        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 2,
                step_type: "llm_scene_patch".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "message_id": user_message.id }),
                output: Some(json!({ "scene_sequence": patch.scene_sequence })),
                error_message: None,
            })
            .await?;

        let updated_scene = Scene {
            id: existing_scene.id,
            sequence: patch.scene_sequence,
            narration: patch.narration,
            visual_description: patch.visual_description,
            emotion: patch.emotion,
            duration_sec: patch.duration_sec,
        };
        let updated_script = self
            .script_repository
            .update_scene(script_id, updated_scene)
            .await?;

        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 3,
                step_type: "update_scene".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "script_id": script_id, "scene_sequence": patch.scene_sequence }),
                output: Some(json!({ "updated_at": updated_script.updated_at })),
                error_message: None,
            })
            .await?;

        let content = if patch.reply.trim().is_empty() {
            format!("已修改第 {} 镜。", patch.scene_sequence)
        } else {
            patch.reply
        };

        self.conversation_repository
            .save_message(CreateAgentMessageInput {
                conversation_id: conversation.id,
                role: AgentMessageRole::Assistant,
                content,
                metadata: json!({
                    "script_id": script_id,
                    "scene_sequence": patch.scene_sequence,
                }),
            })
            .await
            .map_err(AgentRuntimeError::from)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentTurnRequest {
    pub conversation_id: Uuid,
    pub user_message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AgentTurnResponse {
    pub user_message: AgentMessage,
    pub agent_message: AgentMessage,
    pub run: AgentRunRecord,
}

#[derive(Debug, Deserialize)]
struct ScriptScenePatch {
    scene_sequence: i32,
    narration: String,
    visual_description: String,
    emotion: String,
    duration_sec: i32,
    #[serde(default)]
    reply: String,
}

impl ScriptScenePatch {
    fn parse(raw: &str) -> Result<Self, AgentRuntimeError> {
        let json_text = extract_json_object(raw)?;
        let patch: Self = serde_json::from_str(json_text)
            .map_err(|error| AgentRuntimeError::InvalidLlmOutput(error.to_string()))?;
        patch.validate()?;
        Ok(patch)
    }

    fn validate(&self) -> Result<(), AgentRuntimeError> {
        if self.scene_sequence <= 0 {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "scene_sequence must be greater than 0".to_string(),
            ));
        }
        if self.narration.trim().is_empty() {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "narration must not be empty".to_string(),
            ));
        }
        if self.visual_description.trim().is_empty() {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "visual_description must not be empty".to_string(),
            ));
        }
        if self.emotion.trim().is_empty() {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "emotion must not be empty".to_string(),
            ));
        }
        if !(1..=30).contains(&self.duration_sec) {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "duration_sec must be between 1 and 30".to_string(),
            ));
        }
        Ok(())
    }
}

fn build_script_scene_patch_prompt(script: &Script, user_message: &str) -> LLMPrompt {
    let scenes = script
        .scenes
        .iter()
        .map(|scene| {
            format!(
                "第 {} 镜：旁白={}；画面={}；情绪={}；时长={}秒",
                scene.sequence,
                scene.narration,
                scene.visual_description,
                scene.emotion,
                scene.duration_sec
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    LLMPrompt {
        system: "你是短视频脚本改稿 Agent。你必须只输出合法 JSON，不要输出 Markdown 或解释。"
            .to_string(),
        user: format!(
            r#"用户希望修改当前脚本的某个分镜。请根据用户指令和当前脚本，输出一个结构化分镜补丁。

当前脚本：
标题：{title}
hook：{hook}
分镜：
{scenes}

用户指令：{user_message}

输出 JSON Schema：
{{
  "scene_sequence": 3,
  "narration": "修改后的旁白",
  "visual_description": "修改后的画面描述",
  "emotion": "修改后的情绪",
  "duration_sec": 10,
  "reply": "面向用户的简短中文回复"
}}"#,
            title = script.title,
            hook = script.hook,
            scenes = scenes,
            user_message = user_message
        ),
        max_output_tokens: Some(1_200),
    }
}

fn extract_json_object(raw: &str) -> Result<&str, AgentRuntimeError> {
    let start = raw.find('{').ok_or_else(|| {
        AgentRuntimeError::InvalidLlmOutput("missing JSON object start".to_string())
    })?;
    let end = raw.rfind('}').ok_or_else(|| {
        AgentRuntimeError::InvalidLlmOutput("missing JSON object end".to_string())
    })?;
    if start > end {
        return Err(AgentRuntimeError::InvalidLlmOutput(
            "invalid JSON object bounds".to_string(),
        ));
    }
    Ok(&raw[start..=end])
}

#[derive(Debug)]
pub enum AgentRuntimeError {
    Validation(String),
    UnsupportedAgent(String),
    InvalidLlmOutput(String),
    SceneNotFound { script_id: Uuid, sequence: i32 },
    ConversationRepository(ConversationRepositoryError),
    ScriptRepository(ScriptRepositoryError),
    Llm(LLMError),
}

impl From<ConversationRepositoryError> for AgentRuntimeError {
    fn from(error: ConversationRepositoryError) -> Self {
        Self::ConversationRepository(error)
    }
}

impl From<ScriptRepositoryError> for AgentRuntimeError {
    fn from(error: ScriptRepositoryError) -> Self {
        Self::ScriptRepository(error)
    }
}

impl From<LLMError> for AgentRuntimeError {
    fn from(error: LLMError) -> Self {
        Self::Llm(error)
    }
}

impl fmt::Display for AgentRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(message) => {
                write!(formatter, "agent turn validation error: {message}")
            }
            Self::UnsupportedAgent(agent_type) => {
                write!(formatter, "unsupported agent type: {agent_type}")
            }
            Self::InvalidLlmOutput(message) => {
                write!(formatter, "invalid agent LLM output: {message}")
            }
            Self::SceneNotFound {
                script_id,
                sequence,
            } => write!(
                formatter,
                "scene not found for script {script_id}: sequence {sequence}"
            ),
            Self::ConversationRepository(error) => write!(formatter, "{error}"),
            Self::ScriptRepository(error) => write!(formatter, "{error}"),
            Self::Llm(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AgentRuntimeError {}
