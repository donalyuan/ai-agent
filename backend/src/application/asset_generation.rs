//! 素材生成用例，集中维护计划上限、幂等任务和候选素材补全规则。

use crate::agents::ScriptAgentError;
use crate::domain::script::{Scene, Script};
use crate::model_routing::ModelResolveError;
use crate::repositories::{
    AiModelRepository, AssetCandidateSource, AssetCandidateStatus, AssetCandidateType,
    AssetGenerationProvider, AssetGenerationRepository, AssetGenerationRepositoryError,
    AssetGenerationTask, AssetGenerationTaskStatus, AssetGenerationTaskType,
    CreateAssetCandidateInput, CreateAssetGenerationTaskInput, Material, MaterialListFilter,
    MaterialRepository, MaterialRepositoryError, MaterialStatus, MaterialStatusFilter,
    MaterialType, PostgresAiModelRepository, PostgresAssetGenerationRepository,
    PostgresMaterialRepository, PostgresScriptRepository, SceneAssetCandidate, ScriptRepository,
};
use chrono::{DateTime, Utc};
use novex_model::{ApiProtocol, ModelType};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{
    collections::{HashMap, HashSet},
    fmt,
};
use uuid::Uuid;

#[derive(Clone)]
/// 编排画面生成计划、图片任务、候选选择和作品输入清单。
pub struct AssetGenerationService {
    pool: PgPool,
    ai_model_repository: PostgresAiModelRepository,
    asset_repository: PostgresAssetGenerationRepository,
    material_repository: PostgresMaterialRepository,
    script_repository: PostgresScriptRepository,
}

impl AssetGenerationService {
    pub fn new(
        pool: PgPool,
        ai_model_repository: PostgresAiModelRepository,
        asset_repository: PostgresAssetGenerationRepository,
        material_repository: PostgresMaterialRepository,
        script_repository: PostgresScriptRepository,
    ) -> Self {
        Self {
            pool,
            ai_model_repository,
            asset_repository,
            material_repository,
            script_repository,
        }
    }

    pub async fn create_plan(
        &self,
        script_id: Uuid,
        options: AssetGenerationOptions,
    ) -> Result<AssetGenerationPlan, AssetGenerationApplicationError> {
        let provider = self.resolve_image_provider(options.model_id).await?;
        let script = self.get_script(script_id).await?;
        let reference_material_count = if options.use_reference_materials {
            self.active_image_material_ids(script.project_id)
                .await?
                .len() as i32
        } else {
            0
        };
        Ok(build_plan(
            script.id,
            script.scenes.len(),
            options.image_candidates_per_scene,
            options.model_id,
            provider,
            reference_material_count,
        ))
    }

    /// 先创建可复用图片候选，再创建唯一的脚本级图片任务。
    pub async fn create_tasks(
        &self,
        script_id: Uuid,
        options: AssetGenerationOptions,
    ) -> Result<AssetTaskBatch, AssetGenerationApplicationError> {
        let provider = self.resolve_image_provider(options.model_id).await?;
        let script = self.get_script(script_id).await?;
        let plan = build_plan(
            script.id,
            script.scenes.len(),
            options.image_candidates_per_scene,
            options.model_id,
            provider,
            0,
        );
        ensure_plan_can_create(&plan)?;

        let existing_tasks = self.asset_repository.list_tasks(script.id).await?;
        let reference_material_ids = if options.use_reference_materials {
            self.active_image_material_ids(script.project_id).await?
        } else {
            Vec::new()
        };
        self.create_existing_material_candidates(script.project_id, script.id, &script.scenes)
            .await?;
        let scene_ids: Vec<Uuid> = script.scenes.iter().map(|scene| scene.id).collect();
        let image_task_key = script_image_task_idempotency_key(
            script.id,
            options.model_id,
            options.image_candidates_per_scene,
            options.use_reference_materials,
            &reference_material_ids,
        );
        let had_matching_image_task = existing_tasks
            .iter()
            .any(|task| task.params.get("idempotency_key") == Some(&json!(image_task_key)));
        let image_task = self
            .asset_repository
            .create_task(CreateAssetGenerationTaskInput {
                project_id: script.project_id,
                script_id: Some(script.id),
                scene_id: None,
                model_id: Some(options.model_id),
                provider,
                task_type: AssetGenerationTaskType::ImageCandidates,
                status: AssetGenerationTaskStatus::Pending,
                candidate_count: plan.image_candidate_count,
                reference_material_ids: reference_material_ids.clone(),
                idempotency_key: Some(image_task_key.clone()),
                params: json!({
                    "idempotency_key": image_task_key,
                    "image_candidates_per_scene": options.image_candidates_per_scene,
                    "scene_ids": scene_ids,
                    "use_reference_materials": options.use_reference_materials
                }),
            })
            .await?;
        Ok(AssetTaskBatch {
            script_id: script.id,
            reused_all: had_matching_image_task,
            tasks: vec![image_task],
        })
    }

