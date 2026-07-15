use crate::application::asset_generation::{
    AssetCandidateView, AssetGenerationOptions, AssetGenerationPlan, SceneVisualManifest,
    SceneVisualManifestItem,
};
use crate::repositories::{
    AssetCandidateSource, AssetCandidateStatus, AssetCandidateType, AssetGenerationProvider,
    AssetGenerationTask, AssetGenerationTaskStatus, AssetGenerationTaskType, Material,
    SceneAssetCandidate,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AssetGenerationPlanRequest {
    pub model_id: Uuid,
    pub image_candidates_per_scene: i32,
    #[serde(default)]
    pub use_reference_materials: bool,
}

impl AssetGenerationPlanRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.model_id.is_nil() {
            return Err("图片模型不能为空".to_string());
        }
        if !(1..=4).contains(&self.image_candidates_per_scene) {
            return Err("每个分镜图片候选数量必须在 1 到 4 之间".to_string());
        }
        Ok(())
    }

    pub fn into_options(self) -> AssetGenerationOptions {
        AssetGenerationOptions {
            model_id: self.model_id,
            image_candidates_per_scene: self.image_candidates_per_scene,
            use_reference_materials: self.use_reference_materials,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct AssetGenerationTaskRequest {
    pub model_id: Uuid,
    pub image_candidates_per_scene: i32,
    #[serde(default)]
    pub use_reference_materials: bool,
}

impl AssetGenerationTaskRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        AssetGenerationPlanRequest {
            model_id: self.model_id,
            image_candidates_per_scene: self.image_candidates_per_scene,
            use_reference_materials: self.use_reference_materials,
        }
        .validate_for_api()
    }

    pub fn into_options(self) -> AssetGenerationOptions {
        AssetGenerationOptions {
            model_id: self.model_id,
            image_candidates_per_scene: self.image_candidates_per_scene,
            use_reference_materials: self.use_reference_materials,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct AssetGenerationPlanResponse {
    pub script_id: Uuid,
    pub scene_count: usize,
    pub image_candidate_count: i32,
    pub max_image_candidate_count: i32,
    pub model_id: Uuid,
    pub provider: String,
    pub reference_material_count: i32,
    pub can_create: bool,
    pub warnings: Vec<String>,
}

impl From<AssetGenerationPlan> for AssetGenerationPlanResponse {
    fn from(plan: AssetGenerationPlan) -> Self {
        Self {
            script_id: plan.script_id,
            scene_count: plan.scene_count,
            image_candidate_count: plan.image_candidate_count,
            max_image_candidate_count: plan.max_image_candidate_count,
            model_id: plan.model_id,
            provider: plan.provider.as_str().to_string(),
            reference_material_count: plan.reference_material_count,
            can_create: plan.can_create,
            warnings: plan.warnings,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct AssetGenerationTaskResponse {
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub script_id: Option<Uuid>,
    pub scene_id: Option<Uuid>,
    pub model_id: Option<Uuid>,
    pub model_snapshot: Option<Value>,
    pub provider: String,
    pub task_type: String,
    pub status: String,
    pub candidate_count: i32,
    pub reference_material_ids: Vec<Uuid>,
    pub params: Value,
    pub result: Value,
    pub error_message: Option<String>,
    pub retry_count: i32,
    pub dismissed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub read_only: bool,
}

impl From<AssetGenerationTask> for AssetGenerationTaskResponse {
    fn from(task: AssetGenerationTask) -> Self {
        Self {
            task_id: task.id,
            project_id: task.project_id,
            script_id: task.script_id,
            scene_id: task.scene_id,
            model_id: task.model_id,
            model_snapshot: task.model_snapshot,
            provider: task.provider.as_str().to_string(),
            task_type: task.task_type.as_str().to_string(),
            status: task.status.as_str().to_string(),
            candidate_count: task.candidate_count,
            reference_material_ids: task.reference_material_ids,
            params: task.params,
            result: task.result,
            error_message: task.error_message,
            retry_count: task.retry_count,
            dismissed_at: task.dismissed_at,
            created_at: task.created_at,
            updated_at: task.updated_at,
            read_only: task.task_type.is_legacy_read_only(),
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct AssetGenerationTaskListResponse {
    pub script_id: Uuid,
    pub tasks: Vec<AssetGenerationTaskResponse>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SceneAssetCandidateResponse {
    pub candidate_id: Uuid,
    pub project_id: Uuid,
    pub script_id: Uuid,
    pub scene_id: Uuid,
    pub material_id: Option<Uuid>,
    pub candidate_type: String,
    pub source: String,
    pub status: String,
    pub rank: i32,
    pub generation_task_id: Option<Uuid>,
    pub metadata: Value,
    pub file_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub file_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl SceneAssetCandidateResponse {
    pub fn from_candidate(candidate: SceneAssetCandidate, material: Option<Material>) -> Self {
        Self {
            candidate_id: candidate.id,
            project_id: candidate.project_id,
            script_id: candidate.script_id,
            scene_id: candidate.scene_id,
            material_id: candidate.material_id,
            candidate_type: candidate.candidate_type.as_str().to_string(),
            source: candidate.source.as_str().to_string(),
            status: candidate.status.as_str().to_string(),
            rank: candidate.rank,
            generation_task_id: candidate.generation_task_id,
            metadata: candidate.metadata,
            file_url: material.as_ref().map(|material| material.file_url.clone()),
            thumbnail_url: material
                .as_ref()
                .and_then(|material| material.thumbnail_url.clone()),
            file_name: material.map(|material| material.file_name),
            created_at: candidate.created_at,
            updated_at: candidate.updated_at,
        }
    }
}

impl From<AssetCandidateView> for SceneAssetCandidateResponse {
    fn from(view: AssetCandidateView) -> Self {
        Self::from_candidate(view.candidate, view.material)
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SceneAssetCandidateListResponse {
    pub candidates: Vec<SceneAssetCandidateResponse>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct SceneVisualManifestValidationRequest {
    pub expected_input_version: String,
}

impl SceneVisualManifestValidationRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        let value = self.expected_input_version.trim();
        if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("主画面输入版本摘要必须是 64 位十六进制字符串".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SceneVisualManifestItemResponse {
    pub scene_id: Uuid,
    pub sequence: i32,
    pub narration: String,
    pub visual_description: String,
    pub emotion: String,
    pub duration_sec: i32,
    pub candidate_id: Uuid,
    pub material_id: Uuid,
    pub file_url: String,
    pub thumbnail_url: Option<String>,
    pub source_snapshot: Value,
}

impl From<SceneVisualManifestItem> for SceneVisualManifestItemResponse {
    fn from(item: SceneVisualManifestItem) -> Self {
        Self {
            scene_id: item.scene_id,
            sequence: item.sequence,
            narration: item.narration,
            visual_description: item.visual_description,
            emotion: item.emotion,
            duration_sec: item.duration_sec,
            candidate_id: item.candidate_id,
            material_id: item.material_id,
            file_url: item.file_url,
            thumbnail_url: item.thumbnail_url,
            source_snapshot: item.source_snapshot,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SceneVisualManifestResponse {
    pub script_id: Uuid,
    pub script_title: String,
    pub script_updated_at: DateTime<Utc>,
    pub input_version: String,
    pub scenes: Vec<SceneVisualManifestItemResponse>,
}

impl From<SceneVisualManifest> for SceneVisualManifestResponse {
    fn from(manifest: SceneVisualManifest) -> Self {
        Self {
            script_id: manifest.script_id,
            script_title: manifest.script_title,
            script_updated_at: manifest.script_updated_at,
            input_version: manifest.input_version,
            scenes: manifest
                .scenes
                .into_iter()
                .map(SceneVisualManifestItemResponse::from)
                .collect(),
        }
    }
}

#[allow(dead_code)]
fn _asset_generation_response_type_guards(
    _provider: AssetGenerationProvider,
    _task_type: AssetGenerationTaskType,
    _task_status: AssetGenerationTaskStatus,
    _candidate_type: AssetCandidateType,
    _candidate_source: AssetCandidateSource,
    _candidate_status: AssetCandidateStatus,
) {
}
