use super::{canonical_digest, domain_error};
use crate::ProductionResult;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSceneInput {
    pub sequence: u32,
    pub narration: String,
    pub visual_description: String,
    pub emotion: String,
    pub duration_sec: u32,
    pub character_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptDraftInput {
    pub title: String,
    pub hook: String,
    pub scenes: Vec<ScriptSceneInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormalScriptInput {
    pub title: String,
    pub hook: String,
    pub scenes: Vec<ScriptSceneInput>,
    pub digest: String,
}

pub fn map_script_draft(
    draft: &ScriptDraftInput,
    character_ids: &[String],
) -> ProductionResult<FormalScriptInput> {
    if draft.title.trim().is_empty() || draft.hook.trim().is_empty() || draft.scenes.is_empty() {
        return Err(domain_error("script title, hook, and scenes are required"));
    }
    let valid_characters: BTreeSet<&str> = character_ids.iter().map(String::as_str).collect();
    for (index, scene) in draft.scenes.iter().enumerate() {
        if scene.sequence as usize != index + 1
            || scene.narration.trim().is_empty()
            || scene.visual_description.trim().is_empty()
            || scene.emotion.trim().is_empty()
            || scene.duration_sec == 0
            || scene.duration_sec > 30
        {
            return Err(domain_error(format!(
                "invalid scene at sequence {}",
                index + 1
            )));
        }
        if scene
            .character_ids
            .iter()
            .any(|character| !valid_characters.contains(character.as_str()))
        {
            return Err(domain_error("script references an unknown character"));
        }
    }
    let digest = canonical_digest(draft)?;
    Ok(FormalScriptInput {
        title: draft.title.clone(),
        hook: draft.hook.clone(),
        scenes: draft.scenes.clone(),
        digest,
    })
}
