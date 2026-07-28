use crate::domain::work_library::{analyze_version_diff, WorkVersion, WorkVersionStatus};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkVersionRecord {
    pub id: Uuid,
    pub work_id: Uuid,
    pub version_no: i32,
    pub status: String,
    pub source_version_id: Option<Uuid>,
    pub derivation_kind: String,
    pub source_manifest_version: String,
    pub input_snapshot: Value,
    pub model_snapshot: Value,
    pub parameter_snapshot: Value,
    pub prompt_snapshot: Value,
    pub timeline_snapshot: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl WorkVersionRecord {
    pub fn domain(&self) -> Result<WorkVersion, WorkLibraryRepositoryError> {
        let status = match self.status.as_str() {
            "draft" => WorkVersionStatus::Draft,
            "confirmed" => WorkVersionStatus::Confirmed,
            "running" => WorkVersionStatus::Running,
            "completed" => WorkVersionStatus::Completed,
            "failed" => WorkVersionStatus::Failed,
            value => {
                return Err(WorkLibraryRepositoryError::Conflict(format!(
                    "未知版本状态: {value}"
                )))
            }
        };
        Ok(WorkVersion {
            id: self.id,
            work_id: self.work_id,
            version_no: self.version_no,
            status,
            source_version_id: self.source_version_id,
            source_manifest_version: self.source_manifest_version.clone(),
            input_snapshot: self.input_snapshot.clone(),
            model_snapshot: self.model_snapshot.clone(),
            parameter_snapshot: self.parameter_snapshot.clone(),
            prompt_snapshot: self.prompt_snapshot.clone(),
            timeline_snapshot: self.timeline_snapshot.clone(),
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkArtifactRecord {
    pub id: Uuid,
    pub work_version_id: Uuid,
    pub version_status: String,
    pub role: String,
    pub material_id: Option<Uuid>,
    pub file_name: String,
    pub storage_path: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkVersionDiffPlanRecord {
    pub id: Uuid,
    pub work_id: Uuid,
    pub source_version_id: Uuid,
    pub draft_version_id: Uuid,
    pub plan_version: i32,
    pub source_fingerprint: String,
    pub draft_fingerprint: String,
    pub changes: Value,
    pub affected_nodes: Value,
    pub reused_artifact_ids: Value,
    pub resource_usage: Value,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkDiffConfirmation {
    pub run_id: Uuid,
    pub diff_plan_id: Uuid,
    pub created: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkPublicationHandoff {
    pub id: Uuid,
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub final_video_artifact_id: Uuid,
    pub subtitle_artifact_id: Option<Uuid>,
    pub status: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
    pub created: bool,
}

#[derive(Debug)]
pub enum WorkLibraryRepositoryError {
    Database(sqlx::Error),
    NotFound(String),
    Conflict(String),
    StaleDiff,
}

impl fmt::Display for WorkLibraryRepositoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(f, "数据库错误: {error}"),
            Self::NotFound(value) => write!(f, "作品库资源不存在: {value}"),
            Self::Conflict(value) => write!(f, "作品库状态冲突: {value}"),
            Self::StaleDiff => f.write_str("差异计划已过期，请重新分析"),
        }
    }
}

impl std::error::Error for WorkLibraryRepositoryError {}

impl From<sqlx::Error> for WorkLibraryRepositoryError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[derive(Clone)]
pub struct PostgresWorkLibraryRepository {
    pool: PgPool,
}

impl PostgresWorkLibraryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn list_works(
        &self,
        project_id: Uuid,
        archived: bool,
        query: Option<&str>,
    ) -> Result<Value, WorkLibraryRepositoryError> {
        let query = query.unwrap_or("").trim();
        let rows = sqlx::query(
            "SELECT w.id,w.project_id,w.script_id,w.title,w.status,w.current_version_id,
                    w.archived_at,w.created_at,w.updated_at,
                    completed.id AS completed_version_id,completed.version_no AS completed_version_no,
                    completed.parameter_snapshot,completed.timeline_snapshot,
                    cover.id AS cover_artifact_id,cover.storage_path AS cover_storage_path
             FROM works w
             LEFT JOIN LATERAL (
                SELECT v.* FROM work_versions v WHERE v.work_id=w.id AND v.status='completed'
                ORDER BY v.version_no DESC LIMIT 1
             ) completed ON TRUE
             LEFT JOIN LATERAL (
                SELECT a.id,a.storage_path FROM work_artifacts a
                WHERE a.work_version_id=completed.id AND a.role='final_video'
                ORDER BY a.created_at LIMIT 1
             ) cover ON TRUE
             WHERE w.project_id=$1 AND (w.archived_at IS NOT NULL)=$2
               AND ($3='' OR w.title ILIKE '%' || $3 || '%')
             ORDER BY w.updated_at DESC",
        )
        .bind(project_id)
        .bind(archived)
        .bind(query)
        .fetch_all(&self.pool)
        .await?;
        let items = rows.into_iter().map(|row| {
            let status = if row.get::<Option<DateTime<Utc>>,_>("archived_at").is_some() { "archived".to_string() } else { row.get::<String,_>("status") };
            json!({
            "id": row.get::<Uuid,_>("id"), "project_id": row.get::<Uuid,_>("project_id"),
            "script_id": row.get::<Uuid,_>("script_id"), "title": row.get::<String,_>("title"),
            "status": status,
            "archived": row.get::<Option<DateTime<Utc>>,_>("archived_at").is_some(),
            "current_version_id": row.get::<Option<Uuid>,_>("current_version_id"),
            "current_completed_version_id": row.get::<Option<Uuid>,_>("completed_version_id"),
            "current_completed_version_no": row.get::<Option<i32>,_>("completed_version_no"),
            "aspect_ratio": row.get::<Option<Value>,_>("parameter_snapshot").and_then(|v| v.get("aspect_ratio").cloned()),
            "duration_seconds": row.get::<Option<Value>,_>("timeline_snapshot").and_then(|v| v.get("duration_seconds").cloned()),
            "cover_artifact_id": row.get::<Option<Uuid>,_>("cover_artifact_id"),
            "cover_storage_path": row.get::<Option<String>,_>("cover_storage_path"),
            "created_at": row.get::<DateTime<Utc>,_>("created_at"), "updated_at": row.get::<DateTime<Utc>,_>("updated_at")
        })}).collect::<Vec<_>>();
        Ok(json!({"items": items, "archived": archived}))
    }

    pub async fn work_details(&self, work_id: Uuid) -> Result<Value, WorkLibraryRepositoryError> {
        let work = sqlx::query("SELECT id,project_id,script_id,title,status,current_version_id,archived_at,created_at,updated_at FROM works WHERE id=$1")
            .bind(work_id).fetch_optional(&self.pool).await?.ok_or_else(|| WorkLibraryRepositoryError::NotFound(work_id.to_string()))?;
        let versions = sqlx::query("SELECT id,work_id,version_no,status,source_version_id,derivation_kind,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot,created_at,updated_at,completed_at FROM work_versions WHERE work_id=$1 ORDER BY version_no DESC")
            .bind(work_id).fetch_all(&self.pool).await?.into_iter().map(version_from_row).collect::<Vec<_>>();
        let artifacts = sqlx::query("SELECT a.id,a.work_version_id,v.status AS version_status,a.role,a.material_id,a.file_name,a.storage_path,a.mime_type,a.size_bytes,a.sha256,a.metadata FROM work_artifacts a JOIN work_versions v ON v.id=a.work_version_id WHERE v.work_id=$1 ORDER BY v.version_no DESC,a.created_at")
            .bind(work_id).fetch_all(&self.pool).await?.into_iter().map(artifact_from_row).collect::<Vec<_>>();
        let timelines = sqlx::query("SELECT t.work_version_id,t.video_tracks,t.audio_tracks,t.subtitle_tracks FROM work_timelines t JOIN work_versions v ON v.id=t.work_version_id WHERE v.work_id=$1")
            .bind(work_id).fetch_all(&self.pool).await?.into_iter().map(|row| json!({"work_version_id":row.get::<Uuid,_>("work_version_id"),"video":row.get::<Value,_>("video_tracks"),"audio":row.get::<Value,_>("audio_tracks"),"subtitles":row.get::<Value,_>("subtitle_tracks")})).collect::<Vec<_>>();
        let runs = sqlx::query("SELECT r.id,r.work_version_id,r.status,r.current_stage,r.progress_percent,r.error_category,r.error_summary,r.created_at,r.updated_at,(SELECT COUNT(*) FROM work_generation_attempts a JOIN work_generation_steps s ON s.id=a.step_id WHERE s.run_id=r.id) AS attempt_count FROM work_generation_runs r WHERE r.work_id=$1 ORDER BY r.created_at DESC")
            .bind(work_id).fetch_all(&self.pool).await?.into_iter().map(|row| json!({"id":row.get::<Uuid,_>("id"),"work_version_id":row.get::<Uuid,_>("work_version_id"),"status":row.get::<String,_>("status"),"current_stage":row.get::<String,_>("current_stage"),"progress_percent":row.get::<i32,_>("progress_percent"),"error_category":row.get::<Option<String>,_>("error_category"),"error_summary":row.get::<Option<String>,_>("error_summary"),"attempt_count":row.get::<i64,_>("attempt_count"),"created_at":row.get::<DateTime<Utc>,_>("created_at"),"updated_at":row.get::<DateTime<Utc>,_>("updated_at")})).collect::<Vec<_>>();
        let model_catalog = sqlx::query(
            "SELECT id,display_name,model_type FROM ai_models
             WHERE id IN (
                SELECT llm_model_id FROM work_plans WHERE work_id=$1
                UNION SELECT video_model_id FROM work_plans WHERE work_id=$1
                UNION SELECT tts_model_id FROM work_plans WHERE work_id=$1
             )",
        )
        .bind(work_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            (
                row.get::<Uuid, _>("id").to_string(),
                json!({
                    "display_name": row.get::<String, _>("display_name"),
                    "model_type": row.get::<String, _>("model_type"),
                }),
            )
        })
        .collect::<serde_json::Map<String, Value>>();
        let work_status = if work
            .get::<Option<DateTime<Utc>>, _>("archived_at")
            .is_some()
        {
            "archived".to_string()
        } else {
            work.get::<String, _>("status")
        };
        Ok(json!({
            "id":work.get::<Uuid,_>("id"),"project_id":work.get::<Uuid,_>("project_id"),"script_id":work.get::<Uuid,_>("script_id"),"title":work.get::<String,_>("title"),
            "status":work_status,
            "archived":work.get::<Option<DateTime<Utc>>,_>("archived_at").is_some(),"current_version_id":work.get::<Option<Uuid>,_>("current_version_id"),
            "versions":versions,"artifacts":artifacts,"timelines":timelines,"generation_audit":runs,"model_catalog":model_catalog,
            "created_at":work.get::<DateTime<Utc>,_>("created_at"),"updated_at":work.get::<DateTime<Utc>,_>("updated_at")
        }))
    }

    pub async fn derive_version(
        &self,
        source_version_id: Uuid,
        kind: &str,
        patches: [&Option<Value>; 5],
    ) -> Result<(WorkVersionRecord, bool), WorkLibraryRepositoryError> {
        let mut tx = self.pool.begin().await?;
        let source_row = sqlx::query("SELECT v.id,v.work_id,v.version_no,v.status,v.source_version_id,v.derivation_kind,v.source_manifest_version,v.input_snapshot,v.model_snapshot,v.parameter_snapshot,v.prompt_snapshot,v.timeline_snapshot,v.created_at,v.updated_at,v.completed_at,w.archived_at FROM work_versions v JOIN works w ON w.id=v.work_id WHERE v.id=$1 FOR UPDATE OF v,w")
            .bind(source_version_id).fetch_optional(&mut *tx).await?.ok_or_else(|| WorkLibraryRepositoryError::NotFound(source_version_id.to_string()))?;
        if source_row
            .get::<Option<DateTime<Utc>>, _>("archived_at")
            .is_some()
        {
            return Err(WorkLibraryRepositoryError::Conflict(
                "归档作品必须先恢复后再修改".into(),
            ));
        }
        let source = version_from_row(source_row);
        let existing = if kind == "edit" && source.status == "draft" {
            Some(source.clone())
        } else {
            sqlx::query("SELECT id,work_id,version_no,status,source_version_id,derivation_kind,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot,created_at,updated_at,completed_at FROM work_versions WHERE work_id=$1 AND source_version_id=$2 AND status='draft' AND derivation_kind=$3 ORDER BY version_no DESC LIMIT 1 FOR UPDATE")
                .bind(source.work_id).bind(source.id).bind(kind).fetch_optional(&mut *tx).await?.map(version_from_row)
        };
        let mut snapshots = [
            source.input_snapshot.clone(),
            source.model_snapshot.clone(),
            source.parameter_snapshot.clone(),
            source.prompt_snapshot.clone(),
            source.timeline_snapshot.clone(),
        ];
        if let Some(target) = &existing {
            snapshots = [
                target.input_snapshot.clone(),
                target.model_snapshot.clone(),
                target.parameter_snapshot.clone(),
                target.prompt_snapshot.clone(),
                target.timeline_snapshot.clone(),
            ];
        }
        for (target, patch) in snapshots.iter_mut().zip(patches) {
            if let Some(patch) = patch {
                merge_structured_patch(target, patch);
            }
        }
        let record = if let Some(target) = existing {
            let row = sqlx::query("UPDATE work_versions SET input_snapshot=$2,model_snapshot=$3,parameter_snapshot=$4,prompt_snapshot=$5,timeline_snapshot=$6 WHERE id=$1 RETURNING id,work_id,version_no,status,source_version_id,derivation_kind,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot,created_at,updated_at,completed_at")
                .bind(target.id).bind(&snapshots[0]).bind(&snapshots[1]).bind(&snapshots[2]).bind(&snapshots[3]).bind(&snapshots[4]).fetch_one(&mut *tx).await?;
            let record = version_from_row(row);
            update_derived_plan(&mut tx, &record).await?;
            record
        } else {
            let row = sqlx::query("INSERT INTO work_versions (work_id,version_no,status,source_version_id,derivation_kind,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot) VALUES ($1,COALESCE((SELECT MAX(version_no)+1 FROM work_versions WHERE work_id=$1),1),'draft',$2,$3,$4,$5,$6,$7,$8,$9) RETURNING id,work_id,version_no,status,source_version_id,derivation_kind,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot,created_at,updated_at,completed_at")
                .bind(source.work_id).bind(source.id).bind(kind).bind(&source.source_manifest_version).bind(&snapshots[0]).bind(&snapshots[1]).bind(&snapshots[2]).bind(&snapshots[3]).bind(&snapshots[4]).fetch_one(&mut *tx).await?;
            let record = version_from_row(row);
            clone_source_plan(&mut tx, &source, &record).await?;
            record
        };
        sqlx::query(
            "UPDATE works SET current_version_id=$2,status='draft',updated_at=NOW() WHERE id=$1",
        )
        .bind(record.work_id)
        .bind(record.id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((record, true))
    }

    /// 在一个事务内修改当前作品草稿并生成差异，避免 Agent 返回失败时留下半完成草稿。
    pub async fn apply_agent_edit(
        &self,
        work_id: Uuid,
        project_id: Uuid,
        patches: [&Option<Value>; 5],
    ) -> Result<(WorkVersionRecord, WorkVersionDiffPlanRecord), WorkLibraryRepositoryError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(
            "SELECT v.id,v.work_id,v.version_no,v.status,v.source_version_id,v.derivation_kind,
                    v.source_manifest_version,v.input_snapshot,v.model_snapshot,v.parameter_snapshot,
                    v.prompt_snapshot,v.timeline_snapshot,v.created_at,v.updated_at,v.completed_at,
                    w.archived_at
             FROM works w
             JOIN work_versions v ON v.id=w.current_version_id
             WHERE w.id=$1 AND w.project_id=$2
             FOR UPDATE OF w,v",
        )
        .bind(work_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            WorkLibraryRepositoryError::Conflict("作品不存在或不属于当前项目".to_string())
        })?;
        if row.get::<Option<DateTime<Utc>>, _>("archived_at").is_some() {
            return Err(WorkLibraryRepositoryError::Conflict(
                "归档作品必须先恢复后再修改".to_string(),
            ));
        }
        let current = version_from_row(row);
        if current.status != "draft" || current.source_version_id.is_none() {
            return Err(WorkLibraryRepositoryError::Conflict(
                "作品 Agent 只能修改已有来源的当前草稿".to_string(),
            ));
        }

        let mut snapshots = [
            current.input_snapshot.clone(),
            current.model_snapshot.clone(),
            current.parameter_snapshot.clone(),
            current.prompt_snapshot.clone(),
            current.timeline_snapshot.clone(),
        ];
        let original = snapshots.clone();
        for (target, patch) in snapshots.iter_mut().zip(patches) {
            if let Some(patch) = patch {
                merge_structured_patch(target, patch);
            }
        }
        if snapshots == original {
            return Err(WorkLibraryRepositoryError::Conflict(
                "作品 Agent 补丁没有产生实际变化".to_string(),
            ));
        }

        let row = sqlx::query(
            "UPDATE work_versions
             SET input_snapshot=$2,model_snapshot=$3,parameter_snapshot=$4,
                 prompt_snapshot=$5,timeline_snapshot=$6,updated_at=NOW()
             WHERE id=$1
             RETURNING id,work_id,version_no,status,source_version_id,derivation_kind,
                       source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,
                       prompt_snapshot,timeline_snapshot,created_at,updated_at,completed_at",
        )
        .bind(current.id)
        .bind(&snapshots[0])
        .bind(&snapshots[1])
        .bind(&snapshots[2])
        .bind(&snapshots[3])
        .bind(&snapshots[4])
        .fetch_one(&mut *tx)
        .await?;
        let draft = version_from_row(row);
        update_derived_plan(&mut tx, &draft).await?;
        let diff = analyze_diff_in_transaction(&mut tx, draft.id).await?;
        tx.commit().await?;
        Ok((draft, diff))
    }

    pub async fn work_belongs_to_project(
        &self,
        work_id: Uuid,
        project_id: Uuid,
    ) -> Result<bool, WorkLibraryRepositoryError> {
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM works WHERE id=$1 AND project_id=$2)",
        )
        .bind(work_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn work_agent_context(
        &self,
        work_id: Uuid,
        project_id: Uuid,
    ) -> Result<Value, WorkLibraryRepositoryError> {
        let row = sqlx::query(
            "SELECT w.title,v.id,v.version_no,v.status,v.source_version_id,v.derivation_kind,
                    v.input_snapshot,v.model_snapshot,v.parameter_snapshot,v.prompt_snapshot,
                    v.timeline_snapshot
             FROM works w
             JOIN work_versions v ON v.id=w.current_version_id
             WHERE w.id=$1 AND w.project_id=$2 AND w.archived_at IS NULL",
        )
        .bind(work_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| {
            WorkLibraryRepositoryError::Conflict("作品不存在、不属于当前项目或已经归档".to_string())
        })?;
        Ok(json!({
            "work_id": work_id,
            "title": row.get::<String, _>("title"),
            "current_version": {
                "id": row.get::<Uuid, _>("id"),
                "version_no": row.get::<i32, _>("version_no"),
                "status": row.get::<String, _>("status"),
                "source_version_id": row.get::<Option<Uuid>, _>("source_version_id"),
                "derivation_kind": row.get::<String, _>("derivation_kind"),
                "input_snapshot": row.get::<Value, _>("input_snapshot"),
                "model_snapshot": row.get::<Value, _>("model_snapshot"),
                "parameter_snapshot": row.get::<Value, _>("parameter_snapshot"),
                "prompt_snapshot": row.get::<Value, _>("prompt_snapshot"),
                "timeline_snapshot": row.get::<Value, _>("timeline_snapshot"),
            }
        }))
    }

    pub async fn analyze_diff(
        &self,
        draft_id: Uuid,
    ) -> Result<WorkVersionDiffPlanRecord, WorkLibraryRepositoryError> {
        let mut tx = self.pool.begin().await?;
        let result = analyze_diff_in_transaction(&mut tx, draft_id).await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn confirm_diff(
        &self,
        diff_id: Uuid,
        key: &str,
    ) -> Result<WorkDiffConfirmation, WorkLibraryRepositoryError> {
        let mut tx = self.pool.begin().await?;
        if let Some(row)=sqlx::query("SELECT diff_plan_id,generation_run_id FROM work_diff_confirmations WHERE idempotency_key=$1").bind(key).fetch_optional(&mut *tx).await?{
            if row.get::<Uuid,_>("diff_plan_id")!=diff_id{return Err(WorkLibraryRepositoryError::Conflict("Idempotency-Key 已绑定其他差异计划".into()));}
            tx.rollback().await?; return Ok(WorkDiffConfirmation{run_id:row.get("generation_run_id"),diff_plan_id:diff_id,created:false});
        }
        let plan_row=sqlx::query("SELECT id,work_id,source_version_id,draft_version_id,plan_version,source_fingerprint,draft_fingerprint,changes,affected_nodes,reused_artifact_ids,resource_usage,status,created_at FROM work_version_diff_plans WHERE id=$1 FOR UPDATE")
            .bind(diff_id).fetch_optional(&mut *tx).await?.ok_or_else(||WorkLibraryRepositoryError::NotFound(diff_id.to_string()))?;
        let plan = diff_plan_from_row(plan_row);
        if plan.status != "analyzed" {
            return Err(WorkLibraryRepositoryError::Conflict(
                "差异计划已确认或失效".into(),
            ));
        }
        let source = load_version(&mut tx, plan.source_version_id, false).await?;
        let draft = load_version(&mut tx, plan.draft_version_id, true).await?;
        if draft.status != "draft"
            || version_fingerprint(&source) != plan.source_fingerprint
            || version_fingerprint(&draft) != plan.draft_fingerprint
        {
            return Err(WorkLibraryRepositoryError::StaleDiff);
        }
        let work_plan=sqlx::query("SELECT id,capability_snapshot,resource_usage FROM work_plans WHERE work_version_id=$1 AND status='ready' ORDER BY plan_version DESC LIMIT 1 FOR UPDATE")
            .bind(draft.id).fetch_optional(&mut *tx).await?.ok_or_else(||WorkLibraryRepositoryError::Conflict("草稿缺少可确认的作品计划".into()))?;
        let work_plan_id: Uuid = work_plan.get("id");
        let run_id = Uuid::new_v4();
        sqlx::query("INSERT INTO work_generation_runs (id,work_id,work_version_id,work_plan_id,idempotency_key,status,model_snapshot,capability_snapshot,voice_snapshot,prompt_snapshot,timeline_snapshot,parameter_snapshot,resource_usage) VALUES ($1,$2,$3,$4,$5,'queued',$6,$7,$8,$9,$10,$11,$12)")
            .bind(run_id).bind(draft.work_id).bind(draft.id).bind(work_plan_id).bind(key).bind(&draft.model_snapshot).bind(work_plan.get::<Value,_>("capability_snapshot")).bind(draft.timeline_snapshot.get("voice_snapshot").cloned().unwrap_or(json!({}))).bind(&draft.prompt_snapshot).bind(&draft.timeline_snapshot).bind(&draft.parameter_snapshot).bind(&plan.resource_usage).execute(&mut *tx).await?;
        seed_diff_steps(&mut tx, run_id, &plan, &draft).await?;
        sqlx::query("UPDATE work_plans SET status='confirmed' WHERE id=$1")
            .bind(work_plan_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE work_versions SET status='running' WHERE id=$1")
            .bind(draft.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE works SET status='running',current_version_id=$2,updated_at=NOW() WHERE id=$1",
        )
        .bind(draft.work_id)
        .bind(draft.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE work_version_diff_plans SET status='confirmed',confirmed_at=NOW() WHERE id=$1",
        )
        .bind(diff_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO work_diff_confirmations (diff_plan_id,idempotency_key,generation_run_id) VALUES ($1,$2,$3)").bind(diff_id).bind(key).bind(run_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(WorkDiffConfirmation {
            run_id,
            diff_plan_id: diff_id,
            created: true,
        })
    }

    pub async fn delete_blank_work(&self, work_id: Uuid) -> Result<(), WorkLibraryRepositoryError> {
        let mut tx = self.pool.begin().await?;
        let work = sqlx::query("SELECT id FROM works WHERE id=$1 FOR UPDATE")
            .bind(work_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| WorkLibraryRepositoryError::NotFound(work_id.to_string()))?;
        let _ = work;
        let allowed=sqlx::query_scalar::<_,bool>("SELECT COUNT(*)=1 AND BOOL_AND(v.status='draft' AND v.input_snapshot='{}'::jsonb AND v.model_snapshot='{}'::jsonb AND v.parameter_snapshot='{}'::jsonb AND v.prompt_snapshot='{}'::jsonb AND v.timeline_snapshot='{}'::jsonb) AND NOT EXISTS(SELECT 1 FROM work_plans p WHERE p.work_id=$1) AND NOT EXISTS(SELECT 1 FROM work_generation_runs r WHERE r.work_id=$1) AND NOT EXISTS(SELECT 1 FROM work_artifacts a JOIN work_versions av ON av.id=a.work_version_id WHERE av.work_id=$1) AND NOT EXISTS(SELECT 1 FROM publication_handoffs h WHERE h.work_id=$1) FROM work_versions v WHERE v.work_id=$1")
            .bind(work_id).fetch_one(&mut *tx).await?;
        if !allowed {
            return Err(WorkLibraryRepositoryError::Conflict(
                "作品已有配置、运行、provider attempt、artifact 或发布交接，只能归档".into(),
            ));
        }
        sqlx::query("DELETE FROM works WHERE id=$1")
            .bind(work_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn set_archived(
        &self,
        work_id: Uuid,
        archived: bool,
    ) -> Result<Value, WorkLibraryRepositoryError> {
        let row=if archived{
            sqlx::query("UPDATE works SET archived_at=COALESCE(archived_at,NOW()),updated_at=NOW() WHERE id=$1 RETURNING id,title,status,archived_at")
                .bind(work_id).fetch_optional(&self.pool).await?
        }else{
            sqlx::query("UPDATE works SET archived_at=NULL,updated_at=NOW() WHERE id=$1 RETURNING id,title,status,archived_at")
                .bind(work_id).fetch_optional(&self.pool).await?
        }.ok_or_else(||WorkLibraryRepositoryError::NotFound(work_id.to_string()))?;
        let status = if archived {
            "archived".to_string()
        } else {
            row.get::<String, _>("status")
        };
        Ok(
            json!({"id":row.get::<Uuid,_>("id"),"title":row.get::<String,_>("title"),"status":status,"archived":archived}),
        )
    }

    pub async fn artifact(
        &self,
        id: Uuid,
    ) -> Result<WorkArtifactRecord, WorkLibraryRepositoryError> {
        let row=sqlx::query("SELECT a.id,a.work_version_id,v.status AS version_status,a.role,a.material_id,a.file_name,a.storage_path,a.mime_type,a.size_bytes,a.sha256,a.metadata FROM work_artifacts a JOIN work_versions v ON v.id=a.work_version_id WHERE a.id=$1")
            .bind(id).fetch_optional(&self.pool).await?.ok_or_else(||WorkLibraryRepositoryError::NotFound(id.to_string()))?;
        Ok(artifact_from_row(row))
    }

    pub async fn version_artifacts(
        &self,
        version_id: Uuid,
    ) -> Result<Vec<WorkArtifactRecord>, WorkLibraryRepositoryError> {
        let exists =
            sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM work_versions WHERE id=$1)")
                .bind(version_id)
                .fetch_one(&self.pool)
                .await?;
        if !exists {
            return Err(WorkLibraryRepositoryError::NotFound(version_id.to_string()));
        }
        Ok(sqlx::query("SELECT a.id,a.work_version_id,v.status AS version_status,a.role,a.material_id,a.file_name,a.storage_path,a.mime_type,a.size_bytes,a.sha256,a.metadata FROM work_artifacts a JOIN work_versions v ON v.id=a.work_version_id WHERE a.work_version_id=$1 ORDER BY a.role,a.created_at")
            .bind(version_id).fetch_all(&self.pool).await?.into_iter().map(artifact_from_row).collect())
    }

    pub async fn version_package(
        &self,
        version_id: Uuid,
    ) -> Result<Value, WorkLibraryRepositoryError> {
        let version=sqlx::query("SELECT id,work_id,version_no,status,source_version_id,derivation_kind,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot,created_at,updated_at,completed_at FROM work_versions WHERE id=$1")
            .bind(version_id).fetch_optional(&self.pool).await?.map(version_from_row).ok_or_else(||WorkLibraryRepositoryError::NotFound(version_id.to_string()))?;
        if version.status != "completed" {
            return Err(WorkLibraryRepositoryError::Conflict(
                "只有完成版本可以下载制作包".into(),
            ));
        }
        let timeline=sqlx::query("SELECT video_tracks,audio_tracks,subtitle_tracks FROM work_timelines WHERE work_version_id=$1").bind(version_id).fetch_optional(&self.pool).await?;
        let artifacts = self.version_artifacts(version_id).await?;
        Ok(
            json!({"schema":"novex-work-package/v1","version":version,"timeline":timeline.map(|row|json!({"video":row.get::<Value,_>("video_tracks"),"audio":row.get::<Value,_>("audio_tracks"),"subtitles":row.get::<Value,_>("subtitle_tracks")})),"files":artifacts}),
        )
    }

    pub async fn create_handoff(
        &self,
        version_id: Uuid,
        key: &str,
    ) -> Result<WorkPublicationHandoff, WorkLibraryRepositoryError> {
        let mut tx = self.pool.begin().await?;
        if let Some(row)=sqlx::query("SELECT id,work_id,work_version_id,final_video_artifact_id,subtitle_artifact_id,status,payload,created_at FROM publication_handoffs WHERE work_version_id=$1 AND idempotency_key=$2").bind(version_id).bind(key).fetch_optional(&mut *tx).await?{tx.rollback().await?;return Ok(handoff_from_row(row,false));}
        let version =
            sqlx::query("SELECT id,work_id,status FROM work_versions WHERE id=$1 FOR UPDATE")
                .bind(version_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| WorkLibraryRepositoryError::NotFound(version_id.to_string()))?;
        if version.get::<String, _>("status") != "completed" {
            return Err(WorkLibraryRepositoryError::Conflict(
                "只有完成版本可以进入发布运营".into(),
            ));
        }
        let artifacts=sqlx::query("SELECT id,role FROM work_artifacts WHERE work_version_id=$1 AND role IN ('final_video','subtitle') ORDER BY created_at").bind(version_id).fetch_all(&mut *tx).await?;
        let final_id = artifacts
            .iter()
            .find(|row| row.get::<String, _>("role") == "final_video")
            .map(|row| row.get::<Uuid, _>("id"))
            .ok_or_else(|| {
                WorkLibraryRepositoryError::Conflict("完成版本缺少成片 artifact".into())
            })?;
        let subtitle_id = artifacts
            .iter()
            .find(|row| row.get::<String, _>("role") == "subtitle")
            .map(|row| row.get::<Uuid, _>("id"));
        let work_id: Uuid = version.get("work_id");
        let payload = json!({"work_id":work_id,"work_version_id":version_id,"final_video_artifact_id":final_id,"subtitle_artifact_id":subtitle_id});
        let row=sqlx::query("INSERT INTO publication_handoffs (work_id,work_version_id,final_video_artifact_id,subtitle_artifact_id,status,idempotency_key,payload) VALUES ($1,$2,$3,$4,'draft',$5,$6) RETURNING id,work_id,work_version_id,final_video_artifact_id,subtitle_artifact_id,status,payload,created_at")
            .bind(work_id).bind(version_id).bind(final_id).bind(subtitle_id).bind(key).bind(payload).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok(handoff_from_row(row, true))
    }
}

fn version_from_row(row: sqlx::postgres::PgRow) -> WorkVersionRecord {
    WorkVersionRecord {
        id: row.get("id"),
        work_id: row.get("work_id"),
        version_no: row.get("version_no"),
        status: row.get("status"),
        source_version_id: row.get("source_version_id"),
        derivation_kind: row.get("derivation_kind"),
        source_manifest_version: row.get("source_manifest_version"),
        input_snapshot: row.get("input_snapshot"),
        model_snapshot: row.get("model_snapshot"),
        parameter_snapshot: row.get("parameter_snapshot"),
        prompt_snapshot: row.get("prompt_snapshot"),
        timeline_snapshot: row.get("timeline_snapshot"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        completed_at: row.get("completed_at"),
    }
}
fn artifact_from_row(row: sqlx::postgres::PgRow) -> WorkArtifactRecord {
    WorkArtifactRecord {
        id: row.get("id"),
        work_version_id: row.get("work_version_id"),
        version_status: row.get("version_status"),
        role: row.get("role"),
        material_id: row.get("material_id"),
        file_name: row.get("file_name"),
        storage_path: row.get("storage_path"),
        mime_type: row.get("mime_type"),
        size_bytes: row.get("size_bytes"),
        sha256: row.get::<String, _>("sha256").trim().into(),
        metadata: row.get("metadata"),
    }
}
fn diff_plan_from_row(row: sqlx::postgres::PgRow) -> WorkVersionDiffPlanRecord {
    WorkVersionDiffPlanRecord {
        id: row.get("id"),
        work_id: row.get("work_id"),
        source_version_id: row.get("source_version_id"),
        draft_version_id: row.get("draft_version_id"),
        plan_version: row.get("plan_version"),
        source_fingerprint: row.get::<String, _>("source_fingerprint").trim().into(),
        draft_fingerprint: row.get::<String, _>("draft_fingerprint").trim().into(),
        changes: row.get("changes"),
        affected_nodes: row.get("affected_nodes"),
        reused_artifact_ids: row.get("reused_artifact_ids"),
        resource_usage: row.get("resource_usage"),
        status: row.get("status"),
        created_at: row.get("created_at"),
    }
}
fn handoff_from_row(row: sqlx::postgres::PgRow, created: bool) -> WorkPublicationHandoff {
    WorkPublicationHandoff {
        id: row.get("id"),
        work_id: row.get("work_id"),
        work_version_id: row.get("work_version_id"),
        final_video_artifact_id: row.get("final_video_artifact_id"),
        subtitle_artifact_id: row.get("subtitle_artifact_id"),
        status: row.get("status"),
        payload: row.get("payload"),
        created_at: row.get("created_at"),
        created,
    }
}

async fn load_version(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    id: Uuid,
    lock: bool,
) -> Result<WorkVersionRecord, WorkLibraryRepositoryError> {
    let suffix = if lock { " FOR UPDATE" } else { "" };
    let query=format!("SELECT id,work_id,version_no,status,source_version_id,derivation_kind,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot,created_at,updated_at,completed_at FROM work_versions WHERE id=$1{suffix}");
    sqlx::query(&query)
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .map(version_from_row)
        .ok_or_else(|| WorkLibraryRepositoryError::NotFound(id.to_string()))
}

async fn analyze_diff_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    draft_id: Uuid,
) -> Result<WorkVersionDiffPlanRecord, WorkLibraryRepositoryError> {
    let draft = load_version(tx, draft_id, true).await?;
    if draft.status != "draft" {
        return Err(WorkLibraryRepositoryError::Conflict(
            "只有 draft 版本可以进行影响分析".into(),
        ));
    }
    let source_id = draft
        .source_version_id
        .ok_or_else(|| WorkLibraryRepositoryError::Conflict("草稿缺少来源版本".into()))?;
    let source = load_version(tx, source_id, false).await?;
    let mut diff = analyze_version_diff(
        &source
            .domain()
            .map_err(|error| WorkLibraryRepositoryError::Conflict(error.to_string()))?,
        &draft
            .domain()
            .map_err(|error| WorkLibraryRepositoryError::Conflict(error.to_string()))?,
    )
    .map_err(|error| WorkLibraryRepositoryError::Conflict(error.to_string()))?;
    if draft.derivation_kind == "full_regeneration" {
        force_full_regeneration(&draft, &mut diff);
    }
    if diff.affected_nodes.is_empty() {
        return Err(WorkLibraryRepositoryError::Conflict(
            "草稿与来源版本没有可执行差异".into(),
        ));
    }
    let source_fingerprint = version_fingerprint(&source);
    let draft_fingerprint = version_fingerprint(&draft);
    let reused = sqlx::query(
        "SELECT id,role,metadata FROM work_artifacts WHERE work_version_id=$1 ORDER BY created_at",
    )
    .bind(source.id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .filter_map(|row| {
        let role: String = row.get("role");
        let metadata: Value = row.get("metadata");
        let node = metadata
            .get("dag_node")
            .and_then(Value::as_str)
            .unwrap_or_else(|| artifact_default_node(&role));
        (!diff.affected_nodes.iter().any(|affected| affected == node))
            .then(|| row.get::<Uuid, _>("id"))
    })
    .collect::<Vec<_>>();
    sqlx::query(
        "UPDATE work_version_diff_plans SET status='invalidated' WHERE draft_version_id=$1 AND status='analyzed'",
    )
    .bind(draft.id)
    .execute(&mut **tx)
    .await?;
    let plan_version = sqlx::query_scalar::<_, i32>(
        "SELECT COALESCE(MAX(plan_version),0)+1 FROM work_version_diff_plans WHERE draft_version_id=$1",
    )
    .bind(draft.id)
    .fetch_one(&mut **tx)
    .await?;
    let row = sqlx::query(
        "INSERT INTO work_version_diff_plans
            (work_id,source_version_id,draft_version_id,plan_version,source_fingerprint,
             draft_fingerprint,changes,affected_nodes,reused_artifact_ids,resource_usage)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
         RETURNING id,work_id,source_version_id,draft_version_id,plan_version,
                   source_fingerprint,draft_fingerprint,changes,affected_nodes,
                   reused_artifact_ids,resource_usage,status,created_at",
    )
    .bind(draft.work_id)
    .bind(source.id)
    .bind(draft.id)
    .bind(plan_version)
    .bind(source_fingerprint)
    .bind(draft_fingerprint)
    .bind(serde_json::to_value(diff.changes).unwrap_or(json!([])))
    .bind(json!(diff.affected_nodes))
    .bind(json!(reused))
    .bind(serde_json::to_value(diff.resource_usage).unwrap_or(json!({})))
    .fetch_one(&mut **tx)
    .await?;
    Ok(diff_plan_from_row(row))
}

fn merge_structured_patch(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                if value.is_null() {
                    target.remove(key);
                } else if let Ok(index) = key.parse::<usize>() {
                    let _ = index;
                    target.insert(key.clone(), value.clone());
                } else {
                    merge_structured_patch(target.entry(key.clone()).or_insert(Value::Null), value);
                }
            }
        }
        (Value::Array(target), Value::Object(patch)) => {
            for (key, value) in patch {
                if let Ok(index) = key.parse::<usize>() {
                    if let Some(item) = target.get_mut(index) {
                        merge_structured_patch(item, value);
                    }
                }
            }
        }
        (target, patch) => *target = patch.clone(),
    }
}

fn version_fingerprint(version: &WorkVersionRecord) -> String {
    let value = json!([
        version.input_snapshot,
        version.model_snapshot,
        version.parameter_snapshot,
        version.prompt_snapshot,
        version.timeline_snapshot
    ]);
    format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&value).unwrap_or_default())
    )
}
fn artifact_default_node(role: &str) -> &str {
    match role {
        "final_video" | "production_package" => "compose",
        "subtitle" => "subtitle",
        "mix" => "mix",
        "audio_track" => "tts",
        _ => "unknown",
    }
}
fn force_full_regeneration(
    version: &WorkVersionRecord,
    diff: &mut crate::domain::work_library::WorkVersionDiff,
) {
    let scenes = version
        .input_snapshot
        .get("scenes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut nodes = scenes
        .iter()
        .enumerate()
        .map(|(i, scene)| {
            format!(
                "video_segment:{}",
                scene
                    .get("id")
                    .or_else(|| scene.get("scene_id"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| (i + 1).to_string())
            )
        })
        .collect::<Vec<_>>();
    let mode = version
        .timeline_snapshot
        .get("audio_mode")
        .and_then(Value::as_str)
        .unwrap_or("independent_tts");
    if mode.contains("tts") {
        nodes.insert(0, "tts".into());
    }
    if mode == "seedance_original" {
        nodes.insert(0, "asr".into());
    }
    nodes.extend(["subtitle".into(), "mix".into(), "compose".into()]);
    diff.affected_nodes = nodes;
    diff.reused_nodes.clear();
    diff.resource_usage.video_task_count = scenes.len();
    diff.resource_usage.video_seconds = version
        .prompt_snapshot
        .get("segments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|s| s.get("duration_seconds").and_then(Value::as_u64))
        .sum();
    diff.resource_usage.tts_characters = if mode.contains("tts") {
        version.input_snapshot.to_string().chars().count()
    } else {
        0
    };
    diff.resource_usage.asr_seconds = if mode == "seedance_original" {
        diff.resource_usage.video_seconds
    } else {
        0
    };
    diff.changes
        .push(crate::domain::work_library::VersionFieldChange {
            path: "derivation_kind".into(),
            old_value: json!("completed"),
            new_value: json!("full_regeneration"),
        });
}

async fn clone_source_plan(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    source: &WorkVersionRecord,
    draft: &WorkVersionRecord,
) -> Result<(), WorkLibraryRepositoryError> {
    let Some(row)=sqlx::query("SELECT llm_model_id,video_model_id,tts_model_id,capability_snapshot,resource_usage,warnings FROM work_plans WHERE work_version_id=$1 ORDER BY plan_version DESC LIMIT 1").bind(source.id).fetch_optional(&mut **tx).await? else{return Ok(())};
    let plan_version = sqlx::query_scalar::<_, i32>(
        "SELECT COALESCE(MAX(plan_version),0)+1 FROM work_plans WHERE work_id=$1",
    )
    .bind(draft.work_id)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query("INSERT INTO work_plans (work_id,work_version_id,plan_version,status,input_fingerprint,llm_model_id,video_model_id,tts_model_id,capability_snapshot,output_snapshot,prompt_snapshot,timeline_snapshot,resource_usage,warnings) VALUES ($1,$2,$3,'ready',$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)")
        .bind(draft.work_id).bind(draft.id).bind(plan_version).bind(version_fingerprint(draft)).bind(row.get::<Option<Uuid>,_>("llm_model_id")).bind(row.get::<Option<Uuid>,_>("video_model_id")).bind(row.get::<Option<Uuid>,_>("tts_model_id")).bind(row.get::<Value,_>("capability_snapshot")).bind(&draft.parameter_snapshot).bind(&draft.prompt_snapshot).bind(&draft.timeline_snapshot).bind(row.get::<Value,_>("resource_usage")).bind(row.get::<Value,_>("warnings")).execute(&mut **tx).await?;
    Ok(())
}
async fn update_derived_plan(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    draft: &WorkVersionRecord,
) -> Result<(), WorkLibraryRepositoryError> {
    sqlx::query("UPDATE work_plans SET input_fingerprint=$2,output_snapshot=$3,prompt_snapshot=$4,timeline_snapshot=$5,updated_at=NOW() WHERE work_version_id=$1 AND status='ready'").bind(draft.id).bind(version_fingerprint(draft)).bind(&draft.parameter_snapshot).bind(&draft.prompt_snapshot).bind(&draft.timeline_snapshot).execute(&mut **tx).await?;
    Ok(())
}

async fn seed_diff_steps(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_id: Uuid,
    plan: &WorkVersionDiffPlanRecord,
    draft: &WorkVersionRecord,
) -> Result<(), WorkLibraryRepositoryError> {
    let plan_step:Uuid=sqlx::query_scalar("INSERT INTO work_generation_steps (run_id,step_no,step_type,status,is_required,depends_on,input_snapshot,model_snapshot,resource_usage) VALUES ($1,1,'plan','succeeded',TRUE,'[]','{}','{}','{}') RETURNING id").bind(run_id).fetch_one(&mut **tx).await?;
    let nodes = plan.affected_nodes.as_array().cloned().unwrap_or_default();
    let mut previous = plan_step;
    for (index, node) in nodes.iter().filter_map(Value::as_str).enumerate() {
        let step_type = if node.starts_with("video_segment:") {
            "video_segment"
        } else {
            node
        };
        let input = if let Some(scene_id) = node.strip_prefix("video_segment:") {
            json!({"scene_id":scene_id,"version_id":draft.id,"reused_artifact_ids":plan.reused_artifact_ids})
        } else {
            json!({"version_id":draft.id,"reused_artifact_ids":plan.reused_artifact_ids})
        };
        let usage = if step_type == "video_segment" {
            json!({"video_seconds":plan.resource_usage.get("video_seconds")})
        } else {
            json!({})
        };
        previous=sqlx::query_scalar("INSERT INTO work_generation_steps (run_id,step_no,step_type,status,is_required,depends_on,input_snapshot,model_snapshot,resource_usage) VALUES ($1,$2,$3,'queued',TRUE,$4,$5,$6,$7) RETURNING id").bind(run_id).bind(index as i32+2).bind(step_type).bind(json!([previous])).bind(input).bind(&draft.model_snapshot).bind(usage).fetch_one(&mut **tx).await?;
    }
    Ok(())
}
