use crate::agents::conversation::{
    AgentConversation, AgentMessage, AgentMessageRole, AgentRunRecord,
    BindAgentConversationSubjectInput, CreateAgentMessageInput, CreateAgentRunInput,
    CreateAgentStepInput, FinishAgentRunInput,
};
use crate::agents::models::{
    ContentTopicSource, ContentTopicStatus, GenerateScriptRequest, Scene, Script, ScriptStyle,
    TopicGenerationBatchStatus,
};
use crate::agents::{ScriptAgentError, ScriptAgentService};
use crate::repositories::{
    ConversationRepository, ConversationRepositoryError, CreateContentTopicInput,
    CreateTopicGenerationBatchInput, ProjectRepository, ProjectRepositoryError, ScriptRepository,
    ScriptRepositoryError, TopicRepository, TopicRepositoryError, UpdateTopicGenerationBatchInput,
};
use novex_model::{LLMClient, LLMError, LLMJsonSchema, LLMPrompt};
use serde::Deserialize;
use serde_json::json;
use std::fmt;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AgentRuntime {
    conversation_repository: Arc<dyn ConversationRepository>,
    script_repository: Arc<dyn ScriptRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    topic_repository: Option<Arc<dyn TopicRepository>>,
    llm_client: Arc<dyn LLMClient>,
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
            llm_client,
        }
    }

    pub fn with_topic_repository(mut self, topic_repository: Arc<dyn TopicRepository>) -> Self {
        self.topic_repository = Some(topic_repository);
        self
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
            "topic" => {
                self.handle_topic_turn(&conversation, &user_message, &run)
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
        if conversation.subject_id.is_none() && conversation.subject_type.is_none() {
            return self
                .handle_script_generation_turn(conversation, user_message, run)
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
                    "intent": "edit_script",
                    "script_created": false,
                    "needs_input": false,
                    "missing_fields": [],
                }),
            })
            .await
            .map_err(AgentRuntimeError::from)
    }

    async fn handle_script_generation_turn(
        &self,
        conversation: &AgentConversation,
        user_message: &AgentMessage,
        run: &AgentRunRecord,
    ) -> Result<AgentMessage, AgentRuntimeError> {
        let project_id = conversation.project_id.ok_or_else(|| {
            AgentRuntimeError::Validation("脚本生成会话缺少 project_id".to_string())
        })?;

        let raw = self
            .llm_client
            .generate_script(build_script_generation_intent_prompt(&user_message.content))
            .await?;
        let intent = ScriptGenerationIntent::parse(&raw)?;
        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 1,
                step_type: "parse_generation_intent".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "message_id": user_message.id }),
                output: Some(json!({
                    "intent": intent.intent,
                    "missing_fields": intent.missing_fields,
                })),
                error_message: None,
            })
            .await?;

        if intent.needs_input() {
            let content = if intent.reply.trim().is_empty() {
                "请补充选题、风格和分镜数。".to_string()
            } else {
                intent.reply
            };
            return self
                .conversation_repository
                .save_message(CreateAgentMessageInput {
                    conversation_id: conversation.id,
                    role: AgentMessageRole::Assistant,
                    content,
                    metadata: json!({
                        "intent": "generate_script",
                        "script_id": null,
                        "script_created": false,
                        "needs_input": true,
                        "missing_fields": intent.missing_fields,
                    }),
                })
                .await
                .map_err(AgentRuntimeError::from);
        }

        let reply = intent.reply.clone();
        let request = intent.into_generate_request(project_id)?;
        let service = ScriptAgentService::new(
            self.llm_client.clone(),
            self.script_repository.clone(),
            self.project_repository.clone(),
        );
        let script = service.generate_script(request).await?;
        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 2,
                step_type: "generate_script".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "project_id": project_id }),
                output: Some(json!({ "script_id": script.id, "scene_count": script.scenes.len() })),
                error_message: None,
            })
            .await?;

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
        self.conversation_repository
            .save_message(CreateAgentMessageInput {
                conversation_id: conversation.id,
                role: AgentMessageRole::Assistant,
                content,
                metadata: json!({
                    "intent": "generate_script",
                    "script_id": script.id,
                    "script_created": true,
                    "needs_input": false,
                    "missing_fields": [],
                }),
            })
            .await
            .map_err(AgentRuntimeError::from)
    }

    async fn handle_topic_turn(
        &self,
        conversation: &AgentConversation,
        user_message: &AgentMessage,
        run: &AgentRunRecord,
    ) -> Result<AgentMessage, AgentRuntimeError> {
        let project_id = conversation
            .project_id
            .ok_or_else(|| AgentRuntimeError::Validation("选题会话缺少 project_id".to_string()))?;
        let topic_repository = self.topic_repository.as_ref().ok_or_else(|| {
            AgentRuntimeError::Validation("选题 Agent 未配置 topic repository".to_string())
        })?;
        let project = self.project_repository.get_project(project_id).await?;

        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 1,
                step_type: "read_project_context".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "project_id": project_id }),
                output: Some(json!({
                    "name": project.name,
                    "positioning": project.positioning,
                    "description": project.description
                })),
                error_message: None,
            })
            .await?;

        let requested_count = requested_topic_count(&user_message.content);
        let batch = topic_repository
            .create_generation_batch(CreateTopicGenerationBatchInput {
                project_id,
                source_run_id: Some(run.id),
                prompt: user_message.content.clone(),
                requested_count,
                status: TopicGenerationBatchStatus::Running,
                error_message: None,
                metadata: json!({ "conversation_id": conversation.id }),
            })
            .await?;

        let raw = self
            .llm_client
            .generate_script(build_topic_generation_prompt(
                &project.positioning,
                &project.description,
                requested_count,
                &user_message.content,
            ))
            .await;

        let candidates = match raw {
            Ok(raw) => match TopicLLMOutput::parse_and_validate(&raw) {
                Ok(candidates) => candidates,
                Err(error) => {
                    self.mark_topic_batch_failed(
                        topic_repository.as_ref(),
                        batch.id,
                        error.to_string(),
                    )
                    .await;
                    self.add_failed_topic_step(run.id, 2, "generate_topics", error.to_string())
                        .await;
                    return Err(AgentRuntimeError::InvalidLlmOutput(error.to_string()));
                }
            },
            Err(error) => {
                self.mark_topic_batch_failed(
                    topic_repository.as_ref(),
                    batch.id,
                    error.to_string(),
                )
                .await;
                self.add_failed_topic_step(run.id, 2, "generate_topics", error.to_string())
                    .await;
                return Err(AgentRuntimeError::Llm(error));
            }
        };

        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 2,
                step_type: "generate_topics".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "message_id": user_message.id, "requested_count": requested_count }),
                output: Some(json!({ "topic_count": candidates.len() })),
                error_message: None,
            })
            .await?;

        let mut created_topic_ids = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let topic = topic_repository
                .create_topic(CreateContentTopicInput {
                    project_id,
                    batch_id: Some(batch.id),
                    title: candidate.title,
                    angle: candidate.angle,
                    target_audience: candidate.target_audience,
                    hook_points: candidate.hook_points,
                    content_type: candidate.content_type,
                    score: Some(candidate.score.unwrap_or(0.0)),
                    score_reason: candidate.score_reason,
                    tags: candidate.tags,
                    source: ContentTopicSource::Agent,
                    metadata: json!({ "source_run_id": run.id }),
                })
                .await?;
            created_topic_ids.push(topic.id);
        }

        topic_repository
            .update_generation_batch(
                batch.id,
                UpdateTopicGenerationBatchInput {
                    status: TopicGenerationBatchStatus::Succeeded,
                    error_message: None,
                    metadata: json!({
                        "conversation_id": conversation.id,
                        "created_topic_ids": created_topic_ids,
                        "topic_count": created_topic_ids.len()
                    }),
                },
            )
            .await?;

        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 3,
                step_type: "persist_topics".to_string(),
                status: "succeeded".to_string(),
                input: json!({ "batch_id": batch.id }),
                output: Some(json!({
                    "batch_id": batch.id,
                    "created_topic_ids": created_topic_ids,
                    "topic_count": created_topic_ids.len()
                })),
                error_message: None,
            })
            .await?;

        let topic_count = created_topic_ids.len();
        self.conversation_repository
            .save_message(CreateAgentMessageInput {
                conversation_id: conversation.id,
                role: AgentMessageRole::Assistant,
                content: format!("已生成 {topic_count} 个候选选题。"),
                metadata: json!({
                    "intent": "generate_topics",
                    "batch_id": batch.id,
                    "created_topic_ids": created_topic_ids,
                    "topic_count": topic_count,
                    "status": ContentTopicStatus::Idea
                }),
            })
            .await
            .map_err(AgentRuntimeError::from)
    }

    async fn mark_topic_batch_failed(
        &self,
        topic_repository: &dyn TopicRepository,
        batch_id: Uuid,
        error_message: String,
    ) {
        let _ = topic_repository
            .update_generation_batch(
                batch_id,
                UpdateTopicGenerationBatchInput {
                    status: TopicGenerationBatchStatus::Failed,
                    error_message: Some(error_message),
                    metadata: json!({}),
                },
            )
            .await;
    }

    async fn add_failed_topic_step(
        &self,
        run_id: Uuid,
        step_order: i32,
        step_type: &str,
        error_message: String,
    ) {
        let _ = self
            .conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run_id,
                step_order,
                step_type: step_type.to_string(),
                status: "failed".to_string(),
                input: json!({}),
                output: None,
                error_message: Some(error_message),
            })
            .await;
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
    ) -> Result<GenerateScriptRequest, AgentRuntimeError> {
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

        Ok(GenerateScriptRequest {
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

fn build_script_generation_intent_prompt(user_message: &str) -> LLMPrompt {
    LLMPrompt {
        system: "你是短视频脚本生成 Agent。你必须只输出合法 JSON，不要输出 Markdown 或解释。"
            .to_string(),
        user: format!(
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
        ),
        max_output_tokens: Some(800),
        output_schema: None,
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
        output_schema: None,
    }
}

#[derive(Debug, Deserialize)]
struct TopicLLMOutputEnvelope {
    topics: Vec<TopicLLMOutput>,
}

#[derive(Debug, Deserialize)]
struct TopicLLMOutput {
    title: String,
    angle: String,
    target_audience: String,
    hook_points: Vec<String>,
    content_type: String,
    score: Option<f64>,
    score_reason: String,
    tags: Vec<String>,
}

impl TopicLLMOutput {
    fn parse_and_validate(raw: &str) -> Result<Vec<Self>, TopicOutputError> {
        let json_text = extract_topic_json_object(raw)?;
        let mut envelope: TopicLLMOutputEnvelope =
            serde_json::from_str(json_text).map_err(|error| TopicOutputError::InvalidJson {
                message: error.to_string(),
            })?;
        let topics = &mut envelope.topics;
        if topics.is_empty() {
            return Err(TopicOutputError::Validation(
                "topic output must not be empty".to_string(),
            ));
        }
        for topic in topics {
            topic.normalize();
            topic.validate()?;
        }
        Ok(envelope.topics)
    }

    fn normalize(&mut self) {
        self.title = self.title.trim().to_string();
        self.angle = self.angle.trim().to_string();
        self.target_audience = self.target_audience.trim().to_string();
        self.content_type = self.content_type.trim().to_string();
        self.score_reason = self.score_reason.trim().to_string();
        self.hook_points = self
            .hook_points
            .drain(..)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
        self.tags = self
            .tags
            .drain(..)
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect();
    }

    fn validate(&self) -> Result<(), TopicOutputError> {
        if self.title.is_empty() {
            return Err(TopicOutputError::Validation(
                "title is required".to_string(),
            ));
        }
        if self.angle.is_empty() {
            return Err(TopicOutputError::Validation(
                "angle is required".to_string(),
            ));
        }
        if self.target_audience.is_empty() {
            return Err(TopicOutputError::Validation(
                "target_audience is required".to_string(),
            ));
        }
        if self.hook_points.is_empty() {
            return Err(TopicOutputError::Validation(
                "hook_points is required".to_string(),
            ));
        }
        if self.content_type.is_empty() {
            return Err(TopicOutputError::Validation(
                "content_type is required".to_string(),
            ));
        }
        let Some(score) = self.score else {
            return Err(TopicOutputError::Validation(
                "score is required".to_string(),
            ));
        };
        if !(0.0..=100.0).contains(&score) {
            return Err(TopicOutputError::Validation(
                "score must be between 0 and 100".to_string(),
            ));
        }
        if self.score_reason.is_empty() {
            return Err(TopicOutputError::Validation(
                "score_reason is required".to_string(),
            ));
        }
        if self.tags.is_empty() {
            return Err(TopicOutputError::Validation("tags is required".to_string()));
        }
        Ok(())
    }
}

#[derive(Debug)]
enum TopicOutputError {
    InvalidJson { message: String },
    Validation(String),
}

impl fmt::Display for TopicOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { message } => write!(formatter, "invalid topic JSON: {message}"),
            Self::Validation(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for TopicOutputError {}

fn build_topic_generation_prompt(
    project_positioning: &str,
    project_description: &str,
    requested_count: i32,
    user_message: &str,
) -> LLMPrompt {
    LLMPrompt {
        system: "你是短视频内容策略选题 Agent。你必须只输出符合 JSON Schema 的合法 JSON 对象，不要输出 Markdown 或解释。"
            .to_string(),
        user: format!(
            r#"请基于项目定位和用户补充要求生成 {requested_count} 个候选选题。

项目定位：{project_positioning}
项目描述：{project_description}
用户补充要求：{user_message}

输出要求：
1. 必须只输出一个 JSON 对象。
2. 顶层对象必须只包含 topics 字段。
3. topics 数组每项必须包含 title、angle、target_audience、hook_points、content_type、score、score_reason、tags。
4. score 必须是 0 到 100 的数字。
5. hook_points 和 tags 必须是非空字符串数组。
6. 不允许把 topics 写成字符串数组；每个选题必须是包含完整字段的对象。

JSON Schema：
{{
  "topics": [
    {{
      "title": "选题标题",
      "angle": "选题角度",
      "target_audience": "目标受众",
      "hook_points": ["主要看点"],
      "content_type": "knowledge",
      "score": 88,
      "score_reason": "评分理由",
      "tags": ["标签"]
    }}
  ]
}}"#,
            requested_count = requested_count,
            project_positioning = project_positioning,
            project_description = project_description,
            user_message = user_message
        ),
        max_output_tokens: Some(2_000),
        output_schema: Some(topic_generation_output_schema()),
    }
}

fn topic_generation_output_schema() -> LLMJsonSchema {
    LLMJsonSchema {
        name: "topic_generation_batch".to_string(),
        strict: true,
        schema: json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["topics"],
            "properties": {
                "topics": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "additionalProperties": false,
                        "required": [
                            "title",
                            "angle",
                            "target_audience",
                            "hook_points",
                            "content_type",
                            "score",
                            "score_reason",
                            "tags"
                        ],
                        "properties": {
                            "title": { "type": "string", "minLength": 1 },
                            "angle": { "type": "string", "minLength": 1 },
                            "target_audience": { "type": "string", "minLength": 1 },
                            "hook_points": {
                                "type": "array",
                                "minItems": 1,
                                "items": { "type": "string", "minLength": 1 }
                            },
                            "content_type": { "type": "string", "minLength": 1 },
                            "score": { "type": "number", "minimum": 0, "maximum": 100 },
                            "score_reason": { "type": "string", "minLength": 1 },
                            "tags": {
                                "type": "array",
                                "minItems": 1,
                                "items": { "type": "string", "minLength": 1 }
                            }
                        }
                    }
                }
            }
        }),
    }
}

