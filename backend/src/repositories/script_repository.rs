use crate::agents::models::{Script, ScriptListFilter, ScriptStatus, ScriptSummary};
use async_trait::async_trait;
use sqlx::{postgres::PgRow, PgPool, Row};
use std::fmt;
use uuid::Uuid;

#[derive(Clone)]
pub struct PostgresScriptRepository {
    pool: PgPool,
}

impl PostgresScriptRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn load_scenes(
        &self,
        script_id: Uuid,
    ) -> Result<Vec<crate::agents::models::Scene>, ScriptRepositoryError> {
        sqlx::query(
            r#"
            SELECT id, sequence, narration, visual_description, emotion, duration_sec
            FROM scenes
            WHERE script_id = $1
            ORDER BY sequence ASC
            "#,
        )
        .bind(script_id)
        .fetch_all(&self.pool)
        .await
        .map_err(ScriptRepositoryError::from)?
        .into_iter()
        .map(scene_from_row)
        .collect()
    }

    async fn script_from_row(&self, row: PgRow) -> Result<Script, ScriptRepositoryError> {
        let script_id: Uuid = row.get("id");
        let status_value: String = row.get("status");
        let status = ScriptStatus::try_from(status_value.as_str())
            .map_err(|error| ScriptRepositoryError::Storage(error.to_string()))?;
        let scenes = self.load_scenes(script_id).await?;

        Ok(Script::new(
            script_id,
            row.get("project_id"),
            row.get("title"),
            row.get("hook"),
            row.get("content"),
            status,
            row.get("parent_id"),
            scenes,
            row.get("created_at"),
            row.get("updated_at"),
        ))
    }
}

#[async_trait]
pub trait ScriptRepository: Send + Sync {
    async fn save_script(&self, script: Script) -> Result<Script, ScriptRepositoryError>;

    async fn get_script(&self, script_id: Uuid) -> Result<Script, ScriptRepositoryError>;

    async fn list_scripts(
        &self,
        project_id: Uuid,
        filter: ScriptListFilter,
    ) -> Result<Vec<Script>, ScriptRepositoryError>;

    async fn list_script_summaries(
        &self,
        project_id: Uuid,
        filter: ScriptListFilter,
    ) -> Result<Vec<ScriptSummary>, ScriptRepositoryError>;

    async fn count_scripts(
        &self,
        project_id: Uuid,
        status: Option<ScriptStatus>,
    ) -> Result<i64, ScriptRepositoryError>;

    async fn update_script_status(
        &self,
        script_id: Uuid,
        status: ScriptStatus,
    ) -> Result<Script, ScriptRepositoryError>;
}

#[async_trait]
impl ScriptRepository for PostgresScriptRepository {
    async fn save_script(&self, script: Script) -> Result<Script, ScriptRepositoryError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(ScriptRepositoryError::from)?;
        let status = script.status.as_str();

