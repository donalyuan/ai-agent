//! ScriptPackage 到正式 Script/Scene 的事务化晋升边界。
//!
//! 该服务只执行确定性映射和 PostgreSQL 事务，不调用模型。Topic、正式
//! Script/Scene、领域关联、流程 Step 与幂等命令必须全部提交或全部回滚。

use crate::domain::script::{Scene, Script, ScriptStatus};
use chrono::{DateTime, Utc};
use novex_production_crew::{
    durable::{
        canonical_digest,
        command_store::{
            ProductionAggregateType, ProductionCommandScope, ProductionCommandStore,
            ProductionCommandType,
        },
        repository::ProductionActor,
        script::{map_script_draft, ScriptDraftInput, ScriptSceneInput},
    },
    ProductionError, ProductionResult,
};
use serde_json::{json, Value};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ScriptPackagePromotionCommand {
    pub run_id: Uuid,
    pub package_digest: String,
    pub actor: ProductionActor,
    pub idempotency_key: String,
}

#[derive(Clone)]
pub struct ScriptPackagePromotionService {
    pool: PgPool,
}

impl ScriptPackagePromotionService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn promote(
        &self,
        command: ScriptPackagePromotionCommand,
    ) -> ProductionResult<Script> {
        validate_command(&command)?;
        let request_digest = ProductionCommandStore::canonical_request_digest(&json!({
            "run_id": command.run_id,
            "package_digest": command.package_digest,
        }))?;
        let command_scope = ProductionCommandScope::new(
            command.actor.clone(),
            ProductionCommandType::PromoteScript,
            ProductionAggregateType::ProductionRun,
            command.run_id,
            &command.idempotency_key,
        );
        let mut tx = self.pool.begin().await?;

        // The Run lock serializes promotion and its command replay before any
        // formal domain row is created.
        let context = lock_promotion_context(&mut tx, command.run_id).await?;
        if let Some(result) =
            ProductionCommandStore::replay(&mut tx, &command_scope, &request_digest).await?
        {
            let script_id = result
                .get("script_id")
                .and_then(Value::as_str)
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or_else(|| ProductionError::TransitionConflict {
                    reason: "stored promote_script result is invalid".into(),
                })?;
            let script = load_script(&mut tx, script_id).await?;
            tx.commit().await?;
            return Ok(script);
        }

        let parent_script = load_parent_script(&mut tx, &context).await?;
        validate_source_context(&context, parent_script.as_ref())?;
        let package = lock_current_approved_package(
            &mut tx,
            command.run_id,
            context.current_revision_epoch,
            &command.package_digest,
        )
        .await?;
        let promotion_step_id =
            lock_promotion_step(&mut tx, command.run_id, context.current_revision_epoch).await?;
        let artifacts = load_current_package_artifacts(&mut tx, &context, &package).await?;

        let draft: ScriptDraftInput = serde_json::from_value(artifacts.draft.content.clone())
            .map_err(|error| ProductionError::InvalidArtifactSchema {
                details: format!("ScriptDraft schema is invalid: {error}"),
            })?;
        let character_ids = artifacts
            .characters
            .iter()
            .map(|character| character.character_id.clone())
            .collect::<Vec<_>>();
        let formal = map_script_draft(&draft, &character_ids)?;

        let script_id = Uuid::new_v4();
        let now = Utc::now();
        let topic_snapshot = context
            .source_snapshot
            .get("topic")
            .cloned()
            .ok_or_else(|| ProductionError::SourceInvalid {
                reason: "production source snapshot has no Topic".into(),
            })?;
        let source_artifacts = Value::Array(
            artifacts
                .items
                .iter()
                .map(|item| {
                    json!({
                        "artifact_type": item.artifact_type,
                        "artifact_id": item.artifact_id,
                        "artifact_version": item.artifact_version,
                        "content_digest": item.content_digest.trim(),
                        "source_step_id": item.source_step_id,
                        "source_attempt": item.source_attempt,
                    })
                })
                .collect(),
        );
        let script_content = json!({
            "source": "full_crew",
            "topic_snapshot": topic_snapshot,
            "script_package": {
                "id": package.id,
                "digest": package.package_digest.trim(),
                "revision_epoch": package.revision_epoch,
            },
            "story_bible": {
                "id": artifacts.story.id,
                "version": artifacts.story.version,
                "content_digest": artifacts.story.content_digest.trim(),
                "content": artifacts.story.content,
            },
            "character_bibles": artifacts.characters.iter().map(|character| json!({
                "id": character.id,
                "character_id": character.character_id,
                "version": character.version,
                "content_digest": character.content_digest.trim(),
                "content": character.content,
            })).collect::<Vec<_>>(),
            "script_draft_digest": formal.digest,
            "parent_script_id": parent_script.as_ref().map(|parent| parent.id),
        });

        sqlx::query(
            r#"
            INSERT INTO scripts (
                id, project_id, topic_id, title, hook, content, status, parent_id,
                production_run_id, script_package_id, script_package_digest,
                topic_snapshot, source_artifacts, source_revision_epoch,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, 'approved', $7,
                $8, $9, $10, $11, $12, $13, $14, $14
            )
            "#,
        )
        .bind(script_id)
        .bind(context.project_id)
        .bind(context.topic_id)
        .bind(&formal.title)
        .bind(&formal.hook)
        .bind(&script_content)
        .bind(parent_script.as_ref().map(|parent| parent.id))
        .bind(context.run_id)
        .bind(package.id)
        .bind(package.package_digest.trim())
        .bind(&topic_snapshot)
        .bind(&source_artifacts)
        .bind(package.revision_epoch)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let mut scenes = Vec::with_capacity(formal.scenes.len());
        for source_scene in &formal.scenes {
            let scene = insert_scene(&mut tx, script_id, source_scene).await?;
            let scene_digest = canonical_digest(source_scene)?;
            insert_domain_link(
                &mut tx,
                context.run_id,
                promotion_step_id,
                package.revision_epoch,
                "scene",
                None,
                Some(scene.id),
                scene.id.to_string(),
                &scene_digest,
            )
            .await?;
            scenes.push(scene);
        }
        insert_domain_link(
            &mut tx,
            context.run_id,
            promotion_step_id,
            package.revision_epoch,
            "script",
            Some(script_id),
            None,
            script_id.to_string(),
            &formal.digest,
        )
        .await?;

        if let Some(parent) = &parent_script {
            invalidate_superseded_downstream(&mut tx, &context, parent, script_id).await?;
        } else {
            sqlx::query("SELECT set_config('novex.production_script_promotion', 'on', TRUE)")
                .execute(&mut *tx)
                .await?;
            let topic_updated = sqlx::query(
                r#"
                UPDATE content_topics
                SET status = 'scripted'
                WHERE id = $1 AND project_id = $2
                  AND status = 'approved' AND deleted_at IS NULL
                "#,
            )
            .bind(context.topic_id)
            .bind(context.project_id)
            .execute(&mut *tx)
            .await?;
            if topic_updated.rows_affected() != 1 {
                return Err(ProductionError::TransitionConflict {
                    reason: "Topic was consumed while ScriptPackage promotion was in progress"
                        .into(),
                });
            }
        }

        let step_updated = sqlx::query(
            r#"
            UPDATE production_steps
            SET status = 'succeeded', attempt = GREATEST(attempt, 1),
                side_effect_state = 'confirmed', output_digest = $2,
                waiting_reason = NULL, completed_at = NOW(), updated_at = NOW()
            WHERE id = $1 AND status = 'queued'
            "#,
        )
        .bind(promotion_step_id)
        .bind(&formal.digest)
        .execute(&mut *tx)
        .await?;
        if step_updated.rows_affected() != 1 {
            return Err(ProductionError::TransitionConflict {
                reason: "promote_script step is no longer executable".into(),
            });
        }
        unlock_ready_steps(&mut tx, context.run_id, package.revision_epoch).await?;
        sqlx::query(
            "UPDATE production_runs SET status = 'queued', updated_at = NOW() WHERE id = $1",
        )
        .bind(context.run_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            r#"
            UPDATE production_projects
            SET status = 'directing', script_promoted_at = NOW(), updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(context.production_project_id)
        .execute(&mut *tx)
        .await?;
        enqueue_unlocked_steps(&mut tx, context.run_id, package.revision_epoch).await?;

        ProductionCommandStore::record(
            &mut tx,
            &command_scope,
            &request_digest,
            json!({
                "script_id": script_id,
                "scene_ids": scenes.iter().map(|scene| scene.id).collect::<Vec<_>>(),
                "package_id": package.id,
                "package_digest": package.package_digest.trim(),
            }),
        )
        .await?;

        tx.commit().await?;
        Ok(Script::new(
            script_id,
            context.project_id,
            Some(context.topic_id),
            formal.title,
            formal.hook,
            script_content,
            ScriptStatus::Approved,
            parent_script.map(|parent| parent.id),
            scenes,
            now,
            now,
        ))
    }
}

