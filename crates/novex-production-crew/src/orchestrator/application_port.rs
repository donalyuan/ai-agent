//! ProductionOrchestrator 与现有画面、作品 Application Service 的类型化边界。

use crate::{
    durable::{
        canonical_digest,
        command_store::{ProductionActor, ProductionCommandStore},
        production_input::ProductionPackageInput,
    },
    ProductionError, ProductionResult,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneVisualReference {
    pub scene_id: Uuid,
    pub scene_version: String,
    pub candidate_id: Uuid,
    pub material_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneVisualManifestReference {
    pub script_id: Uuid,
    pub script_version: String,
    pub manifest_version: String,
    pub scenes: Vec<SceneVisualReference>,
    pub manifest_digest: String,
}

impl SceneVisualManifestReference {
    pub fn build(
        script_id: Uuid,
        script_version: String,
        manifest_version: String,
        scenes: Vec<SceneVisualReference>,
    ) -> ProductionResult<Self> {
        if script_version.trim().is_empty()
            || manifest_version.trim().is_empty()
            || scenes.is_empty()
        {
            return Err(ProductionError::TransitionConflict {
                reason: "SceneVisualManifest reference is incomplete".into(),
            });
        }
        let mut scene_ids = BTreeSet::new();
        if scenes
            .iter()
            .any(|scene| scene.scene_version.trim().is_empty() || !scene_ids.insert(scene.scene_id))
        {
            return Err(ProductionError::TransitionConflict {
                reason: "SceneVisualManifest contains duplicate or unversioned Scene".into(),
            });
        }
        let manifest_digest = canonical_digest(&(
            script_id,
            script_version.as_str(),
            manifest_version.as_str(),
            scenes.as_slice(),
        ))?;
        Ok(Self {
            script_id,
            script_version,
            manifest_version,
            scenes,
            manifest_digest,
        })
    }

    pub fn validate_for(&self, input: &ProductionPackageInput) -> ProductionResult<()> {
        let rebuilt = Self::build(
            self.script_id,
            self.script_version.clone(),
            self.manifest_version.clone(),
            self.scenes.clone(),
        )?;
        let expected_scenes = input
            .scenes
            .iter()
            .map(|scene| (scene.scene_id, scene.scene_version.as_str()))
            .collect::<Vec<_>>();
        let actual_scenes = self
            .scenes
            .iter()
            .map(|scene| (scene.scene_id, scene.scene_version.as_str()))
            .collect::<Vec<_>>();
        if rebuilt.manifest_digest != self.manifest_digest
            || self.script_id != input.script.script_id
            || self.script_version != input.script.script_version
            || actual_scenes != expected_scenes
        {
            return Err(ProductionError::TransitionConflict {
                reason: "SceneVisualManifest is stale, incomplete, or cross-Script".into(),
            });
        }
        Ok(())
    }
}

#[async_trait]
pub trait SceneVisualManifestPort: Send + Sync {
    async fn prepare_scene_visual_manifest(
        &self,
        input: ProductionPackageInput,
    ) -> ProductionResult<SceneVisualManifestReference>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionWorkPlanRequest {
    pub production: ProductionPackageInput,
    pub manifest: SceneVisualManifestReference,
    pub operator_settings: ProductionWorkPlanSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionWorkPlanSettings {
    pub llm_model_id: Uuid,
    pub video_model_id: Uuid,
    pub tts_model_id: Option<Uuid>,
    pub tts_voice_type: Option<String>,
    pub duration_strategy: String,
    pub duration_seconds: Option<u32>,
    pub aspect_ratio: String,
    pub resolution: String,
    pub audio_mode: String,
    pub narration_override: Option<String>,
    pub audio_material_ids: Vec<Uuid>,
    pub burn_subtitles: bool,
    pub overrides: ProductionWorkPlanOverrides,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionWorkPlanOverrides {
    pub full_prompt: Option<String>,
    pub scene_prompts: Vec<ScenePromptOverride>,
    pub segment_prompts: Option<Vec<String>>,
    pub scene_durations: Vec<SceneDurationOverride>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenePromptOverride {
    pub scene_id: Uuid,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneDurationOverride {
    pub scene_id: Uuid,
    pub duration_sec: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkPlanReference {
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub work_version: u32,
    pub work_version_digest: String,
    pub work_plan_id: Uuid,
    pub plan_version: u32,
    pub input_fingerprint: String,
    pub plan_digest: String,
}

impl WorkPlanReference {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        work_id: Uuid,
        work_version_id: Uuid,
        work_version: u32,
        work_version_digest: String,
        work_plan_id: Uuid,
        plan_version: u32,
        input_fingerprint: String,
    ) -> ProductionResult<Self> {
        if work_version == 0
            || plan_version == 0
            || !valid_digest(&work_version_digest)
            || !valid_digest(&input_fingerprint)
        {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkPlan reference lacks a formal version or digest".into(),
            });
        }
        let plan_digest = canonical_digest(&(
            work_id,
            work_version_id,
            work_version,
            work_version_digest.as_str(),
            work_plan_id,
            plan_version,
            input_fingerprint.as_str(),
        ))?;
        Ok(Self {
            work_id,
            work_version_id,
            work_version,
            work_version_digest,
            work_plan_id,
            plan_version,
            input_fingerprint,
            plan_digest,
        })
    }

    pub fn validate(&self) -> ProductionResult<()> {
        let rebuilt = Self::build(
            self.work_id,
            self.work_version_id,
            self.work_version,
            self.work_version_digest.clone(),
            self.work_plan_id,
            self.plan_version,
            self.input_fingerprint.clone(),
        )?;
        if rebuilt.plan_digest != self.plan_digest {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkPlan reference digest is not canonical".into(),
            });
        }
        Ok(())
    }
}

#[async_trait]
pub trait WorkGenerationPlanningPort: Send + Sync {
    async fn create_work_plan(
        &self,
        input: ProductionWorkPlanRequest,
    ) -> ProductionResult<WorkPlanReference>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkGenerationRunStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    WaitingManual,
    Cancelling,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkGenerationRunReference {
    pub run_id: Uuid,
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub work_plan_id: Uuid,
    pub status: WorkGenerationRunStatus,
    pub error_category: Option<String>,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub retryable: bool,
    pub final_media_ready: bool,
    pub take_inventory_ready: bool,
    pub run_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkGenerationRunDisposition {
    ExternalWait,
    FailedBlocker,
    AttentionRequired,
    Cancelling,
    Cancelled,
    ExternalCancelConflict,
    EvidenceBlocker,
    ReadyForMediaReview,
}

impl WorkGenerationRunReference {
    pub fn build(
        run_id: Uuid,
        work_id: Uuid,
        work_version_id: Uuid,
        work_plan_id: Uuid,
        status: WorkGenerationRunStatus,
        error_category: Option<String>,
        error_code: Option<String>,
        error_summary: Option<String>,
        retryable: bool,
        final_media_ready: bool,
        take_inventory_ready: bool,
    ) -> ProductionResult<Self> {
        let run_digest = canonical_digest(&(
            run_id,
            work_id,
            work_version_id,
            work_plan_id,
            status,
            error_category.as_deref(),
            error_code.as_deref(),
            error_summary.as_deref(),
            retryable,
            final_media_ready,
            take_inventory_ready,
        ))?;
        Ok(Self {
            run_id,
            work_id,
            work_version_id,
            work_plan_id,
            status,
            error_category,
            error_code,
            error_summary,
            retryable,
            final_media_ready,
            take_inventory_ready,
            run_digest,
        })
    }

    pub fn validate_for(&self, plan: &WorkPlanReference) -> ProductionResult<()> {
        let rebuilt = Self::build(
            self.run_id,
            self.work_id,
            self.work_version_id,
            self.work_plan_id,
            self.status,
            self.error_category.clone(),
            self.error_code.clone(),
            self.error_summary.clone(),
            self.retryable,
            self.final_media_ready,
            self.take_inventory_ready,
        )?;
        if rebuilt.run_digest != self.run_digest
            || self.work_id != plan.work_id
            || self.work_version_id != plan.work_version_id
            || self.work_plan_id != plan.work_plan_id
        {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkGenerationRun reference differs from the confirmed WorkPlan".into(),
            });
        }
        Ok(())
    }

    pub fn disposition(&self, cancellation_requested: bool) -> WorkGenerationRunDisposition {
        match self.status {
            WorkGenerationRunStatus::Queued | WorkGenerationRunStatus::Running => {
                WorkGenerationRunDisposition::ExternalWait
            }
            WorkGenerationRunStatus::Failed => WorkGenerationRunDisposition::FailedBlocker,
            WorkGenerationRunStatus::WaitingManual => {
                WorkGenerationRunDisposition::AttentionRequired
            }
            WorkGenerationRunStatus::Cancelling => WorkGenerationRunDisposition::Cancelling,
            WorkGenerationRunStatus::Cancelled if cancellation_requested => {
                WorkGenerationRunDisposition::Cancelled
            }
            WorkGenerationRunStatus::Cancelled => {
                WorkGenerationRunDisposition::ExternalCancelConflict
            }
            WorkGenerationRunStatus::Succeeded
                if !self.final_media_ready || !self.take_inventory_ready =>
            {
                WorkGenerationRunDisposition::EvidenceBlocker
            }
            WorkGenerationRunStatus::Succeeded => WorkGenerationRunDisposition::ReadyForMediaReview,
        }
    }
}

/// 只观察既有人工确认结果和运行状态，不创建 provider 任务。
#[async_trait]
pub trait WorkGenerationRunPort: Send + Sync {
    async fn confirmed_run_for_plan(
        &self,
        plan: WorkPlanReference,
    ) -> ProductionResult<WorkGenerationRunReference>;

    async fn observe_run(&self, run_id: Uuid) -> ProductionResult<WorkGenerationRunReference>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkVersionReworkKind {
    Edit,
    FullRegeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkVersionReworkRequest {
    pub production_run_id: Uuid,
    pub revision_epoch: u32,
    pub work_id: Uuid,
    pub source_work_version_id: Uuid,
    pub inventory_id: Uuid,
    pub inventory_digest: String,
    pub evidence_snapshot_id: Uuid,
    pub evidence_digest: String,
    pub kind: WorkVersionReworkKind,
    pub rejected_take_ids: Vec<Uuid>,
    pub affected_shot_contract_ids: Vec<Uuid>,
    pub reason: String,
    pub actor: ProductionActor,
    pub idempotency_key: String,
}

impl WorkVersionReworkRequest {
    pub fn validate(&self) -> ProductionResult<()> {
        let take_ids: BTreeSet<_> = self.rejected_take_ids.iter().copied().collect();
        let shot_ids: BTreeSet<_> = self.affected_shot_contract_ids.iter().copied().collect();
        if self.reason.trim().is_empty()
            || self.actor.validate().is_err()
            || self.idempotency_key.trim().is_empty()
            || self.idempotency_key.len() > 200
            || self.revision_epoch > i32::MAX as u32
            || !valid_digest(&self.inventory_digest)
            || !valid_digest(&self.evidence_digest)
            || self.rejected_take_ids.is_empty()
            || self.affected_shot_contract_ids.is_empty()
            || take_ids.len() != self.rejected_take_ids.len()
            || shot_ids.len() != self.affected_shot_contract_ids.len()
        {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkVersion rework request is incomplete or ambiguous".into(),
            });
        }
        Ok(())
    }

    pub fn digest(&self) -> ProductionResult<String> {
        self.validate()?;
        ProductionCommandStore::canonical_request_digest(&json!({
            "production_run_id": self.production_run_id,
            "revision_epoch": self.revision_epoch,
            "work_id": self.work_id,
            "source_work_version_id": self.source_work_version_id,
            "inventory_id": self.inventory_id,
            "inventory_digest": self.inventory_digest,
            "evidence_snapshot_id": self.evidence_snapshot_id,
            "evidence_digest": self.evidence_digest,
            "kind": self.kind,
            "rejected_take_ids": self.rejected_take_ids,
            "affected_shot_contract_ids": self.affected_shot_contract_ids,
            "reason": self.reason,
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkVersionReworkReference {
    pub work_id: Uuid,
    pub source_work_version_id: Uuid,
    pub draft_work_version_id: Uuid,
    pub draft_version: u32,
    pub kind: WorkVersionReworkKind,
    pub work_plan_id: Uuid,
    pub work_plan_version: u32,
    pub work_plan_fingerprint: String,
    pub diff_plan_id: Uuid,
    pub diff_plan_version: u32,
    pub source_fingerprint: String,
    pub draft_fingerprint: String,
    pub affected_nodes: Vec<String>,
    pub reused_artifact_ids: Vec<Uuid>,
    pub resource_usage: Value,
    pub requires_confirmation: bool,
    pub reference_digest: String,
}

impl WorkVersionReworkReference {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        request: &WorkVersionReworkRequest,
        draft_work_version_id: Uuid,
        draft_version: u32,
        work_plan_id: Uuid,
        work_plan_version: u32,
        work_plan_fingerprint: String,
        diff_plan_id: Uuid,
        diff_plan_version: u32,
        source_fingerprint: String,
        draft_fingerprint: String,
        affected_nodes: Vec<String>,
        reused_artifact_ids: Vec<Uuid>,
        resource_usage: Value,
    ) -> ProductionResult<Self> {
        request.validate()?;
        let node_set: BTreeSet<_> = affected_nodes.iter().collect();
        let artifact_set: BTreeSet<_> = reused_artifact_ids.iter().copied().collect();
        if draft_version == 0
            || work_plan_version == 0
            || diff_plan_version == 0
            || !valid_digest(&work_plan_fingerprint)
            || !valid_digest(&source_fingerprint)
            || !valid_digest(&draft_fingerprint)
            || affected_nodes.is_empty()
            || node_set.len() != affected_nodes.len()
            || artifact_set.len() != reused_artifact_ids.len()
            || !resource_usage.is_object()
            || contains_forbidden_resource_data(&resource_usage)
            || (request.kind == WorkVersionReworkKind::FullRegeneration
                && !reused_artifact_ids.is_empty())
        {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkVersion rework reference is incomplete or unsafe".into(),
            });
        }
        let reference_digest = canonical_digest(&json!({
            "request_digest": request.digest()?,
            "work_id": request.work_id,
            "source_work_version_id": request.source_work_version_id,
            "draft_work_version_id": draft_work_version_id,
            "draft_version": draft_version,
            "kind": request.kind,
            "work_plan_id": work_plan_id,
            "work_plan_version": work_plan_version,
            "work_plan_fingerprint": work_plan_fingerprint,
            "diff_plan_id": diff_plan_id,
            "diff_plan_version": diff_plan_version,
            "source_fingerprint": source_fingerprint,
            "draft_fingerprint": draft_fingerprint,
            "affected_nodes": affected_nodes,
            "reused_artifact_ids": reused_artifact_ids,
            "resource_usage": resource_usage,
            "requires_confirmation": true,
        }))?;
        Ok(Self {
            work_id: request.work_id,
            source_work_version_id: request.source_work_version_id,
            draft_work_version_id,
            draft_version,
            kind: request.kind,
            work_plan_id,
            work_plan_version,
            work_plan_fingerprint,
            diff_plan_id,
            diff_plan_version,
            source_fingerprint,
            draft_fingerprint,
            affected_nodes,
            reused_artifact_ids,
            resource_usage,
            requires_confirmation: true,
            reference_digest,
        })
    }

    pub fn validate_for(&self, request: &WorkVersionReworkRequest) -> ProductionResult<()> {
        let rebuilt = Self::build(
            request,
            self.draft_work_version_id,
            self.draft_version,
            self.work_plan_id,
            self.work_plan_version,
            self.work_plan_fingerprint.clone(),
            self.diff_plan_id,
            self.diff_plan_version,
            self.source_fingerprint.clone(),
            self.draft_fingerprint.clone(),
            self.affected_nodes.clone(),
            self.reused_artifact_ids.clone(),
            self.resource_usage.clone(),
        )?;
        if rebuilt.reference_digest != self.reference_digest
            || self.work_id != request.work_id
            || self.source_work_version_id != request.source_work_version_id
            || self.kind != request.kind
            || !self.requires_confirmation
        {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkVersion rework reference differs from the governed request".into(),
            });
        }
        Ok(())
    }
}

#[async_trait]
pub trait WorkVersionReworkPort: Send + Sync {
    async fn create_rework_draft(
        &self,
        request: WorkVersionReworkRequest,
    ) -> ProductionResult<WorkVersionReworkReference>;
}

/// 仅在单次媒体读取调用期间存活；该类型故意不实现 Serialize/Deserialize。
pub struct TemporaryMediaAccess {
    pub asset_id: Uuid,
    pub access_url: String,
    pub request_headers: BTreeMap<String, String>,
}

impl fmt::Debug for TemporaryMediaAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TemporaryMediaAccess")
            .field("asset_id", &self.asset_id)
            .field("access_url", &"[REDACTED]")
            .field("request_headers", &"[REDACTED]")
            .finish()
    }
}

impl Drop for TemporaryMediaAccess {
    fn drop(&mut self) {
        self.access_url.clear();
        for value in self.request_headers.values_mut() {
            value.clear();
        }
        self.request_headers.clear();
    }
}

#[derive(Debug, Clone)]
pub struct MediaEvidenceAnalysis {
    pub vision_capability_version: String,
    pub audio_capability_version: String,
    pub redacted_analysis: serde_json::Value,
}

#[async_trait]
pub trait MediaEvidenceProvider: Send + Sync {
    async fn inspect_media(
        &self,
        inventory: crate::durable::media::RequiredTakeInventorySnapshot,
        access: TemporaryMediaAccess,
    ) -> ProductionResult<MediaEvidenceAnalysis>;
}

impl ProductionWorkPlanRequest {
    pub fn validate(&self) -> ProductionResult<()> {
        self.production.package_snapshot()?;
        self.manifest.validate_for(&self.production)?;
        let settings = &self.operator_settings;
        if settings.duration_strategy.trim().is_empty()
            || settings.aspect_ratio.trim().is_empty()
            || settings.resolution.trim().is_empty()
            || settings.audio_mode.trim().is_empty()
        {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkPlan operator settings are incomplete".into(),
            });
        }
        if settings
            .overrides
            .full_prompt
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
            || settings
                .overrides
                .segment_prompts
                .as_ref()
                .is_some_and(|items| {
                    items.is_empty() || items.iter().any(|item| item.trim().is_empty())
                })
        {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkPlan Prompt override must not be blank".into(),
            });
        }
        let scene_ids = self
            .production
            .scenes
            .iter()
            .map(|scene| scene.scene_id)
            .collect::<BTreeSet<_>>();
        let mut prompt_ids = BTreeSet::new();
        if settings.overrides.scene_prompts.iter().any(|item| {
            item.prompt.trim().is_empty()
                || !scene_ids.contains(&item.scene_id)
                || !prompt_ids.insert(item.scene_id)
        }) {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkPlan scene Prompt override is blank, duplicate, or cross-Script"
                    .into(),
            });
        }
        let mut duration_ids = BTreeSet::new();
        if settings.overrides.scene_durations.iter().any(|item| {
            item.duration_sec == 0
                || !scene_ids.contains(&item.scene_id)
                || !duration_ids.insert(item.scene_id)
        }) {
            return Err(ProductionError::TransitionConflict {
                reason: "WorkPlan Scene duration override is invalid, duplicate, or cross-Script"
                    .into(),
            });
        }
        Ok(())
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn contains_forbidden_resource_data(value: &Value) -> bool {
    let encoded = value.to_string().to_ascii_lowercase();
    [
        "price",
        "cost",
        "currency",
        "amount",
        "api_key",
        "authorization",
    ]
    .iter()
    .any(|field| encoded.contains(field))
}
