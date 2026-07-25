use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{postgres::PgRow, PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetGenerationProvider {
    GptImage2,
    VolcengineArk,
}

impl AssetGenerationProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GptImage2 => "gpt-image-2",
            Self::VolcengineArk => "volcengine-ark",
        }
    }
}

impl TryFrom<&str> for AssetGenerationProvider {
    type Error = AssetGenerationParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "gpt-image-2" => Ok(Self::GptImage2),
            "volcengine-ark" => Ok(Self::VolcengineArk),
            other => Err(AssetGenerationParseError::Provider(other.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetGenerationTaskType {
    ImageCandidates,
    VideoDraft,
    VideoGeneration,
}

impl AssetGenerationTaskType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ImageCandidates => "image_candidates",
            Self::VideoDraft => "video_draft",
            Self::VideoGeneration => "video_generation",
        }
    }

    pub fn is_legacy_read_only(self) -> bool {
        matches!(self, Self::VideoDraft | Self::VideoGeneration)
    }
}

impl TryFrom<&str> for AssetGenerationTaskType {
    type Error = AssetGenerationParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "image_candidates" => Ok(Self::ImageCandidates),
            "video_draft" => Ok(Self::VideoDraft),
            "video_generation" => Ok(Self::VideoGeneration),
            other => Err(AssetGenerationParseError::TaskType(other.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetGenerationTaskStatus {
    Draft,
    Pending,
    Processing,
    Completed,
    Failed,
}

impl AssetGenerationTaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for AssetGenerationTaskStatus {
    type Error = AssetGenerationParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "draft" => Ok(Self::Draft),
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            other => Err(AssetGenerationParseError::TaskStatus(other.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetCandidateType {
    Image,
    Video,
}

impl AssetCandidateType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }

    pub fn is_legacy_read_only(self) -> bool {
        matches!(self, Self::Video)
    }
}

impl TryFrom<&str> for AssetCandidateType {
    type Error = AssetGenerationParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "image" => Ok(Self::Image),
            "video" => Ok(Self::Video),
            other => Err(AssetGenerationParseError::CandidateType(other.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetCandidateSource {
    ExistingMaterial,
    AiGenerated,
    VideoTask,
}

impl AssetCandidateSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExistingMaterial => "existing_material",
            Self::AiGenerated => "ai_generated",
            Self::VideoTask => "video_task",
        }
    }

    pub fn is_legacy_read_only(self) -> bool {
        matches!(self, Self::VideoTask)
    }
}

impl TryFrom<&str> for AssetCandidateSource {
    type Error = AssetGenerationParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "existing_material" => Ok(Self::ExistingMaterial),
            "ai_generated" => Ok(Self::AiGenerated),
            "video_task" => Ok(Self::VideoTask),
            other => Err(AssetGenerationParseError::CandidateSource(
                other.to_string(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AssetCandidateStatus {
    Candidate,
    Selected,
    Rejected,
    Failed,
}

impl AssetCandidateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::Selected => "selected",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
        }
    }
}

impl TryFrom<&str> for AssetCandidateStatus {
    type Error = AssetGenerationParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "candidate" => Ok(Self::Candidate),
            "selected" => Ok(Self::Selected),
            "rejected" => Ok(Self::Rejected),
            "failed" => Ok(Self::Failed),
            other => Err(AssetGenerationParseError::CandidateStatus(
                other.to_string(),
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AssetGenerationTask {
    pub id: Uuid,
    pub project_id: Uuid,
    pub script_id: Option<Uuid>,
    pub scene_id: Option<Uuid>,
    pub model_id: Option<Uuid>,
    pub model_snapshot: Option<Value>,
    pub provider: AssetGenerationProvider,
    pub task_type: AssetGenerationTaskType,
    pub status: AssetGenerationTaskStatus,
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

#[derive(Clone, Debug, PartialEq)]
pub struct SceneAssetCandidate {
    pub id: Uuid,
    pub project_id: Uuid,
    pub script_id: Uuid,
    pub scene_id: Uuid,
    pub material_id: Option<Uuid>,
    pub candidate_type: AssetCandidateType,
    pub source: AssetCandidateSource,
    pub status: AssetCandidateStatus,
    pub rank: i32,
    pub generation_task_id: Option<Uuid>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateAssetGenerationTaskInput {
    pub project_id: Uuid,
    pub script_id: Option<Uuid>,
    pub scene_id: Option<Uuid>,
    pub model_id: Option<Uuid>,
    pub provider: AssetGenerationProvider,
    pub task_type: AssetGenerationTaskType,
    pub status: AssetGenerationTaskStatus,
    pub candidate_count: i32,
    pub reference_material_ids: Vec<Uuid>,
    pub idempotency_key: Option<String>,
    pub params: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateOrReuseAssetGenerationTaskResult {
    pub task: AssetGenerationTask,
    pub created: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateAssetCandidateInput {
    pub project_id: Uuid,
    pub script_id: Uuid,
    pub scene_id: Uuid,
    pub material_id: Option<Uuid>,
    pub candidate_type: AssetCandidateType,
    pub source: AssetCandidateSource,
    pub rank: i32,
    pub generation_task_id: Option<Uuid>,
    pub metadata: Value,
}

#[derive(Clone)]
pub struct PostgresAssetGenerationRepository {
    pool: PgPool,
}

impl PostgresAssetGenerationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn validate_candidate_relations(
        &self,
        input: &CreateAssetCandidateInput,
    ) -> Result<(), AssetGenerationRepositoryError> {
        let scene_matches_script = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM scenes sc
                JOIN scripts s ON s.id = sc.script_id
                WHERE sc.id = $1
                  AND sc.script_id = $2
                  AND s.project_id = $3
            )
            "#,
        )
        .bind(input.scene_id)
        .bind(input.script_id)
        .bind(input.project_id)
        .fetch_one(&self.pool)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;
        if !scene_matches_script {
            return Err(AssetGenerationRepositoryError::InvalidCandidateRelation(
                "候选分镜、脚本和项目不一致".to_string(),
            ));
        }

        if let Some(material_id) = input.material_id {
            let material_status = sqlx::query_as::<_, (Uuid, String)>(
                r#"
                SELECT project_id, status
                FROM materials
                WHERE id = $1
                "#,
            )
            .bind(material_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AssetGenerationRepositoryError::from)?;
            match material_status {
                Some((material_project_id, status))
                    if material_project_id == input.project_id && status == "active" => {}
                _ => {
                    return Err(AssetGenerationRepositoryError::InvalidCandidateRelation(
                        "候选素材必须属于当前项目且处于可用状态".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    async fn find_existing_candidate(
        &self,
        input: &CreateAssetCandidateInput,
    ) -> Result<SceneAssetCandidate, AssetGenerationRepositoryError> {
        let row = if let Some(material_id) = input.material_id {
            sqlx::query(
                r#"
                SELECT id, project_id, script_id, scene_id, material_id, candidate_type,
                       source, status, rank, generation_task_id, metadata, created_at, updated_at
                FROM scene_asset_candidates
                WHERE scene_id = $1
                  AND material_id = $2
                  AND source = $3
                ORDER BY created_at ASC, id ASC
                LIMIT 1
                "#,
            )
            .bind(input.scene_id)
            .bind(material_id)
            .bind(input.source.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(AssetGenerationRepositoryError::from)?
        } else if let Some(task_id) = input.generation_task_id {
            sqlx::query(
                r#"
                SELECT id, project_id, script_id, scene_id, material_id, candidate_type,
                       source, status, rank, generation_task_id, metadata, created_at, updated_at
                FROM scene_asset_candidates
                WHERE generation_task_id = $1
                  AND source = $2
                ORDER BY created_at ASC, id ASC
                LIMIT 1
                "#,
            )
            .bind(task_id)
            .bind(input.source.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(AssetGenerationRepositoryError::from)?
        } else {
            None
        };

        row.map(candidate_from_row).transpose()?.ok_or_else(|| {
            AssetGenerationRepositoryError::Storage("候选创建冲突但未找到已有候选".to_string())
        })
    }
}

#[async_trait]
pub trait AssetGenerationRepository: Send + Sync {
    async fn create_task(
        &self,
        input: CreateAssetGenerationTaskInput,
    ) -> Result<AssetGenerationTask, AssetGenerationRepositoryError>;

    async fn create_or_reuse_scene_image_task(
        &self,
        input: CreateAssetGenerationTaskInput,
    ) -> Result<CreateOrReuseAssetGenerationTaskResult, AssetGenerationRepositoryError>;

    async fn create_candidate(
        &self,
        input: CreateAssetCandidateInput,
    ) -> Result<SceneAssetCandidate, AssetGenerationRepositoryError>;

    async fn list_candidates(
        &self,
        script_id: Uuid,
    ) -> Result<Vec<SceneAssetCandidate>, AssetGenerationRepositoryError>;

    async fn list_tasks(
        &self,
        script_id: Uuid,
    ) -> Result<Vec<AssetGenerationTask>, AssetGenerationRepositoryError>;

    async fn select_candidate(
        &self,
        scene_id: Uuid,
        candidate_id: Uuid,
    ) -> Result<SceneAssetCandidate, AssetGenerationRepositoryError>;

    async fn reject_candidate(
        &self,
        scene_id: Uuid,
        candidate_id: Uuid,
    ) -> Result<SceneAssetCandidate, AssetGenerationRepositoryError>;

    async fn update_task_status(
        &self,
        task_id: Uuid,
        status: AssetGenerationTaskStatus,
        result: Value,
        error_message: Option<String>,
    ) -> Result<AssetGenerationTask, AssetGenerationRepositoryError>;

    async fn dismiss_task(
        &self,
        task_id: Uuid,
    ) -> Result<AssetGenerationTask, AssetGenerationRepositoryError>;
}

#[async_trait]
impl AssetGenerationRepository for PostgresAssetGenerationRepository {
    async fn create_task(
        &self,
        input: CreateAssetGenerationTaskInput,
    ) -> Result<AssetGenerationTask, AssetGenerationRepositoryError> {
        if input.task_type.is_legacy_read_only() {
            return Err(AssetGenerationRepositoryError::LegacyVideoReadOnly);
        }
        let row = sqlx::query(
            r#"
            INSERT INTO asset_generation_tasks (
                project_id, script_id, scene_id, model_id, provider, task_type, status,
                candidate_count, reference_material_ids, idempotency_key, params
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, COALESCE($10, ''), $11)
            ON CONFLICT (idempotency_key) WHERE idempotency_key <> ''
            DO UPDATE SET updated_at = asset_generation_tasks.updated_at
            RETURNING id, project_id, script_id, scene_id, model_id, model_snapshot,
                      provider, task_type, status,
                      candidate_count, reference_material_ids, params, result, error_message,
                      retry_count, dismissed_at, created_at, updated_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.script_id)
        .bind(input.scene_id)
        .bind(input.model_id)
        .bind(input.provider.as_str())
        .bind(input.task_type.as_str())
        .bind(input.status.as_str())
        .bind(input.candidate_count)
        .bind(input.reference_material_ids)
        .bind(input.idempotency_key)
        .bind(input.params)
        .fetch_one(&self.pool)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;

        task_from_row(row)
    }

    async fn create_or_reuse_scene_image_task(
        &self,
        input: CreateAssetGenerationTaskInput,
    ) -> Result<CreateOrReuseAssetGenerationTaskResult, AssetGenerationRepositoryError> {
        if input.task_type != AssetGenerationTaskType::ImageCandidates {
            return Err(AssetGenerationRepositoryError::LegacyVideoReadOnly);
        }
        let scene_id = input.scene_id.ok_or_else(|| {
            AssetGenerationRepositoryError::Storage("单镜头图片生成任务必须绑定分镜".to_string())
        })?;
        let idempotency_key = input.idempotency_key.clone().ok_or_else(|| {
            AssetGenerationRepositoryError::Storage("单镜头图片生成任务必须提供幂等键".to_string())
        })?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(AssetGenerationRepositoryError::from)?;

        sqlx::query("SELECT id FROM scenes WHERE id = $1 FOR UPDATE")
            .bind(scene_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(AssetGenerationRepositoryError::from)?
            .ok_or_else(|| {
                AssetGenerationRepositoryError::Storage("单镜头图片生成分镜不存在".to_string())
            })?;

        let matching_key_row = sqlx::query(
            r#"
            SELECT task.id, task.project_id, task.script_id, task.scene_id,
                   task.model_id, task.model_snapshot, task.provider,
                   task.task_type, task.status, task.candidate_count,
                   task.reference_material_ids, task.params, task.result, task.error_message,
                   task.retry_count, task.dismissed_at, task.created_at, task.updated_at
            FROM asset_generation_task_requests request
            JOIN asset_generation_tasks task ON task.id = request.task_id
            WHERE request.idempotency_key = $1
            LIMIT 1
            FOR UPDATE OF task
            "#,
        )
        .bind(&idempotency_key)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;
        if let Some(row) = matching_key_row {
            let task = task_from_row(row)?;
            transaction
                .commit()
                .await
                .map_err(AssetGenerationRepositoryError::from)?;
            return Ok(CreateOrReuseAssetGenerationTaskResult {
                task,
                created: false,
            });
        }

        let in_flight_row = sqlx::query(
            r#"
            SELECT id, project_id, script_id, scene_id, model_id, model_snapshot,
                   provider, task_type, status,
                   candidate_count, reference_material_ids, params, result, error_message,
                   retry_count, dismissed_at, created_at, updated_at
            FROM asset_generation_tasks
            WHERE scene_id = $1
              AND task_type = 'image_candidates'
              AND status IN ('pending', 'processing')
            ORDER BY created_at ASC, id ASC
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(scene_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;
        if let Some(row) = in_flight_row {
            let task = task_from_row(row)?;
            sqlx::query(
                r#"
                INSERT INTO asset_generation_task_requests (idempotency_key, task_id)
                VALUES ($1, $2)
                ON CONFLICT (idempotency_key) DO NOTHING
                "#,
            )
            .bind(&idempotency_key)
            .bind(task.id)
            .execute(&mut *transaction)
            .await
            .map_err(AssetGenerationRepositoryError::from)?;
            transaction
                .commit()
                .await
                .map_err(AssetGenerationRepositoryError::from)?;
            return Ok(CreateOrReuseAssetGenerationTaskResult {
                task,
                created: false,
            });
        }

        let row = sqlx::query(
            r#"
            INSERT INTO asset_generation_tasks (
                project_id, script_id, scene_id, model_id, provider, task_type, status,
                candidate_count, reference_material_ids, idempotency_key, params
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING id, project_id, script_id, scene_id, model_id, model_snapshot,
                      provider, task_type, status,
                      candidate_count, reference_material_ids, params, result, error_message,
                      retry_count, dismissed_at, created_at, updated_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.script_id)
        .bind(scene_id)
        .bind(input.model_id)
        .bind(input.provider.as_str())
        .bind(input.task_type.as_str())
        .bind(input.status.as_str())
        .bind(input.candidate_count)
        .bind(input.reference_material_ids)
        .bind(&idempotency_key)
        .bind(input.params)
        .fetch_one(&mut *transaction)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;
        let task = task_from_row(row)?;
        sqlx::query(
            r#"
            INSERT INTO asset_generation_task_requests (idempotency_key, task_id)
            VALUES ($1, $2)
            "#,
        )
        .bind(&idempotency_key)
        .bind(task.id)
        .execute(&mut *transaction)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;
        transaction
            .commit()
            .await
            .map_err(AssetGenerationRepositoryError::from)?;

        Ok(CreateOrReuseAssetGenerationTaskResult {
            task,
            created: true,
        })
    }

    async fn create_candidate(
        &self,
        input: CreateAssetCandidateInput,
    ) -> Result<SceneAssetCandidate, AssetGenerationRepositoryError> {
        if input.candidate_type.is_legacy_read_only() || input.source.is_legacy_read_only() {
            return Err(AssetGenerationRepositoryError::LegacyVideoReadOnly);
        }
        self.validate_candidate_relations(&input).await?;
        let row = sqlx::query(
            r#"
            INSERT INTO scene_asset_candidates (
                project_id, script_id, scene_id, material_id, candidate_type, source,
                status, rank, generation_task_id, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, 'candidate', $7, $8, $9)
            ON CONFLICT DO NOTHING
            RETURNING id, project_id, script_id, scene_id, material_id, candidate_type,
                      source, status, rank, generation_task_id, metadata, created_at, updated_at
            "#,
        )
        .bind(input.project_id)
        .bind(input.script_id)
        .bind(input.scene_id)
        .bind(input.material_id)
        .bind(input.candidate_type.as_str())
        .bind(input.source.as_str())
        .bind(input.rank)
        .bind(input.generation_task_id)
        .bind(input.metadata.clone())
        .fetch_optional(&self.pool)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;

        match row {
            Some(row) => candidate_from_row(row),
            None => self.find_existing_candidate(&input).await,
        }
    }

    async fn list_candidates(
        &self,
        script_id: Uuid,
    ) -> Result<Vec<SceneAssetCandidate>, AssetGenerationRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT candidate.id, candidate.project_id, candidate.script_id, candidate.scene_id,
                   candidate.material_id, candidate.candidate_type, candidate.source,
                   candidate.status, candidate.rank, candidate.generation_task_id,
                   candidate.metadata, candidate.created_at, candidate.updated_at
            FROM scene_asset_candidates candidate
            LEFT JOIN asset_generation_tasks task ON task.id = candidate.generation_task_id
            WHERE candidate.script_id = $1
              AND (candidate.status <> 'failed' OR task.dismissed_at IS NULL)
            ORDER BY candidate.scene_id ASC, candidate.rank ASC,
                     candidate.created_at ASC, candidate.id ASC
            "#,
        )
        .bind(script_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;

        rows.into_iter().map(candidate_from_row).collect()
    }

    async fn list_tasks(
        &self,
        script_id: Uuid,
    ) -> Result<Vec<AssetGenerationTask>, AssetGenerationRepositoryError> {
        let rows = sqlx::query(
            r#"
            SELECT id, project_id, script_id, scene_id, model_id, model_snapshot,
                   provider, task_type, status,
                   candidate_count, reference_material_ids, params, result, error_message,
                   retry_count, dismissed_at, created_at, updated_at
            FROM asset_generation_tasks
            WHERE script_id = $1
              AND (
                  dismissed_at IS NULL
                  OR task_type IN ('video_draft', 'video_generation')
              )
            ORDER BY created_at ASC, id ASC
            "#,
        )
        .bind(script_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;

        rows.into_iter().map(task_from_row).collect()
    }

    async fn select_candidate(
        &self,
        scene_id: Uuid,
        candidate_id: Uuid,
    ) -> Result<SceneAssetCandidate, AssetGenerationRepositoryError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(AssetGenerationRepositoryError::from)?;

        sqlx::query(
            r#"
            SELECT id
            FROM scenes
            WHERE id = $1
            FOR UPDATE
            "#,
        )
        .bind(scene_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AssetGenerationRepositoryError::from)?
        .ok_or(AssetGenerationRepositoryError::CandidateNotFound(
            candidate_id,
        ))?;

        let candidate_row = sqlx::query(
            r#"
            SELECT c.id, c.project_id, c.script_id, c.scene_id, c.material_id,
                   c.candidate_type, c.source, c.status, c.rank, c.generation_task_id,
                   c.metadata, c.created_at, c.updated_at
            FROM scene_asset_candidates c
            WHERE c.id = $1
              AND c.scene_id = $2
            FOR UPDATE OF c
            "#,
        )
        .bind(candidate_id)
        .bind(scene_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AssetGenerationRepositoryError::from)?
        .ok_or(AssetGenerationRepositoryError::CandidateNotFound(
            candidate_id,
        ))?;

        let candidate = candidate_from_row(candidate_row)?;
        if candidate.candidate_type.is_legacy_read_only() || candidate.source.is_legacy_read_only()
        {
            return Err(AssetGenerationRepositoryError::LegacyVideoReadOnly);
        }
        if candidate.status == AssetCandidateStatus::Failed {
            return Err(AssetGenerationRepositoryError::FailedCandidateNotSelectable(candidate.id));
        }
        let relation_valid = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM scenes sc
                JOIN scripts s ON s.id = sc.script_id
                WHERE sc.id = $1
                  AND sc.script_id = $2
                  AND s.project_id = $3
            )
            "#,
        )
        .bind(candidate.scene_id)
        .bind(candidate.script_id)
        .bind(candidate.project_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;
        if !relation_valid {
            return Err(AssetGenerationRepositoryError::InvalidCandidateRelation(
                "候选分镜、脚本和项目不一致".to_string(),
            ));
        }

        if let Some(material_id) = candidate.material_id {
            let material = sqlx::query_as::<_, (Uuid, String)>(
                r#"
                SELECT project_id, status
                FROM materials
                WHERE id = $1
                FOR UPDATE
                "#,
            )
            .bind(material_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(AssetGenerationRepositoryError::from)?;
            match material {
                Some((material_project_id, status))
                    if material_project_id == candidate.project_id && status == "active" => {}
                Some(_) => {
                    return Err(AssetGenerationRepositoryError::CandidateNotSelectable(
                        candidate.id,
                    ));
                }
                None => {
                    return Err(AssetGenerationRepositoryError::InvalidCandidateRelation(
                        "候选素材不存在".to_string(),
                    ));
                }
            }
        }

        sqlx::query(
            r#"
            UPDATE scene_asset_candidates
            SET status = 'candidate',
                updated_at = NOW()
            WHERE scene_id = $1
              AND status = 'selected'
              AND candidate_type = 'image'
              AND id <> $2
            "#,
        )
        .bind(scene_id)
        .bind(candidate_id)
        .execute(&mut *transaction)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;

        let row = sqlx::query(
            r#"
            UPDATE scene_asset_candidates
            SET status = 'selected',
                updated_at = NOW()
            WHERE id = $1
              AND scene_id = $2
              AND candidate_type = 'image'
              AND source <> 'video_task'
            RETURNING id, project_id, script_id, scene_id, material_id, candidate_type,
                      source, status, rank, generation_task_id, metadata, created_at, updated_at
            "#,
        )
        .bind(candidate_id)
        .bind(scene_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(AssetGenerationRepositoryError::from)?
        .ok_or(AssetGenerationRepositoryError::CandidateNotSelectable(
            candidate_id,
        ))?;

        transaction
            .commit()
            .await
            .map_err(AssetGenerationRepositoryError::from)?;

        candidate_from_row(row)
    }

    async fn reject_candidate(
        &self,
        scene_id: Uuid,
        candidate_id: Uuid,
    ) -> Result<SceneAssetCandidate, AssetGenerationRepositoryError> {
        let legacy_candidate = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT candidate_type = 'video' OR source = 'video_task'
            FROM scene_asset_candidates
            WHERE id = $1 AND scene_id = $2
            "#,
        )
        .bind(candidate_id)
        .bind(scene_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;
        if legacy_candidate == Some(true) {
            return Err(AssetGenerationRepositoryError::LegacyVideoReadOnly);
        }
        let row = sqlx::query(
            r#"
            UPDATE scene_asset_candidates
            SET status = 'rejected',
                updated_at = NOW()
            WHERE id = $1
              AND scene_id = $2
            RETURNING id, project_id, script_id, scene_id, material_id, candidate_type,
                      source, status, rank, generation_task_id, metadata, created_at, updated_at
            "#,
        )
        .bind(candidate_id)
        .bind(scene_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AssetGenerationRepositoryError::from)?
        .ok_or(AssetGenerationRepositoryError::CandidateNotFound(
            candidate_id,
        ))?;

        candidate_from_row(row)
    }

    async fn update_task_status(
        &self,
        task_id: Uuid,
        status: AssetGenerationTaskStatus,
        result: Value,
        error_message: Option<String>,
    ) -> Result<AssetGenerationTask, AssetGenerationRepositoryError> {
        let row = sqlx::query(
            r#"
            UPDATE asset_generation_tasks
            SET status = $2,
                result = $3,
                error_message = $4,
                updated_at = NOW()
            WHERE id = $1
              AND task_type = 'image_candidates'
            RETURNING id, project_id, script_id, scene_id, model_id, model_snapshot,
                      provider, task_type, status,
                      candidate_count, reference_material_ids, params, result, error_message,
                      retry_count, dismissed_at, created_at, updated_at
            "#,
        )
        .bind(task_id)
        .bind(status.as_str())
        .bind(result)
        .bind(error_message)
        .fetch_optional(&self.pool)
        .await
        .map_err(AssetGenerationRepositoryError::from)?
        .ok_or(AssetGenerationRepositoryError::TaskNotFound(task_id))?;

        task_from_row(row)
    }

    async fn dismiss_task(
        &self,
        task_id: Uuid,
    ) -> Result<AssetGenerationTask, AssetGenerationRepositoryError> {
        let task_type = sqlx::query_scalar::<_, String>(
            "SELECT task_type FROM asset_generation_tasks WHERE id = $1",
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;
        match task_type.as_deref() {
            Some("video_draft" | "video_generation") => {
                return Err(AssetGenerationRepositoryError::LegacyVideoReadOnly);
            }
            None => return Err(AssetGenerationRepositoryError::TaskNotFound(task_id)),
            Some(_) => {}
        }
        let row = sqlx::query(
            r#"
            UPDATE asset_generation_tasks
            SET dismissed_at = COALESCE(dismissed_at, NOW()),
                updated_at = CASE WHEN dismissed_at IS NULL THEN NOW() ELSE updated_at END
            WHERE id = $1
              AND status = 'failed'
            RETURNING id, project_id, script_id, scene_id, model_id, model_snapshot,
                      provider, task_type, status,
                      candidate_count, reference_material_ids, params, result, error_message,
                      retry_count, dismissed_at, created_at, updated_at
            "#,
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AssetGenerationRepositoryError::from)?;

        match row {
            Some(row) => task_from_row(row),
            None => {
                let exists = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM asset_generation_tasks WHERE id = $1)",
                )
                .bind(task_id)
                .fetch_one(&self.pool)
                .await
                .map_err(AssetGenerationRepositoryError::from)?;
                if exists {
                    Err(AssetGenerationRepositoryError::TaskNotDismissible(task_id))
                } else {
                    Err(AssetGenerationRepositoryError::TaskNotFound(task_id))
                }
            }
        }
    }
}

fn task_from_row(row: PgRow) -> Result<AssetGenerationTask, AssetGenerationRepositoryError> {
    let provider_value: String = row.get("provider");
    let provider = AssetGenerationProvider::try_from(provider_value.as_str())
        .map_err(|error| AssetGenerationRepositoryError::Storage(error.to_string()))?;
    let task_type_value: String = row.get("task_type");
    let task_type = AssetGenerationTaskType::try_from(task_type_value.as_str())
        .map_err(|error| AssetGenerationRepositoryError::Storage(error.to_string()))?;
    let status_value: String = row.get("status");
    let status = AssetGenerationTaskStatus::try_from(status_value.as_str())
        .map_err(|error| AssetGenerationRepositoryError::Storage(error.to_string()))?;

    Ok(AssetGenerationTask {
        id: row.get("id"),
        project_id: row.get("project_id"),
        script_id: row.get("script_id"),
        scene_id: row.get("scene_id"),
        model_id: row.get("model_id"),
        model_snapshot: row.get("model_snapshot"),
        provider,
        task_type,
        status,
        candidate_count: row.get("candidate_count"),
        reference_material_ids: row.get("reference_material_ids"),
        params: row.get("params"),
        result: row.get("result"),
        error_message: row.get("error_message"),
        retry_count: row.get("retry_count"),
        dismissed_at: row.get("dismissed_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn candidate_from_row(row: PgRow) -> Result<SceneAssetCandidate, AssetGenerationRepositoryError> {
    let candidate_type_value: String = row.get("candidate_type");
    let candidate_type = AssetCandidateType::try_from(candidate_type_value.as_str())
        .map_err(|error| AssetGenerationRepositoryError::Storage(error.to_string()))?;
    let source_value: String = row.get("source");
    let source = AssetCandidateSource::try_from(source_value.as_str())
        .map_err(|error| AssetGenerationRepositoryError::Storage(error.to_string()))?;
    let status_value: String = row.get("status");
    let status = AssetCandidateStatus::try_from(status_value.as_str())
        .map_err(|error| AssetGenerationRepositoryError::Storage(error.to_string()))?;

    Ok(SceneAssetCandidate {
        id: row.get("id"),
        project_id: row.get("project_id"),
        script_id: row.get("script_id"),
        scene_id: row.get("scene_id"),
        material_id: row.get("material_id"),
        candidate_type,
        source,
        status,
        rank: row.get("rank"),
        generation_task_id: row.get("generation_task_id"),
        metadata: row.get("metadata"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

#[derive(Debug, Eq, PartialEq)]
pub enum AssetGenerationParseError {
    Provider(String),
    TaskType(String),
    TaskStatus(String),
    CandidateType(String),
    CandidateSource(String),
    CandidateStatus(String),
}

impl fmt::Display for AssetGenerationParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provider(value) => write!(formatter, "unknown asset provider: {value}"),
            Self::TaskType(value) => write!(formatter, "unknown asset task type: {value}"),
            Self::TaskStatus(value) => write!(formatter, "unknown asset task status: {value}"),
            Self::CandidateType(value) => {
                write!(formatter, "unknown asset candidate type: {value}")
            }
            Self::CandidateSource(value) => {
                write!(formatter, "unknown asset candidate source: {value}")
            }
            Self::CandidateStatus(value) => {
                write!(formatter, "unknown asset candidate status: {value}")
            }
        }
    }
}

impl std::error::Error for AssetGenerationParseError {}

#[derive(Debug)]
pub enum AssetGenerationRepositoryError {
    TaskNotFound(Uuid),
    TaskNotDismissible(Uuid),
    LegacyVideoReadOnly,
    CandidateNotFound(Uuid),
    CandidateNotSelectable(Uuid),
    FailedCandidateNotSelectable(Uuid),
    InvalidCandidateRelation(String),
    Storage(String),
}

impl From<sqlx::Error> for AssetGenerationRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for AssetGenerationRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TaskNotFound(task_id) => {
                write!(formatter, "asset generation task not found: {task_id}")
            }
            Self::TaskNotDismissible(task_id) => {
                write!(
                    formatter,
                    "asset generation task is not dismissible: {task_id}"
                )
            }
            Self::LegacyVideoReadOnly => {
                formatter.write_str("legacy per-scene video records are read-only")
            }
            Self::CandidateNotFound(candidate_id) => {
                write!(formatter, "asset candidate not found: {candidate_id}")
            }
            Self::CandidateNotSelectable(candidate_id) => {
                write!(
                    formatter,
                    "asset candidate is not selectable: {candidate_id}"
                )
            }
            Self::FailedCandidateNotSelectable(candidate_id) => {
                write!(
                    formatter,
                    "failed asset candidate is not selectable: {candidate_id}"
                )
            }
            Self::InvalidCandidateRelation(message) => {
                write!(formatter, "invalid asset candidate relation: {message}")
            }
            Self::Storage(message) => {
                write!(formatter, "asset generation storage error: {message}")
            }
        }
    }
}

impl std::error::Error for AssetGenerationRepositoryError {}