    pub async fn list_tasks(
        &self,
        script_id: Uuid,
    ) -> Result<Vec<AssetGenerationTask>, AssetGenerationApplicationError> {
        self.get_script(script_id).await?;
        self.asset_repository
            .list_tasks(script_id)
            .await
            .map_err(Into::into)
    }

    pub async fn list_candidates(
        &self,
        script_id: Uuid,
    ) -> Result<Vec<AssetCandidateView>, AssetGenerationApplicationError> {
        self.get_script(script_id).await?;
        let candidates = self.asset_repository.list_candidates(script_id).await?;
        self.candidate_views(candidates).await
    }

    /// 输出作品生成唯一可消费的有序主画面输入；不完整时返回逐分镜阻断原因。
    pub async fn scene_visual_manifest(
        &self,
        script_id: Uuid,
    ) -> Result<SceneVisualManifest, AssetGenerationApplicationError> {
        let script = self.get_script(script_id).await?;
        let candidates = self.asset_repository.list_candidates(script_id).await?;
        let failed_scene_ids: HashSet<Uuid> = candidates
            .iter()
            .filter(|candidate| {
                candidate.candidate_type == AssetCandidateType::Image
                    && candidate.status == AssetCandidateStatus::Failed
            })
            .map(|candidate| candidate.scene_id)
            .collect();
        let mut selected_by_scene = HashMap::new();
        for candidate in candidates.into_iter().filter(|candidate| {
            candidate.candidate_type == AssetCandidateType::Image
                && candidate.status == AssetCandidateStatus::Selected
        }) {
            selected_by_scene.insert(candidate.scene_id, self.candidate_view(candidate).await?);
        }

        let mut blockers = Vec::new();
        let mut scenes = Vec::with_capacity(script.scenes.len());
        for scene in &script.scenes {
            let Some(view) = selected_by_scene.remove(&scene.id) else {
                blockers.push(SceneVisualManifestBlocker {
                    scene_id: scene.id,
                    sequence: scene.sequence,
                    reason: if failed_scene_ids.contains(&scene.id) {
                        "image_generation_failed"
                    } else {
                        "selected_image_missing"
                    }
                    .to_string(),
                });
                continue;
            };
            let Some(material) = view.material else {
                blockers.push(SceneVisualManifestBlocker {
                    scene_id: scene.id,
                    sequence: scene.sequence,
                    reason: "selected_material_missing".to_string(),
                });
                continue;
            };
            if material.material_type != MaterialType::Image {
                blockers.push(SceneVisualManifestBlocker {
                    scene_id: scene.id,
                    sequence: scene.sequence,
                    reason: "selected_material_not_image".to_string(),
                });
                continue;
            }
            if material.status != MaterialStatus::Active {
                blockers.push(SceneVisualManifestBlocker {
                    scene_id: scene.id,
                    sequence: scene.sequence,
                    reason: "material_archived".to_string(),
                });
                continue;
            }
            if material.file_url.trim().is_empty() {
                blockers.push(SceneVisualManifestBlocker {
                    scene_id: scene.id,
                    sequence: scene.sequence,
                    reason: "material_url_missing".to_string(),
                });
                continue;
            }

            scenes.push(SceneVisualManifestItem {
                scene_id: scene.id,
                sequence: scene.sequence,
                narration: scene.narration.clone(),
                visual_description: scene.visual_description.clone(),
                emotion: scene.emotion.clone(),
                duration_sec: scene.duration_sec,
                candidate_id: view.candidate.id,
                material_id: material.id,
                file_url: material.file_url.clone(),
                thumbnail_url: material.thumbnail_url.clone(),
                source_snapshot: json!({
                    "candidate_source": view.candidate.source.as_str(),
                    "candidate_metadata": view.candidate.metadata,
                    "candidate_updated_at": view.candidate.updated_at,
                    "material_metadata": material.metadata,
                    "material_updated_at": material.updated_at
                }),
            });
        }

        if !blockers.is_empty() {
            return Err(AssetGenerationApplicationError::ManifestIncomplete {
                script_id,
                blockers,
            });
        }

        let input_version = manifest_input_version(&script, &scenes)?;
        Ok(SceneVisualManifest {
            script_id,
            project_id: script.project_id,
            script_title: script.title,
            script_updated_at: script.updated_at,
            input_version,
            scenes,
        })
    }

