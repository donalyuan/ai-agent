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

    /// 查询项目中指定产物类型的最新就绪版本（approved 优先，其次 draft）。
    ///
    /// 返回 `HashMap<ArtifactType, Value>` — 有内容才包含对应类型。
    /// 用于角色执行前装配 ContextCandidate 列表。
    pub async fn get_input_artifacts(
        &self,
        project_id: Uuid,
        required: &[crate::state::artifacts::ArtifactType],
    ) -> ProductionResult<std::collections::HashMap<crate::state::artifacts::ArtifactType, Value>> {
        let mut result = std::collections::HashMap::new();

        for &artifact_type in required {
            let table = Self::artifact_type_to_table_key(artifact_type);
            // 按状态优先级（approved=0 > draft=1）和版本降序取最新一条
            let sql = format!(
                "SELECT row_to_json(t) FROM {} t \
                 WHERE production_project_id = $1 AND status IN ('approved','draft') \
                 ORDER BY CASE status WHEN 'approved' THEN 0 ELSE 1 END, version DESC LIMIT 1",
                table
            );
            let row: Option<Value> = sqlx::query_scalar(&sql)
                .bind(project_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(ProductionError::Database)?;

            if let Some(row) = row {
                result.insert(artifact_type, row);
            }
        }
        Ok(result)
    }

    /// 将 AI 输出的产物写入数据库（version 自增，status=draft）。
    ///
    /// `output` 是整个 AI 响应 JSON；方法按 `artifact_type` 提取对应键并插入。
    /// 返回本次插入的 `ArtifactSummary` 列表（一个产物类型可能对应多条记录，如 character_bibles）。
    pub async fn save_artifact(
        &self,
        project_id: Uuid,
        artifact_type: crate::state::artifacts::ArtifactType,
        output: &Value,
        created_by: &str,
    ) -> ProductionResult<Vec<crate::executor::role_executor::ArtifactSummary>> {
        use crate::executor::role_executor::ArtifactSummary;
        use crate::state::artifacts::ArtifactType;

        let mut summaries = Vec::new();

        match artifact_type {
            // --- 无额外约束字段的单条产物 ---
            ArtifactType::CreativeBrief
            | ArtifactType::StoryBible
            | ArtifactType::ScriptDraft
            | ArtifactType::DirectorialTreatment
            | ArtifactType::SoundPlan => {
                let json_key = Self::artifact_type_to_output_key(artifact_type);
                let content = output
                    .get(json_key)
                    .cloned()
                    .ok_or_else(|| ProductionError::InvalidArtifactSchema {
                        details: format!("AI 输出缺少必需键: {}", json_key),
                    })?;
                let table = Self::artifact_type_to_table_key(artifact_type);
                let (id, version) = self
                    .insert_simple_artifact(project_id, table, &content, created_by)
                    .await?;
                summaries.push(ArtifactSummary {
                    artifact_type,
                    id,
                    version,
                    character_id: None,
                    shot_id: None,
                });
            }

            // --- 按 character_id 分条的产物数组 ---
            ArtifactType::CharacterBible | ArtifactType::PerformanceBrief => {
                let json_key = Self::artifact_type_to_output_key(artifact_type);
                let items = output
                    .get(json_key)
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| ProductionError::InvalidArtifactSchema {
                        details: format!("AI 输出键 {} 应为数组", json_key),
                    })?;
                let table = Self::artifact_type_to_table_key(artifact_type);
                for item in items {
                    let character_id = item
                        .get("character_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("default")
                        .to_string();
                    let (id, version) = self
                        .insert_character_artifact(
                            project_id, table, &character_id, item, created_by,
                        )
                        .await?;
                    summaries.push(ArtifactSummary {
                        artifact_type,
                        id,
                        version,
                        character_id: Some(character_id),
                        shot_id: None,
                    });
                }
            }

            // --- 按 shot_id 分条的产物数组 ---
            ArtifactType::ShotContract
            | ArtifactType::ContinuityLedger
            | ArtifactType::TakeReview => {
                let json_key = Self::artifact_type_to_output_key(artifact_type);
                let items = output
                    .get(json_key)
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| ProductionError::InvalidArtifactSchema {
                        details: format!("AI 输出键 {} 应为数组", json_key),
                    })?;
                let table = Self::artifact_type_to_table_key(artifact_type);
                for item in items {
                    let shot_id = item
                        .get("shot_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("default")
                        .to_string();
                    let scene_id = item
                        .get("scene_id")
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            item.get("scene_number")
                                .and_then(|v| v.as_i64())
                                .map(|_| "1")
                        })
                        .unwrap_or("1")
                        .to_string();
                    let (id, version) = self
                        .insert_shot_artifact(
                            project_id, table, &shot_id, &scene_id, item, created_by,
                        )
                        .await?;
                    summaries.push(ArtifactSummary {
                        artifact_type,
                        id,
                        version,
                        character_id: None,
                        shot_id: Some(shot_id),
                    });
                }
            }
        }

        Ok(summaries)
    }

    /// 插入无额外约束字段的单条产物，返回 (id, version)
    async fn insert_simple_artifact(
        &self,
        project_id: Uuid,
        table: &str,
        content: &Value,
        created_by: &str,
    ) -> ProductionResult<(Uuid, i32)> {
        let sql = format!(
            "INSERT INTO {} (production_project_id, version, content, created_by) \
             VALUES ($1, (SELECT COALESCE(MAX(version), 0) + 1 FROM {} WHERE production_project_id = $1), $2, $3) \
             RETURNING id, version",
            table, table
        );
        let row: (Uuid, i32) = sqlx::query_as(&sql)
            .bind(project_id)
            .bind(content)
            .bind(created_by)
            .fetch_one(&self.pool)
            .await
            .map_err(ProductionError::Database)?;
        Ok(row)
    }

    /// 插入按 character_id 分条的产物，返回 (id, version)
    async fn insert_character_artifact(
        &self,
        project_id: Uuid,
        table: &str,
        character_id: &str,
        content: &Value,
        created_by: &str,
    ) -> ProductionResult<(Uuid, i32)> {
        let sql = format!(
            "INSERT INTO {} (production_project_id, character_id, version, content, created_by) \
             VALUES ($1, $2, \
               (SELECT COALESCE(MAX(version), 0) + 1 FROM {} \
                WHERE production_project_id = $1 AND character_id = $2), \
               $3, $4) \
             RETURNING id, version",
            table, table
        );
        let row: (Uuid, i32) = sqlx::query_as(&sql)
            .bind(project_id)
            .bind(character_id)
            .bind(content)
            .bind(created_by)
            .fetch_one(&self.pool)
            .await
            .map_err(ProductionError::Database)?;
        Ok(row)
    }

    /// 插入按 shot_id 分条的产物（shot_contracts、continuity_ledgers、take_reviews），返回 (id, version)
    async fn insert_shot_artifact(
        &self,
        project_id: Uuid,
        table: &str,
        shot_id: &str,
        scene_id: &str,
        content: &Value,
        created_by: &str,
    ) -> ProductionResult<(Uuid, i32)> {
        // shot_contracts 表需要 scene_id；其他表（continuity_ledger、take_review）只需 shot_id
        // 统一包含 scene_id，无 scene_id 列的表忽略该插入（通过表名区分）
        let sql = if table == "shot_contracts" {
            format!(
                "INSERT INTO {} (production_project_id, shot_id, scene_id, version, content, created_by) \
                 VALUES ($1, $2, $3, \
                   (SELECT COALESCE(MAX(version), 0) + 1 FROM {} \
                    WHERE production_project_id = $1 AND shot_id = $2), \
                   $4, $5) \
                 RETURNING id, version",
                table, table
            )
        } else {
            format!(
                "INSERT INTO {} (production_project_id, shot_id, version, content, created_by) \
                 VALUES ($1, $2, \
                   (SELECT COALESCE(MAX(version), 0) + 1 FROM {} \
                    WHERE production_project_id = $1 AND shot_id = $2), \
                   $3, $4) \
                 RETURNING id, version",
                table, table
            )
        };

        let row: (Uuid, i32) = if table == "shot_contracts" {
            sqlx::query_as(&sql)
                .bind(project_id)
                .bind(shot_id)
                .bind(scene_id)
                .bind(content)
                .bind(created_by)
                .fetch_one(&self.pool)
                .await
                .map_err(ProductionError::Database)?
        } else {
            sqlx::query_as(&sql)
                .bind(project_id)
                .bind(shot_id)
                .bind(content)
                .bind(created_by)
                .fetch_one(&self.pool)
                .await
                .map_err(ProductionError::Database)?
        };
        Ok(row)
    }

    /// 产物类型 → DB 表名（用于动态 SQL，来自白名单，不存在注入风险）
    fn artifact_type_to_table_key(artifact_type: crate::state::artifacts::ArtifactType) -> &'static str {
        use crate::state::artifacts::ArtifactType;
        match artifact_type {
            ArtifactType::CreativeBrief => "creative_briefs",
            ArtifactType::StoryBible => "story_bibles",
            ArtifactType::CharacterBible => "character_bibles",
            ArtifactType::ScriptDraft => "script_drafts",
            ArtifactType::DirectorialTreatment => "directorial_treatments",
            ArtifactType::ShotContract => "shot_contracts",
            ArtifactType::PerformanceBrief => "performance_briefs",
            ArtifactType::SoundPlan => "sound_plans",
            ArtifactType::ContinuityLedger => "continuity_ledgers",
            ArtifactType::TakeReview => "take_reviews",
        }
    }

    /// 产物类型 → AI 输出 JSON 顶层键名（与 validate_output 中的 key 保持一致）
    fn artifact_type_to_output_key(artifact_type: crate::state::artifacts::ArtifactType) -> &'static str {
        use crate::state::artifacts::ArtifactType;
        match artifact_type {
            ArtifactType::CreativeBrief => "creative_brief",
            ArtifactType::StoryBible => "story_bible",
            ArtifactType::CharacterBible => "character_bibles",
            ArtifactType::ScriptDraft => "script_draft",
            ArtifactType::DirectorialTreatment => "directorial_treatment",
            ArtifactType::ShotContract => "shot_contracts",
            ArtifactType::PerformanceBrief => "performance_briefs",
            ArtifactType::SoundPlan => "sound_plan",
            ArtifactType::ContinuityLedger => "continuity_ledgers",
            ArtifactType::TakeReview => "take_reviews",
        }
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
