use super::{canonical_digest, domain_error};
use crate::ProductionResult;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Role,
    Gate,
    DomainCommand,
    ExternalWait,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub key: String,
    pub kind: StepKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_key: Option<String>,
    pub dependencies: Vec<String>,
    pub optional: bool,
}

impl PlanStep {
    fn role(key: &str, dependencies: &[&str], optional: bool) -> Self {
        Self {
            key: key.into(),
            kind: StepKind::Role,
            role_key: Some(key.into()),
            dependencies: dependencies.iter().map(|value| (*value).into()).collect(),
            optional,
        }
    }

    fn system(key: &str, kind: StepKind, dependencies: &[&str]) -> Self {
        Self {
            key: key.into(),
            kind,
            role_key: None,
            dependencies: dependencies.iter().map(|value| (*value).into()).collect(),
            optional: false,
        }
    }
}

/// Run 创建时冻结的全部非金额资源边界。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    pub max_role_calls: u64,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
    pub max_role_retries: u64,
    pub max_quality_reworks: u64,
    pub max_video_tasks: u64,
    pub max_video_duration_sec: u64,
    pub max_tts_characters: u64,
    pub max_asr_tasks: u64,
    pub max_concurrency: u64,
    pub max_provider_retries: u64,
}

impl ResourceLimits {
    pub fn strict_default() -> Self {
        Self {
            max_role_calls: 16,
            max_input_tokens: 240_000,
            max_output_tokens: 48_000,
            max_role_retries: 1,
            max_quality_reworks: 2,
            max_video_tasks: 20,
            max_video_duration_sec: 60,
            max_tts_characters: 8_000,
            max_asr_tasks: 20,
            max_concurrency: 2,
            max_provider_retries: 1,
        }
    }

