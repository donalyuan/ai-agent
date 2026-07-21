//! 作品生成领域规则：计划版本、能力校验、Seedance 分段和运行快照。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use uuid::Uuid;

pub const MIN_SEGMENT_SECONDS: u32 = 4;
pub const MAX_SEGMENT_SECONDS: u32 = 15;
pub const MAX_WORK_SECONDS: u32 = 60;
pub const MAX_REFERENCE_IMAGES: usize = 9;
pub const MAX_CHINESE_PROMPT_CHARS: usize = 500;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DurationStrategy {
    Preset15,
    Preset30,
    Preset45,
    Preset60,
    Custom,
    FollowNarration,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioMode {
    IndependentTts,
    SeedanceOriginal,
    SeedanceOriginalAndTts,
    Silent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleSource {
    TtsTimestamp,
    Asr,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceImageMode {
    FirstLastFrames,
    #[default]
    MultiReference,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VideoCapability {
    pub version: String,
    #[serde(default)]
    pub reference_image_mode: ReferenceImageMode,
    pub min_duration_seconds: u32,
    pub max_duration_seconds: u32,
    pub max_reference_images: usize,
    pub max_prompt_chars: usize,
    pub aspect_ratios: Vec<String>,
    pub resolutions: Vec<String>,
    pub audio_supported: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OutputSpec {
    pub duration_strategy: DurationStrategy,
    pub duration_seconds: Option<u32>,
    pub aspect_ratio: String,
    pub resolution: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SceneInput {
    pub scene_id: Uuid,
    pub sequence: i32,
    pub image_material_id: Uuid,
    pub image_url: String,
    pub prompt: String,
    pub narration: String,
    pub duration_seconds: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VideoSegment {
    pub sequence: usize,
    pub scene_ids: Vec<Uuid>,
    pub reference_image_ids: Vec<Uuid>,
    pub prompt: String,
    pub duration_seconds: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResourceUsage {
    pub video_task_count: usize,
    pub video_seconds: u32,
    pub tts_characters: usize,
    pub asr_seconds: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkPlan {
    pub plan_id: Uuid,
    pub plan_version: i32,
    pub input_fingerprint: String,
    pub video_model_id: Uuid,
    pub tts_model_id: Option<Uuid>,
    pub llm_model_id: Uuid,
    pub video_capability: VideoCapability,
    pub output: OutputSpec,
    pub audio_mode: AudioMode,
    pub subtitle_source: Option<SubtitleSource>,
    pub full_prompt: String,
    pub segments: Vec<VideoSegment>,
    pub resource_usage: ResourceUsage,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Work {
    pub id: Uuid,
    pub project_id: Uuid,
    pub script_id: Uuid,
    pub title: String,
    pub status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkVersion {
    pub id: Uuid,
    pub work_id: Uuid,
    pub version_no: i32,
    pub source_manifest_version: String,
    pub input_snapshot: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkGenerationRun {
    pub id: Uuid,
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub plan_id: Uuid,
    pub status: String,
    pub snapshot: WorkGenerationSnapshot,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkGenerationSnapshot {
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub plan_id: Uuid,
    pub plan_version: i32,
    pub model_snapshot: Value,
    pub capability_snapshot: Value,
    pub voice_snapshot: Value,
    pub prompt_snapshot: Value,
    pub timeline_snapshot: Value,
    pub parameter_snapshot: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationStepStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    Blocked,
    WaitingManual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GenerationRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    WaitingManual,
    Cancelling,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationTaskStep {
    pub step_type: String,
    pub status: GenerationStepStatus,
    pub is_required: bool,
    /// 是否已创建过 provider attempt，用于区分“未生成”和“生成中”。
    pub attempt_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GenerationTaskAggregation {
    pub status: GenerationRunStatus,
    pub current_stage: String,
    pub progress_percent: i32,
    pub successful_steps: usize,
    pub running_steps: usize,
    pub queued_steps: usize,
    pub failed_steps: usize,
}

/// 按锁定 DAG 中的必需节点确定性聚合运行状态，展示计数则保留全部实际节点。
pub fn aggregate_generation_task(
    steps: &[GenerationTaskStep],
    cancellation_requested: bool,
) -> GenerationTaskAggregation {
    let required = steps
        .iter()
        .filter(|step| step.is_required)
        .collect::<Vec<_>>();
    let successful_required = required
        .iter()
        .filter(|step| step.status == GenerationStepStatus::Succeeded)
        .count();
    let progress_percent = if required.is_empty() {
        0
    } else {
        ((successful_required * 100) / required.len()) as i32
    };
    let first_stage = |statuses: &[GenerationStepStatus]| {
        required
            .iter()
            .find(|step| statuses.contains(&step.status))
            .map(|step| step.step_type.clone())
    };
    let has_waiting_manual = required
        .iter()
        .any(|step| step.status == GenerationStepStatus::WaitingManual);
    let has_failed = required
        .iter()
        .any(|step| step.status == GenerationStepStatus::Failed);
    let has_active = required
        .iter()
        .any(|step| step.status == GenerationStepStatus::Running);
    let has_cancelled = required
        .iter()
        .any(|step| step.status == GenerationStepStatus::Cancelled);
    let external_started = has_active || steps.iter().any(|step| step.attempt_count > 0);
    let all_succeeded = !required.is_empty() && successful_required == required.len();

    let (status, current_stage) = if has_waiting_manual {
        (
            GenerationRunStatus::WaitingManual,
            first_stage(&[GenerationStepStatus::WaitingManual])
                .unwrap_or_else(|| "waiting_manual".into()),
        )
    } else if has_failed && !has_active {
        (
            GenerationRunStatus::Failed,
            first_stage(&[GenerationStepStatus::Failed]).unwrap_or_else(|| "failed".into()),
        )
    } else if cancellation_requested && has_active {
        (GenerationRunStatus::Cancelling, "cancelling".into())
    } else if all_succeeded {
        (GenerationRunStatus::Succeeded, "completed".into())
    } else if has_cancelled && !has_active {
        (GenerationRunStatus::Cancelled, "cancelled".into())
    } else {
        let stage = first_stage(&[
            GenerationStepStatus::Running,
            GenerationStepStatus::Queued,
            GenerationStepStatus::Blocked,
        ])
        .unwrap_or_else(|| "queued".into());
        (
            if external_started {
                GenerationRunStatus::Running
            } else {
                GenerationRunStatus::Queued
            },
            stage,
        )
    };

    GenerationTaskAggregation {
        status,
        current_stage,
        progress_percent,
        successful_steps: steps
            .iter()
            .filter(|step| step.status == GenerationStepStatus::Succeeded)
            .count(),
        running_steps: steps
            .iter()
            .filter(|step| step.status == GenerationStepStatus::Running)
            .count(),
        queued_steps: steps
            .iter()
            .filter(|step| {
                matches!(
                    step.status,
                    GenerationStepStatus::Queued | GenerationStepStatus::Blocked
                )
            })
            .count(),
        failed_steps: steps
            .iter()
            .filter(|step| {
                matches!(
                    step.status,
                    GenerationStepStatus::Failed | GenerationStepStatus::WaitingManual
                )
            })
            .count(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkGenerationError {
    InvalidDuration(String),
    UnsupportedOutput(String),
    InvalidAudioMode(String),
    InvalidSegments(String),
    PromptTooLong,
    MissingInput(String),
    StalePlan,
    DuplicateIdempotencyKey,
}

impl fmt::Display for WorkGenerationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDuration(value) => write!(f, "时长参数无效: {value}"),
            Self::UnsupportedOutput(value) => write!(f, "输出规格不支持: {value}"),
            Self::InvalidAudioMode(value) => write!(f, "声音模式无效: {value}"),
            Self::InvalidSegments(value) => write!(f, "无法形成合法分段: {value}"),
            Self::PromptTooLong => write!(f, "中文分段提示词不得超过 500 字"),
            Self::MissingInput(value) => write!(f, "缺少生成输入: {value}"),
            Self::StalePlan => write!(f, "作品计划已失效，请重新规划并确认"),
            Self::DuplicateIdempotencyKey => write!(f, "幂等键已绑定其他作品计划"),
        }
    }
}

impl std::error::Error for WorkGenerationError {}

pub fn target_duration(
    output: &OutputSpec,
    narration_seconds: Option<u32>,
) -> Result<u32, WorkGenerationError> {
    let duration = match output.duration_strategy {
        DurationStrategy::Preset15 => 15,
        DurationStrategy::Preset30 => 30,
        DurationStrategy::Preset45 => 45,
        DurationStrategy::Preset60 => 60,
        DurationStrategy::Custom => output.duration_seconds.ok_or_else(|| {
            WorkGenerationError::InvalidDuration("自定义时长必须提供 duration_seconds".into())
        })?,
        DurationStrategy::FollowNarration => narration_seconds.ok_or_else(|| {
            WorkGenerationError::InvalidDuration("跟随配音需要先取得实际 TTS 时长".into())
        })?,
    };
    if !(MIN_SEGMENT_SECONDS..=MAX_WORK_SECONDS).contains(&duration) {
        return Err(WorkGenerationError::InvalidDuration(
            "最终成片时长必须在 4~60 秒".into(),
        ));
    }
    Ok(duration)
}

pub fn validate_output(
    output: &OutputSpec,
    capability: &VideoCapability,
    narration_seconds: Option<u32>,
) -> Result<u32, WorkGenerationError> {
    let duration = target_duration(output, narration_seconds)?;
    if duration < capability.min_duration_seconds || duration > MAX_WORK_SECONDS {
        return Err(WorkGenerationError::InvalidDuration(format!(
            "模型支持 {}~{} 秒",
            capability.min_duration_seconds, capability.max_duration_seconds
        )));
    }
    if duration > capability.max_duration_seconds && capability.max_duration_seconds > 0 {
        // 作品可以超过单任务上限，但不得超过最终模型能力上限。
        if capability.max_duration_seconds < MAX_SEGMENT_SECONDS {
            return Err(WorkGenerationError::InvalidDuration(
                "模型无法覆盖作品时长".into(),
            ));
        }
    }
    if !capability
        .aspect_ratios
        .iter()
        .any(|v| v == &output.aspect_ratio)
    {
        return Err(WorkGenerationError::UnsupportedOutput(format!(
            "比例 {} 不在模型能力目录",
            output.aspect_ratio
        )));
    }
    if !capability
        .resolutions
        .iter()
        .any(|v| v == &output.resolution)
    {
        return Err(WorkGenerationError::UnsupportedOutput(format!(
            "分辨率 {} 不在模型能力目录",
            output.resolution
        )));
    }
    Ok(duration)
}

pub fn validate_audio_mode(
    mode: AudioMode,
    audio_supported: bool,
) -> Result<Option<SubtitleSource>, WorkGenerationError> {
    match mode {
        AudioMode::IndependentTts => Ok(Some(SubtitleSource::TtsTimestamp)),
        AudioMode::SeedanceOriginal => {
            if !audio_supported {
                return Err(WorkGenerationError::InvalidAudioMode(
                    "当前视频模型不支持原声".into(),
                ));
            }
            Ok(Some(SubtitleSource::Asr))
        }
        AudioMode::SeedanceOriginalAndTts => {
            if !audio_supported {
                return Err(WorkGenerationError::InvalidAudioMode(
                    "当前视频模型不支持原声".into(),
                ));
            }
            Ok(Some(SubtitleSource::TtsTimestamp))
        }
        AudioMode::Silent => Ok(None),
    }
}

pub fn build_segments(
    scenes: &[SceneInput],
    target_seconds: u32,
    capability: &VideoCapability,
) -> Result<Vec<VideoSegment>, WorkGenerationError> {
    if scenes.is_empty() {
        return Err(WorkGenerationError::MissingInput(
            "至少需要一个主画面分镜".into(),
        ));
    }
    if capability.max_reference_images == 0 || capability.max_prompt_chars == 0 {
        return Err(WorkGenerationError::InvalidSegments(
            "模型能力目录不完整".into(),
        ));
    }
    let min = capability.min_duration_seconds.max(MIN_SEGMENT_SECONDS);
    let max = capability.max_duration_seconds.min(MAX_SEGMENT_SECONDS);
    if min > max {
        return Err(WorkGenerationError::InvalidSegments(
            "模型没有 4~15 秒合法区间".into(),
        ));
    }
    for scene in scenes {
        if scene.image_url.trim().is_empty() {
            return Err(WorkGenerationError::MissingInput(format!(
                "分镜 {} 缺少主画面",
                scene.sequence
            )));
        }
        if scene.prompt.chars().count() > MAX_CHINESE_PROMPT_CHARS {
            return Err(WorkGenerationError::PromptTooLong);
        }
    }

    let duration_segment_count = target_seconds.div_ceil(max) as usize;
    let image_segment_count = match capability.reference_image_mode {
        ReferenceImageMode::FirstLastFrames => 1,
        ReferenceImageMode::MultiReference => {
            scenes.len().div_ceil(capability.max_reference_images)
        }
    };
    let segment_count = duration_segment_count.max(image_segment_count).max(1);
    if target_seconds < min * segment_count as u32 || target_seconds > max * segment_count as u32 {
        return Err(WorkGenerationError::InvalidSegments(
            "目标时长、分镜数量和参考图上限无法组成全部合法分段".into(),
        ));
    }

    let mut groups: Vec<Vec<&SceneInput>> = Vec::with_capacity(segment_count);
    if segment_count <= scenes.len() {
        let base_group_size = scenes.len() / segment_count;
        let larger_group_count = scenes.len() % segment_count;
        let mut offset = 0;
        for index in 0..segment_count {
            let size = base_group_size + usize::from(index < larger_group_count);
            groups.push(scenes[offset..offset + size].iter().collect());
            offset += size;
        }
    } else {
        // 长单镜头允许在相邻子任务复用同一主画面，仍保持一次作品提交语义。
        for index in 0..segment_count {
            groups.push(vec![&scenes[index * scenes.len() / segment_count]]);
        }
    }
    if matches!(
        capability.reference_image_mode,
        ReferenceImageMode::MultiReference
    ) && groups
        .iter()
        .any(|group| group.len() > capability.max_reference_images)
    {
        return Err(WorkGenerationError::InvalidSegments(
            "存在超过参考图上限的分段".into(),
        ));
    }

    // 先给每段最小时长，再均匀分配剩余秒数；可确定地消除不足 4 秒的尾段。
    let mut durations = vec![min; segment_count];
    let mut remaining = target_seconds - min * segment_count as u32;
    for index in 0..segment_count {
        let remaining_groups = (segment_count - index) as u32;
        let addition = remaining.div_ceil(remaining_groups).min(max - min);
        durations[index] += addition;
        remaining -= addition;
    }
    if remaining != 0 {
        return Err(WorkGenerationError::InvalidSegments(
            "尾段时长无法重分配".into(),
        ));
    }

    let segments = groups
        .into_iter()
        .zip(durations)
        .enumerate()
        .map(|(index, (group, duration_seconds))| {
            let reference_image_ids = match capability.reference_image_mode {
                ReferenceImageMode::FirstLastFrames if group.len() > 1 => vec![
                    group.first().expect("group is non-empty").image_material_id,
                    group.last().expect("group is non-empty").image_material_id,
                ],
                _ => group.iter().map(|scene| scene.image_material_id).collect(),
            };
            VideoSegment {
                sequence: index + 1,
                scene_ids: group.iter().map(|scene| scene.scene_id).collect(),
                reference_image_ids,
                prompt: group
                    .iter()
                    .map(|scene| scene.prompt.trim())
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>()
                    .join("；"),
                duration_seconds,
            }
        })
        .collect::<Vec<_>>();
    for segment in &segments {
        if segment.reference_image_ids.len() > MAX_REFERENCE_IMAGES
            || segment.prompt.chars().count() > capability.max_prompt_chars
        {
            return Err(WorkGenerationError::InvalidSegments(format!(
                "分段 {} 超出模型限制",
                segment.sequence
            )));
        }
    }
    debug_assert_eq!(
        segments
            .iter()
            .map(|segment| segment.duration_seconds)
            .sum::<u32>(),
        target_seconds
    );
    Ok(segments)
}

pub fn apply_segment_prompt_overrides(
    segments: &mut [VideoSegment],
    overrides: Option<Vec<String>>,
    max_prompt_chars: usize,
) -> Result<(), WorkGenerationError> {
    let Some(overrides) = overrides else {
        return Ok(());
    };
    if overrides.len() != segments.len() {
        return Err(WorkGenerationError::InvalidSegments(format!(
            "分段提示词数量必须为 {}",
            segments.len()
        )));
    }
    for (segment, prompt) in segments.iter_mut().zip(overrides) {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(WorkGenerationError::InvalidSegments(format!(
                "第 {} 段提示词不能为空",
                segment.sequence
            )));
        }
        if prompt.chars().count() > max_prompt_chars {
            return Err(WorkGenerationError::PromptTooLong);
        }
        segment.prompt = prompt.to_string();
    }
    Ok(())
}

pub fn snapshot(
    work_id: Uuid,
    work_version_id: Uuid,
    plan: &WorkPlan,
    voice_snapshot: Value,
) -> WorkGenerationSnapshot {
    WorkGenerationSnapshot {
        work_id,
        work_version_id,
        plan_id: plan.plan_id,
        plan_version: plan.plan_version,
        model_snapshot: json!({"llm_model_id": plan.llm_model_id, "video_model_id": plan.video_model_id, "tts_model_id": plan.tts_model_id}),
        capability_snapshot: serde_json::to_value(&plan.video_capability)
            .unwrap_or_else(|_| json!({})),
        voice_snapshot,
        prompt_snapshot: json!({"full_prompt": plan.full_prompt, "segments": plan.segments}),
        timeline_snapshot: json!({"duration_seconds": plan.resource_usage.video_seconds}),
        parameter_snapshot: json!({"output": plan.output, "audio_mode": plan.audio_mode, "subtitle_source": plan.subtitle_source}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task_step(
        step_type: &str,
        status: GenerationStepStatus,
        required: bool,
    ) -> GenerationTaskStep {
        GenerationTaskStep {
            step_type: step_type.into(),
            status,
            is_required: required,
            attempt_count: 0,
        }
    }

    fn capability() -> VideoCapability {
        VideoCapability {
            version: "seedance-test-v1".into(),
            reference_image_mode: ReferenceImageMode::MultiReference,
            min_duration_seconds: 4,
            max_duration_seconds: 15,
            max_reference_images: 9,
            max_prompt_chars: 500,
            aspect_ratios: vec!["16:9".into()],
            resolutions: vec!["1080p".into()],
            audio_supported: true,
        }
    }

    #[test]
    fn first_last_mode_keeps_six_scene_semantics_in_one_twelve_second_task() {
        let scenes = (1..=6)
            .map(|sequence| SceneInput {
                scene_id: Uuid::new_v4(),
                sequence,
                image_material_id: Uuid::new_v4(),
                image_url: "https://assets/image.png".into(),
                prompt: format!("镜头{sequence}"),
                narration: "".into(),
                duration_seconds: 2,
            })
            .collect::<Vec<_>>();
        let mut first_last = capability();
        first_last.reference_image_mode = ReferenceImageMode::FirstLastFrames;
        first_last.max_duration_seconds = 12;
        first_last.max_reference_images = 2;

        let segments = build_segments(&scenes, 12, &first_last).unwrap();

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].duration_seconds, 12);
        assert_eq!(segments[0].scene_ids.len(), 6);
        assert_eq!(segments[0].reference_image_ids.len(), 2);
        assert_eq!(
            segments[0].reference_image_ids[0],
            scenes[0].image_material_id
        );
        assert_eq!(
            segments[0].reference_image_ids[1],
            scenes[5].image_material_id
        );
        assert_eq!(
            segments[0].prompt,
            "镜头1；镜头2；镜头3；镜头4；镜头5；镜头6"
        );
    }

    #[test]
    fn rejects_duration_outside_range() {
        let output = OutputSpec {
            duration_strategy: DurationStrategy::Custom,
            duration_seconds: Some(3),
            aspect_ratio: "16:9".into(),
            resolution: "1080p".into(),
        };
        assert!(matches!(
            target_duration(&output, None),
            Err(WorkGenerationError::InvalidDuration(_))
        ));
    }

    #[test]
    fn validates_audio_capability() {
        assert!(validate_audio_mode(AudioMode::SeedanceOriginal, false).is_err());
        assert_eq!(
            validate_audio_mode(AudioMode::IndependentTts, false).unwrap(),
            Some(SubtitleSource::TtsTimestamp)
        );
        assert_eq!(validate_audio_mode(AudioMode::Silent, false).unwrap(), None);
    }

    #[test]
    fn segments_by_duration_and_reference_limit() {
        let scenes = (1..=4)
            .map(|sequence| SceneInput {
                scene_id: Uuid::new_v4(),
                sequence,
                image_material_id: Uuid::new_v4(),
                image_url: "https://assets/image.png".into(),
                prompt: "镜头动作".into(),
                narration: "旁白".into(),
                duration_seconds: 4,
            })
            .collect::<Vec<_>>();
        let segments = build_segments(&scenes, 16, &capability()).unwrap();
        assert_eq!(segments.len(), 2);
        assert!(segments
            .iter()
            .all(|segment| (4..=15).contains(&segment.duration_seconds)));
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.duration_seconds)
                .sum::<u32>(),
            16
        );
    }

    #[test]
    fn long_single_scene_reuses_reference_and_preserves_total_duration() {
        let scene = SceneInput {
            scene_id: Uuid::new_v4(),
            sequence: 1,
            image_material_id: Uuid::new_v4(),
            image_url: "https://assets/image.png".into(),
            prompt: "连续长镜头".into(),
            narration: "旁白".into(),
            duration_seconds: 30,
        };
        let segments = build_segments(&[scene], 30, &capability()).unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.duration_seconds)
                .sum::<u32>(),
            30
        );
        assert!(segments
            .iter()
            .all(|segment| segment.reference_image_ids.len() == 1));
    }

    #[test]
    fn rejects_missing_primary_image() {
        let scene = SceneInput {
            scene_id: Uuid::new_v4(),
            sequence: 1,
            image_material_id: Uuid::new_v4(),
            image_url: "".into(),
            prompt: "镜头".into(),
            narration: "".into(),
            duration_seconds: 4,
        };
        assert!(matches!(
            build_segments(&[scene], 4, &capability()),
            Err(WorkGenerationError::MissingInput(_))
        ));
    }

    #[test]
    fn applies_exact_segment_prompt_overrides() {
        let scene = SceneInput {
            scene_id: Uuid::new_v4(),
            sequence: 1,
            image_material_id: Uuid::new_v4(),
            image_url: "https://assets/image.png".into(),
            prompt: "原始提示词".into(),
            narration: "旁白".into(),
            duration_seconds: 30,
        };
        let mut segments = build_segments(&[scene], 30, &capability()).unwrap();
        apply_segment_prompt_overrides(
            &mut segments,
            Some(vec![" 开场覆盖 ".into(), "结尾覆盖".into()]),
            500,
        )
        .unwrap();
        assert_eq!(segments[0].prompt, "开场覆盖");
        assert_eq!(segments[1].prompt, "结尾覆盖");
    }

    #[test]
    fn rejects_mismatched_or_oversized_segment_prompt_overrides() {
        let scene = SceneInput {
            scene_id: Uuid::new_v4(),
            sequence: 1,
            image_material_id: Uuid::new_v4(),
            image_url: "https://assets/image.png".into(),
            prompt: "原始提示词".into(),
            narration: "旁白".into(),
            duration_seconds: 30,
        };
        let segments = build_segments(&[scene], 30, &capability()).unwrap();
        assert!(matches!(
            apply_segment_prompt_overrides(
                &mut segments.clone(),
                Some(vec!["只有一段".into()]),
                500
            ),
            Err(WorkGenerationError::InvalidSegments(_))
        ));
        assert_eq!(
            apply_segment_prompt_overrides(
                &mut segments.clone(),
                Some(vec!["a".repeat(501), "结尾".into()]),
                500,
            ),
            Err(WorkGenerationError::PromptTooLong)
        );
    }

    #[test]
    fn aggregates_parallel_segments_from_required_step_terminal_states() {
        let mut steps = vec![
            task_step("plan", GenerationStepStatus::Succeeded, true),
            task_step("video_segment", GenerationStepStatus::Succeeded, true),
            task_step("video_segment", GenerationStepStatus::Running, true),
            task_step("video_segment", GenerationStepStatus::Queued, true),
            task_step("compose", GenerationStepStatus::Queued, true),
        ];
        steps[1].attempt_count = 1;
        steps[2].attempt_count = 1;

        let aggregation = aggregate_generation_task(&steps, false);

        assert_eq!(aggregation.status, GenerationRunStatus::Running);
        assert_eq!(aggregation.current_stage, "video_segment");
        assert_eq!(aggregation.progress_percent, 40);
        assert_eq!(aggregation.successful_steps, 2);
        assert_eq!(aggregation.running_steps, 1);
        assert_eq!(aggregation.queued_steps, 2);
        assert_eq!(aggregation.failed_steps, 0);
    }

    #[test]
    fn ignores_unplanned_conditional_steps_when_computing_progress() {
        let steps = vec![
            task_step("plan", GenerationStepStatus::Succeeded, true),
            task_step("tts", GenerationStepStatus::Blocked, false),
            task_step("asr", GenerationStepStatus::Succeeded, true),
            task_step("compose", GenerationStepStatus::Succeeded, true),
        ];

        let aggregation = aggregate_generation_task(&steps, false);

        assert_eq!(aggregation.status, GenerationRunStatus::Succeeded);
        assert_eq!(aggregation.current_stage, "completed");
        assert_eq!(aggregation.progress_percent, 100);
        assert_eq!(aggregation.queued_steps, 1);
    }

    #[test]
    fn required_failure_takes_precedence_and_keeps_downstream_blocked() {
        let steps = vec![
            task_step("plan", GenerationStepStatus::Succeeded, true),
            task_step("tts", GenerationStepStatus::Failed, true),
            task_step("subtitle", GenerationStepStatus::Blocked, true),
            task_step("compose", GenerationStepStatus::Blocked, true),
        ];

        let aggregation = aggregate_generation_task(&steps, false);

        assert_eq!(aggregation.status, GenerationRunStatus::Failed);
        assert_eq!(aggregation.current_stage, "tts");
        assert_eq!(aggregation.failed_steps, 1);
        assert_eq!(aggregation.queued_steps, 2);
    }

    #[test]
    fn waiting_manual_and_cancelling_are_not_reported_as_normal_completion() {
        let waiting = aggregate_generation_task(
            &[
                task_step("plan", GenerationStepStatus::Succeeded, true),
                task_step("video_segment", GenerationStepStatus::WaitingManual, true),
            ],
            false,
        );
        assert_eq!(waiting.status, GenerationRunStatus::WaitingManual);

        let cancelling = aggregate_generation_task(
            &[
                task_step("plan", GenerationStepStatus::Succeeded, true),
                task_step("video_segment", GenerationStepStatus::Running, true),
                task_step("compose", GenerationStepStatus::Cancelled, true),
            ],
            true,
        );
        assert_eq!(cancelling.status, GenerationRunStatus::Cancelling);
    }
}