#[derive(FromRow)]
struct PromotionContext {
    run_id: Uuid,
    production_project_id: Uuid,
    current_revision_epoch: i32,
    revision_reason_type: String,
    run_status: String,
    production_status: String,
    project_id: Uuid,
    topic_id: Uuid,
    project_status: String,
    topic_project_id: Uuid,
    topic_status: String,
    topic_deleted_at: Option<DateTime<Utc>>,
    source_snapshot: Value,
    source_fingerprint: String,
    current_topic_snapshot: Value,
}

#[derive(FromRow)]
struct PackageIdentity {
    id: Uuid,
    package_digest: String,
    revision_epoch: i32,
}

#[derive(FromRow)]
struct ParentScript {
    id: Uuid,
}

#[derive(Clone, FromRow)]
struct PackageItem {
    artifact_type: String,
    artifact_id: Uuid,
    artifact_version: i32,
    content_digest: String,
    source_step_id: Uuid,
    source_attempt: i32,
}

#[derive(Clone, FromRow)]
struct ArtifactContent {
    id: Uuid,
    version: i32,
    content: Value,
    content_digest: String,
}

#[derive(Clone, FromRow)]
struct CharacterArtifactContent {
    id: Uuid,
    character_id: String,
    version: i32,
    content: Value,
    content_digest: String,
}