    pub async fn validate_scene_visual_manifest(
        &self,
        script_id: Uuid,
        expected_input_version: &str,
    ) -> Result<SceneVisualManifest, AssetGenerationApplicationError> {
        let manifest = self.scene_visual_manifest(script_id).await?;
        if manifest.input_version != expected_input_version {
            return Err(AssetGenerationApplicationError::ManifestStale {
                expected_input_version: expected_input_version.to_string(),
                actual_input_version: manifest.input_version,
            });
        }
        Ok(manifest)
    }

    pub async fn select_candidate(
        &self,
        scene_id: Uuid,
        candidate_id: Uuid,
    ) -> Result<AssetCandidateView, AssetGenerationApplicationError> {
        let candidate = self
            .asset_repository
            .select_candidate(scene_id, candidate_id)
            .await?;
        self.candidate_view(candidate).await
    }

    pub async fn reject_candidate(
        &self,
        scene_id: Uuid,
        candidate_id: Uuid,
    ) -> Result<AssetCandidateView, AssetGenerationApplicationError> {
        let candidate = self
            .asset_repository
            .reject_candidate(scene_id, candidate_id)
            .await?;
        self.candidate_view(candidate).await
    }

    /// UUID 请求键与数据库的一次在途约束共同保证单镜头重生幂等。
    pub async fn create_scene_task(
        &self,
        scene_id: Uuid,
        request_idempotency_key: Uuid,
        options: AssetGenerationOptions,
    ) -> Result<SceneTaskCreation, AssetGenerationApplicationError> {
        let provider = self.resolve_image_provider(options.model_id).await?;
        let (script_id, project_id) = self.scene_context(scene_id).await?;
        let reference_material_ids = if options.use_reference_materials {
            self.active_image_material_ids(project_id).await?
        } else {
            Vec::new()
        };
        let idempotency_key = format!("scene-image:{scene_id}:{request_idempotency_key}");
        let result = self
            .asset_repository
            .create_or_reuse_scene_image_task(CreateAssetGenerationTaskInput {
                project_id,
                script_id: Some(script_id),
                scene_id: Some(scene_id),
                model_id: Some(options.model_id),
                provider,
                task_type: AssetGenerationTaskType::ImageCandidates,
                status: AssetGenerationTaskStatus::Pending,
                candidate_count: options.image_candidates_per_scene,
                reference_material_ids,
                idempotency_key: Some(idempotency_key.clone()),
                params: json!({
                    "idempotency_key": idempotency_key,
                    "image_candidates_per_scene": options.image_candidates_per_scene,
                    "scene_id": scene_id,
                    "use_reference_materials": options.use_reference_materials
                }),
            })
            .await?;
        Ok(SceneTaskCreation {
            created: result.created,
            task: result.task,
        })
    }

    pub async fn dismiss_task(
        &self,
        task_id: Uuid,
    ) -> Result<AssetGenerationTask, AssetGenerationApplicationError> {
        self.asset_repository
            .dismiss_task(task_id)
            .await
            .map_err(Into::into)
    }

    async fn resolve_image_provider(
        &self,
        model_id: Uuid,
    ) -> Result<AssetGenerationProvider, AssetGenerationApplicationError> {
        let runtime = self
            .ai_model_repository
            .resolve_enabled(model_id, ModelType::Image)
            .await
            .map_err(|error| ModelResolveError::from_repository(error, model_id))?;
        image_provider_for_protocol(runtime.snapshot.api_protocol)
    }

