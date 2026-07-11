use super::{
    ContentTopic, ContentTopicSource, ContentTopicStatus, Scene, Script, ScriptStatus,
    ScriptSummary, TopicGenerationBatchStatus, TopicGenerationBatchSummary,
    TopicGroupReviewFreshness, TopicGroupScriptPriority, TopicGroupSort, TopicGroupSummary,
    TopicQualityEvaluation, TopicQualityEvaluationStatus, TopicQualityGateResult,
    TopicReviewResult, TopicReviewSnapshot, TopicReviewSnapshotStatus,
};
use crate::agents::conversation::{
    AgentConversation, AgentConversationStatus, AgentMessage, AgentMessageRole, AgentRunRecord,
};
use crate::repositories::WorkspaceMenuTreeNode;
use crate::repositories::{
    AccountStrategyProfile, AssetCandidateSource, AssetCandidateStatus, AssetCandidateType,
    AssetGenerationProvider, AssetGenerationTask, AssetGenerationTaskStatus,
    AssetGenerationTaskType, CreateMaterialInput, Material, MaterialListFilter, MaterialStatus,
    MaterialStatusFilter, MaterialType, Project, SceneAssetCandidate, UpdateMaterialInput,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use validator::Validate;

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct CreateProjectRequest {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[serde(default)]
    #[validate(length(max = 500))]
    pub positioning: String,
    #[serde(default)]
    #[validate(length(max = 2000))]
    pub description: String,
    #[serde(default)]
    pub strategy_profile: Option<AccountStrategyProfileRequest>,
}

impl CreateProjectRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("项目名称不能为空".to_string());
        }

        self.validate()
            .map_err(|error| format!("项目参数无效: {error}"))?;
        if let Some(strategy_profile) = &self.strategy_profile {
            strategy_profile.normalize()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UpdateProjectStrategyProfileRequest {
    pub name: String,
    #[serde(default)]
    pub positioning: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub strategy_profile: AccountStrategyProfileRequest,
}

impl UpdateProjectStrategyProfileRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("账号名称不能为空".to_string());
        }
        if self.name.trim().chars().count() > 120 {
            return Err("账号名称不能超过 120 个字符".to_string());
        }
        if self.positioning.trim().chars().count() > 500 {
            return Err("定位摘要不能超过 500 个字符".to_string());
        }
        if self.description.trim().chars().count() > 2_000 {
            return Err("账号描述不能超过 2000 个字符".to_string());
        }
        self.strategy_profile.normalize()?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct StrategyProfileDraftRequest {
    pub model_id: Uuid,
    #[serde(default)]
    pub direction_notes: String,
}