struct CurrentPackageArtifacts {
    items: Vec<PackageItem>,
    story: ArtifactContent,
    characters: Vec<CharacterArtifactContent>,
    draft: ArtifactContent,
}

fn validate_command(command: &ScriptPackagePromotionCommand) -> ProductionResult<()> {
    command.actor.validate()?;
    if !is_digest(&command.package_digest) {
        return Err(ProductionError::StalePackage);
    }
    Ok(())
}

async fn lock_promotion_context(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
) -> ProductionResult<PromotionContext> {
    sqlx::query_as::<_, PromotionContext>(
        r#"
        SELECT
            run.id AS run_id,
            production.id AS production_project_id,
            run.current_revision_epoch,
            revision.reason_type AS revision_reason_type,
            run.status AS run_status,
            production.status AS production_status,
            production.project_id,
            production.topic_id,
            project.status AS project_status,
            topic.project_id AS topic_project_id,
            topic.status AS topic_status,
            topic.deleted_at AS topic_deleted_at,
            production.source_snapshot,
            production.source_fingerprint,
            jsonb_build_object(
                'id', topic.id,
                'project_id', topic.project_id,
                'title', topic.title,
                'angle', topic.angle,
                'target_audience', topic.target_audience,
                'hook_points', topic.hook_points,
                'content_type', topic.content_type,
                'tags', topic.tags,
                'status', topic.status
            ) AS current_topic_snapshot
        FROM production_runs run
        JOIN production_projects production ON production.id = run.production_project_id
        JOIN production_revision_epochs revision
          ON revision.run_id = run.id AND revision.epoch = run.current_revision_epoch
        JOIN projects project ON project.id = production.project_id
        JOIN content_topics topic ON topic.id = production.topic_id
        WHERE run.id = $1 AND production.project_type = 'full_crew'
        FOR UPDATE OF run, production, project, topic
        "#,
    )
    .bind(run_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProductionError::SourceInvalid {
        reason: "Full Crew run and its bound source do not exist".into(),
    })
}

