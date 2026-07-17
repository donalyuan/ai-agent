use super::{AgentRuntime, AgentRuntimeError};
use crate::domain::conversation::{
    AgentConversation, AgentMessage, AgentMessageRole, AgentRunRecord, CreateAgentMessageInput,
    CreateAgentStepInput,
};
use crate::repositories::VoiceCatalogEntry;
use novex_model::{LLMJsonSchema, LLMPrompt};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

impl AgentRuntime {
    pub(super) async fn handle_sound_turn(
        &self,
        conversation: &AgentConversation,
        user_message: &AgentMessage,
        run: &AgentRunRecord,
    ) -> Result<AgentMessage, AgentRuntimeError> {
        let model_id = speech_model_id(conversation)?;
        let repository = self.voice_catalog_repository.as_ref().ok_or_else(|| {
            AgentRuntimeError::Validation("声音 Agent 未配置音色目录 repository".to_string())
        })?;
        let catalog = repository.catalog(model_id, false).await?;
        if catalog.voices.is_empty() {
            return Err(AgentRuntimeError::Validation(
                "当前 TTS 模型没有可用音色，请先同步目录".to_string(),
            ));
        }
        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 1,
                step_type: "read_voice_catalog".to_string(),
                status: "succeeded".to_string(),
                input: json!({"speech_model_id": model_id}),
                output: Some(json!({
                    "available_voice_count": catalog.voices.len(),
                    "last_sync_id": catalog.last_sync.as_ref().map(|sync| sync.id),
                    "last_sync_completed_at": catalog.last_sync.as_ref().and_then(|sync| sync.completed_at),
                })),
                error_message: None,
            })
            .await?;

        let raw = self
            .llm_client
            .generate_script(sound_recommendation_prompt(
                &user_message.content,
                &catalog.voices,
            ))
            .await?;
        let recommendation = SoundRecommendation::parse(&raw)?;
        let voice = catalog
            .voices
            .iter()
            .find(|voice| voice.voice_type == recommendation.recommended_voice_type)
            .ok_or_else(|| {
                AgentRuntimeError::InvalidLlmOutput("声音 Agent 推荐了目录外音色".to_string())
            })?;
        validate_recommendation(&recommendation, voice)?;
        self.conversation_repository
            .add_step(CreateAgentStepInput {
                agent_run_id: run.id,
                step_order: 2,
                step_type: "recommend_sound".to_string(),
                status: "succeeded".to_string(),
                input: json!({"message_id": user_message.id}),
                output: Some(json!({
                    "voice_type": recommendation.recommended_voice_type,
                    "language": recommendation.language,
                    "character_count": recommendation.tts_text.chars().count(),
                    "subtitle_segment_count": recommendation.subtitle_segments.len(),
                    "requires_confirmation": true,
                })),
                error_message: None,
            })
            .await?;

        self.conversation_repository
            .save_message(CreateAgentMessageInput {
                conversation_id: conversation.id,
                role: AgentMessageRole::Assistant,
                content: recommendation.reply,
                metadata: json!({
                    "intent": "recommend_sound",
                    "speech_model_id": model_id,
                    "recommended_voice_type": recommendation.recommended_voice_type,
                    "language": recommendation.language,
                    "tts_text": recommendation.tts_text,
                    "subtitle_segments": recommendation.subtitle_segments,
                    "parameters": recommendation.parameters,
                    "requires_confirmation": true,
                    "tool_execution": false,
                }),
            })
            .await
            .map_err(AgentRuntimeError::from)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SoundRecommendation {
    reply: String,
    recommended_voice_type: String,
    language: String,
    tts_text: String,
    subtitle_segments: Vec<String>,
    parameters: Value,
}

