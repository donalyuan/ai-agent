//! 处理脚本首次生成与已有脚本分镜修改，保持会话绑定和 step 记录语义一致。

use super::{record_step, AgentRuntimeError, ScriptAgentAdapter};
use crate::agents::{AuditedScriptModelExecutor, ScriptAgentService};
use crate::domain::conversation::{BindAgentConversationSubjectInput, CreateAgentStepInput};
use crate::domain::script::{Scene, Script, ScriptGenerationInput, ScriptStyle};
use novex_agent::{
    AgentOutcome, AgentSession, AuditedCallOwner, AuditedModelRequest, ModelExecutionRef,
    StepRecorder, StoredMessage,
};
use novex_ai_core::{DynamicFragment, PromptCompileInput, TrustLevel};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

impl ScriptAgentAdapter {
    pub(super) async fn handle_script_turn(
        &self,
        conversation: &AgentSession,
        user_message: &StoredMessage,
        run_id: Uuid,
        model: ModelExecutionRef,
        steps: Arc<dyn StepRecorder>,
    ) -> Result<AgentOutcome, AgentRuntimeError> {
        if conversation.subject_id.is_none() && conversation.subject_type.is_none() {
            return self
                .handle_script_generation_turn(conversation, user_message, run_id, model, steps)
                .await;
        }

        let script_id = conversation
            .subject_id
            .ok_or_else(|| AgentRuntimeError::Validation("脚本会话缺少 subject_id".to_string()))?;
        if conversation.subject_type.as_deref() != Some("script") {
            return Err(AgentRuntimeError::Validation(
                "脚本会话 subject_type 必须为 script".to_string(),
            ));
        }

        let script = self.script_repository.get_script(script_id).await?;
        record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 1,
                step_type: "read_script".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "script_id": script_id }),
                output: Some(json!({ "scene_count": script.scenes.len() })),
                error_message: None,
            },
        )
        .await?;

        let audited = model.audited.as_ref().ok_or_else(|| {
            AgentRuntimeError::Kernel("audited model execution is required".into())
        })?;
        let fragment_id = format!("script:{script_id}:user-message:{}", user_message.id);
        let response = audited
            .executor
            .execute_parsed(
                AuditedModelRequest {
                    owner: AuditedCallOwner::AgentRun(run_id),
                    step_id: None,
                    root_call_id: None,
                    parent_call_id: None,
                    attempt: 1,
                    agent_key: audited.agent_key.clone(),
                    agent_version: audited.agent_version.clone(),
                    node_key: "script.scene_patch".into(),
                    compile_input: PromptCompileInput {
                        schema_version: "1".into(),
                        variables: BTreeMap::new(),
                        fragments: vec![DynamicFragment {
                            id: fragment_id.clone(),
                            trust: TrustLevel::UserInstruction,
                            source: "conversation_script_scene_patch".into(),
                            content: Some(build_script_scene_patch_input(
                                &script,
                                &user_message.content,
                            )),
                            asset: None,
                        }],
                    },
                    tool_profile: "chat".into(),
                    tool_schema: None,
                    binding: audited.binding.clone(),
                    context_sources: json!([{
                        "id": fragment_id,
                        "trust": "user_instruction",
                        "source": "conversation_script_scene_patch"
                    }]),
                    memory_sources: json!([]),
                    parameters: json!({ "max_output_tokens": 1200 }),
                    asset_references: json!([]),
                },
                |raw| ScriptScenePatch::parse(raw).map_err(|error| error.to_string()),
            )
            .await
            .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;
        let (patch, model_call_id) = (response.output, response.model_call_id);
        let existing_scene = script
            .scenes
            .iter()
            .find(|scene| scene.sequence == patch.scene_sequence)
            .cloned()
            .ok_or(AgentRuntimeError::SceneNotFound {
                script_id,
                sequence: patch.scene_sequence,
            })?;

        let step_id = record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 2,
                step_type: "llm_scene_patch".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "message_id": user_message.id }),
                output: Some(json!({ "scene_sequence": patch.scene_sequence })),
                error_message: None,
            },
        )
        .await?;
        audited
            .executor
            .associate_step(model_call_id, step_id)
            .await
            .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;

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

        record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 3,
                step_type: "update_scene".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "script_id": script_id, "scene_sequence": patch.scene_sequence }),
                output: Some(json!({ "updated_at": updated_script.updated_at })),
                error_message: None,
            },
        )
        .await?;

        let content = if patch.reply.trim().is_empty() {
            format!("已修改第 {} 镜。", patch.scene_sequence)
        } else {
            patch.reply
        };

        Ok(AgentOutcome::new(
            content,
            json!({
                "script_id": script_id,
                "scene_sequence": patch.scene_sequence,
                "intent": "edit_script",
                "script_created": false,
                "needs_input": false,
                "missing_fields": [],
            }),
        ))
    }

    async fn handle_script_generation_turn(
        &self,
        conversation: &AgentSession,
        user_message: &StoredMessage,
        run_id: Uuid,
        model: ModelExecutionRef,
        steps: Arc<dyn StepRecorder>,
    ) -> Result<AgentOutcome, AgentRuntimeError> {
        let project_id = conversation.project_id.ok_or_else(|| {
            AgentRuntimeError::Validation("脚本生成会话缺少 project_id".to_string())
        })?;

        let audited = model.audited.as_ref().ok_or_else(|| {
            AgentRuntimeError::Kernel("audited model execution is required".into())
        })?;
        let fragment_id = format!(
            "conversation:{}:user-message:{}",
            conversation.id, user_message.id
        );
        let response = audited
            .executor
            .execute_parsed(
                AuditedModelRequest {
                    owner: AuditedCallOwner::AgentRun(run_id),
                    step_id: None,
                    root_call_id: None,
                    parent_call_id: None,
                    attempt: 1,
                    agent_key: audited.agent_key.clone(),
                    agent_version: audited.agent_version.clone(),
                    node_key: "script.generation_intent".into(),
                    compile_input: PromptCompileInput {
                        schema_version: "1".into(),
                        variables: BTreeMap::new(),
                        fragments: vec![DynamicFragment {
                            id: fragment_id.clone(),
                            trust: TrustLevel::UserInstruction,
                            source: "conversation_user_message".into(),
                            content: Some(build_script_generation_intent_input(
                                &user_message.content,
                            )),
                            asset: None,
                        }],
                    },
                    tool_profile: "chat".into(),
                    tool_schema: None,
                    binding: audited.binding.clone(),
                    context_sources: json!([{
                        "id": fragment_id,
                        "trust": "user_instruction",
                        "source": "conversation_user_message"
                    }]),
                    memory_sources: json!([]),
                    parameters: json!({ "max_output_tokens": 800 }),
                    asset_references: json!([]),
                },
                |raw| ScriptGenerationIntent::parse(raw).map_err(|error| error.to_string()),
            )
            .await
            .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;
        let (intent, intent_model_call_id) = (response.output, response.model_call_id);
        let intent_step_id = record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 1,
                step_type: "parse_generation_intent".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "message_id": user_message.id }),
                output: Some(json!({
                    "intent": intent.intent,
                    "missing_fields": intent.missing_fields,
                })),
                error_message: None,
            },
        )
        .await?;
        audited
            .executor
            .associate_step(intent_model_call_id, intent_step_id)
            .await
            .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;

        if intent.needs_input() {
            let content = if intent.reply.trim().is_empty() {
                "请补充选题、风格和分镜数。".to_string()
            } else {
                intent.reply
            };
            return Ok(AgentOutcome::new(
                content,
                json!({
                    "intent": "generate_script",
                    "script_id": null,
                    "script_created": false,
                    "needs_input": true,
                    "missing_fields": intent.missing_fields,
                }),
            ));
        }

        let reply = intent.reply.clone();
        let request = intent.into_generate_request(project_id)?;
        let audited_generation_calls = Arc::new(Mutex::new(Vec::new()));
        let service = ScriptAgentService::new(
            Arc::new(
                AuditedScriptModelExecutor::new(
                    audited.executor.clone(),
                    AuditedCallOwner::AgentRun(run_id),
                    audited.agent_key.clone(),
                    audited.agent_version.clone(),
                    audited.binding.clone(),
                )
                .with_call_ids(audited_generation_calls.clone()),
            ),
            self.script_repository.clone(),
            self.project_repository.clone(),
        );
        let script = service.generate(request).await?;
        let generation_step_id = record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
                step_order: 2,
                step_type: "generate_script".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "project_id": project_id }),
                output: Some(json!({ "script_id": script.id, "scene_count": script.scenes.len() })),
                error_message: None,
            },
        )
        .await?;
        let call_ids = audited_generation_calls
            .lock()
            .map_err(|_| AgentRuntimeError::Kernel("audited call IDs lock poisoned".into()))?
            .clone();
        for model_call_id in call_ids {
            audited
                .executor
                .associate_step(model_call_id, generation_step_id)
                .await
                .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;
        }

        self.conversation_repository
            .bind_conversation_subject(BindAgentConversationSubjectInput {
                conversation_id: conversation.id,
                subject_type: "script".to_string(),
                subject_id: script.id,
            })
            .await?;

        let content = if reply.trim().is_empty() {
            format!("已生成脚本《{}》。", script.title)
        } else {
            reply
        };
        Ok(AgentOutcome::new(
            content,
            json!({
                "intent": "generate_script",
                "script_id": script.id,
                "script_created": true,
                "needs_input": false,
                "missing_fields": [],
            }),
        ))
    }
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