fn validate_source_context(
    context: &PromotionContext,
    parent_script: Option<&ParentScript>,
) -> ProductionResult<()> {
    if !matches!(context.run_status.as_str(), "queued" | "running")
        || !matches!(
            context.production_status.as_str(),
            "active" | "scripting" | "waiting_approval"
        )
    {
        return Err(ProductionError::TransitionConflict {
            reason: "production run is not in a promotable state".into(),
        });
    }
    if context.project_status != "active"
        || context.project_id != context.topic_project_id
        || context.topic_deleted_at.is_some()
    {
        return Err(ProductionError::SourceInvalid {
            reason: "project/topic must remain active, same-project, and not deleted".into(),
        });
    }
    let semantic_revision = context.revision_reason_type == "script_semantic_revision";
    if context.topic_status == "scripted" && !(semantic_revision && parent_script.is_some()) {
        return Err(ProductionError::TransitionConflict {
            reason: "Topic has already been consumed by a formal Script".into(),
        });
    }
    if context.topic_status != "approved" && context.topic_status != "scripted" {
        return Err(ProductionError::SourceInvalid {
            reason: "Topic must remain approved until promotion commits".into(),
        });
    }
    if semantic_revision != parent_script.is_some() {
        return Err(ProductionError::TransitionConflict {
            reason: "only a script semantic revision can promote a child Script".into(),
        });
    }
    if canonical_digest(&context.source_snapshot)? != context.source_fingerprint.trim() {
        return Err(ProductionError::SourceLocked);
    }
    let mut stored_topic = context
        .source_snapshot
        .get("topic")
        .and_then(Value::as_object)
        .cloned()
        .ok_or(ProductionError::SourceLocked)?;
    stored_topic.remove("updated_at");
    let mut current_topic = context
        .current_topic_snapshot
        .as_object()
        .cloned()
        .ok_or(ProductionError::SourceLocked)?;
    if semantic_revision {
        current_topic.insert("status".into(), Value::String("approved".into()));
    }
    if Value::Object(stored_topic) != Value::Object(current_topic) {
        return Err(ProductionError::SourceLocked);
    }
    Ok(())
}

async fn load_parent_script(
    tx: &mut Transaction<'_, Postgres>,
    context: &PromotionContext,
) -> ProductionResult<Option<ParentScript>> {
    if context.revision_reason_type != "script_semantic_revision" {
        return Ok(None);
    }
    sqlx::query_as::<_, ParentScript>(
        r#"
        SELECT script.id
        FROM scripts script
        JOIN production_domain_links link
          ON link.run_id = script.production_run_id
         AND link.link_type = 'script' AND link.script_id = script.id
        WHERE script.production_run_id = $1
          AND script.source_revision_epoch < $2
          AND script.status = 'approved'
        ORDER BY script.source_revision_epoch DESC, script.created_at DESC
        LIMIT 1
        FOR UPDATE OF script
        "#,
    )
    .bind(context.run_id)
    .bind(context.current_revision_epoch)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProductionError::TransitionConflict {
        reason: "script semantic revision has no current parent Script".into(),
    })
    .map(Some)
}

