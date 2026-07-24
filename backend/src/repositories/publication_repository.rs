use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PublicationPlanRecord {
    pub id: Uuid,
    pub handoff_id: Uuid,
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub final_video_artifact_id: Uuid,
    pub subtitle_artifact_id: Option<Uuid>,
    pub targets: Value,
    pub created: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PublicationTargetRecord {
    pub id: Uuid,
    pub publication_plan_id: Uuid,
    pub platform: String,
    pub status: String,
    pub title: String,
    pub body: String,
    pub tags: Value,
    pub cover_artifact_id: Option<Uuid>,
    pub planned_at: Option<DateTime<Utc>>,
    pub draft_revision: i32,
    pub handed_off_at: Option<DateTime<Utc>>,
    pub published_at: Option<DateTime<Utc>>,
    pub published_url: Option<String>,
    pub result_snapshot: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct SavePublicationTarget {
    pub title: String,
    pub body: String,
    pub tags: Value,
    pub cover_artifact_id: Option<Uuid>,
    pub planned_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PublicationPackageContext {
    pub target: PublicationTargetRecord,
    pub work_id: Uuid,
    pub work_version_id: Uuid,
    pub work_title: String,
    pub final_video_artifact_id: Uuid,
    pub subtitle_artifact_id: Option<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PublicationPackageRecord {
    pub id: Uuid,
    pub publication_target_id: Uuid,
    pub draft_revision: i32,
    pub platform_rule_version: String,
    pub manifest: Value,
    pub manifest_sha256: String,
    pub package_storage_path: String,
    pub created_at: DateTime<Utc>,
    pub created: bool,
}

#[derive(Debug)]
pub enum PublicationRepositoryError {
    Database(sqlx::Error),
    NotFound(String),
    Conflict(String),
}

impl fmt::Display for PublicationRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(error) => write!(formatter, "数据库错误: {error}"),
            Self::NotFound(value) => write!(formatter, "发布资源不存在: {value}"),
            Self::Conflict(value) => write!(formatter, "发布状态冲突: {value}"),
        }
    }
}

impl std::error::Error for PublicationRepositoryError {}

impl From<sqlx::Error> for PublicationRepositoryError {
    fn from(value: sqlx::Error) -> Self {
        Self::Database(value)
    }
}

#[derive(Clone)]
pub struct PostgresPublicationRepository {
    pool: PgPool,
}

impl PostgresPublicationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn get_or_create_plan(
        &self,
        handoff_id: Uuid,
    ) -> Result<PublicationPlanRecord, PublicationRepositoryError> {
        let mut tx = self.pool.begin().await?;
        let handoff = sqlx::query("SELECT work_id,work_version_id,final_video_artifact_id,subtitle_artifact_id FROM publication_handoffs WHERE id=$1")
            .bind(handoff_id).fetch_optional(&mut *tx).await?
            .ok_or_else(|| PublicationRepositoryError::NotFound(handoff_id.to_string()))?;
        let existing =
            sqlx::query_scalar::<_, Uuid>("SELECT id FROM publication_plans WHERE handoff_id=$1")
                .bind(handoff_id)
                .fetch_optional(&mut *tx)
                .await?;
        let (id, created) = match existing {
            Some(id) => (id, false),
            None => (
                sqlx::query_scalar(
                    "INSERT INTO publication_plans (handoff_id) VALUES ($1) RETURNING id",
                )
                .bind(handoff_id)
                .fetch_one(&mut *tx)
                .await?,
                true,
            ),
        };
        let targets = target_rows(&mut tx, id).await?;
        tx.commit().await?;
        Ok(PublicationPlanRecord {
            id,
            handoff_id,
            work_id: handoff.get("work_id"),
            work_version_id: handoff.get("work_version_id"),
            final_video_artifact_id: handoff.get("final_video_artifact_id"),
            subtitle_artifact_id: handoff.get("subtitle_artifact_id"),
            targets: json!(targets),
            created,
        })
    }

    pub async fn save_target(
        &self,
        plan_id: Uuid,
        platform: &str,
        expected_revision: Option<i32>,
        idempotency_key: &str,
        input: SavePublicationTarget,
    ) -> Result<PublicationTargetRecord, PublicationRepositoryError> {
        let mut tx = self.pool.begin().await?;
        let existing = sqlx::query("SELECT id,draft_revision,status FROM publication_targets WHERE publication_plan_id=$1 AND platform=$2 FOR UPDATE")
            .bind(plan_id).bind(platform).fetch_optional(&mut *tx).await?;
        let was_existing = existing.is_some();
        let id: Uuid = if let Some(row) = existing {
            let existing_id: Uuid = row.get("id");
            if let Some(event_type) = sqlx::query_scalar::<_, String>("SELECT event_type FROM publication_events WHERE publication_target_id=$1 AND idempotency_key=$2")
                .bind(existing_id).bind(idempotency_key).fetch_optional(&mut *tx).await? {
                if event_type != "draft_updated" && event_type != "created" { return Err(PublicationRepositoryError::Conflict("Idempotency-Key 已用于其他动作".into())); }
                let result=sqlx::query("SELECT * FROM publication_targets WHERE id=$1").bind(existing_id).fetch_one(&mut *tx).await?;
                tx.rollback().await?;
                return Ok(target_from_row(result));
            }
            let revision: i32 = row.get("draft_revision");
            if expected_revision != Some(revision) {
                return Err(PublicationRepositoryError::Conflict(
                    "草稿 revision 已过期".into(),
                ));
            }
            if matches!(
                row.get::<String, _>("status").as_str(),
                "published" | "cancelled"
            ) {
                return Err(PublicationRepositoryError::Conflict(
                    "终态目标不可修改".into(),
                ));
            }
            sqlx::query_scalar("UPDATE publication_targets SET title=$2,body=$3,tags=$4,cover_artifact_id=$5,planned_at=$6,draft_revision=draft_revision+1,status='draft',handed_off_at=NULL WHERE id=$1 RETURNING id")
                .bind(existing_id).bind(input.title).bind(input.body).bind(input.tags).bind(input.cover_artifact_id).bind(input.planned_at).fetch_one(&mut *tx).await?
        } else {
            sqlx::query_scalar("INSERT INTO publication_targets (publication_plan_id,platform,title,body,tags,cover_artifact_id,planned_at) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id")
                .bind(plan_id).bind(platform).bind(input.title).bind(input.body).bind(input.tags).bind(input.cover_artifact_id).bind(input.planned_at).fetch_one(&mut *tx).await?
        };
        let event_type = if was_existing {
            "draft_updated"
        } else {
            "created"
        };
        sqlx::query("INSERT INTO publication_events(publication_target_id,event_type,idempotency_key) VALUES($1,$2,$3)").bind(id).bind(event_type).bind(idempotency_key).execute(&mut *tx).await?;
        let row = sqlx::query("SELECT * FROM publication_targets WHERE id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(target_from_row(row))
    }

    pub async fn details(&self, id: Uuid) -> Result<Value, PublicationRepositoryError> {
        let row = sqlx::query("SELECT p.id,p.handoff_id,h.work_id,h.work_version_id,h.final_video_artifact_id,h.subtitle_artifact_id,p.created_at,p.updated_at FROM publication_plans p JOIN publication_handoffs h ON h.id=p.handoff_id WHERE p.id=$1")
            .bind(id).fetch_optional(&self.pool).await?.ok_or_else(|| PublicationRepositoryError::NotFound(id.to_string()))?;
        let targets = sqlx::query(
            "SELECT * FROM publication_targets WHERE publication_plan_id=$1 ORDER BY platform",
        )
        .bind(id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(target_from_row)
        .collect::<Vec<_>>();
        Ok(
            json!({"id":row.get::<Uuid,_>("id"),"handoff_id":row.get::<Uuid,_>("handoff_id"),"work_id":row.get::<Uuid,_>("work_id"),"work_version_id":row.get::<Uuid,_>("work_version_id"),"final_video_artifact_id":row.get::<Uuid,_>("final_video_artifact_id"),"subtitle_artifact_id":row.get::<Option<Uuid>,_>("subtitle_artifact_id"),"targets":targets,"created_at":row.get::<DateTime<Utc>,_>("created_at"),"updated_at":row.get::<DateTime<Utc>,_>("updated_at")}),
        )
    }

    pub async fn list(&self) -> Result<Value, PublicationRepositoryError> {
        let rows = sqlx::query("SELECT p.id,p.handoff_id,h.work_id,h.work_version_id,w.title,p.created_at,p.updated_at FROM publication_plans p JOIN publication_handoffs h ON h.id=p.handoff_id JOIN works w ON w.id=h.work_id WHERE p.archived_at IS NULL ORDER BY p.updated_at DESC")
            .fetch_all(&self.pool).await?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let id: Uuid = row.get("id");
            let targets = sqlx::query(
                "SELECT * FROM publication_targets WHERE publication_plan_id=$1 ORDER BY platform",
            )
            .bind(id)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(target_from_row)
            .collect::<Vec<_>>();
            items.push(json!({"id":id,"handoff_id":row.get::<Uuid,_>("handoff_id"),"work_id":row.get::<Uuid,_>("work_id"),"work_version_id":row.get::<Uuid,_>("work_version_id"),"work_title":row.get::<String,_>("title"),"targets":targets,"created_at":row.get::<DateTime<Utc>,_>("created_at"),"updated_at":row.get::<DateTime<Utc>,_>("updated_at")}));
        }
        Ok(json!({"items":items}))
    }

    pub async fn target(
        &self,
        id: Uuid,
    ) -> Result<PublicationTargetRecord, PublicationRepositoryError> {
        sqlx::query("SELECT * FROM publication_targets WHERE id=$1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .map(target_from_row)
            .ok_or_else(|| PublicationRepositoryError::NotFound(id.to_string()))
    }

    pub async fn package_context(
        &self,
        id: Uuid,
    ) -> Result<PublicationPackageContext, PublicationRepositoryError> {
        let row=sqlx::query("SELECT t.*,h.work_id,h.work_version_id,h.final_video_artifact_id,h.subtitle_artifact_id,w.title AS work_title FROM publication_targets t JOIN publication_plans p ON p.id=t.publication_plan_id JOIN publication_handoffs h ON h.id=p.handoff_id JOIN works w ON w.id=h.work_id WHERE t.id=$1").bind(id).fetch_optional(&self.pool).await?.ok_or_else(||PublicationRepositoryError::NotFound(id.to_string()))?;
        let work_id = row.get("work_id");
        let work_version_id = row.get("work_version_id");
        let work_title = row.get("work_title");
        let final_video_artifact_id = row.get("final_video_artifact_id");
        let subtitle_artifact_id = row.get("subtitle_artifact_id");
        let target = target_from_row(row);
        Ok(PublicationPackageContext {
            target,
            work_id,
            work_version_id,
            work_title,
            final_video_artifact_id,
            subtitle_artifact_id,
        })
    }

    pub async fn save_package(
        &self,
        target_id: Uuid,
        revision: i32,
        rule_version: &str,
        manifest: Value,
        hash: &str,
        path: &str,
        key: &str,
    ) -> Result<PublicationPackageRecord, PublicationRepositoryError> {
        let mut tx = self.pool.begin().await?;
        if let Some(row)=sqlx::query("SELECT p.* FROM publication_packages p JOIN publication_events e ON e.publication_target_id=p.publication_target_id AND e.idempotency_key=$3 WHERE p.publication_target_id=$1 AND p.draft_revision=$2").bind(target_id).bind(revision).bind(key).fetch_optional(&mut *tx).await?{tx.rollback().await?;return Ok(package_from_row(row,false));}
        let target = sqlx::query(
            "SELECT draft_revision,status FROM publication_targets WHERE id=$1 FOR UPDATE",
        )
        .bind(target_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| PublicationRepositoryError::NotFound(target_id.to_string()))?;
        let current: i32 = target.get("draft_revision");
        if current != revision {
            return Err(PublicationRepositoryError::Conflict(
                "草稿 revision 已过期".into(),
            ));
        }
        if !matches!(
            target.get::<String, _>("status").as_str(),
            "draft" | "needs_attention" | "ready"
        ) {
            return Err(PublicationRepositoryError::Conflict(
                "当前状态不可生成发布包".into(),
            ));
        }
        if let Some(row)=sqlx::query("SELECT * FROM publication_packages WHERE publication_target_id=$1 AND draft_revision=$2").bind(target_id).bind(revision).fetch_optional(&mut *tx).await?{if row.get::<String,_>("manifest_sha256")!=hash{return Err(PublicationRepositoryError::Conflict("当前 revision 已存在不同发布包".into()));}tx.rollback().await?;return Ok(package_from_row(row,false));}
        let row=sqlx::query("INSERT INTO publication_packages(publication_target_id,draft_revision,platform_rule_version,manifest,manifest_sha256,package_storage_path)VALUES($1,$2,$3,$4,$5,$6)RETURNING *").bind(target_id).bind(revision).bind(rule_version).bind(&manifest).bind(hash).bind(path).fetch_one(&mut *tx).await?;
        sqlx::query("UPDATE publication_targets SET status='ready' WHERE id=$1")
            .bind(target_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO publication_events(publication_target_id,event_type,payload,idempotency_key)VALUES($1,'package_generated',$2,$3)").bind(target_id).bind(json!({"draft_revision":revision,"manifest_sha256":hash})).bind(key).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(package_from_row(row, true))
    }

    pub async fn current_package(
        &self,
        target_id: Uuid,
    ) -> Result<PublicationPackageRecord, PublicationRepositoryError> {
        sqlx::query("SELECT p.* FROM publication_packages p JOIN publication_targets t ON t.id=p.publication_target_id AND t.draft_revision=p.draft_revision WHERE p.publication_target_id=$1").bind(target_id).fetch_optional(&self.pool).await?.map(|r|package_from_row(r,false)).ok_or_else(||PublicationRepositoryError::NotFound("当前发布包不存在".into()))
    }
    pub async fn current_package_by_id(
        &self,
        id: Uuid,
    ) -> Result<PublicationPackageRecord, PublicationRepositoryError> {
        sqlx::query("SELECT p.* FROM publication_packages p JOIN publication_targets t ON t.id=p.publication_target_id AND t.draft_revision=p.draft_revision WHERE p.id=$1").bind(id).fetch_optional(&self.pool).await?.map(|r|package_from_row(r,false)).ok_or_else(||PublicationRepositoryError::NotFound(id.to_string()))
    }

    pub async fn transition(
        &self,
        id: Uuid,
        expected: &[&str],
        next: &str,
        event_type: &str,
        key: &str,
        fields: Value,
    ) -> Result<PublicationTargetRecord, PublicationRepositoryError> {
        let mut tx = self.pool.begin().await?;
        if let Some(existing_type) = sqlx::query_scalar::<_,String>("SELECT event_type FROM publication_events WHERE publication_target_id=$1 AND idempotency_key=$2").bind(id).bind(key).fetch_optional(&mut *tx).await? {
            if existing_type != event_type { return Err(PublicationRepositoryError::Conflict("Idempotency-Key 已用于其他动作".into())); }
            let row=sqlx::query("SELECT * FROM publication_targets WHERE id=$1").bind(id).fetch_one(&mut *tx).await?;
            tx.rollback().await?;
            return Ok(target_from_row(row));
        }
        let row = sqlx::query("SELECT status FROM publication_targets WHERE id=$1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| PublicationRepositoryError::NotFound(id.to_string()))?;
        let status: String = row.get("status");
        if !expected.contains(&status.as_str()) {
            return Err(PublicationRepositoryError::Conflict(format!(
                "状态 {status} 不允许执行该动作"
            )));
        }
        if event_type == "handed_off" {
            let valid=sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM publication_packages p JOIN publication_targets t ON t.id=p.publication_target_id AND t.draft_revision=p.draft_revision WHERE p.publication_target_id=$1)").bind(id).fetch_one(&mut *tx).await?;
            if !valid {
                return Err(PublicationRepositoryError::Conflict(
                    "当前 revision 缺少有效发布包".into(),
                ));
            }
        }
        let published_at = fields
            .get("published_at")
            .and_then(Value::as_str)
            .and_then(|v| v.parse::<DateTime<Utc>>().ok());
        let published_url = fields.get("published_url").and_then(Value::as_str);
        sqlx::query("UPDATE publication_targets SET status=$2,handed_off_at=CASE WHEN $2='handed_off' THEN NOW() ELSE handed_off_at END,published_at=CASE WHEN $2='published' THEN $3 ELSE published_at END,published_url=CASE WHEN $2='published' THEN $4 ELSE published_url END,result_snapshot=CASE WHEN $2='published' THEN $5 ELSE result_snapshot END WHERE id=$1")
            .bind(id).bind(next).bind(published_at).bind(published_url).bind(&fields).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO publication_events (publication_target_id,event_type,payload,idempotency_key) VALUES ($1,$2,$3,$4)").bind(id).bind(event_type).bind(fields).bind(key).execute(&mut *tx).await?;
        let result = sqlx::query("SELECT * FROM publication_targets WHERE id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(target_from_row(result))
    }

    pub async fn correct_result(
        &self,
        id: Uuid,
        url: &str,
        published_at: DateTime<Utc>,
        key: &str,
    ) -> Result<PublicationTargetRecord, PublicationRepositoryError> {
        let mut tx = self.pool.begin().await?;
        if let Some(event_type)=sqlx::query_scalar::<_,String>("SELECT event_type FROM publication_events WHERE publication_target_id=$1 AND idempotency_key=$2").bind(id).bind(key).fetch_optional(&mut *tx).await?{if event_type!="result_corrected"{return Err(PublicationRepositoryError::Conflict("Idempotency-Key 已用于其他动作".into()));}}
        else {
            let status=sqlx::query_scalar::<_,String>("SELECT status FROM publication_targets WHERE id=$1 FOR UPDATE").bind(id).fetch_optional(&mut *tx).await?.ok_or_else(||PublicationRepositoryError::NotFound(id.to_string()))?;
            if status!="published"{return Err(PublicationRepositoryError::Conflict("只有已发布目标可修正结果".into()));}
            let payload=json!({"published_url":url,"published_at":published_at,"confirmation":"manual"});
            sqlx::query("UPDATE publication_targets SET published_url=$2,published_at=$3,result_snapshot=$4 WHERE id=$1").bind(id).bind(url).bind(published_at).bind(&payload).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO publication_events(publication_target_id,event_type,payload,idempotency_key) VALUES($1,'result_corrected',$2,$3)").bind(id).bind(payload).bind(key).execute(&mut *tx).await?;
        }
        let row = sqlx::query("SELECT * FROM publication_targets WHERE id=$1")
            .bind(id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(target_from_row(row))
    }

    pub async fn record_event(
        &self,
        id: Uuid,
        event_type: &str,
        key: &str,
        payload: Value,
    ) -> Result<(), PublicationRepositoryError> {
        if !matches!(event_type, "downloaded" | "copied") {
            return Err(PublicationRepositoryError::Conflict(
                "不支持的审计事件".into(),
            ));
        }
        sqlx::query("INSERT INTO publication_events(publication_target_id,event_type,payload,idempotency_key)VALUES($1,$2,$3,$4) ON CONFLICT(publication_target_id,idempotency_key) DO NOTHING").bind(id).bind(event_type).bind(payload).bind(key).execute(&self.pool).await?;
        Ok(())
    }
}

async fn target_rows(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    plan_id: Uuid,
) -> Result<Vec<PublicationTargetRecord>, sqlx::Error> {
    Ok(sqlx::query(
        "SELECT * FROM publication_targets WHERE publication_plan_id=$1 ORDER BY platform",
    )
    .bind(plan_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(target_from_row)
    .collect())
}

fn target_from_row(row: sqlx::postgres::PgRow) -> PublicationTargetRecord {
    PublicationTargetRecord {
        id: row.get("id"),
        publication_plan_id: row.get("publication_plan_id"),
        platform: row.get("platform"),
        status: row.get("status"),
        title: row.get("title"),
        body: row.get("body"),
        tags: row.get("tags"),
        cover_artifact_id: row.get("cover_artifact_id"),
        planned_at: row.get("planned_at"),
        draft_revision: row.get("draft_revision"),
        handed_off_at: row.get("handed_off_at"),
        published_at: row.get("published_at"),
        published_url: row.get("published_url"),
        result_snapshot: row.get("result_snapshot"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn package_from_row(row: sqlx::postgres::PgRow, created: bool) -> PublicationPackageRecord {
    PublicationPackageRecord {
        id: row.get("id"),
        publication_target_id: row.get("publication_target_id"),
        draft_revision: row.get("draft_revision"),
        platform_rule_version: row.get("platform_rule_version"),
        manifest: row.get("manifest"),
        manifest_sha256: row.get("manifest_sha256"),
        package_storage_path: row.get("package_storage_path"),
        created_at: row.get("created_at"),
        created,
    }
}
