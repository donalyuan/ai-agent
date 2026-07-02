use crate::agents::models::GenerateScriptRequest;
use serde::Deserialize;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptPrompt {
    pub system: String,
    pub user: String,
}

pub struct ScriptPromptBuilder;

impl ScriptPromptBuilder {
    pub fn build(request: &GenerateScriptRequest) -> ScriptPrompt {
        let style = request.style_or_default();
        let scene_count = request.scene_count_or_default();
        let variant_instruction = if request.parent_id.is_some() {
            "\n7. 这是 A/B 测试的差异化版本，必须避免复用相同表达、相同开场结构和相同分镜节奏。"
        } else {
            ""
        };

        ScriptPrompt {
            system: "你是专业的短视频脚本创作者，擅长创作15-60秒的抖音/小红书短视频脚本。你必须只输出合法 JSON，不要输出解释、Markdown 或额外文本。".to_string(),
            user: format!(
                r#"请根据以下选题生成{scene_count}个分镜的中文短视频脚本。

选题：{topic}
风格：{style_label}（{style_code}）

输出要求：
1. 标题不超过30个中文字符。
2. hook 必须能在前3秒抓住观众注意力。
3. 必须严格输出 {scene_count} 个分镜，sequence 从 1 连续递增。
4. 每个分镜包含 narration、visual_description、emotion、duration_sec。
5. 每个分镜 narration 为 50-150 个中文字符，不能少于50字。
6. 每个分镜 duration_sec 为 1-30 秒，总时长建议 45-60 秒。{variant_instruction}

JSON Schema：
{{
  "title": "标题",
  "hook": "前3秒吸引点",
  "scenes": [
    {{
      "sequence": 1,
      "narration": "旁白文本",
      "visual_description": "视觉描述",
      "emotion": "情绪标签",
      "duration_sec": 8
    }}
  ]
}}"#,
                scene_count = scene_count,
                topic = request.topic,
                style_label = style.label(),
                style_code = style.as_str(),
                variant_instruction = variant_instruction,
            ),
        }
    }
}

impl From<ScriptPrompt> for novex_model::LLMPrompt {
    fn from(prompt: ScriptPrompt) -> Self {
        Self {
            system: prompt.system,
            user: prompt.user,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ScriptLLMOutput {
    pub title: String,
    pub hook: String,
    pub scenes: Vec<ScriptLLMScene>,
}

impl ScriptLLMOutput {
    pub fn parse_and_validate(raw: &str, expected_scene_count: u8) -> Result<Self, LLMOutputError> {
        let json_text = extract_json_object(raw)?;
        let mut output: Self =
            serde_json::from_str(json_text).map_err(|error| LLMOutputError::InvalidJson {
                message: error.to_string(),
            })?;

        output.title = output.title.trim().to_string();
        output.hook = output.hook.trim().to_string();
        if output.title.is_empty() {
            return Err(LLMOutputError::Validation(
                "title must not be empty".to_string(),
            ));
        }
        if output.title.chars().count() > 30 {
            return Err(LLMOutputError::Validation(
                "title must be 30 characters or fewer".to_string(),
            ));
        }
        if output.hook.is_empty() {
            return Err(LLMOutputError::Validation(
                "hook must not be empty".to_string(),
            ));
        }
        if output.scenes.len() != usize::from(expected_scene_count) {
            return Err(LLMOutputError::Validation(format!(
                "expected {expected_scene_count} scenes, got {}",
                output.scenes.len()
            )));
        }

        output.scenes.sort_by_key(|scene| scene.sequence);
        for (index, scene) in output.scenes.iter().enumerate() {
            let expected_sequence = i32::try_from(index + 1).unwrap_or(i32::MAX);
            if scene.sequence != expected_sequence {
                return Err(LLMOutputError::Validation(format!(
                    "scene sequence must be contiguous from 1; expected {expected_sequence}, got {}",
                    scene.sequence
                )));
            }
            scene.validate()?;
        }

        Ok(output)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ScriptLLMScene {
    pub sequence: i32,
    pub narration: String,
    pub visual_description: String,
    pub emotion: String,
    pub duration_sec: i32,
}

impl ScriptLLMScene {
    fn validate(&self) -> Result<(), LLMOutputError> {
        if self.narration.trim().is_empty() {
            return Err(LLMOutputError::Validation(format!(
                "scene {} narration must not be empty",
                self.sequence
            )));
        }
        let narration_length = self.narration.trim().chars().count();
        if !(50..=150).contains(&narration_length) {
            return Err(LLMOutputError::Validation(format!(
                "scene {} narration must be between 50 and 150 characters",
                self.sequence
            )));
        }
        if self.visual_description.trim().is_empty() {
            return Err(LLMOutputError::Validation(format!(
                "scene {} visual_description must not be empty",
                self.sequence
            )));
        }
        if self.emotion.trim().is_empty() {
            return Err(LLMOutputError::Validation(format!(
                "scene {} emotion must not be empty",
                self.sequence
            )));
        }
        if !(1..=30).contains(&self.duration_sec) {
            return Err(LLMOutputError::Validation(format!(
                "scene {} duration_sec must be between 1 and 30",
                self.sequence
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LLMOutputError {
    InvalidJson { message: String },
    Validation(String),
}

impl fmt::Display for LLMOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson { message } => write!(formatter, "invalid JSON: {message}"),
            Self::Validation(message) => write!(formatter, "invalid script output: {message}"),
        }
    }
}

impl std::error::Error for LLMOutputError {}

fn extract_json_object(raw: &str) -> Result<&str, LLMOutputError> {
    let start = raw.find('{').ok_or_else(|| LLMOutputError::InvalidJson {
        message: "missing JSON object start".to_string(),
    })?;
    let end = raw.rfind('}').ok_or_else(|| LLMOutputError::InvalidJson {
        message: "missing JSON object end".to_string(),
    })?;
    if start > end {
        return Err(LLMOutputError::InvalidJson {
            message: "invalid JSON object bounds".to_string(),
        });
    }
    Ok(&raw[start..=end])
}