        sqlx::query(
            r#"
            INSERT INTO scripts (
                id, project_id, title, hook, content, status, parent_id, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
        )
        .bind(script.id)
        .bind(script.project_id)
        .bind(&script.title)
        .bind(&script.hook)
        .bind(&script.content)
        .bind(status)
        .bind(script.parent_id)
        .bind(script.created_at)
        .bind(script.updated_at)
        .execute(&mut *transaction)
        .await
        .map_err(ScriptRepositoryError::from)?;

        for scene in &script.scenes {
            sqlx::query(
                r#"
                INSERT INTO scenes (
                    id, script_id, sequence, narration, visual_description, emotion, duration_sec
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(scene.id)
            .bind(script.id)
            .bind(scene.sequence)
            .bind(&scene.narration)
            .bind(&scene.visual_description)
            .bind(&scene.emotion)
            .bind(scene.duration_sec)
            .execute(&mut *transaction)
            .await
            .map_err(ScriptRepositoryError::from)?;
        }

        transaction
            .commit()
            .await
            .map_err(ScriptRepositoryError::from)?;
        self.get_script(script.id).await
    }

    async fn get_script(&self, script_id: Uuid) -> Result<Script, ScriptRepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT id, project_id, title, hook, content, status, parent_id, created_at, updated_at
            FROM scripts
            WHERE id = $1
            "#,
        )
        .bind(script_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(ScriptRepositoryError::from)?
        .ok_or(ScriptRepositoryError::NotFound(script_id))?;

        self.script_from_row(row).await
    }

    async fn list_scripts(
        &self,
        project_id: Uuid,
        filter: ScriptListFilter,
    ) -> Result<Vec<Script>, ScriptRepositoryError> {
        let limit = i64::from(filter.limit_or_default());
        let offset = i64::from(filter.offset_or_default());
        let rows = if let Some(status) = filter.status {
            sqlx::query(
                r#"
                SELECT id, project_id, title, hook, content, status, parent_id, created_at, updated_at
                FROM scripts
                WHERE project_id = $1 AND status = $2
                ORDER BY created_at DESC
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(project_id)
            .bind(status.as_str())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(ScriptRepositoryError::from)?
        } else {
            sqlx::query(
                r#"
                SELECT id, project_id, title, hook, content, status, parent_id, created_at, updated_at
                FROM scripts
                WHERE project_id = $1
                ORDER BY created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(project_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(ScriptRepositoryError::from)?
        };

        let mut scripts = Vec::with_capacity(rows.len());
        for row in rows {
            scripts.push(self.script_from_row(row).await?);
        }
        Ok(scripts)
    }

    async fn list_script_summaries(
        &self,
        project_id: Uuid,
        filter: ScriptListFilter,
    ) -> Result<Vec<ScriptSummary>, ScriptRepositoryError> {
        let limit = i64::from(filter.limit_or_default());
        let offset = i64::from(filter.offset_or_default());
        let rows = if let Some(status) = filter.status {
            sqlx::query(
                r#"
                SELECT
                    s.id,
                    s.title,
                    s.status,
                    s.parent_id,
                    s.created_at,
                    COUNT(sc.id) AS scene_count
                FROM scripts s
                LEFT JOIN scenes sc ON sc.script_id = s.id
                WHERE s.project_id = $1 AND s.status = $2
                GROUP BY s.id
                ORDER BY s.created_at DESC
                LIMIT $3 OFFSET $4
                "#,
            )
            .bind(project_id)
            .bind(status.as_str())
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(ScriptRepositoryError::from)?
        } else {
            sqlx::query(
                r#"
                SELECT
                    s.id,
                    s.title,
                    s.status,
                    s.parent_id,
                    s.created_at,
                    COUNT(sc.id) AS scene_count
                FROM scripts s
                LEFT JOIN scenes sc ON sc.script_id = s.id
                WHERE s.project_id = $1
                GROUP BY s.id
                ORDER BY s.created_at DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(project_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(&self.pool)
            .await
            .map_err(ScriptRepositoryError::from)?
        };

        rows.into_iter().map(script_summary_from_row).collect()
    }

    async fn count_scripts(
        &self,
        project_id: Uuid,
        status: Option<ScriptStatus>,
    ) -> Result<i64, ScriptRepositoryError> {
        if let Some(status) = status {
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)
                FROM scripts
                WHERE project_id = $1 AND status = $2
                "#,
            )
            .bind(project_id)
            .bind(status.as_str())
            .fetch_one(&self.pool)
            .await
            .map_err(ScriptRepositoryError::from)
        } else {
            sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)
                FROM scripts
                WHERE project_id = $1
                "#,
            )
            .bind(project_id)
            .fetch_one(&self.pool)
            .await
            .map_err(ScriptRepositoryError::from)
        }
    }

    async fn update_script_status(
        &self,
        script_id: Uuid,
        status: ScriptStatus,
    ) -> Result<Script, ScriptRepositoryError> {
        let updated_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            UPDATE scripts
            SET status = $2
            WHERE id = $1
            RETURNING id
            "#,
        )
        .bind(script_id)
        .bind(status.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(ScriptRepositoryError::from)?
        .ok_or(ScriptRepositoryError::NotFound(script_id))?;

        self.get_script(updated_id).await
    }
}

#[derive(Debug)]
pub enum ScriptRepositoryError {
    NotFound(Uuid),
    Storage(String),
}

impl From<sqlx::Error> for ScriptRepositoryError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for ScriptRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(script_id) => write!(formatter, "script not found: {script_id}"),
            Self::Storage(message) => write!(formatter, "script storage error: {message}"),
        }
    }
}

impl std::error::Error for ScriptRepositoryError {}

fn scene_from_row(row: PgRow) -> Result<crate::agents::models::Scene, ScriptRepositoryError> {
    Ok(crate::agents::models::Scene {
        id: row.get("id"),
        sequence: row.get("sequence"),
        narration: row.get("narration"),
        visual_description: row.get("visual_description"),
        emotion: row.get("emotion"),
        duration_sec: row.get("duration_sec"),
    })
}

fn script_summary_from_row(row: PgRow) -> Result<ScriptSummary, ScriptRepositoryError> {
    let status_value: String = row.get("status");
    let status = ScriptStatus::try_from(status_value.as_str())
        .map_err(|error| ScriptRepositoryError::Storage(error.to_string()))?;

    Ok(ScriptSummary {
        script_id: row.get("id"),
        title: row.get("title"),
        status,
        scene_count: row.get("scene_count"),
        parent_id: row.get("parent_id"),
        created_at: row.get("created_at"),
    })
}