fn requested_topic_count(user_message: &str) -> i32 {
    let parsed = user_message
        .split(|character: char| !character.is_ascii_digit())
        .find_map(|segment| segment.parse::<i32>().ok())
        .unwrap_or(5);
    parsed.clamp(1, 20)
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

fn extract_topic_json_object(raw: &str) -> Result<&str, TopicOutputError> {
    let start = raw
        .find('{')
        .ok_or_else(|| TopicOutputError::Validation("missing JSON object start".to_string()))?;
    let end = raw
        .rfind('}')
        .ok_or_else(|| TopicOutputError::Validation("missing JSON object end".to_string()))?;
    if start > end {
        return Err(TopicOutputError::Validation(
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
    ProjectRepository(ProjectRepositoryError),
    TopicRepository(TopicRepositoryError),
    ScriptAgent(ScriptAgentError),
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

impl From<ProjectRepositoryError> for AgentRuntimeError {
    fn from(error: ProjectRepositoryError) -> Self {
        Self::ProjectRepository(error)
    }
}

impl From<TopicRepositoryError> for AgentRuntimeError {
    fn from(error: TopicRepositoryError) -> Self {
        Self::TopicRepository(error)
    }
}

impl From<LLMError> for AgentRuntimeError {
    fn from(error: LLMError) -> Self {
        Self::Llm(error)
    }
}

impl From<ScriptAgentError> for AgentRuntimeError {
    fn from(error: ScriptAgentError) -> Self {
        Self::ScriptAgent(error)
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
            Self::ProjectRepository(error) => write!(formatter, "{error}"),
            Self::TopicRepository(error) => write!(formatter, "{error}"),
            Self::ScriptAgent(error) => write!(formatter, "{error}"),
            Self::Llm(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for AgentRuntimeError {}