#[derive(Debug, Deserialize)]
struct ScriptGenerationIntent {
    intent: String,
    topic: Option<String>,
    style: Option<String>,
    scene_count: Option<u8>,
    #[serde(default)]
    reply: String,
    #[serde(default)]
    missing_fields: Vec<String>,
}

impl ScriptGenerationIntent {
    fn parse(raw: &str) -> Result<Self, AgentRuntimeError> {
        let json_text = extract_json_object(raw)?;
        let mut intent: Self = serde_json::from_str(json_text)
            .map_err(|error| AgentRuntimeError::InvalidLlmOutput(error.to_string()))?;
        intent.intent = intent.intent.trim().to_string();
        intent.topic = intent
            .topic
            .map(|topic| topic.trim().to_string())
            .filter(|topic| !topic.is_empty());
        intent.style = intent
            .style
            .map(|style| style.trim().to_string())
            .filter(|style| !style.is_empty());
        intent.reply = intent.reply.trim().to_string();
        intent.missing_fields = intent
            .missing_fields
            .into_iter()
            .map(|field| field.trim().to_string())
            .filter(|field| !field.is_empty())
            .collect();
        intent.validate()?;
        Ok(intent)
    }

    fn validate(&self) -> Result<(), AgentRuntimeError> {
        if self.intent != "generate_script" {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "intent must be generate_script".to_string(),
            ));
        }
        if let Some(style) = self.style.as_deref() {
            parse_script_style(style)?;
        }
        if let Some(scene_count) = self.scene_count {
            if !(3..=12).contains(&scene_count) {
                return Err(AgentRuntimeError::InvalidLlmOutput(
                    "scene_count must be between 3 and 12".to_string(),
                ));
            }
        }
        Ok(())
    }

    fn needs_input(&self) -> bool {
        !self.missing_fields.is_empty()
            || self.topic.is_none()
            || self.style.is_none()
            || self.scene_count.is_none()
    }

    fn into_generate_request(
        self,
        project_id: Uuid,
    ) -> Result<ScriptGenerationInput, AgentRuntimeError> {
        let topic = self
            .topic
            .ok_or_else(|| AgentRuntimeError::InvalidLlmOutput("topic is required".to_string()))?;
        let style = self
            .style
            .as_deref()
            .map(parse_script_style)
            .transpose()?
            .ok_or_else(|| AgentRuntimeError::InvalidLlmOutput("style is required".to_string()))?;
        let scene_count = self.scene_count.ok_or_else(|| {
            AgentRuntimeError::InvalidLlmOutput("scene_count is required".to_string())
        })?;

        Ok(ScriptGenerationInput {
            project_id,
            topic,
            topic_id: None,
            style: Some(style),
            scene_count: Some(scene_count),
            parent_id: None,
        })
    }
}