async fn lock_current_approved_package(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    revision_epoch: i32,
    submitted_digest: &str,
) -> ProductionResult<PackageIdentity> {
    let package = sqlx::query_as::<_, PackageIdentity>(
        r#"
        SELECT package.id, package.package_digest, package.revision_epoch
        FROM artifact_package_snapshots package
        WHERE package.run_id = $1
          AND package.package_type = 'script'
          AND package.revision_epoch = $2
          AND package.package_digest = $3
          AND package.package_version = (
              SELECT MAX(candidate.package_version)
              FROM artifact_package_snapshots candidate
              WHERE candidate.run_id = package.run_id
                AND candidate.package_type = 'script'
                AND candidate.revision_epoch = package.revision_epoch
          )
        FOR UPDATE OF package
        "#,
    )
    .bind(run_id)
    .bind(revision_epoch)
    .bind(submitted_digest)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ProductionError::StalePackage)?;
    let approved = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM production_gate_decisions decision
            JOIN production_steps gate ON gate.id = decision.gate_step_id
            WHERE decision.run_id = $1
              AND decision.package_id = $2
              AND decision.package_digest = $3
              AND decision.revision_epoch = $4
              AND decision.decision = 'approved'
              AND gate.step_key = 'script_package_approval'
              AND gate.status = 'succeeded'
              AND gate.output_digest = $3
        )
        "#,
    )
    .bind(run_id)
    .bind(package.id)
    .bind(submitted_digest)
    .bind(revision_epoch)
    .fetch_one(&mut **tx)
    .await?;
    if !approved {
        return Err(ProductionError::TransitionConflict {
            reason: "current ScriptPackage has not passed its exact Gate digest".into(),
        });
    }
    Ok(package)
}

async fn lock_promotion_step(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    revision_epoch: i32,
) -> ProductionResult<Uuid> {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id FROM production_steps
        WHERE run_id = $1 AND revision_epoch = $2
          AND step_key = 'promote_script' AND step_type = 'domain_command'
          AND status = 'queued'
        FOR UPDATE
        "#,
    )
    .bind(run_id)
    .bind(revision_epoch)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProductionError::TransitionConflict {
        reason: "promote_script step is not queued for the current revision".into(),
    })
}

async fn load_current_package_artifacts(
    tx: &mut Transaction<'_, Postgres>,
    context: &PromotionContext,
    package: &PackageIdentity,
) -> ProductionResult<CurrentPackageArtifacts> {
    let items = sqlx::query_as::<_, PackageItem>(
        r#"
        SELECT artifact_type, artifact_id, artifact_version,
               content_digest, source_step_id, source_attempt
        FROM artifact_package_items
        WHERE package_id = $1
        ORDER BY ordinal
        "#,
    )
    .bind(package.id)
    .fetch_all(&mut **tx)
    .await?;
    let story_items = items
        .iter()
        .filter(|item| item.artifact_type == "story_bible")
        .collect::<Vec<_>>();
    let character_items = items
        .iter()
        .filter(|item| item.artifact_type == "character_bible")
        .collect::<Vec<_>>();
    let draft_items = items
        .iter()
        .filter(|item| item.artifact_type == "script_draft")
        .collect::<Vec<_>>();
    if items.len() != story_items.len() + character_items.len() + draft_items.len()
        || story_items.len() != 1
        || character_items.is_empty()
        || draft_items.len() != 1
    {
        return Err(ProductionError::InvalidArtifactSchema {
            details:
                "ScriptPackage must contain one StoryBible, CharacterBible[], and one ScriptDraft"
                    .into(),
        });
    }

    let story = load_story(tx, context, package, story_items[0]).await?;
    let draft = load_draft(tx, context, package, draft_items[0]).await?;
    let mut characters = Vec::with_capacity(character_items.len());
    for item in character_items {
        characters.push(load_character(tx, context, package, item).await?);
    }
    validate_current_artifact_versions(tx, context, package, &story, &characters, &draft).await?;
    validate_artifact_content(&story.content, &story.content_digest)?;
    validate_artifact_content(&draft.content, &draft.content_digest)?;
    if story
        .content
        .as_object()
        .is_none_or(|value| value.is_empty())
    {
        return Err(ProductionError::InvalidArtifactSchema {
            details: "StoryBible content must be a non-empty object".into(),
        });
    }
    let mut character_identity = BTreeSet::new();
    for character in &characters {
        validate_artifact_content(&character.content, &character.content_digest)?;
        if character.character_id.trim().is_empty()
            || character
                .content
                .as_object()
                .is_none_or(|value| value.is_empty())
            || !character_identity.insert(character.character_id.as_str())
        {
            return Err(ProductionError::InvalidArtifactSchema {
                details: "CharacterBible identities and content must be complete and unique".into(),
            });
        }
    }
    Ok(CurrentPackageArtifacts {
        items,
        story,
        characters,
        draft,
    })
}

