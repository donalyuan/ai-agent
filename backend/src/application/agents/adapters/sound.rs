use super::{record_step, AgentRuntimeError, SoundAgentAdapter, SoundAgentContext};
use crate::domain::conversation::CreateAgentStepInput;
use crate::repositories::VoiceCatalogEntry;
use novex_agent::{
    AgentOutcome, AgentSession, AuditedCallOwner, AuditedModelError, AuditedModelRequest,
    ModelExecutionRef, StepRecorder, StoredMessage,
};
use novex_ai_core::{DynamicFragment, PromptCompileInput, TrustLevel};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use uuid::Uuid;

impl SoundAgentAdapter {
    pub(super) async fn handle_sound_turn(
        &self,
        conversation: &AgentSession,
        user_message: &StoredMessage,
        run_id: Uuid,
        sound_context: &SoundAgentContext,
        model: ModelExecutionRef,
        steps: Arc<dyn StepRecorder>,
    ) -> Result<AgentOutcome, AgentRuntimeError> {
        let model_id = speech_model_id(conversation)?;
        let repository = self.voice_catalog_repository.as_ref();
        let catalog = repository.catalog(model_id, false).await?;
        if catalog.voices.is_empty() {
            return Err(AgentRuntimeError::Validation(
                "当前 TTS 模型没有可用音色，请先同步目录".to_string(),
            ));
        }
        record_step(steps.as_ref(), CreateAgentStepInput {
                agent_run_id: run_id,
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

        let prompt = sound_recommendation_prompt(
            &user_message.content,
            sound_context,
            &catalog.voices,
            &catalog.model_settings,
        );
        let audited = model.audited.as_ref().ok_or_else(|| {
            AgentRuntimeError::Kernel("audited model execution is required".into())
        })?;
        let fragment_id = format!("voice-catalog:{model_id}:recommendation");
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
                            node_key: "sound.recommend".into(),
                            compile_input: PromptCompileInput {
                                schema_version: "1".into(),
                                variables: BTreeMap::new(),
                                fragments: vec![DynamicFragment {
                                    id: fragment_id.clone(),
                                    trust: TrustLevel::Reference,
                                    source: "sound_recommendation_context".into(),
                                    content: Some(prompt),
                                    asset: None,
                                }],
                            },
                            tool_profile: "chat".into(),
                            tool_schema: None,
                            binding: audited.binding.clone(),
                            context_sources: json!([
                                {"id": format!("message:{}", user_message.id), "trust": "user_instruction", "source": "conversation_user_message"},
                                {"id": format!("voice-catalog:{model_id}"), "trust": "confirmed_fact", "source": "voice_catalog"},
                                {"id": fragment_id, "trust": "reference", "source": "sound_recommendation_context"}
                            ]),
                            memory_sources: json!([]),
                            parameters: json!({"max_output_tokens": 1500}),
                            asset_references: json!([]),
                        },
                        |raw| SoundRecommendation::parse(raw).map_err(|error| error.to_string()),
            )
            .await
            .map_err(sound_audited_error)?;
        let (recommendation, model_call_id) = (response.output, response.model_call_id);
        let voice = catalog
            .voices
            .iter()
            .find(|voice| voice.voice_type == recommendation.recommended_voice_type)
            .ok_or_else(|| {
                AgentRuntimeError::InvalidLlmOutput("声音 Agent 推荐了目录外音色".to_string())
            })?;
        validate_recommendation(&recommendation, voice, &catalog.model_settings)?;
        let recommendation_step_id = record_step(
            steps.as_ref(),
            CreateAgentStepInput {
                agent_run_id: run_id,
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
            },
        )
        .await?;
        audited
            .executor
            .associate_step(model_call_id, recommendation_step_id)
            .await
            .map_err(|error| AgentRuntimeError::Kernel(error.to_string()))?;

        Ok(AgentOutcome::new(
            recommendation.reply,
            json!({
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
        ))
    }
}

fn sound_audited_error(error: AuditedModelError) -> AgentRuntimeError {
    match error {
        AuditedModelError::Provider { source, .. } => AgentRuntimeError::Llm(source),
        AuditedModelError::StructuredParse { message, .. } => {
            AgentRuntimeError::InvalidLlmOutput(message)
        }
        error => AgentRuntimeError::Kernel(error.to_string()),
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

fn speech_model_id(conversation: &AgentSession) -> Result<Uuid, AgentRuntimeError> {
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
    model_settings: &Value,
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
    validate_recommended_parameters(&recommendation.parameters, model_settings)?;
    Ok(())
}

fn validate_recommended_parameters(
    parameters: &Value,
    model_settings: &Value,
) -> Result<(), AgentRuntimeError> {
    let definitions = model_settings.get("parameters").and_then(Value::as_object);
    let values = parameters.as_object().ok_or_else(|| {
        AgentRuntimeError::InvalidLlmOutput("声音 Agent 参数必须是 object".to_string())
    })?;
    for (name, value) in values {
        let definition = definitions
            .and_then(|items| items.get(name))
            .ok_or_else(|| {
                AgentRuntimeError::InvalidLlmOutput(format!(
                    "声音 Agent 推荐了模型未声明的参数: {name}"
                ))
            })?;
        if !parameter_value_is_supported(value, definition) {
            return Err(AgentRuntimeError::InvalidLlmOutput(format!(
                "声音 Agent 推荐了不支持的参数值: {name}"
            )));
        }
    }
    Ok(())
}

fn parameter_value_is_supported(value: &Value, definition: &Value) -> bool {
    if let Some(options) = definition
        .get("enum")
        .or_else(|| definition.get("options"))
        .and_then(Value::as_array)
    {
        return options.contains(value);
    }
    match definition.get("type").and_then(Value::as_str) {
        Some("number") => value.as_f64().is_some_and(|number| {
            definition
                .get("minimum")
                .or_else(|| definition.get("min"))
                .and_then(Value::as_f64)
                .is_none_or(|minimum| number >= minimum)
                && definition
                    .get("maximum")
                    .or_else(|| definition.get("max"))
                    .and_then(Value::as_f64)
                    .is_none_or(|maximum| number <= maximum)
        }),
        Some("integer") => value.as_i64().is_some(),
        Some("boolean") => value.is_boolean(),
        Some("string") => value.is_string(),
        _ => false,
    }
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

fn sound_recommendation_prompt(
    user_message: &str,
    sound_context: &SoundAgentContext,
    voices: &[VoiceCatalogEntry],
    model_settings: &Value,
) -> String {
    let voices = voices
        .iter()
        .map(|voice| {
            json!({
                "voice_type": voice.voice_type,
                "name": voice.name,
                "language_codes": language_codes(&voice.languages),
                "description": voice.description,
            })
        })
        .collect::<Vec<_>>();
    let parameter_definitions = model_settings
        .get("parameters")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let output_schema = sound_recommendation_schema();
    format!(
            "用户要求：\n{}\n\n当前编辑上下文：\n{}\n\n模型可调参数定义：\n{}\n\n完整可用音色目录（共 {} 项）：\n{}\n\n声音建议 JSON 输出契约：\n{}\n\n只输出符合契约的 JSON object。",
            user_message,
            serde_json::to_string(sound_context).unwrap_or_else(|_| "{}".to_string()),
            serde_json::to_string(&parameter_definitions).unwrap_or_else(|_| "{}".to_string()),
            voices.len(),
            serde_json::to_string(&voices).unwrap_or_else(|_| "[]".to_string()),
        serde_json::to_string(&output_schema).unwrap_or_else(|_| "{}".to_string())
    )
}

fn sound_recommendation_schema() -> Value {
    json!({
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
    })
}

fn language_codes(value: &Value) -> Vec<String> {
    let mut codes = BTreeSet::new();
    if let Some(items) = value.as_array() {
        for item in items {
            let candidate = item.as_str().or_else(|| {
                item.as_object().and_then(|object| {
                    ["Language", "language", "Value", "value"]
                        .iter()
                        .find_map(|key| object.get(*key).and_then(Value::as_str))
                })
            });
            if let Some(code) = candidate.map(str::trim).filter(|code| !code.is_empty()) {
                codes.insert(code.to_ascii_lowercase());
            }
        }
    }
    codes.into_iter().collect()
}