    pub fn value(&self, key: &str) -> Option<u64> {
        match key {
            "role_calls" => Some(self.max_role_calls),
            "input_tokens" => Some(self.max_input_tokens),
            "output_tokens" => Some(self.max_output_tokens),
            "role_retries" => Some(self.max_role_retries),
            "quality_reworks" => Some(self.max_quality_reworks),
            "video_tasks" => Some(self.max_video_tasks),
            "video_duration_sec" => Some(self.max_video_duration_sec),
            "tts_characters" => Some(self.max_tts_characters),
            "asr_tasks" => Some(self.max_asr_tasks),
            "concurrency" => Some(self.max_concurrency),
            "provider_retries" => Some(self.max_provider_retries),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSnapshot {
    pub plan_key: String,
    pub plan_version: String,
    pub digest: String,
    pub steps: Vec<PlanStep>,
    pub include_character_critic: bool,
    pub max_package_revisions: BTreeMap<String, u32>,
    pub max_quality_reworks: u32,
    pub role_bindings: Value,
    pub resource_limits: ResourceLimits,
}

impl PlanSnapshot {
    pub fn step(&self, key: &str) -> Option<&PlanStep> {
        self.steps.iter().find(|step| step.key == key)
    }

    /// 验证快照仍与代码发布的固定计划一致，防止公开字段被修改后沿用旧 digest。
    pub fn validate_frozen(&self) -> ProductionResult<()> {
        let expected = FullCrewPlanRegistry::snapshot_v1(
            self.include_character_critic,
            self.role_bindings.clone(),
            self.resource_limits.clone(),
        )?;
        if self.plan_key != expected.plan_key
            || self.plan_version != expected.plan_version
            || self.digest != expected.digest
            || self.steps != expected.steps
            || self.max_package_revisions != expected.max_package_revisions
            || self.max_quality_reworks != expected.max_quality_reworks
        {
            return Err(domain_error(
                "plan snapshot does not match the published Full Crew definition",
            ));
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct DigestInput<'a> {
    plan_key: &'a str,
    plan_version: &'a str,
    steps: &'a [PlanStep],
    include_character_critic: bool,
    max_package_revisions: &'a BTreeMap<String, u32>,
    max_quality_reworks: u32,
    role_bindings: &'a Value,
    resource_limits: &'a ResourceLimits,
}

pub struct FullCrewPlanRegistry;

impl FullCrewPlanRegistry {
    /// 构造 Full Crew v1 固定 DAG。运行中不允许替换该快照。
    pub fn snapshot_v1(
        include_character_critic: bool,
        role_bindings: Value,
        resource_limits: ResourceLimits,
    ) -> ProductionResult<PlanSnapshot> {
        if !role_bindings.is_object() || role_bindings.as_object().is_some_and(|map| map.is_empty())
        {
            return Err(domain_error(
                "role binding snapshot must be a non-empty object",
            ));
        }

        let mut steps = vec![
            PlanStep::system("validate_source", StepKind::DomainCommand, &[]),
            PlanStep::role("producer", &["validate_source"], false),
            PlanStep::system("brief_approval", StepKind::Gate, &["producer"]),
            PlanStep::role("screenwriter", &["brief_approval"], false),
        ];
        if include_character_critic {
            steps.push(PlanStep::role("character_critic", &["screenwriter"], true));
            steps.push(PlanStep::system(
                "character_suggestion_resolution",
                StepKind::DomainCommand,
                &["character_critic"],
            ));
        } else {
            steps.push(PlanStep::system(
                "character_suggestion_resolution",
                StepKind::DomainCommand,
                &["screenwriter"],
            ));
        }
        steps.extend([
            PlanStep::system(
                "script_package_approval",
                StepKind::Gate,
                &["character_suggestion_resolution"],
            ),
            PlanStep::system(
                "promote_script",
                StepKind::DomainCommand,
                &["script_package_approval"],
            ),
            PlanStep::role("director", &["promote_script"], false),
            PlanStep::role("cinematographer", &["director"], false),
            PlanStep::system(
                "suggestion_resolution",
                StepKind::DomainCommand,
                &["cinematographer"],
            ),
            PlanStep::system(
                "director_revision",
                StepKind::DomainCommand,
                &["suggestion_resolution"],
            ),
            PlanStep::role("performance_director", &["director_revision"], false),
            PlanStep::role("sound_director", &["director_revision"], false),
            PlanStep::system(
                "production_package_approval",
                StepKind::Gate,
                &["performance_director", "sound_director"],
            ),
            PlanStep::system(
                "wait_scene_visual_manifest",
                StepKind::ExternalWait,
                &["production_package_approval"],
            ),
            PlanStep::system(
                "create_work_plan",
                StepKind::DomainCommand,
                &["wait_scene_visual_manifest"],
            ),
            PlanStep::system(
                "work_plan_confirmation",
                StepKind::ExternalWait,
                &["create_work_plan"],
            ),
            PlanStep::system(
                "wait_work_generation",
                StepKind::ExternalWait,
                &["work_plan_confirmation"],
            ),
            PlanStep::role("editor", &["wait_work_generation"], false),
            PlanStep::role("qc", &["editor"], false),
            PlanStep::system("quality_gate", StepKind::Gate, &["qc"]),
        ]);

        let max_package_revisions = BTreeMap::from([
            ("brief".into(), 2),
            ("script".into(), 2),
            ("production".into(), 2),
        ]);
        let max_quality_reworks = resource_limits.max_quality_reworks as u32;
        let digest = canonical_digest(&DigestInput {
            plan_key: "full_crew",
            plan_version: "1.0.0",
            steps: &steps,
            include_character_critic,
            max_package_revisions: &max_package_revisions,
            max_quality_reworks,
            role_bindings: &role_bindings,
            resource_limits: &resource_limits,
        })?;

        Ok(PlanSnapshot {
            plan_key: "full_crew".into(),
            plan_version: "1.0.0".into(),
            digest,
            steps,
            include_character_critic,
            max_package_revisions,
            max_quality_reworks,
            role_bindings,
            resource_limits,
        })
    }
}