async fn load_story(
    tx: &mut Transaction<'_, Postgres>,
    context: &PromotionContext,
    package: &PackageIdentity,
    item: &PackageItem,
) -> ProductionResult<ArtifactContent> {
    sqlx::query_as::<_, ArtifactContent>(
        r#"
        SELECT id, version, content, content_digest
        FROM story_bibles
        WHERE id = $1 AND production_project_id = $2 AND version = $3
          AND run_id = $4 AND step_id = $5 AND attempt = $6
          AND revision_epoch = $7 AND content_digest = $8
          AND audit_status = 'complete'
        FOR UPDATE
        "#,
    )
    .bind(item.artifact_id)
    .bind(context.production_project_id)
    .bind(item.artifact_version)
    .bind(context.run_id)
    .bind(item.source_step_id)
    .bind(item.source_attempt)
    .bind(package.revision_epoch)
    .bind(item.content_digest.trim())
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ProductionError::StalePackage)
}

async fn load_draft(
    tx: &mut Transaction<'_, Postgres>,
    context: &PromotionContext,
    package: &PackageIdentity,
    item: &PackageItem,
) -> ProductionResult<ArtifactContent> {
    sqlx::query_as::<_, ArtifactContent>(
        r#"
        SELECT id, version, content, content_digest
        FROM script_drafts
        WHERE id = $1 AND production_project_id = $2 AND version = $3
          AND run_id = $4 AND step_id = $5 AND attempt = $6
          AND revision_epoch = $7 AND content_digest = $8
          AND audit_status = 'complete'
        FOR UPDATE
        "#,
    )
    .bind(item.artifact_id)
    .bind(context.production_project_id)
    .bind(item.artifact_version)
    .bind(context.run_id)
    .bind(item.source_step_id)
    .bind(item.source_attempt)
    .bind(package.revision_epoch)
    .bind(item.content_digest.trim())
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ProductionError::StalePackage)
}

async fn load_character(
    tx: &mut Transaction<'_, Postgres>,
    context: &PromotionContext,
    package: &PackageIdentity,
    item: &PackageItem,
) -> ProductionResult<CharacterArtifactContent> {
    sqlx::query_as::<_, CharacterArtifactContent>(
        r#"
        SELECT id, character_id, version, content, content_digest
        FROM character_bibles
        WHERE id = $1 AND production_project_id = $2 AND version = $3
          AND run_id = $4 AND step_id = $5 AND attempt = $6
          AND revision_epoch = $7 AND content_digest = $8
          AND audit_status = 'complete'
        FOR UPDATE
        "#,
    )
    .bind(item.artifact_id)
    .bind(context.production_project_id)
    .bind(item.artifact_version)
    .bind(context.run_id)
    .bind(item.source_step_id)
    .bind(item.source_attempt)
    .bind(package.revision_epoch)
    .bind(item.content_digest.trim())
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ProductionError::StalePackage)
}

