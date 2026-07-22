use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkRecord {
    pub id: Uuid,
    pub project_id: Uuid,
    pub script_id: Uuid,
    pub title: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkPlanRecord {
    pub id: Uuid,
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub plan_version: i32,
    pub status: String,
    pub input_fingerprint: String,
    pub llm_model_id: Option<Uuid>,
    pub video_model_id: Option<Uuid>,
    pub tts_model_id: Option<Uuid>,
    pub capability_snapshot: Value,
    pub output_snapshot: Value,
    pub prompt_snapshot: Value,
    pub timeline_snapshot: Value,
    pub resource_usage: Value,
    pub warnings: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkGenerationRunRecord {
    pub id: Uuid,
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub work_plan_id: Uuid,
    pub idempotency_key: String,
    pub status: String,
    pub model_snapshot: Value,
    pub capability_snapshot: Value,
    pub voice_snapshot: Value,
    pub prompt_snapshot: Value,
    pub timeline_snapshot: Value,
    pub parameter_snapshot: Value,
    pub resource_usage: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkGenerationTaskRecord {
    pub id: Uuid,
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub work_plan_id: Uuid,
    pub title: String,
    pub version_no: i32,
    pub status: String,
    pub current_stage: String,
    pub progress_percent: i32,
    pub successful_steps: i64,
    pub running_steps: i64,
    pub queued_steps: i64,
    pub failed_steps: i64,
    pub can_cancel: bool,
    pub cancel_mode: String,
    pub cancel_block_reason: Option<String>,
    pub resource_usage: Value,
    pub error_category: Option<String>,
    pub error_summary: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub dismissed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkGenerationAttemptRecord {
    pub id: Uuid,
    pub attempt_no: i32,
    pub status: String,
    pub model_snapshot: Value,
    pub resource_usage: Value,
    pub error_category: Option<String>,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub request_trace_id: Option<String>,
    pub upstream_task_id: Option<String>,
    pub provider_cancel_supported: bool,
    pub cancel_requested_at: Option<DateTime<Utc>>,
    pub cancel_response: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkGenerationStepRecord {
    pub id: Uuid,
    pub step_no: i32,
    pub step_type: String,
    pub status: String,
    pub is_required: bool,
    pub depends_on: Value,
    pub model_snapshot: Value,
    pub resource_usage: Value,
    pub result_material_ids: Value,
    pub external_task_id: Option<String>,
    pub error_category: Option<String>,
    pub error_code: Option<String>,
    pub error_summary: Option<String>,
    pub attempts: Vec<WorkGenerationAttemptRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkGenerationTaskDetails {
    pub task: WorkGenerationTaskRecord,
    pub steps: Vec<WorkGenerationStepRecord>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkGenerationTaskCounts {
    pub pending: i64,
    pub running: i64,
    pub completed: i64,
    pub attention: i64,
    pub cancelled: i64,
    pub total: i64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct WorkGenerationTaskFilter {
    pub status_view: Option<String>,
    pub stage: Option<String>,
    pub query: Option<String>,
    pub include_hidden: bool,
}

#[derive(Debug)]
pub enum WorkRepositoryError {
    Database(sqlx::Error),
    NotFound(String),
    Conflict(String),
}

impl fmt::Display for WorkRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "数据库错误: {error}"),
            Self::NotFound(value) => write!(f, "作品资源不存在: {value}"),
            Self::Conflict(value) => write!(f, "作品生成冲突: {value}"),
        }
    }
}
impl std::error::Error for WorkRepositoryError {}
impl From<sqlx::Error> for WorkRepositoryError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[async_trait]
pub trait WorkGenerationRepository: Send + Sync {
    async fn find_or_create_work(
        &self,
        project_id: Uuid,
        script_id: Uuid,
        title: &str,
    ) -> Result<WorkRecord, WorkRepositoryError>;
    async fn latest_plan(
        &self,
        work_id: Uuid,
    ) -> Result<Option<WorkPlanRecord>, WorkRepositoryError>;
    async fn save_plan(
        &self,
        work_id: Uuid,
        source_manifest_version: &str,
        input_snapshot: Value,
        plan: &WorkPlanRecord,
    ) -> Result<WorkPlanRecord, WorkRepositoryError>;
    async fn confirm_run(
        &self,
        plan_id: Uuid,
        idempotency_key: &str,
        snapshot: &crate::domain::work_generation::WorkGenerationSnapshot,
        usage: Value,
    ) -> Result<(WorkGenerationRunRecord, bool), WorkRepositoryError>;
    async fn get_run(&self, run_id: Uuid) -> Result<WorkGenerationRunRecord, WorkRepositoryError>;
    async fn list_tasks(
        &self,
        project_id: Uuid,
        filter: WorkGenerationTaskFilter,
    ) -> Result<Vec<WorkGenerationTaskRecord>, WorkRepositoryError>;
    async fn task_counts(
        &self,
        project_id: Uuid,
        include_hidden: bool,
    ) -> Result<WorkGenerationTaskCounts, WorkRepositoryError>;
    async fn task_details(
        &self,
        run_id: Uuid,
    ) -> Result<WorkGenerationTaskDetails, WorkRepositoryError>;
    async fn cancel_run(
        &self,
        run_id: Uuid,
    ) -> Result<WorkGenerationTaskRecord, WorkRepositoryError>;
    async fn dismiss_run(
        &self,
        run_id: Uuid,
    ) -> Result<WorkGenerationTaskRecord, WorkRepositoryError>;
    async fn retry_step(
        &self,
        step_id: Uuid,
        idempotency_key: &str,
    ) -> Result<WorkGenerationAttemptRecord, WorkRepositoryError>;
}

#[derive(Clone)]
pub struct PostgresWorkGenerationRepository {
    pool: PgPool,
}
impl PostgresWorkGenerationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
impl PostgresWorkGenerationRepository {
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl WorkGenerationRepository for PostgresWorkGenerationRepository {
    async fn find_or_create_work(
        &self,
        project_id: Uuid,
        script_id: Uuid,
        title: &str,
    ) -> Result<WorkRecord, WorkRepositoryError> {
        let row = sqlx::query(
            "INSERT INTO works (project_id, script_id, title) VALUES ($1,$2,$3) ON CONFLICT (script_id) WHERE archived_at IS NULL DO UPDATE SET updated_at = NOW() RETURNING id, project_id, script_id, title, status, created_at, updated_at"
        ).bind(project_id).bind(script_id).bind(title.trim()).fetch_one(&self.pool).await?;
        Ok(work_from_row(row))
    }

    async fn latest_plan(
        &self,
        work_id: Uuid,
    ) -> Result<Option<WorkPlanRecord>, WorkRepositoryError> {
        let row = sqlx::query("SELECT id, work_id, work_version_id, plan_version, status, input_fingerprint, llm_model_id, video_model_id, tts_model_id, capability_snapshot, output_snapshot, prompt_snapshot, timeline_snapshot, resource_usage, warnings FROM work_plans WHERE work_id=$1 ORDER BY plan_version DESC LIMIT 1")
            .bind(work_id).fetch_optional(&self.pool).await?;
        Ok(row.map(plan_from_row))
    }

    async fn save_plan(
        &self,
        work_id: Uuid,
        source_manifest_version: &str,
        input_snapshot: Value,
        plan: &WorkPlanRecord,
    ) -> Result<WorkPlanRecord, WorkRepositoryError> {
        let mut tx = self.pool.begin().await?;
        // 作品行锁同时保护草稿选择、版本号和计划修订号，保证同一生产意图串行保存。
        let work = sqlx::query("SELECT current_version_id FROM works WHERE id=$1 FOR UPDATE")
            .bind(work_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| WorkRepositoryError::NotFound(work_id.to_string()))?;
        let current_version_id = work.get::<Option<Uuid>, _>("current_version_id");
        let current = if let Some(version_id) = current_version_id {
            sqlx::query(
                "SELECT id,status,source_manifest_version,
                        EXISTS(SELECT 1 FROM work_generation_runs run WHERE run.work_version_id=work_versions.id) AS has_run
                 FROM work_versions WHERE id=$1 AND work_id=$2",
            )
            .bind(version_id)
            .bind(work_id)
            .fetch_optional(&mut *tx)
            .await?
        } else {
            sqlx::query(
                "SELECT id,status,source_manifest_version,
                        EXISTS(SELECT 1 FROM work_generation_runs run WHERE run.work_version_id=work_versions.id) AS has_run
                 FROM work_versions WHERE work_id=$1 ORDER BY version_no DESC LIMIT 1",
            )
            .bind(work_id)
            .fetch_optional(&mut *tx)
            .await?
        };
        let reusable_draft_id = current.as_ref().and_then(|version| {
            (version.get::<String, _>("status") == "draft" && !version.get::<bool, _>("has_run"))
                .then(|| version.get::<Uuid, _>("id"))
        });
        let model_ids = [plan.llm_model_id, plan.video_model_id, plan.tts_model_id]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let model_names = sqlx::query("SELECT id,display_name FROM ai_models WHERE id=ANY($1)")
            .bind(&model_ids)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|row| {
                (
                    row.get::<Uuid, _>("id"),
                    row.get::<String, _>("display_name"),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        let model_snapshot = json!({
            "llm_model_id": plan.llm_model_id,
            "llm_model_name": plan.llm_model_id.and_then(|id| model_names.get(&id).cloned()),
            "video_model_id": plan.video_model_id,
            "video_model_name": plan.video_model_id.and_then(|id| model_names.get(&id).cloned()),
            "tts_model_id": plan.tts_model_id,
            "tts_model_name": plan.tts_model_id.and_then(|id| model_names.get(&id).cloned()),
        });
        let work_version_id = if let Some(version_id) = reusable_draft_id {
            sqlx::query_scalar::<_, Uuid>(
                "UPDATE work_versions
                 SET source_manifest_version=$2,input_snapshot=$3,model_snapshot=$4,
                     parameter_snapshot=$5,timeline_snapshot=$6,prompt_snapshot=$7
                 WHERE id=$1 RETURNING id",
            )
            .bind(version_id)
            .bind(source_manifest_version)
            .bind(input_snapshot)
            .bind(model_snapshot)
            .bind(plan.output_snapshot.clone())
            .bind(plan.timeline_snapshot.clone())
            .bind(plan.prompt_snapshot.clone())
            .fetch_one(&mut *tx)
            .await?
        } else {
            let source_version_id = current.as_ref().map(|version| version.get::<Uuid, _>("id"));
            let derivation_kind = if source_version_id.is_some() {
                "full_regeneration"
            } else {
                "initial"
            };
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO work_versions (
                    work_id,version_no,status,source_version_id,derivation_kind,
                    source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,
                    timeline_snapshot,prompt_snapshot
                 ) VALUES (
                    $1,COALESCE((SELECT MAX(version_no)+1 FROM work_versions WHERE work_id=$1),1),
                    'draft',$2,$3,$4,$5,$6,$7,$8,$9
                 ) RETURNING id",
            )
            .bind(work_id)
            .bind(source_version_id)
            .bind(derivation_kind)
            .bind(source_manifest_version)
            .bind(input_snapshot)
            .bind(model_snapshot)
            .bind(plan.output_snapshot.clone())
            .bind(plan.timeline_snapshot.clone())
            .bind(plan.prompt_snapshot.clone())
            .fetch_one(&mut *tx)
            .await?
        };
        let plan_version = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(plan_version),0)+1 FROM work_plans WHERE work_id=$1",
        )
        .bind(work_id)
        .fetch_one(&mut *tx)
        .await?;
        let row = sqlx::query("INSERT INTO work_plans (work_id, work_version_id, plan_version, status, input_fingerprint, llm_model_id, video_model_id, tts_model_id, capability_snapshot, output_snapshot, prompt_snapshot, timeline_snapshot, resource_usage, warnings) VALUES ($1,$2,$3,'ready',$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) RETURNING id, work_id, work_version_id, plan_version, status, input_fingerprint, llm_model_id, video_model_id, tts_model_id, capability_snapshot, output_snapshot, prompt_snapshot, timeline_snapshot, resource_usage, warnings")
            .bind(work_id).bind(work_version_id).bind(plan_version).bind(&plan.input_fingerprint).bind(plan.llm_model_id).bind(plan.video_model_id).bind(plan.tts_model_id).bind(plan.capability_snapshot.clone()).bind(plan.output_snapshot.clone()).bind(plan.prompt_snapshot.clone()).bind(plan.timeline_snapshot.clone()).bind(plan.resource_usage.clone()).bind(plan.warnings.clone()).fetch_one(&mut *tx).await?;
        sqlx::query("UPDATE work_plans SET status='invalidated', invalidated_at=NOW() WHERE work_id=$1 AND id<>$2 AND status IN ('draft','ready')").bind(work_id).bind(row.get::<Uuid,_>("id")).execute(&mut *tx).await?;
        sqlx::query(
            "UPDATE works SET status='planned',current_version_id=$2,updated_at=NOW() WHERE id=$1",
        )
        .bind(work_id)
        .bind(work_version_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(plan_from_row(row))
    }

    async fn confirm_run(
        &self,
        plan_id: Uuid,
        idempotency_key: &str,
        snapshot: &crate::domain::work_generation::WorkGenerationSnapshot,
        usage: Value,
    ) -> Result<(WorkGenerationRunRecord, bool), WorkRepositoryError> {
        let plan =
            sqlx::query("SELECT work_id, work_version_id, status FROM work_plans WHERE id=$1")
                .bind(plan_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or_else(|| WorkRepositoryError::NotFound(plan_id.to_string()))?;
        let existing = sqlx::query("SELECT id, work_id, work_version_id, work_plan_id, idempotency_key, status, model_snapshot, capability_snapshot, voice_snapshot, prompt_snapshot, timeline_snapshot, parameter_snapshot, resource_usage FROM work_generation_runs WHERE work_id=$1 AND idempotency_key=$2")
            .bind(plan.get::<Uuid,_>("work_id")).bind(idempotency_key).fetch_optional(&self.pool).await?;
        if let Some(row) = existing {
            return Ok((run_from_row(row), false));
        }
        if plan.get::<String, _>("status") != "ready" {
            return Err(WorkRepositoryError::Conflict("计划已失效或已确认".into()));
        }
        let mut tx = self.pool.begin().await?;
        let mut locked_model_snapshot = snapshot.model_snapshot.clone();
        let model_object = locked_model_snapshot
            .as_object_mut()
            .ok_or_else(|| WorkRepositoryError::Conflict("作品模型快照无效".into()))?;
        for (id_key, version_key, protocol_key) in [
            (
                "video_model_id",
                "video_registry_version",
                "video_api_protocol",
            ),
            ("tts_model_id", "tts_registry_version", "tts_api_protocol"),
        ] {
            let Some(model_id) = model_object
                .get(id_key)
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
            else {
                continue;
            };
            let locked = sqlx::query(
                "SELECT version, api_protocol FROM ai_models WHERE id=$1 AND status='enabled' AND deleted_at IS NULL FOR SHARE",
            )
            .bind(model_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| WorkRepositoryError::Conflict("作品模型已停用或删除".into()))?;
            model_object.insert(version_key.into(), json!(locked.get::<i64, _>("version")));
            model_object.insert(
                protocol_key.into(),
                json!(locked.get::<String, _>("api_protocol")),
            );
        }
        let tos = sqlx::query(
            "SELECT id, version FROM tos_staging_tool_configs WHERE is_current=TRUE AND is_enabled=TRUE FOR SHARE",
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| WorkRepositoryError::Conflict("系统 TOS 暂存工具未配置".into()))?;
        model_object.insert(
            "tos_staging_config_id".into(),
            json!(tos.get::<Uuid, _>("id")),
        );
        model_object.insert(
            "tos_staging_config_version".into(),
            json!(tos.get::<i64, _>("version")),
        );
        let row = sqlx::query("INSERT INTO work_generation_runs (work_id, work_version_id, work_plan_id, idempotency_key, model_snapshot, capability_snapshot, voice_snapshot, prompt_snapshot, timeline_snapshot, parameter_snapshot, resource_usage) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11) ON CONFLICT (work_id, idempotency_key) DO NOTHING RETURNING id, work_id, work_version_id, work_plan_id, idempotency_key, status, model_snapshot, capability_snapshot, voice_snapshot, prompt_snapshot, timeline_snapshot, parameter_snapshot, resource_usage")
            .bind(plan.get::<Uuid,_>("work_id")).bind(plan.get::<Uuid,_>("work_version_id")).bind(plan_id).bind(idempotency_key.trim()).bind(locked_model_snapshot).bind(snapshot.capability_snapshot.clone()).bind(snapshot.voice_snapshot.clone()).bind(snapshot.prompt_snapshot.clone()).bind(snapshot.timeline_snapshot.clone()).bind(snapshot.parameter_snapshot.clone()).bind(usage).fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            tx.rollback().await?;
            let existing = sqlx::query("SELECT id, work_id, work_version_id, work_plan_id, idempotency_key, status, model_snapshot, capability_snapshot, voice_snapshot, prompt_snapshot, timeline_snapshot, parameter_snapshot, resource_usage FROM work_generation_runs WHERE work_id=$1 AND idempotency_key=$2")
                .bind(plan.get::<Uuid,_>("work_id")).bind(idempotency_key).fetch_one(&self.pool).await?;
            return Ok((run_from_row(existing), false));
        };
        seed_generation_steps(&mut tx, row.get("id"), snapshot).await?;
        sqlx::query("UPDATE work_plans SET status='confirmed' WHERE id=$1")
            .bind(plan_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE works SET status='running', updated_at=NOW() WHERE id=$1")
            .bind(plan.get::<Uuid, _>("work_id"))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok((run_from_row(row), true))
    }

    async fn get_run(&self, run_id: Uuid) -> Result<WorkGenerationRunRecord, WorkRepositoryError> {
        let row = sqlx::query("SELECT id, work_id, work_version_id, work_plan_id, idempotency_key, status, model_snapshot, capability_snapshot, voice_snapshot, prompt_snapshot, timeline_snapshot, parameter_snapshot, resource_usage FROM work_generation_runs WHERE id=$1").bind(run_id).fetch_optional(&self.pool).await?.ok_or_else(|| WorkRepositoryError::NotFound(run_id.to_string()))?;
        Ok(run_from_row(row))
    }

    async fn list_tasks(
        &self,
        project_id: Uuid,
        filter: WorkGenerationTaskFilter,
    ) -> Result<Vec<WorkGenerationTaskRecord>, WorkRepositoryError> {
        let view = filter.status_view.unwrap_or_else(|| "all".into());
        let stage = filter.stage.unwrap_or_default();
        let query = filter.query.unwrap_or_default();
        let rows = sqlx::query(
            "SELECT r.id, r.work_id, r.work_version_id, r.work_plan_id, w.title,
                    wv.version_no, r.status,
                    r.current_stage, r.progress_percent,
                    COUNT(s.id) FILTER (WHERE s.status='succeeded') AS successful_steps,
                    COUNT(s.id) FILTER (WHERE s.status='running') AS running_steps,
                    COUNT(s.id) FILTER (WHERE s.status IN ('queued','blocked')) AS queued_steps,
                    COUNT(s.id) FILTER (WHERE s.status IN ('failed','waiting_manual')) AS failed_steps,
                    CASE
                      WHEN r.status='queued' THEN TRUE
                      WHEN r.status='running'
                        AND EXISTS (SELECT 1 FROM work_generation_steps cs JOIN work_generation_attempts ca ON ca.step_id=cs.id WHERE cs.run_id=r.id AND cs.status='running' AND ca.status='running')
                        AND NOT EXISTS (SELECT 1 FROM work_generation_steps cs JOIN work_generation_attempts ca ON ca.step_id=cs.id WHERE cs.run_id=r.id AND cs.status='running' AND ca.status='running' AND (NOT ca.provider_cancel_supported OR ca.upstream_task_id IS NULL))
                      THEN TRUE ELSE FALSE END AS can_cancel,
                    CASE WHEN r.status='queued' THEN 'local' WHEN r.status='running' THEN 'provider' ELSE 'none' END AS cancel_mode,
                    CASE
                      WHEN r.status='running' AND EXISTS (SELECT 1 FROM work_generation_steps cs JOIN work_generation_attempts ca ON ca.step_id=cs.id WHERE cs.run_id=r.id AND cs.status='running' AND ca.status='running' AND (NOT ca.provider_cancel_supported OR ca.upstream_task_id IS NULL))
                      THEN '当前 provider 不支持运行中取消，任务仍需等待上游终态'
                      ELSE NULL END AS cancel_block_reason,
                    r.resource_usage, r.error_category, r.error_summary,
                    r.created_at, r.updated_at, r.dismissed_at
             FROM work_generation_runs r
             JOIN works w ON w.id=r.work_id
             JOIN work_versions wv ON wv.id=r.work_version_id
             LEFT JOIN work_generation_steps s ON s.run_id=r.id
             WHERE w.project_id=$1
               AND ($2 OR r.dismissed_at IS NULL)
               AND ($3='all' OR ($3='pending' AND r.status='queued')
                    OR ($3='running' AND r.status IN ('running','cancelling'))
                    OR ($3='completed' AND r.status='succeeded')
                    OR ($3='cancelled' AND r.status='cancelled')
                    OR ($3='attention' AND r.status IN ('failed','waiting_manual')))
               AND ($4='' OR r.current_stage=$4)
               AND ($5='' OR w.title ILIKE '%' || $5 || '%' OR r.id::text ILIKE '%' || $5 || '%')
             GROUP BY r.id, w.title, wv.version_no
             ORDER BY r.updated_at DESC"
        )
        .bind(project_id)
        .bind(filter.include_hidden)
        .bind(view)
        .bind(stage)
        .bind(query)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(task_from_row).collect())
    }

    async fn task_counts(
        &self,
        project_id: Uuid,
        include_hidden: bool,
    ) -> Result<WorkGenerationTaskCounts, WorkRepositoryError> {
        let row = sqlx::query(
            "SELECT
                COUNT(*) FILTER (WHERE r.status='queued') AS pending,
                COUNT(*) FILTER (WHERE r.status IN ('running','cancelling')) AS running,
                COUNT(*) FILTER (WHERE r.status='succeeded') AS completed,
                COUNT(*) FILTER (WHERE r.status IN ('failed','waiting_manual')) AS attention,
                COUNT(*) FILTER (WHERE r.status='cancelled') AS cancelled,
                COUNT(*) AS total
             FROM work_generation_runs r
             JOIN works w ON w.id=r.work_id
             WHERE w.project_id=$1 AND ($2 OR r.dismissed_at IS NULL)",
        )
        .bind(project_id)
        .bind(include_hidden)
        .fetch_one(&self.pool)
        .await?;
        Ok(WorkGenerationTaskCounts {
            pending: row.get("pending"),
            running: row.get("running"),
            completed: row.get("completed"),
            attention: row.get("attention"),
            cancelled: row.get("cancelled"),
            total: row.get("total"),
        })
    }

    async fn task_details(
        &self,
        run_id: Uuid,
    ) -> Result<WorkGenerationTaskDetails, WorkRepositoryError> {
        let task = sqlx::query(
            "SELECT r.id, r.work_id, r.work_version_id, r.work_plan_id, w.title,
                    wv.version_no, r.status,
                    r.current_stage, r.progress_percent,
                    COUNT(s.id) FILTER (WHERE s.status='succeeded') AS successful_steps,
                    COUNT(s.id) FILTER (WHERE s.status='running') AS running_steps,
                    COUNT(s.id) FILTER (WHERE s.status IN ('queued','blocked')) AS queued_steps,
                    COUNT(s.id) FILTER (WHERE s.status IN ('failed','waiting_manual')) AS failed_steps,
                    CASE
                      WHEN r.status='queued' THEN TRUE
                      WHEN r.status='running'
                        AND EXISTS (SELECT 1 FROM work_generation_steps cs JOIN work_generation_attempts ca ON ca.step_id=cs.id WHERE cs.run_id=r.id AND cs.status='running' AND ca.status='running')
                        AND NOT EXISTS (SELECT 1 FROM work_generation_steps cs JOIN work_generation_attempts ca ON ca.step_id=cs.id WHERE cs.run_id=r.id AND cs.status='running' AND ca.status='running' AND (NOT ca.provider_cancel_supported OR ca.upstream_task_id IS NULL))
                      THEN TRUE ELSE FALSE END AS can_cancel,
                    CASE WHEN r.status='queued' THEN 'local' WHEN r.status='running' THEN 'provider' ELSE 'none' END AS cancel_mode,
                    CASE
                      WHEN r.status='running' AND EXISTS (SELECT 1 FROM work_generation_steps cs JOIN work_generation_attempts ca ON ca.step_id=cs.id WHERE cs.run_id=r.id AND cs.status='running' AND ca.status='running' AND (NOT ca.provider_cancel_supported OR ca.upstream_task_id IS NULL))
                      THEN '当前 provider 不支持运行中取消，任务仍需等待上游终态'
                      ELSE NULL END AS cancel_block_reason,
                    r.resource_usage, r.error_category, r.error_summary,
                    r.created_at, r.updated_at, r.dismissed_at
             FROM work_generation_runs r
             JOIN works w ON w.id=r.work_id
             JOIN work_versions wv ON wv.id=r.work_version_id
             LEFT JOIN work_generation_steps s ON s.run_id=r.id
             WHERE r.id=$1
             GROUP BY r.id, w.title, wv.version_no"
        )
        .bind(run_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| WorkRepositoryError::NotFound(run_id.to_string()))?;
        let steps = sqlx::query(
            "SELECT id, step_no, step_type, status, is_required, depends_on,
                    model_snapshot, resource_usage, result_material_ids,
                    external_task_id, error_category, error_code, error_summary
             FROM work_generation_steps WHERE run_id=$1 ORDER BY step_no",
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await?;
        let mut result = Vec::with_capacity(steps.len());
        for step in steps {
            let attempts = sqlx::query(
                "SELECT id, attempt_no, status, model_snapshot, resource_usage,
                        error_category, error_code, error_summary, request_trace_id,
                        upstream_task_id, provider_cancel_supported, cancel_requested_at,
                        cancel_response, created_at, updated_at
                 FROM work_generation_attempts WHERE step_id=$1 ORDER BY attempt_no DESC",
            )
            .bind(step.get::<Uuid, _>("id"))
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(attempt_from_row)
            .collect();
            result.push(step_from_row(step, attempts));
        }
        Ok(WorkGenerationTaskDetails {
            task: task_from_row(task),
            steps: result,
        })
    }

    async fn cancel_run(
        &self,
        run_id: Uuid,
    ) -> Result<WorkGenerationTaskRecord, WorkRepositoryError> {
        let mut tx = self.pool.begin().await?;
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM work_generation_runs WHERE id=$1 FOR UPDATE",
        )
        .bind(run_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| WorkRepositoryError::NotFound(run_id.to_string()))?;
        if status == "cancelling" {
            tx.rollback().await?;
            return self.task_details(run_id).await.map(|details| details.task);
        }
        if status == "queued" {
            let attempt_count = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM work_generation_attempts a JOIN work_generation_steps s ON s.id=a.step_id WHERE s.run_id=$1",
            )
            .bind(run_id)
            .fetch_one(&mut *tx)
            .await?;
            if attempt_count > 0 {
                return Err(WorkRepositoryError::Conflict(
                    "任务已有 provider attempt，不能按未生成任务取消".into(),
                ));
            }
            sqlx::query("UPDATE work_generation_steps SET status='cancelled' WHERE run_id=$1 AND status IN ('queued','blocked')")
                .bind(run_id).execute(&mut *tx).await?;
            sqlx::query("UPDATE work_generation_runs SET status='cancelled', current_stage='cancelled', completed_at=NOW(), updated_at=NOW() WHERE id=$1")
                .bind(run_id).execute(&mut *tx).await?;
            tx.commit().await?;
            return self.task_details(run_id).await.map(|details| details.task);
        }
        if status != "running" {
            return Err(WorkRepositoryError::Conflict(
                "当前任务状态不允许取消".into(),
            ));
        }
        let active_attempts = sqlx::query(
            "SELECT COUNT(*) AS count,
                    COALESCE(BOOL_AND(a.provider_cancel_supported AND a.upstream_task_id IS NOT NULL), FALSE) AS all_supported
             FROM work_generation_attempts a
             JOIN work_generation_steps s ON s.id=a.step_id
             WHERE s.run_id=$1 AND s.status='running' AND a.status='running'",
        )
        .bind(run_id)
        .fetch_one(&mut *tx)
        .await?;
        if active_attempts.get::<i64, _>("count") == 0 {
            return Err(WorkRepositoryError::Conflict(
                "当前运行步骤没有可取消的上游任务".into(),
            ));
        }
        if !active_attempts.get::<bool, _>("all_supported") {
            return Err(WorkRepositoryError::Conflict(
                "当前 provider 不支持运行中取消，任务仍需等待上游终态".into(),
            ));
        }
        sqlx::query("UPDATE work_generation_runs SET status='cancelling', current_stage='cancelling', cancel_requested_at=NOW(), updated_at=NOW() WHERE id=$1")
            .bind(run_id).execute(&mut *tx).await?;
        sqlx::query("UPDATE work_generation_steps SET status='cancelled', error_category='cancelled', error_summary='运行已请求取消，不再领取后续步骤' WHERE run_id=$1 AND status IN ('queued','blocked')")
            .bind(run_id).execute(&mut *tx).await?;
        tx.commit().await?;
        self.task_details(run_id).await.map(|details| details.task)
    }

    async fn dismiss_run(
        &self,
        run_id: Uuid,
    ) -> Result<WorkGenerationTaskRecord, WorkRepositoryError> {
        let result = sqlx::query("UPDATE work_generation_runs SET dismissed_at=NOW(), updated_at=NOW() WHERE id=$1 AND status IN ('failed','waiting_manual') RETURNING id")
            .bind(run_id).fetch_optional(&self.pool).await?;
        if result.is_none() {
            return Err(WorkRepositoryError::Conflict(
                "只有失败或需人工处理的任务可以隐藏".into(),
            ));
        }
        self.task_details(run_id).await.map(|details| details.task)
    }

    async fn retry_step(
        &self,
        step_id: Uuid,
        idempotency_key: &str,
    ) -> Result<WorkGenerationAttemptRecord, WorkRepositoryError> {
        const IDEMPOTENCY_QUERY: &str = "SELECT a.step_id, a.id, a.attempt_no, a.status, a.model_snapshot, a.resource_usage, a.error_category, a.error_code, a.error_summary, a.request_trace_id, a.upstream_task_id, a.provider_cancel_supported, a.cancel_requested_at, a.cancel_response, a.created_at, a.updated_at FROM work_generation_retry_idempotency k JOIN work_generation_attempts a ON a.id=k.attempt_id WHERE k.idempotency_key=$1";
        let mut tx = self.pool.begin().await?;
        if let Some(existing) = sqlx::query(IDEMPOTENCY_QUERY)
            .bind(idempotency_key.trim())
            .fetch_optional(&mut *tx)
            .await?
        {
            if existing.get::<Uuid, _>("step_id") != step_id {
                return Err(WorkRepositoryError::Conflict(
                    "Idempotency-Key 已绑定其他失败节点".into(),
                ));
            }
            tx.rollback().await?;
            return Ok(attempt_from_row(existing));
        }
        let step = sqlx::query("SELECT id, run_id, status, model_snapshot, resource_usage FROM work_generation_steps WHERE id=$1 FOR UPDATE")
            .bind(step_id).fetch_optional(&mut *tx).await?.ok_or_else(|| WorkRepositoryError::NotFound(step_id.to_string()))?;
        // 首次预查与节点锁之间可能有并发请求提交，锁定后必须再次确认幂等映射。
        if let Some(existing) = sqlx::query(IDEMPOTENCY_QUERY)
            .bind(idempotency_key.trim())
            .fetch_optional(&mut *tx)
            .await?
        {
            if existing.get::<Uuid, _>("step_id") != step_id {
                return Err(WorkRepositoryError::Conflict(
                    "Idempotency-Key 已绑定其他失败节点".into(),
                ));
            }
            tx.rollback().await?;
            return Ok(attempt_from_row(existing));
        }
        if !matches!(
            step.get::<String, _>("status").as_str(),
            "failed" | "waiting_manual"
        ) {
            return Err(WorkRepositoryError::Conflict(
                "只有失败或需人工处理的节点可以重试".into(),
            ));
        }
        let attempt_no = sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(attempt_no),0)+1 FROM work_generation_attempts WHERE step_id=$1",
        )
        .bind(step_id)
        .fetch_one(&mut *tx)
        .await?;
        let attempt = sqlx::query("INSERT INTO work_generation_attempts (step_id, attempt_no, status, model_snapshot, resource_usage) VALUES ($1,$2,'queued',$3,$4) RETURNING id, attempt_no, status, model_snapshot, resource_usage, error_category, error_code, error_summary, request_trace_id, upstream_task_id, provider_cancel_supported, cancel_requested_at, cancel_response, created_at, updated_at")
            .bind(step_id).bind(attempt_no).bind(step.get::<Value,_>("model_snapshot")).bind(step.get::<Value,_>("resource_usage")).fetch_one(&mut *tx).await?;
        sqlx::query("INSERT INTO work_generation_retry_idempotency (idempotency_key, attempt_id) VALUES ($1,$2)")
            .bind(idempotency_key.trim()).bind(attempt.get::<Uuid,_>("id")).execute(&mut *tx).await?;
        sqlx::query(
            "WITH RECURSIVE affected AS (
                SELECT id FROM work_generation_steps WHERE id=$1
                UNION
                SELECT child.id
                FROM work_generation_steps child
                JOIN affected parent ON child.depends_on ? parent.id::text
                WHERE child.run_id=$2 AND child.is_required
             )
             UPDATE work_generation_steps
             SET status='queued', output_snapshot=NULL, result_material_ids='[]'::jsonb,
                 external_task_id=NULL, error_category=NULL, error_code=NULL, error_summary=NULL
             WHERE id IN (SELECT id FROM affected)",
        )
        .bind(step_id)
        .bind(step.get::<Uuid, _>("run_id"))
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE work_generation_runs SET dismissed_at=NULL, completed_at=NULL, updated_at=NOW() WHERE id=$1")
            .bind(step.get::<Uuid, _>("run_id")).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(attempt_from_row(attempt))
    }
}

async fn seed_generation_steps(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_id: Uuid,
    snapshot: &crate::domain::work_generation::WorkGenerationSnapshot,
) -> Result<(), sqlx::Error> {
    let mode = snapshot
        .timeline_snapshot
        .get("audio_mode")
        .and_then(Value::as_str)
        .unwrap_or("independent_tts");
    let (tts_required, asr_required, subtitle_required) = generation_requirements(mode);
    let mut step_no = 1_i32;
    let plan_id = insert_generation_step(
        tx,
        run_id,
        step_no,
        "plan",
        "succeeded",
        true,
        json!([]),
        snapshot.prompt_snapshot.clone(),
        json!({}),
        snapshot.model_snapshot.clone(),
    )
    .await?;
    step_no += 1;
    let tts_id = insert_generation_step(
        tx,
        run_id,
        step_no,
        "tts",
        if tts_required { "queued" } else { "blocked" },
        tts_required,
        json!([plan_id]),
        json!({"voice_snapshot": snapshot.voice_snapshot}),
        snapshot
            .timeline_snapshot
            .get("tts_usage")
            .cloned()
            .unwrap_or_else(|| json!({})),
        snapshot.model_snapshot.clone(),
    )
    .await?;
    let segments = snapshot
        .prompt_snapshot
        .get("segments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut video_step_nos = Vec::with_capacity(segments.len());
    for segment in segments {
        step_no += 1;
        let input = segment.clone();
        let usage = json!({"video_seconds": segment.get("duration_seconds").and_then(Value::as_u64).unwrap_or(0)});
        video_step_nos.push(
            insert_generation_step(
                tx,
                run_id,
                step_no,
                "video_segment",
                "queued",
                true,
                json!([]),
                input,
                usage,
                snapshot.model_snapshot.clone(),
            )
            .await?,
        );
    }
    step_no += 1;
    let asr_id = insert_generation_step(
        tx,
        run_id,
        step_no,
        "asr",
        if asr_required { "queued" } else { "blocked" },
        asr_required,
        json!(video_step_nos),
        json!({}),
        snapshot
            .timeline_snapshot
            .get("asr_usage")
            .cloned()
            .unwrap_or_else(|| json!({})),
        snapshot.model_snapshot.clone(),
    )
    .await?;
    step_no += 1;
    let subtitle_dependencies = if asr_required {
        json!([asr_id])
    } else {
        json!([tts_id])
    };
    let subtitle_id = insert_generation_step(
        tx,
        run_id,
        step_no,
        "subtitle",
        if subtitle_required {
            "queued"
        } else {
            "blocked"
        },
        subtitle_required,
        subtitle_dependencies,
        json!({"source": snapshot.timeline_snapshot.get("subtitle_source")}),
        json!({}),
        snapshot.model_snapshot.clone(),
    )
    .await?;
    step_no += 1;
    let mut mix_dependencies = video_step_nos.clone();
    if subtitle_required {
        mix_dependencies.push(subtitle_id);
    }
    let mix_id = insert_generation_step(
        tx,
        run_id,
        step_no,
        "mix",
        "queued",
        true,
        json!(mix_dependencies),
        json!({}),
        json!({}),
        json!({"tool":"ffmpeg"}),
    )
    .await?;
    step_no += 1;
    insert_generation_step(
        tx,
        run_id,
        step_no,
        "compose",
        "queued",
        true,
        json!([mix_id]),
        json!({}),
        json!({}),
        json!({"tool":"ffmpeg","format":"mp4_h264_aac"}),
    )
    .await?;
    Ok(())
}

fn generation_requirements(mode: &str) -> (bool, bool, bool) {
    match mode {
        "silent" => (false, false, false),
        "seedance_original" => (false, true, true),
        "seedance_original_and_tts" => (true, false, true),
        _ => (true, false, true),
    }
}

#[cfg(test)]
mod generation_requirement_tests {
    use super::generation_requirements;

    #[test]
    fn silent_dag_requires_only_video_mix_and_compose() {
        assert_eq!(generation_requirements("silent"), (false, false, false));
        assert_eq!(
            generation_requirements("independent_tts"),
            (true, false, true)
        );
    }
}

async fn insert_generation_step(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_id: Uuid,
    step_no: i32,
    step_type: &str,
    status: &str,
    required: bool,
    depends_on: Value,
    input: Value,
    usage: Value,
    model: Value,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query("INSERT INTO work_generation_steps (run_id, step_no, step_type, status, is_required, depends_on, input_snapshot, model_snapshot, resource_usage) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(run_id).bind(step_no).bind(step_type).bind(status).bind(required).bind(depends_on).bind(input).bind(model).bind(usage).execute(&mut **tx).await?;
    sqlx::query_scalar("SELECT id FROM work_generation_steps WHERE run_id=$1 AND step_no=$2")
        .bind(run_id)
        .bind(step_no)
        .fetch_one(&mut **tx)
        .await
}

fn task_from_row(row: sqlx::postgres::PgRow) -> WorkGenerationTaskRecord {
    WorkGenerationTaskRecord {
        id: row.get("id"),
        work_id: row.get("work_id"),
        work_version_id: row.get("work_version_id"),
        work_plan_id: row.get("work_plan_id"),
        title: row.get("title"),
        version_no: row.get("version_no"),
        status: row.get("status"),
        current_stage: row.get("current_stage"),
        progress_percent: row.get("progress_percent"),
        successful_steps: row.get("successful_steps"),
        running_steps: row.get("running_steps"),
        queued_steps: row.get("queued_steps"),
        failed_steps: row.get("failed_steps"),
        can_cancel: row.get("can_cancel"),
        cancel_mode: row.get("cancel_mode"),
        cancel_block_reason: row.get("cancel_block_reason"),
        resource_usage: row.get("resource_usage"),
        error_category: row.get("error_category"),
        error_summary: row.get("error_summary"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        dismissed_at: row.get("dismissed_at"),
    }
}

fn step_from_row(
    row: sqlx::postgres::PgRow,
    attempts: Vec<WorkGenerationAttemptRecord>,
) -> WorkGenerationStepRecord {
    WorkGenerationStepRecord {
        id: row.get("id"),
        step_no: row.get("step_no"),
        step_type: row.get("step_type"),
        status: row.get("status"),
        is_required: row.get("is_required"),
        depends_on: row.get("depends_on"),
        model_snapshot: row.get("model_snapshot"),
        resource_usage: row.get("resource_usage"),
        result_material_ids: row.get("result_material_ids"),
        external_task_id: row.get("external_task_id"),
        error_category: row.get("error_category"),
        error_code: row.get("error_code"),
        error_summary: row.get("error_summary"),
        attempts,
    }
}

fn attempt_from_row(row: sqlx::postgres::PgRow) -> WorkGenerationAttemptRecord {
    WorkGenerationAttemptRecord {
        id: row.get("id"),
        attempt_no: row.get("attempt_no"),
        status: row.get("status"),
        model_snapshot: row.get("model_snapshot"),
        resource_usage: row.get("resource_usage"),
        error_category: row.get("error_category"),
        error_code: row.get("error_code"),
        error_summary: row.get("error_summary"),
        request_trace_id: row.get("request_trace_id"),
        upstream_task_id: row.get("upstream_task_id"),
        provider_cancel_supported: row.get("provider_cancel_supported"),
        cancel_requested_at: row.get("cancel_requested_at"),
        cancel_response: row.get("cancel_response"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn work_from_row(row: sqlx::postgres::PgRow) -> WorkRecord {
    WorkRecord {
        id: row.get("id"),
        project_id: row.get("project_id"),
        script_id: row.get("script_id"),
        title: row.get("title"),
        status: row.get("status"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}
fn plan_from_row(row: sqlx::postgres::PgRow) -> WorkPlanRecord {
    WorkPlanRecord {
        id: row.get("id"),
        work_id: row.get("work_id"),
        work_version_id: row.get("work_version_id"),
        plan_version: row.get("plan_version"),
        status: row.get("status"),
        input_fingerprint: row.get("input_fingerprint"),
        llm_model_id: row.get("llm_model_id"),
        video_model_id: row.get("video_model_id"),
        tts_model_id: row.get("tts_model_id"),
        capability_snapshot: row.get("capability_snapshot"),
        output_snapshot: row.get("output_snapshot"),
        prompt_snapshot: row.get("prompt_snapshot"),
        timeline_snapshot: row.get("timeline_snapshot"),
        resource_usage: row.get("resource_usage"),
        warnings: row.get("warnings"),
    }
}
fn run_from_row(row: sqlx::postgres::PgRow) -> WorkGenerationRunRecord {
    WorkGenerationRunRecord {
        id: row.get("id"),
        work_id: row.get("work_id"),
        work_version_id: row.get("work_version_id"),
        work_plan_id: row.get("work_plan_id"),
        idempotency_key: row.get("idempotency_key"),
        status: row.get("status"),
        model_snapshot: row.get("model_snapshot"),
        capability_snapshot: row.get("capability_snapshot"),
        voice_snapshot: row.get("voice_snapshot"),
        prompt_snapshot: row.get("prompt_snapshot"),
        timeline_snapshot: row.get("timeline_snapshot"),
        parameter_snapshot: row.get("parameter_snapshot"),
        resource_usage: row.get("resource_usage"),
    }
}