impl StrategyProfileDraftRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.model_id.is_nil() {
            return Err("必须选择文本模型".to_string());
        }
        if self.direction_notes.trim().chars().count() > ACCOUNT_STRATEGY_TEXT_LIMIT {
            return Err("补充方向不能超过 1000 个字符".to_string());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StrategyProfileDraftResponse {
    pub draft: AccountStrategyProfile,
    pub draft_summary: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct AccountStrategyProfileRequest {
    #[serde(default)]
    pub target_audience: String,
    #[serde(default)]
    pub content_pillars: Vec<String>,
    #[serde(default)]
    pub tone_style: String,
    #[serde(default)]
    pub forbidden_topics: Vec<String>,
    #[serde(default)]
    pub reference_accounts: Vec<String>,
    #[serde(default)]
    pub topic_preferences: String,
}

impl AccountStrategyProfileRequest {
    pub fn normalize(&self) -> Result<AccountStrategyProfile, String> {
        Ok(AccountStrategyProfile {
            target_audience: normalize_text_field(
                "目标受众",
                &self.target_audience,
                ACCOUNT_STRATEGY_TEXT_LIMIT,
            )?,
            content_pillars: normalize_string_list(
                "内容支柱",
                &self.content_pillars,
                ACCOUNT_STRATEGY_LIST_LIMIT,
                ACCOUNT_STRATEGY_LIST_ITEM_LIMIT,
            )?,
            tone_style: normalize_text_field(
                "表达风格",
                &self.tone_style,
                ACCOUNT_STRATEGY_TEXT_LIMIT,
            )?,
            forbidden_topics: normalize_string_list(
                "禁区方向",
                &self.forbidden_topics,
                ACCOUNT_STRATEGY_LIST_LIMIT,
                ACCOUNT_STRATEGY_LIST_ITEM_LIMIT,
            )?,
            reference_accounts: normalize_string_list(
                "参考账号",
                &self.reference_accounts,
                ACCOUNT_STRATEGY_LIST_LIMIT,
                ACCOUNT_STRATEGY_LIST_ITEM_LIMIT,
            )?,
            topic_preferences: normalize_text_field(
                "选题偏好",
                &self.topic_preferences,
                ACCOUNT_STRATEGY_TEXT_LIMIT,
            )?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectResponse {
    pub project_id: Uuid,
    pub name: String,
    pub positioning: String,
    pub description: String,
    pub strategy_profile: AccountStrategyProfile,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Project> for ProjectResponse {
    fn from(project: Project) -> Self {
        Self {
            project_id: project.id,
            name: project.name,
            positioning: project.positioning,
            description: project.description,
            strategy_profile: project.strategy_profile,
            status: project.status,
            created_at: project.created_at,
            updated_at: project.updated_at,
        }
    }
}

const ACCOUNT_STRATEGY_TEXT_LIMIT: usize = 1_000;
const ACCOUNT_STRATEGY_LIST_LIMIT: usize = 20;
const ACCOUNT_STRATEGY_LIST_ITEM_LIMIT: usize = 120;

fn normalize_text_field(label: &str, value: &str, max_chars: usize) -> Result<String, String> {
    let normalized = value.trim().to_string();
    if normalized.chars().count() > max_chars {
        return Err(format!("{label}不能超过 {max_chars} 个字符"));
    }
    Ok(normalized)
}

fn normalize_string_list(
    label: &str,
    values: &[String],
    max_items: usize,
    max_item_chars: usize,
) -> Result<Vec<String>, String> {
    if values.len() > max_items {
        return Err(format!("{label}最多填写 {max_items} 项"));
    }

    let mut normalized = Vec::new();
    for value in values {
        let item = value.trim();
        if item.is_empty() {
            continue;
        }
        if item.chars().count() > max_item_chars {
            return Err(format!("{label}单项不能超过 {max_item_chars} 个字符"));
        }
        if !normalized.iter().any(|existing| existing == item) {
            normalized.push(item.to_string());
        }
    }
    Ok(normalized)
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ProjectListResponse {
    pub projects: Vec<ProjectResponse>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MaterialPayloadRequest {
    pub material_type: String,
    pub file_url: String,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    pub file_name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: Value,
}

impl MaterialPayloadRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        self.normalized_parts().map(|_| ())
    }

    pub fn into_create_input(self, project_id: Uuid) -> Result<CreateMaterialInput, String> {
        let parts = self.normalized_parts()?;
        Ok(CreateMaterialInput {
            project_id,
            material_type: parts.material_type,
            file_url: parts.file_url,
            file_name: parts.file_name,
            thumbnail_url: parts.thumbnail_url,
            tags: parts.tags,
            metadata: parts.metadata,
        })
    }

    pub fn into_update_input(self, project_id: Uuid) -> Result<UpdateMaterialInput, String> {
        let parts = self.normalized_parts()?;
        Ok(UpdateMaterialInput {
            project_id,
            material_type: parts.material_type,
            file_url: parts.file_url,
            file_name: parts.file_name,
            thumbnail_url: parts.thumbnail_url,
            tags: parts.tags,
            metadata: parts.metadata,
        })
    }

    fn normalized_parts(&self) -> Result<NormalizedMaterialPayload, String> {
        let file_name = self.file_name.trim().to_string();
        if file_name.is_empty() {
            return Err("素材名称不能为空".to_string());
        }
        if file_name.chars().count() > 255 {
            return Err("素材名称不能超过 255 个字符".to_string());
        }

        let material_type = MaterialType::try_from(self.material_type.trim())
            .map_err(|_| "素材类型无效".to_string())?;
        let file_url = normalize_http_url("素材 URL", &self.file_url)?;
        let thumbnail_url = self
            .thumbnail_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| normalize_http_url("缩略图 URL", value))
            .transpose()?;
        let tags = normalize_material_tags(&self.tags)?;
        if !self.metadata.is_object() {
            return Err("素材 metadata 必须是 JSON 对象".to_string());
        }

        Ok(NormalizedMaterialPayload {
            material_type,
            file_url,
            thumbnail_url,
            file_name,
            tags,
            metadata: self.metadata.clone(),
        })
    }
}

struct NormalizedMaterialPayload {
    material_type: MaterialType,
    file_url: String,
    thumbnail_url: Option<String>,
    file_name: String,
    tags: Vec<String>,
    metadata: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MaterialStatusRequest {
    pub status: String,
}

impl MaterialStatusRequest {
    pub fn parse_status(&self) -> Result<MaterialStatus, String> {
        MaterialStatus::try_from(self.status.trim()).map_err(|_| "素材状态无效".to_string())
    }
}

#[derive(Clone, Debug, Deserialize, Default, PartialEq)]
pub struct MaterialListQuery {
    #[serde(rename = "type")]
    pub material_type: Option<String>,
    pub status: Option<String>,
    pub q: Option<String>,
    pub tag: Option<String>,
}

impl MaterialListQuery {
    pub fn into_filter(self) -> Result<MaterialListFilter, String> {
        let material_type = self
            .material_type
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "all")
            .map(MaterialType::try_from)
            .transpose()
            .map_err(|_| "素材类型筛选无效".to_string())?;
        let status = self
            .status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(MaterialStatusFilter::try_from)
            .transpose()
            .map_err(|_| "素材状态筛选无效".to_string())?
            .unwrap_or_default();

        Ok(MaterialListFilter {
            material_type,
            status,
            q: self.q,
            tag: self.tag,
        })
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MaterialResponse {
    pub material_id: Uuid,
    pub project_id: Uuid,
    pub material_type: String,
    pub file_url: String,
    pub thumbnail_url: Option<String>,
    pub file_name: String,
    pub tags: Vec<String>,
    pub metadata: Value,
    pub usage_count: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Material> for MaterialResponse {
    fn from(material: Material) -> Self {
        Self {
            material_id: material.id,
            project_id: material.project_id,
            material_type: material.material_type.as_str().to_string(),
            file_url: material.file_url,
            thumbnail_url: material.thumbnail_url,
            file_name: material.file_name,
            tags: material.tags,
            metadata: material.metadata,
            usage_count: material.usage_count,
            status: material.status.as_str().to_string(),
            created_at: material.created_at,
            updated_at: material.updated_at,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MaterialListResponse {
    pub materials: Vec<MaterialResponse>,
}

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
    pub video_task_count: i32,
    pub can_create: bool,
    pub warnings: Vec<String>,
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

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct SceneAssetCandidateListResponse {
    pub candidates: Vec<SceneAssetCandidateResponse>,
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

fn normalize_http_url(label: &str, value: &str) -> Result<String, String> {
    let normalized = value.trim().to_string();
    let uri = normalized
        .parse::<axum::http::Uri>()
        .map_err(|_| format!("{label}必须是 http 或 https URL"))?;
    let scheme = uri.scheme_str().unwrap_or_default();
    if !matches!(scheme, "http" | "https") || uri.host().is_none() {
        return Err(format!("{label}必须是 http 或 https URL"));
    }
    Ok(normalized)
}

fn normalize_material_tags(values: &[String]) -> Result<Vec<String>, String> {
    if values.len() > 30 {
        return Err("素材标签最多填写 30 个".to_string());
    }

    let mut normalized = Vec::new();
    for value in values {
        let item = value.trim();
        if item.is_empty() {
            continue;
        }
        if item.chars().count() > 40 {
            return Err("素材标签单项不能超过 40 个字符".to_string());
        }
        if !normalized.iter().any(|existing| existing == item) {
            normalized.push(item.to_string());
        }
    }
    Ok(normalized)
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CreateContentTopicRequest {
    pub title: String,
    #[serde(default)]
    pub angle: String,
    #[serde(default)]
    pub target_audience: String,
    #[serde(default)]
    pub hook_points: Vec<String>,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub score_reason: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl CreateContentTopicRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        validate_topic_payload(TopicPayloadValidation {
            title: &self.title,
            angle: &self.angle,
            target_audience: &self.target_audience,
            hook_points: &self.hook_points,
            content_type: &self.content_type,
            score: self.score,
            score_reason: &self.score_reason,
            tags: &self.tags,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UpdateContentTopicRequest {
    pub title: String,
    #[serde(default)]
    pub angle: String,
    #[serde(default)]
    pub target_audience: String,
    #[serde(default)]
    pub hook_points: Vec<String>,
    #[serde(default)]
    pub content_type: String,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub score_reason: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl UpdateContentTopicRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        validate_topic_payload(TopicPayloadValidation {
            title: &self.title,
            angle: &self.angle,
            target_audience: &self.target_audience,
            hook_points: &self.hook_points,
            content_type: &self.content_type,
            score: self.score,
            score_reason: &self.score_reason,
            tags: &self.tags,
        })
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UpdateContentTopicStatusRequest {
    pub status: ContentTopicStatus,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct PrepareScriptFromTopicRequest {
    #[serde(default)]
    pub style: Option<ScriptStyle>,
    #[serde(default)]
    #[validate(range(min = 3, max = 12))]
    pub scene_count: Option<u8>,
}

impl PrepareScriptFromTopicRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        self.validate()
            .map_err(|error| format!("脚本确认参数无效: {error}"))
    }

    pub fn style_or_default(&self) -> ScriptStyle {
        self.style.clone().unwrap_or_default()
    }

    pub fn scene_count_or_default(&self) -> u8 {
        self.scene_count.unwrap_or(6)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ContentTopicResponse {
    pub topic_id: Uuid,
    pub project_id: Uuid,
    pub batch_id: Option<Uuid>,
    pub title: String,
    pub angle: String,
    pub target_audience: String,
    pub hook_points: Vec<String>,
    pub content_type: String,
    pub score: Option<f64>,
    pub score_reason: String,
    pub tags: Vec<String>,
    pub source: ContentTopicSource,
    pub status: ContentTopicStatus,
    pub metadata: Value,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ContentTopic> for ContentTopicResponse {
    fn from(topic: ContentTopic) -> Self {
        Self {
            topic_id: topic.id,
            project_id: topic.project_id,
            batch_id: topic.batch_id,
            title: topic.title,
            angle: topic.angle,
            target_audience: topic.target_audience,
            hook_points: topic.hook_points,
            content_type: topic.content_type,
            score: topic.score,
            score_reason: topic.score_reason,
            tags: topic.tags,
            source: topic.source,
            status: topic.status,
            metadata: topic.metadata,
            deleted_at: topic.deleted_at,
            created_at: topic.created_at,
            updated_at: topic.updated_at,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ContentTopicStatsResponse {
    pub total: i64,
    pub idea: i64,
    pub approved: i64,
    pub scripted: i64,
    pub archived: i64,
}

impl ContentTopicStatsResponse {
    pub fn from_counts(counts: Vec<(ContentTopicStatus, i64)>) -> Self {
        let mut stats = Self::default();
        for (status, count) in counts {
            stats.total += count;
            match status {
                ContentTopicStatus::Idea => stats.idea = count,
                ContentTopicStatus::Approved => stats.approved = count,
                ContentTopicStatus::Scripted => stats.scripted = count,
                ContentTopicStatus::Archived => stats.archived = count,
            }
        }
        stats
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ContentTopicListResponse {
    pub topics: Vec<ContentTopicResponse>,
    pub stats: ContentTopicStatsResponse,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicGenerationBatchSummaryResponse {
    pub batch_id: Uuid,
    pub project_id: Uuid,
    pub supplement_of_batch_id: Option<Uuid>,
    pub prompt: String,
    pub requested_count: i32,
    pub topic_count: i64,
    pub status: TopicGenerationBatchStatus,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TopicGenerationBatchSummary> for TopicGenerationBatchSummaryResponse {
    fn from(summary: TopicGenerationBatchSummary) -> Self {
        Self {
            batch_id: summary.batch.id,
            project_id: summary.batch.project_id,
            supplement_of_batch_id: summary.batch.supplement_of_batch_id,
            prompt: summary.batch.prompt,
            requested_count: summary.batch.requested_count,
            topic_count: summary.topic_count,
            status: summary.batch.status,
            error_message: summary.batch.error_message,
            created_at: summary.batch.created_at,
            updated_at: summary.batch.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicGenerationBatchListResponse {
    pub batches: Vec<TopicGenerationBatchSummaryResponse>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct TopicGroupListQuery {
    #[serde(default)]
    pub sort: TopicGroupSort,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicGroupSummaryResponse {
    pub root_batch_id: Uuid,
    pub project_id: Uuid,
    pub prompt: String,
    pub created_at: DateTime<Utc>,
    pub topic_count: i64,
    pub supplement_batch_count: i64,
    pub latest_review_snapshot_id: Option<Uuid>,
    pub review_freshness: TopicGroupReviewFreshness,
    pub script_priority: TopicGroupScriptPriority,
}

impl From<TopicGroupSummary> for TopicGroupSummaryResponse {
    fn from(summary: TopicGroupSummary) -> Self {
        Self {
            root_batch_id: summary.root_batch_id,
            project_id: summary.project_id,
            prompt: summary.prompt,
            created_at: summary.created_at,
            topic_count: summary.topic_count,
            supplement_batch_count: summary.supplement_batch_count,
            latest_review_snapshot_id: summary.latest_review_snapshot_id,
            review_freshness: summary.review_freshness,
            script_priority: summary.script_priority,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicGroupListResponse {
    pub topic_groups: Vec<TopicGroupSummaryResponse>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicReviewSnapshotResponse {
    pub snapshot_id: Uuid,
    pub project_id: Uuid,
    pub root_batch_id: Uuid,
    pub source_run_id: Option<Uuid>,
    pub status: TopicReviewSnapshotStatus,
    pub review_summary: String,
    pub result: TopicReviewResult,
    pub error_message: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TopicReviewSnapshot> for TopicReviewSnapshotResponse {
    fn from(snapshot: TopicReviewSnapshot) -> Self {
        Self {
            snapshot_id: snapshot.id,
            project_id: snapshot.project_id,
            root_batch_id: snapshot.root_batch_id,
            source_run_id: snapshot.source_run_id,
            status: snapshot.status,
            review_summary: snapshot.review_summary,
            result: snapshot.result,
            error_message: snapshot.error_message,
            metadata: snapshot.metadata,
            created_at: snapshot.created_at,
            updated_at: snapshot.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicQualityEvaluationResponse {
    pub evaluation_id: Uuid,
    pub project_id: Uuid,
    pub batch_id: Uuid,
    pub source_run_id: Option<Uuid>,
    pub status: TopicQualityEvaluationStatus,
    pub pass_count: i32,
    pub reject_count: i32,
    pub rewrite_triggered: bool,
    pub result: TopicQualityGateResult,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TopicQualityEvaluation> for TopicQualityEvaluationResponse {
    fn from(evaluation: TopicQualityEvaluation) -> Self {
        Self {
            evaluation_id: evaluation.id,
            project_id: evaluation.project_id,
            batch_id: evaluation.batch_id,
            source_run_id: evaluation.source_run_id,
            status: evaluation.status,
            pass_count: evaluation.pass_count,
            reject_count: evaluation.reject_count,
            rewrite_triggered: evaluation.rewrite_triggered,
            result: evaluation.result,
            error_message: evaluation.error_message,
            created_at: evaluation.created_at,
            updated_at: evaluation.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TopicScriptRequestPreview {
    pub project_id: Uuid,
    pub topic_id: Uuid,
    pub topic: String,
    pub style: ScriptStyle,
    pub scene_count: u8,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PrepareScriptFromTopicResponse {
    pub topic: ContentTopicResponse,
    pub topic_snapshot: Value,
    pub script_request: TopicScriptRequestPreview,
}

struct TopicPayloadValidation<'a> {
    title: &'a str,
    angle: &'a str,
    target_audience: &'a str,
    hook_points: &'a [String],
    content_type: &'a str,
    score: Option<f64>,
    score_reason: &'a str,
    tags: &'a [String],
}

fn validate_topic_payload(payload: TopicPayloadValidation<'_>) -> Result<(), String> {
    if payload.title.trim().is_empty() {
        return Err("选题标题不能为空".to_string());
    }
    if payload.title.chars().count() > 160 {
        return Err("选题标题不能超过160个字符".to_string());
    }
    if payload.angle.chars().count() > 1000 {
        return Err("选题角度不能超过1000个字符".to_string());
    }
    if payload.target_audience.chars().count() > 500 {
        return Err("目标受众不能超过500个字符".to_string());
    }
    if payload.content_type.chars().count() > 80 {
        return Err("内容类型不能超过80个字符".to_string());
    }
    if let Some(score) = payload.score {
        if !(0.0..=100.0).contains(&score) {
            return Err("选题评分必须在0到100之间".to_string());
        }
    }
    if payload.score_reason.chars().count() > 1000 {
        return Err("评分理由不能超过1000个字符".to_string());
    }
    if payload.hook_points.len() > 10 {
        return Err("选题看点不能超过10个".to_string());
    }
    if payload.tags.len() > 20 {
        return Err("选题标签不能超过20个".to_string());
    }
    if payload
        .hook_points
        .iter()
        .any(|value| value.trim().is_empty())
    {
        return Err("选题看点不能为空".to_string());
    }
    if payload.tags.iter().any(|value| value.trim().is_empty()) {
        return Err("选题标签不能为空".to_string());
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkspaceMenuListResponse {
    pub menus: Vec<WorkspaceMenuNodeResponse>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct WorkspaceMenuNodeResponse {
    pub menu_id: Uuid,
    pub menu_key: String,
    pub label: String,
    pub description: String,
    pub route_path: Option<String>,
    pub icon: String,
    pub menu_type: String,
    pub module_key: Option<String>,
    pub agent_key: Option<String>,
    pub sort_order: i32,
    pub is_enabled: bool,
    pub status: String,
    pub metadata: Value,
    pub children: Vec<WorkspaceMenuNodeResponse>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct CreateAgentConversationRequest {
    pub agent_type: String,
    #[serde(default)]
    pub project_id: Option<Uuid>,
    #[serde(default)]
    pub subject_type: Option<String>,
    #[serde(default)]
    pub subject_id: Option<Uuid>,
    #[validate(length(min = 1, max = 160))]
    pub title: String,
    #[serde(default)]
    pub metadata: Value,
}

impl CreateAgentConversationRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.title.trim().is_empty() {
            return Err("会话标题不能为空".to_string());
        }
        let agent_type = self.agent_type.trim();
        if !matches!(agent_type, "script" | "topic") {
            return Err("暂不支持该 Agent 类型".to_string());
        }
        if self.project_id.is_none() {
            return Err("Agent 会话必须绑定项目".to_string());
        }
        let subject_type = self.subject_type.as_deref().map(str::trim);
        if agent_type == "topic" && (self.subject_id.is_some() || subject_type.is_some()) {
            return Err("选题会话暂不绑定 subject".to_string());
        }
        if agent_type == "script" && self.subject_id.is_some() && subject_type != Some("script") {
            return Err("脚本会话 subject_type 必须为 script".to_string());
        }
        if agent_type == "script"
            && self.subject_id.is_none()
            && subject_type.is_some_and(|value| !value.is_empty())
        {
            return Err("未绑定脚本会话不能传 subject_type".to_string());
        }
        self.validate()
            .map_err(|error| format!("会话参数无效: {error}"))
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct SendAgentMessageRequest {
    pub model_id: Uuid,
    #[validate(length(min = 1, max = 4000))]
    pub content: String,
    #[serde(default)]
    pub supplement_of_batch_id: Option<Uuid>,
}

impl SendAgentMessageRequest {
    pub fn validate_for_api(&self) -> Result<(), String> {
        if self.model_id.is_nil() {
            return Err("必须选择文本模型".to_string());
        }
        if self.content.trim().is_empty() {
            return Err("消息不能为空".to_string());
        }
        self.validate()
            .map_err(|error| format!("消息参数无效: {error}"))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentConversationResponse {
    pub conversation_id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_type: String,
    pub subject_type: Option<String>,
    pub subject_id: Option<Uuid>,
    pub title: String,
    pub status: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<AgentConversation> for AgentConversationResponse {
    fn from(conversation: AgentConversation) -> Self {
        Self {
            conversation_id: conversation.id,
            project_id: conversation.project_id,
            agent_type: conversation.agent_type,
            subject_type: conversation.subject_type,
            subject_id: conversation.subject_id,
            title: conversation.title,
            status: conversation_status_value(&conversation.status).to_string(),
            metadata: conversation.metadata,
            created_at: conversation.created_at,
            updated_at: conversation.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentMessageResponse {
    pub message_id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
}

impl From<AgentMessage> for AgentMessageResponse {
    fn from(message: AgentMessage) -> Self {
        Self {
            message_id: message.id,
            conversation_id: message.conversation_id,
            role: message_role_value(&message.role).to_string(),
            content: message.content,
            metadata: message.metadata,
            created_at: message.created_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentRunResponse {
    pub run_id: Uuid,
    pub project_id: Option<Uuid>,
    pub agent_type: String,
    pub status: String,
    pub input: Value,
    pub output: Option<Value>,
    pub error_message: Option<String>,
    pub model_id: Option<Uuid>,
    pub model_snapshot: Option<Value>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

impl From<AgentRunRecord> for AgentRunResponse {
    fn from(run: AgentRunRecord) -> Self {
        Self {
            run_id: run.id,
            project_id: run.project_id,
            agent_type: run.agent_type,
            status: run.status,
            input: run.input,
            output: run.output,
            error_message: run.error_message,
            model_id: run.model_id,
            model_snapshot: run.model_snapshot,
            started_at: run.started_at,
            ended_at: run.ended_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentMessageListResponse {
    pub messages: Vec<AgentMessageResponse>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AgentTurnResponseBody {
    pub user_message: AgentMessageResponse,
    pub assistant_message: AgentMessageResponse,
    pub run: AgentRunResponse,
}

fn conversation_status_value(status: &AgentConversationStatus) -> &'static str {
    match status {
        AgentConversationStatus::Active => "active",
        AgentConversationStatus::Archived => "archived",
    }
}

fn message_role_value(role: &AgentMessageRole) -> &'static str {
    match role {
        AgentMessageRole::System => "system",
        AgentMessageRole::User => "user",
        AgentMessageRole::Assistant => "assistant",
        AgentMessageRole::Tool => "tool",
    }
}

impl From<WorkspaceMenuTreeNode> for WorkspaceMenuNodeResponse {
    fn from(node: WorkspaceMenuTreeNode) -> Self {
        Self {
            menu_id: node.menu.id,
            menu_key: node.menu.menu_key,
            label: node.menu.label,
            description: node.menu.description,
            route_path: node.menu.route_path,
            icon: node.menu.icon,
            menu_type: node.menu.menu_type,
            module_key: node.menu.module_key,
            agent_key: node.menu.agent_key,
            sort_order: node.menu.sort_order,
            is_enabled: node.menu.is_enabled,
            status: node.menu.status,
            metadata: node.menu.metadata,
            children: node
                .children
                .into_iter()
                .map(WorkspaceMenuNodeResponse::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptStyle {
    #[default]
    Knowledge,
    Story,
    Tutorial,
}

impl ScriptStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Knowledge => "knowledge",
            Self::Story => "story",
            Self::Tutorial => "tutorial",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Knowledge => "知识科普类",
            Self::Story => "故事叙述类",
            Self::Tutorial => "教程讲解类",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct GenerateScriptRequest {
    pub model_id: Uuid,
    pub project_id: Uuid,
    #[serde(default)]
    #[validate(length(max = 200))]
    pub topic: String,
    #[serde(default)]
    pub topic_id: Option<Uuid>,
    #[serde(default)]
    pub style: Option<ScriptStyle>,
    #[serde(default)]
    #[validate(range(min = 3, max = 12))]
    pub scene_count: Option<u8>,
    #[serde(default)]
    pub parent_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ModelSelectionRequest {
    pub model_id: Uuid,
}

impl GenerateScriptRequest {
    pub fn style_or_default(&self) -> ScriptStyle {
        self.style.clone().unwrap_or_default()
    }

    pub fn scene_count_or_default(&self) -> u8 {
        self.scene_count.unwrap_or(6)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Validate)]
pub struct ScriptListFilter {
    #[serde(default)]
    pub status: Option<ScriptStatus>,
    #[serde(default)]
    #[validate(range(min = 1, max = 100))]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

impl ScriptListFilter {
    pub fn limit_or_default(&self) -> u32 {
        self.limit.unwrap_or(20)
    }

    pub fn offset_or_default(&self) -> u32 {
        self.offset.unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScriptResponse {
    pub script_id: Uuid,
    pub project_id: Uuid,
    pub topic_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_snapshot: Option<Value>,
    pub title: String,
    pub hook: String,
    pub scenes: Vec<SceneResponse>,
    pub status: ScriptStatus,
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Script> for ScriptResponse {
    fn from(script: Script) -> Self {
        let topic_snapshot = script.content.get("topic_snapshot").cloned();
        Self {
            script_id: script.id,
            project_id: script.project_id,
            topic_id: script.topic_id,
            topic_snapshot,
            title: script.title,
            hook: script.hook,
            scenes: script.scenes.into_iter().map(SceneResponse::from).collect(),
            status: script.status,
            parent_id: script.parent_id,
            created_at: script.created_at,
            updated_at: script.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScriptSummaryResponse {
    pub script_id: Uuid,
    pub topic_id: Option<Uuid>,
    pub source_topic_title: Option<String>,
    pub title: String,
    pub status: ScriptStatus,
    pub scene_count: usize,
    pub parent_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl From<Script> for ScriptSummaryResponse {
    fn from(script: Script) -> Self {
        let source_topic_title = script_source_topic_title(&script.content);
        Self {
            script_id: script.id,
            topic_id: script.topic_id,
            source_topic_title,
            title: script.title,
            status: script.status,
            scene_count: script.scenes.len(),
            parent_id: script.parent_id,
            created_at: script.created_at,
        }
    }
}

impl From<ScriptSummary> for ScriptSummaryResponse {
    fn from(summary: ScriptSummary) -> Self {
        Self {
            script_id: summary.script_id,
            topic_id: summary.topic_id,
            source_topic_title: summary.source_topic_title,
            title: summary.title,
            status: summary.status,
            scene_count: usize::try_from(summary.scene_count).unwrap_or(usize::MAX),
            parent_id: summary.parent_id,
            created_at: summary.created_at,
        }
    }
}

fn script_source_topic_title(content: &Value) -> Option<String> {
    content
        .get("topic_snapshot")
        .and_then(|snapshot| snapshot.get("title"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToString::to_string)
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ScriptListResponse {
    pub scripts: Vec<ScriptSummaryResponse>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct UpdateScriptStatusRequest {
    pub status: ScriptStatus,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct UpdateScriptStatusResponse {
    pub script_id: Uuid,
    pub status: ScriptStatus,
    pub updated_at: DateTime<Utc>,
}

impl From<Script> for UpdateScriptStatusResponse {
    fn from(script: Script) -> Self {
        Self {
            script_id: script.id,
            status: script.status,
            updated_at: script.updated_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SceneResponse {
    pub scene_id: Uuid,
    pub sequence: i32,
    pub narration: String,
    pub visual_description: String,
    pub emotion: String,
    pub duration_sec: i32,
}

impl From<Scene> for SceneResponse {
    fn from(scene: Scene) -> Self {
        Self {
            scene_id: scene.id,
            sequence: scene.sequence,
            narration: scene.narration,
            visual_description: scene.visual_description,
            emotion: scene.emotion,
            duration_sec: scene.duration_sec,
        }
    }
}