async fn validate_current_artifact_versions(
    tx: &mut Transaction<'_, Postgres>,
    context: &PromotionContext,
    package: &PackageIdentity,
    story: &ArtifactContent,
    characters: &[CharacterArtifactContent],
    draft: &ArtifactContent,
) -> ProductionResult<()> {
    let latest_story = sqlx::query_scalar::<_, Option<i32>>(
        r#"
        SELECT MAX(version) FROM story_bibles
        WHERE production_project_id = $1 AND run_id = $2
          AND revision_epoch = $3 AND audit_status = 'complete'
        "#,
    )
    .bind(context.production_project_id)
    .bind(context.run_id)
    .bind(package.revision_epoch)
    .fetch_one(&mut **tx)
    .await?;
    let latest_draft = sqlx::query_scalar::<_, Option<i32>>(
        r#"
        SELECT MAX(version) FROM script_drafts
        WHERE production_project_id = $1 AND run_id = $2
          AND revision_epoch = $3 AND audit_status = 'complete'
        "#,
    )
    .bind(context.production_project_id)
    .bind(context.run_id)
    .bind(package.revision_epoch)
    .fetch_one(&mut **tx)
    .await?;
    let latest_character_ids = sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT candidate.id
        FROM character_bibles candidate
        WHERE candidate.production_project_id = $1
          AND candidate.run_id = $2
          AND candidate.revision_epoch = $3
          AND candidate.audit_status = 'complete'
          AND candidate.version = (
              SELECT MAX(current.version)
              FROM character_bibles current
              WHERE current.production_project_id = candidate.production_project_id
                AND current.run_id = candidate.run_id
                AND current.revision_epoch = candidate.revision_epoch
                AND current.character_id = candidate.character_id
                AND current.audit_status = 'complete'
          )
        ORDER BY candidate.id
        "#,
    )
    .bind(context.production_project_id)
    .bind(context.run_id)
    .bind(package.revision_epoch)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .collect::<BTreeSet<_>>();
    let packaged_character_ids = characters
        .iter()
        .map(|character| character.id)
        .collect::<BTreeSet<_>>();
    if latest_story != Some(story.version)
        || latest_draft != Some(draft.version)
        || latest_character_ids != packaged_character_ids
    {
        return Err(ProductionError::StalePackage);
    }
    Ok(())
}

fn validate_artifact_content(content: &Value, stored_digest: &str) -> ProductionResult<()> {
    if canonical_digest(content)? != stored_digest.trim() {
        return Err(ProductionError::StalePackage);
    }
    Ok(())
}