    async fn get_script(&self, script_id: Uuid) -> Result<Script, AssetGenerationApplicationError> {
        self.script_repository
            .get_script(script_id)
            .await
            .map_err(ScriptAgentError::from)
            .map_err(Into::into)
    }

    async fn active_image_material_ids(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<Uuid>, AssetGenerationApplicationError> {
        let materials = self
            .material_repository
            .list_materials(
                project_id,
                MaterialListFilter {
                    material_type: Some(MaterialType::Image),
                    status: MaterialStatusFilter::Active,
                    ..MaterialListFilter::default()
                },
            )
            .await?;
        Ok(materials.into_iter().map(|material| material.id).collect())
    }

    async fn create_existing_material_candidates(
        &self,
        project_id: Uuid,
        script_id: Uuid,
        scenes: &[Scene],
    ) -> Result<(), AssetGenerationApplicationError> {
        let materials = self
            .material_repository
            .list_materials(
                project_id,
                MaterialListFilter {
                    material_type: Some(MaterialType::Image),
                    status: MaterialStatusFilter::Active,
                    ..MaterialListFilter::default()
                },
            )
            .await?;

        for scene in scenes {
            for (index, material) in materials.iter().enumerate() {
                self.asset_repository
                    .create_candidate(CreateAssetCandidateInput {
                        project_id,
                        script_id,
                        scene_id: scene.id,
                        material_id: Some(material.id),
                        candidate_type: AssetCandidateType::Image,
                        source: AssetCandidateSource::ExistingMaterial,
                        rank: index as i32 + 1,
                        generation_task_id: None,
                        metadata: json!({ "reuse_reason": "active image material" }),
                    })
                    .await?;
            }
        }
        Ok(())
    }

    async fn candidate_views(
        &self,
        candidates: Vec<SceneAssetCandidate>,
    ) -> Result<Vec<AssetCandidateView>, AssetGenerationApplicationError> {
        let mut views = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            views.push(self.candidate_view(candidate).await?);
        }
        Ok(views)
    }

    async fn candidate_view(
        &self,
        candidate: SceneAssetCandidate,
    ) -> Result<AssetCandidateView, AssetGenerationApplicationError> {
        let material = match candidate.material_id {
            Some(material_id) => Some(self.material_repository.get_material(material_id).await?),
            None => None,
        };
        Ok(AssetCandidateView {
            candidate,
            material,
        })
    }

