//! ProductionState Repository：项目和所有产物的数据库 CRUD 层

use crate::error::{ProductionError, ProductionResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

/// 制作项目实体（对应 production_projects 表）
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ProductionProject {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub project_type: String,
    pub status: String,
    pub user_id: Uuid,
    pub metadata: Value,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

pub struct ProductionStateRepository {
    pool: PgPool,
}

impl ProductionStateRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// 创建新制作项目
    pub async fn create_project(
        &self,
        user_id: Uuid,
        title: String,
        description: Option<String>,
        project_type: String,
        initial_input: Value,
    ) -> ProductionResult<ProductionProject> {
        let project = sqlx::query_as!(
            ProductionProject,
            r#"
            INSERT INTO production_projects (title, description, project_type, user_id, metadata)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, title, description, project_type, status, user_id, metadata, created_at, updated_at, deleted_at
            "#,
            title,
            description,
            project_type,
            user_id,
            initial_input,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(ProductionError::Database)?;
        Ok(project)
    }

    /// 查询单个项目
    pub async fn get_project(&self, id: Uuid) -> ProductionResult<ProductionProject> {
        sqlx::query_as!(
            ProductionProject,
            r#"
            SELECT id, title, description, project_type, status, user_id, metadata, created_at, updated_at, deleted_at
            FROM production_projects
            WHERE id = $1 AND deleted_at IS NULL
            "#,
            id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(ProductionError::Database)?
        .ok_or_else(|| ProductionError::ProjectNotFound { project_id: id })
    }

    /// 分页列出用户的项目
    pub async fn list_projects(
        &self,
        user_id: Uuid,
        page: i64,
        page_size: i64,
    ) -> ProductionResult<(Vec<ProductionProject>, i64)> {
        let offset = (page - 1) * page_size;
        let projects = sqlx::query_as!(
            ProductionProject,
            r#"
            SELECT id, title, description, project_type, status, user_id, metadata, created_at, updated_at, deleted_at
            FROM production_projects
            WHERE user_id = $1 AND deleted_at IS NULL
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            user_id,
            page_size,
            offset
        )
        .fetch_all(&self.pool)
        .await
        .map_err(ProductionError::Database)?;

        let total: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM production_projects WHERE user_id = $1 AND deleted_at IS NULL",
            user_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(ProductionError::Database)?
        .unwrap_or(0);

        Ok((projects, total))
    }

    /// 更新项目状态
    pub async fn update_project_status(&self, id: Uuid, status: String) -> ProductionResult<()> {
        sqlx::query!(
            "UPDATE production_projects SET status = $1, updated_at = NOW() WHERE id = $2 AND deleted_at IS NULL",
            status,
            id
        )
        .execute(&self.pool)
        .await
        .map_err(ProductionError::Database)?;
        Ok(())
    }

    /// 软删除项目
    pub async fn delete_project(&self, id: Uuid) -> ProductionResult<()> {
        let result = sqlx::query!(
            "UPDATE production_projects SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
            id
        )
        .execute(&self.pool)
        .await
        .map_err(ProductionError::Database)?;

        if result.rows_affected() == 0 {
            return Err(ProductionError::ProjectNotFound { project_id: id });
        }
        Ok(())
    }

    // =========================================================================
    // 通用产物读写接口
    // =========================================================================

    /// 按类型查询最新产物（通用实现）
    pub async fn get_artifact_by_type(
        &self,
        project_id: Uuid,
        artifact_type: &str,
        version: Option<i32>,
        character_id: Option<String>,
        shot_id: Option<String>,
    ) -> ProductionResult<Option<Value>> {
        // 根据 artifact_type 选择对应表名（白名单，防 SQL 注入）
        let table_name = match artifact_type {
            "creative_brief" => "creative_briefs",
            "story_bible" => "story_bibles",
            "character_bible" => "character_bibles",
            "script_draft" => "script_drafts",
            "directorial_treatment" => "directorial_treatments",
            "shot_contract" => "shot_contracts",
            "performance_brief" => "performance_briefs",
            "sound_plan" => "sound_plans",
            "continuity_ledger" => "continuity_ledgers",
            "take_review" => "take_reviews",
            _ => return Err(ProductionError::InvalidArtifactSchema {
                details: format!("未知的产物类型: {}", artifact_type),
            }),
        };

        // 使用动态 SQL，但表名来自白名单不存在注入风险
        let sql = if let Some(v) = version {
            format!(
                "SELECT row_to_json(t) FROM {} t WHERE production_project_id = $1 AND version = {} LIMIT 1",
                table_name, v
            )
        } else if let Some(cid) = character_id {
            format!(
                "SELECT row_to_json(t) FROM {} t WHERE production_project_id = $1 AND character_id = '{}' ORDER BY version DESC LIMIT 1",
                table_name, cid.replace('\'', "''")
            )
        } else if let Some(sid) = shot_id {
            format!(
                "SELECT row_to_json(t) FROM {} t WHERE production_project_id = $1 AND shot_id = '{}' ORDER BY version DESC LIMIT 1",
                table_name, sid.replace('\'', "''")
            )
        } else {
            format!(
                "SELECT row_to_json(t) FROM {} t WHERE production_project_id = $1 ORDER BY version DESC LIMIT 1",
                table_name
            )
        };

        let row: Option<Value> = sqlx::query_scalar(&sql)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(ProductionError::Database)?;

        Ok(row)
    }

    /// 列出特定类型的所有产物
    pub async fn list_artifacts_by_type(
        &self,
        project_id: Uuid,
        artifact_type: &str,
    ) -> ProductionResult<Vec<Value>> {
        let table_name = Self::validate_artifact_table(artifact_type)?;
        let sql = format!(
            "SELECT row_to_json(t) FROM {} t WHERE production_project_id = $1 ORDER BY version DESC",
            table_name
        );
        let rows: Vec<Value> = sqlx::query_scalar(&sql)
            .bind(project_id)
            .fetch_all(&self.pool)
            .await
            .map_err(ProductionError::Database)?;
        Ok(rows)
    }

    /// 批准产物：将指定产物设为 approved，同类型旧版本标记为 superseded
    pub async fn approve_artifact(
        &self,
        project_id: Uuid,
        artifact_type: &str,
        artifact_id: Uuid,
        user_id: Uuid,
    ) -> ProductionResult<()> {
        let table_name = Self::validate_artifact_table(artifact_type)?;

        // 先将同类型所有 approved 版本改为 superseded
        let supersede_sql = format!(
            "UPDATE {} SET status = 'superseded', updated_at = NOW() WHERE production_project_id = $1 AND status = 'approved'",
            table_name
        );
        sqlx::query(&supersede_sql)
            .bind(project_id)
            .execute(&self.pool)
            .await
            .map_err(ProductionError::Database)?;

        // 批准指定产物
        let approve_sql = format!(
            "UPDATE {} SET status = 'approved', approved_by = $2, approved_at = NOW(), updated_at = NOW() WHERE id = $1",
            table_name
        );
        let result = sqlx::query(&approve_sql)
            .bind(artifact_id)
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map_err(ProductionError::Database)?;

        if result.rows_affected() == 0 {
            return Err(ProductionError::ArtifactNotFound {
                artifact_type: artifact_type.to_string(),
                artifact_id,
            });
        }
        Ok(())
    }

    /// 创建协作建议
    pub async fn create_collaboration_suggestion(
        &self,
        project_id: Uuid,
        data: Value,
    ) -> ProductionResult<Value> {
        let from_role = data.get("from_role").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let to_role = data.get("to_role").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artifact_type = data.get("artifact_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let artifact_id: Uuid = data.get("artifact_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(Uuid::new_v4);
        let suggestion_type = data.get("suggestion_type").and_then(|v| v.as_str()).unwrap_or("revision").to_string();
        let content = data.get("content").cloned().unwrap_or(Value::Null);

        let row: Value = sqlx::query_scalar(
            r#"
            INSERT INTO collaboration_suggestions
                (production_project_id, from_role, to_role, artifact_type, artifact_id, suggestion_type, content)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING row_to_json(collaboration_suggestions.*)
            "#
        )
        .bind(project_id)
        .bind(from_role)
        .bind(to_role)
        .bind(artifact_type)
        .bind(artifact_id)
        .bind(suggestion_type)
        .bind(content)
        .fetch_one(&self.pool)
        .await
        .map_err(ProductionError::Database)?;

        Ok(row)
    }

    /// 列出协作建议
    pub async fn list_collaboration_suggestions(
        &self,
        project_id: Uuid,
        to_role: Option<String>,
        status: Option<String>,
    ) -> ProductionResult<(Vec<Value>, i64)> {
        // 使用参数化查询构建过滤条件
        let mut conditions = vec!["production_project_id = $1".to_string()];
        let mut bind_count = 1i32;

        if to_role.is_some() {
            bind_count += 1;
            conditions.push(format!("to_role = ${}", bind_count));
        }
        if status.is_some() {
            bind_count += 1;
            conditions.push(format!("status = ${}", bind_count));
        }

        let where_clause = conditions.join(" AND ");
        let sql = format!(
            "SELECT row_to_json(t) FROM collaboration_suggestions t WHERE {} ORDER BY created_at DESC",
            where_clause
        );

        let mut query = sqlx::query_scalar(&sql).bind(project_id);
        if let Some(role) = &to_role {
            query = query.bind(role);
        }
        if let Some(s) = &status {
            query = query.bind(s);
        }

        let items: Vec<Value> = query.fetch_all(&self.pool).await.map_err(ProductionError::Database)?;
        let total = items.len() as i64;
        Ok((items, total))
    }

    /// 响应协作建议
    pub async fn respond_to_suggestion(
        &self,
        _project_id: Uuid,
        suggestion_id: Uuid,
        user_id: Uuid,
        status: String,
        note: Option<String>,
    ) -> ProductionResult<Value> {
        let row: Value = sqlx::query_scalar(
            r#"
            UPDATE collaboration_suggestions
            SET status = $2, responded_by = $3, responded_at = NOW(), response_note = $4, updated_at = NOW()
            WHERE id = $1
            RETURNING row_to_json(collaboration_suggestions.*)
            "#
        )
        .bind(suggestion_id)
        .bind(status)
        .bind(user_id)
        .bind(note)
        .fetch_optional(&self.pool)
        .await
        .map_err(ProductionError::Database)?
        .ok_or_else(|| ProductionError::SuggestionNotFound { suggestion_id })?;

        Ok(row)
    }

    /// 查询项目审计日志（model_call 记录）
    /// 注意：当前从 model_calls 表查询关联该项目的记录，需 model_calls 表有 production_project_id 关联
    /// 如无该字段，返回空列表（后续可扩展审计表）
    pub async fn get_audit_log(&self, project_id: Uuid) -> ProductionResult<Vec<Value>> {
        // TODO: 扩展审计日志表以关联 production_project_id
        // 当前返回空，避免查询不存在的字段导致编译错误
        tracing::info!(project_id = %project_id, "审计日志查询（当前为空实现）");
        Ok(vec![])
    }

    /// 校验 artifact_type 并返回对应表名（白名单）
    fn validate_artifact_table(artifact_type: &str) -> ProductionResult<&'static str> {
        match artifact_type {
            "creative_brief" => Ok("creative_briefs"),
            "story_bible" => Ok("story_bibles"),
            "character_bible" => Ok("character_bibles"),
            "script_draft" => Ok("script_drafts"),
            "directorial_treatment" => Ok("directorial_treatments"),
            "shot_contract" => Ok("shot_contracts"),
            "performance_brief" => Ok("performance_briefs"),
            "sound_plan" => Ok("sound_plans"),
            "continuity_ledger" => Ok("continuity_ledgers"),
            "take_review" => Ok("take_reviews"),
            _ => Err(ProductionError::InvalidArtifactSchema {
                details: format!("未知的产物类型: {}", artifact_type),
            }),
        }
    }
}