async fn invalidate_superseded_downstream(
    tx: &mut Transaction<'_, Postgres>,
    context: &PromotionContext,
    parent: &ParentScript,
    replacement_script_id: Uuid,
) -> ProductionResult<()> {
    sqlx::query(
        r#"
        INSERT INTO production_script_invalidations (
            run_id, revision_epoch, source_script_id, replacement_script_id, reason
        ) VALUES ($1, $2, $3, $4, 'script_semantic_revision')
        "#,
    )
    .bind(context.run_id)
    .bind(context.current_revision_epoch)
    .bind(parent.id)
    .bind(replacement_script_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO production_package_invalidations (
            package_id, run_id, revision_epoch, source_script_id,
            replacement_script_id, reason
        )
        SELECT package.id, package.run_id, $2, $3, $4, 'script_semantic_revision'
        FROM artifact_package_snapshots package
        WHERE package.run_id = $1 AND package.package_type = 'production'
          AND package.revision_epoch < $2
        ON CONFLICT (package_id) DO NOTHING
        "#,
    )
    .bind(context.run_id)
    .bind(context.current_revision_epoch)
    .bind(parent.id)
    .bind(replacement_script_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        UPDATE work_plans plan
        SET status = 'invalidated', invalidated_at = NOW(), updated_at = NOW()
        FROM works work
        WHERE plan.work_id = work.id AND work.script_id = $1
          AND plan.status IN ('draft', 'ready')
        "#,
    )
    .bind(parent.id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        UPDATE work_versions version
        SET status = 'invalidated', invalidated_at = NOW(), updated_at = NOW()
        FROM works work
        WHERE version.work_id = work.id AND work.script_id = $1
          AND version.status = 'draft'
          AND NOT EXISTS (
              SELECT 1 FROM work_generation_runs run
              WHERE run.work_version_id = version.id
          )
        "#,
    )
    .bind(parent.id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_scene(
    tx: &mut Transaction<'_, Postgres>,
    script_id: Uuid,
    source: &ScriptSceneInput,
) -> ProductionResult<Scene> {
    let scene = Scene {
        id: Uuid::new_v4(),
        sequence: source.sequence as i32,
        narration: source.narration.clone(),
        visual_description: source.visual_description.clone(),
        emotion: source.emotion.clone(),
        duration_sec: source.duration_sec as i32,
    };
    sqlx::query(
        r#"
        INSERT INTO scenes (
            id, script_id, sequence, narration, visual_description, emotion, duration_sec
        ) VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(scene.id)
    .bind(script_id)
    .bind(scene.sequence)
    .bind(&scene.narration)
    .bind(&scene.visual_description)
    .bind(&scene.emotion)
    .bind(scene.duration_sec)
    .execute(&mut **tx)
    .await?;
    Ok(scene)
}

#[allow(clippy::too_many_arguments)]
async fn insert_domain_link(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    source_step_id: Uuid,
    revision_epoch: i32,
    link_type: &str,
    script_id: Option<Uuid>,
    scene_id: Option<Uuid>,
    target_version: String,
    target_digest: &str,
) -> ProductionResult<()> {
    sqlx::query(
        r#"
        INSERT INTO production_domain_links (
            run_id, source_step_id, revision_epoch, link_type,
            script_id, scene_id, target_version, target_digest
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(run_id)
    .bind(source_step_id)
    .bind(revision_epoch)
    .bind(link_type)
    .bind(script_id)
    .bind(scene_id)
    .bind(target_version)
    .bind(target_digest)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn unlock_ready_steps(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    revision_epoch: i32,
) -> ProductionResult<()> {
    sqlx::query(
        r#"
        UPDATE production_steps candidate
        SET status = 'queued', updated_at = NOW()
        WHERE candidate.run_id = $1
          AND candidate.revision_epoch = $2
          AND candidate.status = 'blocked'
          AND NOT EXISTS (
              SELECT 1
              FROM jsonb_array_elements_text(candidate.dependencies) dependency(step_key)
              WHERE NOT EXISTS (
                  SELECT 1 FROM production_steps completed
                  WHERE completed.run_id = candidate.run_id
                    AND completed.revision_epoch = candidate.revision_epoch
                    AND completed.step_key = dependency.step_key
                    AND completed.status = 'succeeded'
              )
          )
        "#,
    )
    .bind(run_id)
    .bind(revision_epoch)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn enqueue_unlocked_steps(
    tx: &mut Transaction<'_, Postgres>,
    run_id: Uuid,
    revision_epoch: i32,
) -> ProductionResult<()> {
    sqlx::query(
        r#"
        INSERT INTO production_wakeups (run_id, step_id)
        SELECT step.run_id, step.id
        FROM production_steps step
        WHERE step.run_id = $1 AND step.revision_epoch = $2 AND step.status = 'queued'
        ON CONFLICT (step_id, status) DO NOTHING
        "#,
    )
    .bind(run_id)
    .bind(revision_epoch)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn load_script(
    tx: &mut Transaction<'_, Postgres>,
    script_id: Uuid,
) -> ProductionResult<Script> {
    let row = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            Option<Uuid>,
            String,
            String,
            Value,
            String,
            Option<Uuid>,
            DateTime<Utc>,
            DateTime<Utc>,
        ),
    >(
        r#"
        SELECT id, project_id, topic_id, title, hook, content, status,
               parent_id, created_at, updated_at
        FROM scripts WHERE id = $1
        "#,
    )
    .bind(script_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| ProductionError::TransitionConflict {
        reason: "stored promoted Script does not exist".into(),
    })?;
    let scenes = sqlx::query_as::<_, (Uuid, i32, String, String, String, i32)>(
        r#"
        SELECT id, sequence, narration, visual_description, emotion, duration_sec
        FROM scenes WHERE script_id = $1 ORDER BY sequence
        "#,
    )
    .bind(script_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|scene| Scene {
        id: scene.0,
        sequence: scene.1,
        narration: scene.2,
        visual_description: scene.3,
        emotion: scene.4,
        duration_sec: scene.5,
    })
    .collect();
    let status = ScriptStatus::try_from(row.6.as_str()).map_err(|error| {
        ProductionError::TransitionConflict {
            reason: error.to_string(),
        }
    })?;
    Ok(Script::new(
        row.0, row.1, row.2, row.3, row.4, row.5, status, row.7, scenes, row.8, row.9,
    ))
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}