    async fn scene_context(
        &self,
        scene_id: Uuid,
    ) -> Result<(Uuid, Uuid), AssetGenerationApplicationError> {
        sqlx::query_as::<_, (Uuid, Uuid)>(
            r#"
            SELECT s.id AS script_id, s.project_id
            FROM scenes sc
            JOIN scripts s ON s.id = sc.script_id
            WHERE sc.id = $1
            "#,
        )
        .bind(scene_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| AssetGenerationApplicationError::Validation(error.to_string()))?
        .ok_or_else(|| AssetGenerationApplicationError::Validation("分镜不存在".to_string()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetGenerationOptions {
    pub model_id: Uuid,
    pub image_candidates_per_scene: i32,
    pub use_reference_materials: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssetGenerationPlan {
    pub script_id: Uuid,
    pub scene_count: usize,
    pub image_candidate_count: i32,
    pub max_image_candidate_count: i32,
    pub model_id: Uuid,
    pub provider: AssetGenerationProvider,
    pub reference_material_count: i32,
    pub can_create: bool,
    pub warnings: Vec<String>,
}

pub struct AssetTaskBatch {
    pub script_id: Uuid,
    pub reused_all: bool,
    pub tasks: Vec<AssetGenerationTask>,
}

pub struct SceneTaskCreation {
    pub created: bool,
    pub task: AssetGenerationTask,
}

pub struct AssetCandidateView {
    pub candidate: SceneAssetCandidate,
    pub material: Option<Material>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SceneVisualManifestBlocker {
    pub scene_id: Uuid,
    pub sequence: i32,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SceneVisualManifestItem {
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

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SceneVisualManifest {
    pub script_id: Uuid,
    pub project_id: Uuid,
    pub script_title: String,
    pub script_updated_at: DateTime<Utc>,
    pub input_version: String,
    pub scenes: Vec<SceneVisualManifestItem>,
}

fn build_plan(
    script_id: Uuid,
    scene_count: usize,
    image_candidates_per_scene: i32,
    model_id: Uuid,
    provider: AssetGenerationProvider,
    reference_material_count: i32,
) -> AssetGenerationPlan {
    let image_candidate_count = scene_count as i32 * image_candidates_per_scene;
    let can_create = image_candidate_count <= 48;
    let warnings = if can_create {
        Vec::new()
    } else {
        vec!["单次最多生成 48 张图片候选，请减少分镜或候选数量".to_string()]
    };
    AssetGenerationPlan {
        script_id,
        scene_count,
        image_candidate_count,
        max_image_candidate_count: 48,
        model_id,
        provider,
        reference_material_count,
        can_create,
        warnings,
    }
}

fn image_provider_for_protocol(
    protocol: ApiProtocol,
) -> Result<AssetGenerationProvider, AssetGenerationApplicationError> {
    match protocol {
        ApiProtocol::OpenAiImages => Ok(AssetGenerationProvider::GptImage2),
        ApiProtocol::VolcengineArkImages => Ok(AssetGenerationProvider::VolcengineArk),
        _ => Err(ModelResolveError::InvalidConfig(Uuid::nil()).into()),
    }
}

fn script_image_task_idempotency_key(
    script_id: Uuid,
    model_id: Uuid,
    image_candidates_per_scene: i32,
    use_reference_materials: bool,
    reference_material_ids: &[Uuid],
) -> String {
    let mut references: Vec<String> = reference_material_ids.iter().map(Uuid::to_string).collect();
    references.sort();
    format!(
        "script:{script_id}:image:{model_id}:{image_candidates_per_scene}:{use_reference_materials}:{}",
        references.join("|")
    )
}

fn ensure_plan_can_create(
    plan: &AssetGenerationPlan,
) -> Result<(), AssetGenerationApplicationError> {
    if plan.can_create {
        Ok(())
    } else {
        Err(AssetGenerationApplicationError::Validation(
            "单次最多生成 48 张图片候选，请减少分镜或候选数量".to_string(),
        ))
    }
}

fn manifest_input_version(
    script: &Script,
    scenes: &[SceneVisualManifestItem],
) -> Result<String, AssetGenerationApplicationError> {
    let canonical = json!({
        "script_id": script.id,
        "script_title": script.title,
        "script_updated_at": script.updated_at,
        "scenes": scenes
    });
    let bytes = serde_json::to_vec(&canonical)
        .map_err(|error| AssetGenerationApplicationError::Validation(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[derive(Debug)]
pub enum AssetGenerationApplicationError {
    Agent(ScriptAgentError),
    AssetRepository(AssetGenerationRepositoryError),
    MaterialRepository(MaterialRepositoryError),
    ModelResolve(ModelResolveError),
    ManifestIncomplete {
        script_id: Uuid,
        blockers: Vec<SceneVisualManifestBlocker>,
    },
    ManifestStale {
        expected_input_version: String,
        actual_input_version: String,
    },
    Validation(String),
}

impl From<ScriptAgentError> for AssetGenerationApplicationError {
    fn from(error: ScriptAgentError) -> Self {
        Self::Agent(error)
    }
}

impl From<AssetGenerationRepositoryError> for AssetGenerationApplicationError {
    fn from(error: AssetGenerationRepositoryError) -> Self {
        Self::AssetRepository(error)
    }
}

impl From<MaterialRepositoryError> for AssetGenerationApplicationError {
    fn from(error: MaterialRepositoryError) -> Self {
        Self::MaterialRepository(error)
    }
}

impl From<ModelResolveError> for AssetGenerationApplicationError {
    fn from(error: ModelResolveError) -> Self {
        Self::ModelResolve(error)
    }
}

impl fmt::Display for AssetGenerationApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Agent(error) => write!(formatter, "{error}"),
            Self::AssetRepository(error) => write!(formatter, "{error}"),
            Self::MaterialRepository(error) => write!(formatter, "{error}"),
            Self::ModelResolve(error) => write!(formatter, "{error}"),
            Self::ManifestIncomplete { script_id, .. } => {
                write!(
                    formatter,
                    "scene visual manifest is incomplete: {script_id}"
                )
            }
            Self::ManifestStale { .. } => {
                formatter.write_str("scene visual manifest input version is stale")
            }
            Self::Validation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for AssetGenerationApplicationError {}
