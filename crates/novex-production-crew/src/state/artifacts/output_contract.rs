//! Full Crew 角色输出的强类型契约与无 I/O 校验。

use crate::{ProductionError, ProductionResult};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProducerOutput {
    pub creative_brief: CreativeBriefOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CreativeBriefOutput {
    pub target_audience: String,
    pub tone: Vec<String>,
    pub key_messages: Vec<String>,
    pub constraints: Map<String, Value>,
    pub success_criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScreenwriterOutput {
    pub story_bible: StoryBibleOutput,
    pub character_bibles: Vec<CharacterBibleOutput>,
    pub script_draft: ScriptDraftOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StoryBibleOutput {
    pub premise: String,
    pub theme: String,
    pub narrative_structure: String,
    pub world: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CharacterBibleOutput {
    pub character_id: String,
    pub name: String,
    pub role: String,
    pub personality: String,
    pub motivation: String,
    pub arc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScriptDraftOutput {
    pub title: String,
    pub hook: String,
    pub scenes: Vec<ScriptSceneOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScriptSceneOutput {
    pub sequence: u32,
    pub narration: String,
    pub visual_description: String,
    pub emotion: String,
    pub duration_sec: u32,
    pub character_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DirectorOutput {
    pub directorial_treatment: DirectorialTreatmentOutput,
    pub shot_contracts: Vec<ShotContractOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DirectorialTreatmentOutput {
    pub visual_style: String,
    pub pacing: String,
    pub emotional_arc: String,
    pub color_palette: Vec<String>,
    pub reference_works: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShotContractOutput {
    pub shot_id: String,
    pub sequence: u32,
    pub scene_id: Uuid,
    pub shot_type: String,
    pub camera_movement: String,
    pub duration_sec: u32,
    pub description: String,
    pub character_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CollaborationRoleOutput {
    pub collaboration_suggestions: Vec<CollaborationSuggestionOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CollaborationSuggestionOutput {
    pub target_artifact_id: Uuid,
    pub target_artifact_version: u32,
    pub suggestion_type: String,
    pub content: String,
    pub priority: String,
    pub blocking: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerformanceDirectorOutput {
    pub performance_briefs: Vec<PerformanceBriefOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerformanceBriefOutput {
    pub character_bible_id: Uuid,
    pub character_id: String,
    pub script_id: Uuid,
    pub emotional_arc: Vec<PerformanceSceneOutput>,
    pub body_language: String,
    pub vocal_direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PerformanceSceneOutput {
    pub sequence: u32,
    pub scene_id: Uuid,
    pub emotion: String,
    pub intensity: u8,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SoundDirectorOutput {
    pub sound_plan: SoundPlanOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SoundPlanOutput {
    pub script_id: Uuid,
    pub music_style: String,
    pub scene_sound_notes: Vec<SceneSoundNoteOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SceneSoundNoteOutput {
    pub sequence: u32,
    pub scene_id: Uuid,
    pub music_cue: String,
    pub sfx_notes: Vec<String>,
    pub dialogue_direction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EditorOutput {
    pub continuity_ledgers: Vec<ContinuityLedgerOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ContinuityLedgerOutput {
    pub order: u32,
    pub shot_contract_id: Uuid,
    pub work_version_id: Uuid,
    pub inventory_id: Uuid,
    pub evidence_snapshot_id: Uuid,
    pub visual_facts: Vec<String>,
    pub continuity_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QcOutput {
    pub take_reviews: Vec<TakeReviewOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TakeReviewOutput {
    pub required_take_id: Uuid,
    pub work_version_id: Uuid,
    pub inventory_id: Uuid,
    pub evidence_snapshot_id: Uuid,
    pub applicable_shot_contract_ids: Vec<Uuid>,
    pub review_status: String,
    pub quality_assessment: Map<String, Value>,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role", content = "output", rename_all = "snake_case")]
pub enum ValidatedRoleOutput {
    Producer(ProducerOutput),
    Screenwriter(ScreenwriterOutput),
    Director(DirectorOutput),
    Cinematographer(CollaborationRoleOutput),
    PerformanceDirector(PerformanceDirectorOutput),
    SoundDirector(SoundDirectorOutput),
    Editor(EditorOutput),
    Qc(QcOutput),
    CharacterCritic(CollaborationRoleOutput),
}

/// durable-role-output-contract@1 的 canonical JSON Schema。
pub fn role_output_contract_schema(role_key: &str) -> ProductionResult<Value> {
    let non_blank = || json!({"type": "string", "minLength": 1});
    let uuid = || json!({"type": "string", "format": "uuid"});
    let strings = |min_items: u64| {
        json!({
            "type": "array", "minItems": min_items, "uniqueItems": true,
            "items": non_blank()
        })
    };
    let collaboration = || {
        strict_schema(
            &["collaboration_suggestions"],
            json!({
                "collaboration_suggestions": {
                    "type": "array",
                    "items": strict_schema(
                        &["target_artifact_id", "target_artifact_version", "suggestion_type", "content", "priority", "blocking", "rationale"],
                        json!({
                            "target_artifact_id": uuid(),
                            "target_artifact_version": {"type": "integer", "minimum": 1},
                            "suggestion_type": {"enum": ["revision", "addition", "deletion"]},
                            "content": non_blank(),
                            "priority": {"enum": ["low", "medium", "high"]},
                            "blocking": {"type": "boolean"},
                            "rationale": non_blank()
                        })
                    )
                }
            }),
        )
    };

    let schema = match role_key {
        "producer" => strict_schema(
            &["creative_brief"],
            json!({
                "creative_brief": strict_schema(
                    &["target_audience", "tone", "key_messages", "constraints", "success_criteria"],
                    json!({
                        "target_audience": non_blank(),
                        "tone": strings(1),
                        "key_messages": strings(1),
                        "constraints": {"type": "object"},
                        "success_criteria": strings(1)
                    })
                )
            }),
        ),
        "screenwriter" => strict_schema(
            &["story_bible", "character_bibles", "script_draft"],
            json!({
                "story_bible": strict_schema(
                    &["premise", "theme", "narrative_structure", "world"],
                    json!({"premise": non_blank(), "theme": non_blank(), "narrative_structure": non_blank(), "world": non_blank()})
                ),
                "character_bibles": {
                    "type": "array", "minItems": 1,
                    "items": strict_schema(
                        &["character_id", "name", "role", "personality", "motivation", "arc"],
                        json!({"character_id": non_blank(), "name": non_blank(), "role": non_blank(), "personality": non_blank(), "motivation": non_blank(), "arc": non_blank()})
                    )
                },
                "script_draft": strict_schema(
                    &["title", "hook", "scenes"],
                    json!({
                        "title": non_blank(), "hook": non_blank(),
                        "scenes": {
                            "type": "array", "minItems": 3, "maxItems": 12,
                            "items": strict_schema(
                                &["sequence", "narration", "visual_description", "emotion", "duration_sec", "character_ids"],
                                json!({
                                    "sequence": {"type": "integer", "minimum": 1},
                                    "narration": non_blank(), "visual_description": non_blank(), "emotion": non_blank(),
                                    "duration_sec": {"type": "integer", "minimum": 1, "maximum": 30},
                                    "character_ids": strings(0)
                                })
                            )
                        }
                    })
                )
            }),
        ),
        "director" => strict_schema(
            &["directorial_treatment", "shot_contracts"],
            json!({
                "directorial_treatment": strict_schema(
                    &["visual_style", "pacing", "emotional_arc", "color_palette", "reference_works"],
                    json!({"visual_style": non_blank(), "pacing": non_blank(), "emotional_arc": non_blank(), "color_palette": strings(1), "reference_works": strings(0)})
                ),
                "shot_contracts": {
                    "type": "array", "minItems": 1,
                    "items": strict_schema(
                        &["shot_id", "sequence", "scene_id", "shot_type", "camera_movement", "duration_sec", "description", "character_ids"],
                        json!({
                            "shot_id": non_blank(), "sequence": {"type": "integer", "minimum": 1}, "scene_id": uuid(),
                            "shot_type": non_blank(), "camera_movement": non_blank(),
                            "duration_sec": {"type": "integer", "minimum": 1, "maximum": 30},
                            "description": non_blank(), "character_ids": strings(0)
                        })
                    )
                }
            }),
        ),
        "cinematographer" | "character_critic" => collaboration(),
        "performance_director" => strict_schema(
            &["performance_briefs"],
            json!({
                "performance_briefs": {
                    "type": "array", "minItems": 1,
                    "items": strict_schema(
                        &["character_bible_id", "character_id", "script_id", "emotional_arc", "body_language", "vocal_direction"],
                        json!({
                            "character_bible_id": uuid(), "character_id": non_blank(), "script_id": uuid(),
                            "emotional_arc": {
                                "type": "array", "minItems": 1,
                                "items": strict_schema(
                                    &["sequence", "scene_id", "emotion", "intensity", "notes"],
                                    json!({"sequence": {"type": "integer", "minimum": 1}, "scene_id": uuid(), "emotion": non_blank(), "intensity": {"type": "integer", "minimum": 1, "maximum": 10}, "notes": non_blank()})
                                )
                            },
                            "body_language": non_blank(), "vocal_direction": non_blank()
                        })
                    )
                }
            }),
        ),
        "sound_director" => strict_schema(
            &["sound_plan"],
            json!({
                "sound_plan": strict_schema(
                    &["script_id", "music_style", "scene_sound_notes"],
                    json!({
                        "script_id": uuid(), "music_style": non_blank(),
                        "scene_sound_notes": {
                            "type": "array", "minItems": 1,
                            "items": strict_schema(
                                &["sequence", "scene_id", "music_cue", "sfx_notes", "dialogue_direction"],
                                json!({"sequence": {"type": "integer", "minimum": 1}, "scene_id": uuid(), "music_cue": non_blank(), "sfx_notes": strings(0), "dialogue_direction": non_blank()})
                            )
                        }
                    })
                )
            }),
        ),
        "editor" => strict_schema(
            &["continuity_ledgers"],
            json!({
                "continuity_ledgers": {
                    "type": "array", "minItems": 1,
                    "items": strict_schema(
                        &["order", "shot_contract_id", "work_version_id", "inventory_id", "evidence_snapshot_id", "visual_facts", "continuity_flags"],
                        json!({"order": {"type": "integer", "minimum": 1}, "shot_contract_id": uuid(), "work_version_id": uuid(), "inventory_id": uuid(), "evidence_snapshot_id": uuid(), "visual_facts": strings(1), "continuity_flags": strings(0)})
                    )
                }
            }),
        ),
        "qc" => strict_schema(
            &["take_reviews"],
            json!({
                "take_reviews": {
                    "type": "array", "minItems": 1,
                    "items": strict_schema(
                        &["required_take_id", "work_version_id", "inventory_id", "evidence_snapshot_id", "applicable_shot_contract_ids", "review_status", "quality_assessment", "issues", "suggestions"],
                        json!({
                            "required_take_id": uuid(), "work_version_id": uuid(), "inventory_id": uuid(), "evidence_snapshot_id": uuid(),
                            "applicable_shot_contract_ids": {"type": "array", "minItems": 1, "uniqueItems": true, "items": uuid()},
                            "review_status": {"enum": ["approved", "needs_revision", "rejected"]},
                            "quality_assessment": {"type": "object", "minProperties": 1, "additionalProperties": {"type": "number", "minimum": 0, "maximum": 10}},
                            "issues": strings(0), "suggestions": strings(0)
                        })
                    )
                }
            }),
        ),
        _ => return invalid(format!("unknown production role: {role_key}")),
    };
    Ok(schema)
}

/// active Prompt 必须精确发布 durable-role-output-contract@1，放宽或缺字段都不兼容。
pub fn validate_role_output_schema_compatibility(
    role_key: &str,
    output_schema: Option<&Value>,
) -> ProductionResult<()> {
    let actual = output_schema
        .filter(|value| value.get("strict") == Some(&Value::Bool(true)))
        .and_then(|value| value.get("schema"));
    let expected = role_output_contract_schema(role_key)?;
    if actual != Some(&expected) {
        return Err(ProductionError::CapabilityMismatch {
            reason: format!(
                "active output schema for role {role_key} is incompatible with durable-role-output-contract@1"
            ),
        });
    }
    Ok(())
}

fn strict_schema(required: &[&str], properties: Value) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": required,
        "properties": properties
    })
}

pub fn validate_role_output(
    role_key: &str,
    output: &Value,
) -> ProductionResult<ValidatedRoleOutput> {
    let parsed = match role_key {
        "producer" => {
            let value: ProducerOutput = parse(output)?;
            validate_producer(&value)?;
            ValidatedRoleOutput::Producer(value)
        }
        "screenwriter" => {
            let value: ScreenwriterOutput = parse(output)?;
            validate_screenwriter(&value)?;
            ValidatedRoleOutput::Screenwriter(value)
        }
        "director" => {
            let value: DirectorOutput = parse(output)?;
            validate_director(&value)?;
            ValidatedRoleOutput::Director(value)
        }
        "cinematographer" => {
            let value: CollaborationRoleOutput = parse(output)?;
            validate_collaboration(&value)?;
            ValidatedRoleOutput::Cinematographer(value)
        }
        "performance_director" => {
            let value: PerformanceDirectorOutput = parse(output)?;
            validate_performance(&value)?;
            ValidatedRoleOutput::PerformanceDirector(value)
        }
        "sound_director" => {
            let value: SoundDirectorOutput = parse(output)?;
            validate_sound(&value)?;
            ValidatedRoleOutput::SoundDirector(value)
        }
        "editor" => {
            let value: EditorOutput = parse(output)?;
            validate_editor(&value)?;
            ValidatedRoleOutput::Editor(value)
        }
        "qc" => {
            let value: QcOutput = parse(output)?;
            validate_qc(&value)?;
            ValidatedRoleOutput::Qc(value)
        }
        "character_critic" => {
            let value: CollaborationRoleOutput = parse(output)?;
            validate_collaboration(&value)?;
            ValidatedRoleOutput::CharacterCritic(value)
        }
        _ => return invalid(format!("unknown production role: {role_key}")),
    };
    Ok(parsed)
}

fn parse<T>(output: &Value) -> ProductionResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_value(output.clone()).map_err(|error| ProductionError::InvalidArtifactSchema {
        details: error.to_string(),
    })
}

fn validate_producer(output: &ProducerOutput) -> ProductionResult<()> {
    non_blank(
        &output.creative_brief.target_audience,
        "creative_brief.target_audience",
    )?;
    non_empty_strings(&output.creative_brief.tone, "creative_brief.tone")?;
    non_empty_strings(
        &output.creative_brief.key_messages,
        "creative_brief.key_messages",
    )?;
    non_empty_strings(
        &output.creative_brief.success_criteria,
        "creative_brief.success_criteria",
    )
}

fn validate_screenwriter(output: &ScreenwriterOutput) -> ProductionResult<()> {
    for (field, value) in [
        ("story_bible.premise", &output.story_bible.premise),
        ("story_bible.theme", &output.story_bible.theme),
        (
            "story_bible.narrative_structure",
            &output.story_bible.narrative_structure,
        ),
        ("story_bible.world", &output.story_bible.world),
        ("script_draft.title", &output.script_draft.title),
        ("script_draft.hook", &output.script_draft.hook),
    ] {
        non_blank(value, field)?;
    }
    if output.character_bibles.is_empty() {
        return invalid("character_bibles must not be empty");
    }
    let mut character_ids = BTreeSet::new();
    for character in &output.character_bibles {
        for (field, value) in [
            ("character_id", &character.character_id),
            ("name", &character.name),
            ("role", &character.role),
            ("personality", &character.personality),
            ("motivation", &character.motivation),
            ("arc", &character.arc),
        ] {
            non_blank(value, field)?;
        }
        if !character_ids.insert(character.character_id.as_str()) {
            return invalid("character_bibles contains duplicate character_id");
        }
    }
    if !(3..=12).contains(&output.script_draft.scenes.len()) {
        return invalid("script_draft.scenes must contain 3 to 12 scenes");
    }
    let mut total_duration = 0u32;
    for (index, scene) in output.script_draft.scenes.iter().enumerate() {
        if scene.sequence as usize != index + 1 {
            return invalid("script scene sequence must be continuous from 1");
        }
        for (field, value) in [
            ("scene.narration", &scene.narration),
            ("scene.visual_description", &scene.visual_description),
            ("scene.emotion", &scene.emotion),
        ] {
            non_blank(value, field)?;
        }
        if !(1..=30).contains(&scene.duration_sec) {
            return invalid("scene.duration_sec must be between 1 and 30");
        }
        total_duration = total_duration.saturating_add(scene.duration_sec);
        let mut scene_characters = BTreeSet::new();
        for character_id in &scene.character_ids {
            if !character_ids.contains(character_id.as_str()) {
                return invalid("script scene references an unknown character_id");
            }
            if !scene_characters.insert(character_id) {
                return invalid("script scene contains duplicate character_id");
            }
        }
    }
    if total_duration > 60 {
        return invalid("script total duration must not exceed 60 seconds");
    }
    Ok(())
}

fn validate_director(output: &DirectorOutput) -> ProductionResult<()> {
    for (field, value) in [
        (
            "directorial_treatment.visual_style",
            &output.directorial_treatment.visual_style,
        ),
        (
            "directorial_treatment.pacing",
            &output.directorial_treatment.pacing,
        ),
        (
            "directorial_treatment.emotional_arc",
            &output.directorial_treatment.emotional_arc,
        ),
    ] {
        non_blank(value, field)?;
    }
    non_empty_strings(
        &output.directorial_treatment.color_palette,
        "directorial_treatment.color_palette",
    )?;
    if output.shot_contracts.is_empty() {
        return invalid("shot_contracts must not be empty");
    }
    let mut shot_ids = BTreeSet::new();
    for (index, shot) in output.shot_contracts.iter().enumerate() {
        if shot.sequence as usize != index + 1 {
            return invalid("shot sequence must be continuous from 1");
        }
        for (field, value) in [
            ("shot_id", &shot.shot_id),
            ("shot_type", &shot.shot_type),
            ("camera_movement", &shot.camera_movement),
            ("description", &shot.description),
        ] {
            non_blank(value, field)?;
        }
        if !shot_ids.insert(shot.shot_id.as_str()) {
            return invalid("shot_contracts contains duplicate shot_id");
        }
        if !(1..=30).contains(&shot.duration_sec) {
            return invalid("shot duration_sec must be between 1 and 30");
        }
        unique_non_blank_strings(&shot.character_ids, "shot.character_ids")?;
    }
    Ok(())
}

fn validate_collaboration(output: &CollaborationRoleOutput) -> ProductionResult<()> {
    for suggestion in &output.collaboration_suggestions {
        if suggestion.target_artifact_version == 0 {
            return invalid("target_artifact_version must be positive");
        }
        if !matches!(
            suggestion.suggestion_type.as_str(),
            "revision" | "addition" | "deletion"
        ) {
            return invalid("suggestion_type is invalid");
        }
        if !matches!(suggestion.priority.as_str(), "low" | "medium" | "high") {
            return invalid("suggestion priority is invalid");
        }
        non_blank(&suggestion.content, "suggestion.content")?;
        non_blank(&suggestion.rationale, "suggestion.rationale")?;
    }
    Ok(())
}

fn validate_performance(output: &PerformanceDirectorOutput) -> ProductionResult<()> {
    if output.performance_briefs.is_empty() {
        return invalid("performance_briefs must not be empty");
    }
    let script_id = output.performance_briefs[0].script_id;
    let mut characters = BTreeSet::new();
    for brief in &output.performance_briefs {
        if brief.script_id != script_id {
            return invalid("performance_briefs must reference one script_id");
        }
        if !characters.insert(brief.character_bible_id) {
            return invalid("performance_briefs contains duplicate character_bible_id");
        }
        non_blank(&brief.character_id, "performance_brief.character_id")?;
        non_blank(&brief.body_language, "performance_brief.body_language")?;
        non_blank(&brief.vocal_direction, "performance_brief.vocal_direction")?;
        if brief.emotional_arc.is_empty() {
            return invalid("performance_brief.emotional_arc must not be empty");
        }
        let mut scenes = BTreeSet::new();
        for (index, scene) in brief.emotional_arc.iter().enumerate() {
            if scene.sequence as usize != index + 1 {
                return invalid("performance scene sequence must be continuous from 1");
            }
            if !(1..=10).contains(&scene.intensity) {
                return invalid("performance intensity must be between 1 and 10");
            }
            if !scenes.insert(scene.scene_id) {
                return invalid("performance emotional_arc contains duplicate scene_id");
            }
            non_blank(&scene.emotion, "performance scene emotion")?;
            non_blank(&scene.notes, "performance scene notes")?;
        }
    }
    Ok(())
}

fn validate_sound(output: &SoundDirectorOutput) -> ProductionResult<()> {
    non_blank(&output.sound_plan.music_style, "sound_plan.music_style")?;
    if output.sound_plan.scene_sound_notes.is_empty() {
        return invalid("sound_plan.scene_sound_notes must not be empty");
    }
    let mut scenes = BTreeSet::new();
    for (index, note) in output.sound_plan.scene_sound_notes.iter().enumerate() {
        if note.sequence as usize != index + 1 {
            return invalid("sound scene sequence must be continuous from 1");
        }
        if !scenes.insert(note.scene_id) {
            return invalid("sound plan contains duplicate scene_id");
        }
        non_blank(&note.music_cue, "sound scene music_cue")?;
        non_blank(&note.dialogue_direction, "sound scene dialogue_direction")?;
        unique_non_blank_strings(&note.sfx_notes, "sound scene sfx_notes")?;
    }
    Ok(())
}

fn validate_editor(output: &EditorOutput) -> ProductionResult<()> {
    if output.continuity_ledgers.is_empty() {
        return invalid("continuity_ledgers must not be empty");
    }
    let first = &output.continuity_ledgers[0];
    let mut shots = BTreeSet::new();
    for (index, ledger) in output.continuity_ledgers.iter().enumerate() {
        if ledger.order as usize != index + 1 {
            return invalid("continuity ledger order must be continuous from 1");
        }
        if ledger.work_version_id != first.work_version_id
            || ledger.inventory_id != first.inventory_id
            || ledger.evidence_snapshot_id != first.evidence_snapshot_id
        {
            return invalid("continuity ledgers must share work/inventory/evidence scope");
        }
        if !shots.insert(ledger.shot_contract_id) {
            return invalid("continuity ledgers contains duplicate shot_contract_id");
        }
        non_empty_strings(&ledger.visual_facts, "continuity_ledger.visual_facts")?;
        unique_non_blank_strings(
            &ledger.continuity_flags,
            "continuity_ledger.continuity_flags",
        )?;
    }
    Ok(())
}

fn validate_qc(output: &QcOutput) -> ProductionResult<()> {
    if output.take_reviews.is_empty() {
        return invalid("take_reviews must not be empty");
    }
    let first = &output.take_reviews[0];
    let mut takes = BTreeSet::new();
    for review in &output.take_reviews {
        if review.work_version_id != first.work_version_id
            || review.inventory_id != first.inventory_id
            || review.evidence_snapshot_id != first.evidence_snapshot_id
        {
            return invalid("take reviews must share work/inventory/evidence scope");
        }
        if !takes.insert(review.required_take_id) {
            return invalid("take_reviews contains duplicate required_take_id");
        }
        if !matches!(
            review.review_status.as_str(),
            "approved" | "needs_revision" | "rejected"
        ) {
            return invalid("take review status is invalid");
        }
        if review.quality_assessment.is_empty() {
            return invalid("take review quality_assessment must not be empty");
        }
        for score in review.quality_assessment.values() {
            let Some(value) = score.as_f64() else {
                return invalid("take review quality scores must be numeric");
            };
            if !(0.0..=10.0).contains(&value) {
                return invalid("take review quality scores must be between 0 and 10");
            }
        }
        if review.applicable_shot_contract_ids.is_empty() {
            return invalid("take review must reference applicable shot contracts");
        }
        let unique_shots: BTreeSet<_> = review.applicable_shot_contract_ids.iter().collect();
        if unique_shots.len() != review.applicable_shot_contract_ids.len() {
            return invalid("take review contains duplicate shot contract references");
        }
        unique_non_blank_strings(&review.issues, "take_review.issues")?;
        unique_non_blank_strings(&review.suggestions, "take_review.suggestions")?;
    }
    Ok(())
}

fn non_blank(value: &str, field: &str) -> ProductionResult<()> {
    if value.trim().is_empty() {
        return invalid(format!("{field} must not be blank"));
    }
    Ok(())
}

fn non_empty_strings(values: &[String], field: &str) -> ProductionResult<()> {
    if values.is_empty() {
        return invalid(format!("{field} must not be empty"));
    }
    unique_non_blank_strings(values, field)
}

fn unique_non_blank_strings(values: &[String], field: &str) -> ProductionResult<()> {
    let mut unique = BTreeSet::new();
    for value in values {
        non_blank(value, field)?;
        if !unique.insert(value.as_str()) {
            return invalid(format!("{field} contains duplicates"));
        }
    }
    Ok(())
}

fn invalid<T>(details: impl Into<String>) -> ProductionResult<T> {
    Err(ProductionError::InvalidArtifactSchema {
        details: details.into(),
    })
}