impl SoundRecommendation {
    fn parse(raw: &str) -> Result<Self, AgentRuntimeError> {
        let start = raw.find('{').ok_or_else(|| {
            AgentRuntimeError::InvalidLlmOutput("声音建议缺少 JSON object".to_string())
        })?;
        let end = raw.rfind('}').ok_or_else(|| {
            AgentRuntimeError::InvalidLlmOutput("声音建议缺少 JSON object".to_string())
        })?;
        let mut value: Self = serde_json::from_str(&raw[start..=end])
            .map_err(|error| AgentRuntimeError::InvalidLlmOutput(error.to_string()))?;
        value.reply = value.reply.trim().to_string();
        value.recommended_voice_type = value.recommended_voice_type.trim().to_string();
        value.language = value.language.trim().to_string();
        value.tts_text = value.tts_text.trim().to_string();
        value.subtitle_segments = value
            .subtitle_segments
            .into_iter()
            .map(|segment| segment.trim().to_string())
            .filter(|segment| !segment.is_empty())
            .collect();
        if value.reply.is_empty()
            || value.recommended_voice_type.is_empty()
            || value.language.is_empty()
            || value.tts_text.is_empty()
            || value.subtitle_segments.is_empty()
            || !value.parameters.is_object()
        {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "声音建议缺少必要字段".to_string(),
            ));
        }
        if alignment_text(&value.tts_text) != alignment_text(&value.subtitle_segments.join("")) {
            return Err(AgentRuntimeError::InvalidLlmOutput(
                "字幕断句与 TTS 文本不一致".to_string(),
            ));
        }
        Ok(value)
    }
}

fn speech_model_id(conversation: &AgentConversation) -> Result<Uuid, AgentRuntimeError> {
    conversation
        .metadata
        .get("speech_model_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .ok_or_else(|| {
            AgentRuntimeError::Validation("声音会话缺少有效 speech_model_id".to_string())
        })
}

fn validate_recommendation(
    recommendation: &SoundRecommendation,
    voice: &VoiceCatalogEntry,
) -> Result<(), AgentRuntimeError> {
    if !catalog_contains(
        &voice.languages,
        &recommendation.language,
        &["Language", "language", "Value", "value"],
    ) {
        return Err(AgentRuntimeError::InvalidLlmOutput(
            "声音 Agent 推荐了音色不支持的语言".to_string(),
        ));
    }
    Ok(())
}

fn catalog_contains(value: &Value, expected: &str, keys: &[&str]) -> bool {
    value.as_array().is_some_and(|items| {
        items.iter().any(|item| {
            item.as_str()
                .is_some_and(|value| value.eq_ignore_ascii_case(expected))
                || item.as_object().is_some_and(|object| {
                    keys.iter().any(|key| {
                        object
                            .get(*key)
                            .and_then(Value::as_str)
                            .is_some_and(|value| value.eq_ignore_ascii_case(expected))
                    })
                })
        })
    })
}

fn alignment_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn sound_recommendation_prompt(user_message: &str, voices: &[VoiceCatalogEntry]) -> LLMPrompt {
    let voices = voices
        .iter()
        .take(80)
        .map(|voice| {
            json!({
                "voice_type": voice.voice_type,
                "name": voice.name,
                "languages": voice.languages,
                "description": voice.description,
            })
        })
        .collect::<Vec<_>>();
    LLMPrompt {
        system: "你是声音 Agent。只能从给定可用目录推荐音色、语言和声音参数；当前 TTS 协议不支持结构化情绪字段。只输出建议，不执行 TTS/ASR。字幕断句必须完整覆盖 TTS 文本。".to_string(),
        user: format!(
            "用户要求：\n{}\n\n可用音色目录：\n{}\n\n输出严格 JSON。",
            user_message,
            serde_json::to_string(&voices).unwrap_or_else(|_| "[]".to_string())
        ),
        max_output_tokens: Some(1_500),
        output_schema: Some(LLMJsonSchema {
            name: "sound_recommendation".to_string(),
            strict: true,
            schema: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["reply", "recommended_voice_type", "language", "tts_text", "subtitle_segments", "parameters"],
                "properties": {
                    "reply": {"type": "string", "minLength": 1},
                    "recommended_voice_type": {"type": "string", "minLength": 1},
                    "language": {"type": "string", "minLength": 1},
                    "tts_text": {"type": "string", "minLength": 1},
                    "subtitle_segments": {"type": "array", "minItems": 1, "items": {"type": "string", "minLength": 1}},
                    "parameters": {"type": "object"}
                }
            }),
        }),
    }
}