fn parse_script_style(style: &str) -> Result<ScriptStyle, AgentRuntimeError> {
    match style {
        "knowledge" => Ok(ScriptStyle::Knowledge),
        "story" => Ok(ScriptStyle::Story),
        "tutorial" => Ok(ScriptStyle::Tutorial),
        _ => Err(AgentRuntimeError::InvalidLlmOutput(format!(
            "unsupported script style: {style}"
        ))),
    }
}

fn build_script_generation_intent_input(user_message: &str) -> String {
    format!(
        r#"请从用户消息中提取生成脚本参数。生成脚本参数包括 topic、style、scene_count。

规则：
1. intent 固定输出 generate_script。
2. topic 是用户提供的选题文本，不足时输出 null 并加入 missing_fields。
3. style 只能是 knowledge、story、tutorial，不足或不在范围内时输出 null 并加入 missing_fields。
4. scene_count 只能是 3 到 12 的整数，不足或越界时输出 null 并加入 missing_fields。
5. missing_fields 为空时可以生成脚本；非空时 reply 必须追问缺失字段。

用户消息：{user_message}

JSON Schema：
{{
  "intent": "generate_script",
  "topic": "ChatGPT 如何改变程序员工作流",
  "style": "knowledge",
  "scene_count": 6,
  "reply": "面向用户的简短中文回复或追问",
  "missing_fields": []
}}"#,
        user_message = user_message
    )
}

fn build_script_scene_patch_input(script: &Script, user_message: &str) -> String {
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

    format!(
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
    )
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
