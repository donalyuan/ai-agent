use novex_api::application::script_package_promotion::{
    ScriptPackagePromotionCommand, ScriptPackagePromotionService,
};
use novex_api::application::script_version_governance::{
    DirectorSceneReference, ProductionArtifactOwner, ScriptRevisionCommand, ScriptRevisionScope,
    ScriptVersionGovernanceService,
};
use novex_production_crew::durable::{
    canonical_digest,
    package::{ArtifactPackageSnapshot, ArtifactRef, GateDecision, PackageType},
    plan::{FullCrewPlanRegistry, ResourceLimits},
    repository::{
        CreateCollaborationSuggestionCommand, CreateIntentCommand, DurableProductionRepository,
        PackageDecisionCommand, ProductionActor, StartRunCommand, SuggestionDecision,
    },
};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

mod support;
use support::test_database::{insert_enabled_text_model, TestDatabase};

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@biga-postgres:5432/video_agent".into())
}

fn with_database_name(database_url: &str, database_name: &str) -> String {
    let (base, query) = database_url
        .split_once('?')
        .map_or((database_url, ""), |(base, _)| {
            (base, &database_url[base.len()..])
        });
    let slash = base.rfind('/').unwrap();
    format!("{}{}{}", &base[..=slash], database_name, query)
}

async fn database() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let database_name = format!("script_promotion_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);
    let admin = PgPoolOptions::new()
        .max_connections(2)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{}""#, database_name))
        .execute(&admin)
        .await
        .unwrap();
    let guard = TestDatabase::new(&admin_url, &database_name);
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin, pool, guard)
}

fn plan() -> novex_production_crew::durable::plan::PlanSnapshot {
    let bindings = [
        "producer",
        "screenwriter",
        "director",
        "cinematographer",
        "performance_director",
        "sound_director",
        "editor",
        "qc",
    ]
    .into_iter()
    .map(|role| {
        (
            role.to_string(),
            json!({
                "definition_key": format!("production.{role}"),
                "definition_version": "1.0.0",
                "lifecycle": "active"
            }),
        )
    })
    .collect::<serde_json::Map<_, _>>();
    FullCrewPlanRegistry::snapshot_v1(
        false,
        Value::Object(bindings),
        ResourceLimits::strict_default(),
    )
    .unwrap()
}

fn production_package_metadata(items: &[ArtifactRef]) -> Value {
    let run_id = items[0].run_id;
    let script_id = Uuid::new_v5(&run_id, b"old-production-script");
    let scene_id = Uuid::new_v5(&run_id, b"old-production-scene");
    let character_bible_id = Uuid::new_v5(&run_id, b"old-production-character");
    let shot_id = items
        .iter()
        .find(|item| item.artifact_type == "shot_contract")
        .unwrap()
        .artifact_id;
    let performance_id = items
        .iter()
        .find(|item| item.artifact_type == "performance_brief")
        .unwrap()
        .artifact_id;
    let sound_id = items
        .iter()
        .find(|item| item.artifact_type == "sound_plan")
        .unwrap()
        .artifact_id;
    json!({
        "script_id": script_id,
        "script_version": script_id.to_string(),
        "script_digest": format!("{:064x}", 90),
        "scenes": [{
            "scene_id": scene_id,
            "scene_version": scene_id.to_string(),
            "scene_digest": format!("{:064x}", 91),
            "sequence": 1,
            "duration_sec": 1,
            "character_bible_ids": [character_bible_id]
        }],
        "characters": [{
            "character_bible_id": character_bible_id,
            "character_id": "lead"
        }],
        "shots": [{
            "artifact_id": shot_id,
            "shot_id": "shot-1",
            "sequence": 1,
            "scene_id": scene_id,
            "duration_sec": 1,
            "character_bible_ids": [character_bible_id]
        }],
        "performance_briefs": [{
            "artifact_id": performance_id,
            "script_id": script_id,
            "character_bible_id": character_bible_id,
            "character_id": "lead",
            "scene_ids": [scene_id]
        }],
        "sound_plan": {
            "artifact_id": sound_id,
            "script_id": script_id,
            "scene_ids": [scene_id]
        },
        "suggestion_resolutions": []
    })
}

#[derive(Clone)]
struct PromotionFixture {
    project_id: Uuid,
    topic_id: Uuid,
    intent_id: Uuid,
    run_id: Uuid,
    screenwriter_step_id: Uuid,
    package_id: Uuid,
    package_digest: String,
}

async fn insert_source(pool: &PgPool, suffix: &str) -> (Uuid, Uuid) {
    let project_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO projects (name, positioning, status) VALUES ($1, '知识视频', 'active') RETURNING id",
    )
    .bind(format!("晋升账号-{suffix}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let topic_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO content_topics (
            project_id, title, angle, target_audience, hook_points,
            content_type, tags, status
        ) VALUES (
            $1, $2, '工程审计', '开发者', ARRAY['事务一致性'],
            'knowledge', ARRAY['production'], 'approved'
        ) RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(format!("持久脚本晋升-{suffix}"))
    .fetch_one(pool)
    .await
    .unwrap();
    (project_id, topic_id)
}

async fn promotion_fixture(pool: &PgPool, suffix: &str) -> PromotionFixture {
    let (project_id, topic_id) = insert_source(pool, suffix).await;
    let repository = DurableProductionRepository::new(pool.clone());
    let intent = repository
        .create_intent(CreateIntentCommand {
            project_id,
            topic_id,
            title: format!("制作意图-{suffix}"),
            description: Some("ScriptPackagePromotion contract".into()),
            initial_input: json!({"brief": "使用确定性脚本晋升"}),
            actor: ProductionActor::local_operator(),
            idempotency_key: format!("create-{suffix}"),
        })
        .await
        .unwrap();
    let run = repository
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: format!("start-{suffix}"),
        })
        .await
        .unwrap();
    let run_steps = repository.get_run(run.id).await.unwrap().steps;
    let screenwriter_step_id = run_steps
        .iter()
        .find(|step| step.step_key == "screenwriter")
        .unwrap()
        .id;
    let suggestion_resolution_step_id = run_steps
        .iter()
        .find(|step| step.step_key == "character_suggestion_resolution")
        .unwrap()
        .id;
    sqlx::query(
        "UPDATE production_steps SET status = 'succeeded', attempt = 1 WHERE id IN ($1, $2)",
    )
    .bind(screenwriter_step_id)
    .bind(suggestion_resolution_step_id)
    .execute(pool)
    .await
    .unwrap();

    let story = json!({
        "premise": "可靠工作流必须保留精确事实",
        "themes": ["一致性", "可恢复性"],
        "narrative_arc": {
            "setup": "问题出现",
            "conflict": "并发写入",
            "climax": "事务阻断",
            "resolution": "原子提交"
        }
    });
    let character = json!({
        "name": "工程师",
        "archetype": "讲述者",
        "personality_traits": ["严谨"],
        "visual_description": "站在控制台前的工程师"
    });
    let draft = json!({
        "title": "一次完整提交",
        "hook": "一个失败点，为什么不能留下半个脚本？",
        "scenes": [
            {
                "sequence": 1,
                "narration": "流程从已批准选题开始。",
                "visual_description": "选题卡片进入制作流水线",
                "emotion": "专注",
                "duration_sec": 8,
                "character_ids": ["engineer"]
            },
            {
                "sequence": 2,
                "narration": "脚本、分镜和状态在同一事务提交。",
                "visual_description": "数据库事务完整提交",
                "emotion": "笃定",
                "duration_sec": 10,
                "character_ids": ["engineer"]
            }
        ]
    });
    let story_digest = canonical_digest(&story).unwrap();
    let character_digest = canonical_digest(&character).unwrap();
    let draft_digest = canonical_digest(&draft).unwrap();

    let story_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO story_bibles (
            production_project_id, version, content, run_id, step_id, attempt,
            revision_epoch, content_digest, applied_suggestion_ids, audit_status
        ) VALUES ($1, 1, $2, $3, $4, 1, 0, $5, '[]'::jsonb, 'complete')
        RETURNING id
        "#,
    )
    .bind(intent.id)
    .bind(&story)
    .bind(run.id)
    .bind(screenwriter_step_id)
    .bind(&story_digest)
    .fetch_one(pool)
    .await
    .unwrap();
    let character_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO character_bibles (
            production_project_id, character_id, version, content, run_id, step_id,
            attempt, revision_epoch, content_digest, applied_suggestion_ids, audit_status
        ) VALUES (
            $1, 'engineer', 1, $2, $3, $4, 1, 0, $5, '[]'::jsonb, 'complete'
        ) RETURNING id
        "#,
    )
    .bind(intent.id)
    .bind(&character)
    .bind(run.id)
    .bind(screenwriter_step_id)
    .bind(&character_digest)
    .fetch_one(pool)
    .await
    .unwrap();
    let draft_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO script_drafts (
            production_project_id, version, content, run_id, step_id, attempt,
            revision_epoch, content_digest, applied_suggestion_ids, audit_status
        ) VALUES ($1, 1, $2, $3, $4, 1, 0, $5, '[]'::jsonb, 'complete')
        RETURNING id
        "#,
    )
    .bind(intent.id)
    .bind(&draft)
    .bind(run.id)
    .bind(screenwriter_step_id)
    .bind(&draft_digest)
    .fetch_one(pool)
    .await
    .unwrap();

    let artifact = |artifact_type: &str, artifact_id: Uuid, digest: String| ArtifactRef {
        run_id: run.id,
        artifact_type: artifact_type.into(),
        artifact_id,
        version: 1,
        content_digest: digest,
        source_step_id: screenwriter_step_id,
        source_attempt: 1,
    };
    let package = ArtifactPackageSnapshot::build(
        PackageType::Script,
        run.id,
        screenwriter_step_id,
        1,
        0,
        1,
        vec![
            artifact("story_bible", story_id, story_digest),
            artifact("character_bible", character_id, character_digest),
            artifact("script_draft", draft_id, draft_digest),
        ],
        json!({}),
    )
    .unwrap();
    repository.save_package(&package).await.unwrap();
    repository
        .decide_package(PackageDecisionCommand {
            run_id: run.id,
            package_digest: package.package_digest.clone(),
            decision: GateDecision::Approve,
            reason: None,
            affected_owners: Vec::new(),
            actor: ProductionActor::local_operator(),
            idempotency_key: format!("approve-{suffix}"),
        })
        .await
        .unwrap();

    PromotionFixture {
        project_id,
        topic_id,
        intent_id: intent.id,
        run_id: run.id,
        screenwriter_step_id,
        package_id: package.id,
        package_digest: package.package_digest,
    }
}

fn promotion_command(fixture: &PromotionFixture, key: &str) -> ScriptPackagePromotionCommand {
    ScriptPackagePromotionCommand {
        run_id: fixture.run_id,
        package_digest: fixture.package_digest.clone(),
        actor: ProductionActor::local_operator(),
        idempotency_key: key.into(),
    }
}

async fn assert_no_promotion_side_effect(pool: &PgPool, fixture: &PromotionFixture) {
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scripts WHERE production_run_id = $1",)
            .bind(fixture.run_id)
            .fetch_one(pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_domain_links WHERE run_id = $1 AND link_type IN ('script', 'scene')",
        )
        .bind(fixture.run_id)
        .fetch_one(pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM content_topics WHERE id = $1")
            .bind(fixture.topic_id)
            .fetch_one(pool)
            .await
            .unwrap(),
        "approved"
    );
}

#[tokio::test]
async fn approved_script_package_is_promoted_with_script_scenes_links_and_topic_atomically() {
    let (_admin, pool, _guard) = database().await;
    let fixture = promotion_fixture(&pool, "success").await;
    let service = ScriptPackagePromotionService::new(pool.clone());

    let script = service
        .promote(promotion_command(&fixture, "promote-success"))
        .await
        .unwrap();

    assert_eq!(script.project_id, fixture.project_id);
    assert_eq!(script.topic_id, Some(fixture.topic_id));
    assert_eq!(script.status.as_str(), "approved");
    assert_eq!(script.scenes.len(), 2);
    assert_eq!(script.scenes[0].sequence, 1);
    assert_eq!(script.scenes[1].sequence, 2);

    let source = sqlx::query_as::<_, (Uuid, Uuid, String, Value, Value, i32)>(
        r#"
        SELECT production_run_id, script_package_id, script_package_digest,
               topic_snapshot, source_artifacts, source_revision_epoch
        FROM scripts WHERE id = $1
        "#,
    )
    .bind(script.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(source.0, fixture.run_id);
    assert_eq!(source.1, fixture.package_id);
    assert_eq!(source.2.trim(), fixture.package_digest);
    assert_eq!(source.3["id"], json!(fixture.topic_id));
    assert_eq!(source.4.as_array().unwrap().len(), 3);
    assert_eq!(source.5, 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM content_topics WHERE id = $1")
            .bind(fixture.topic_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "scripted"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_domain_links WHERE run_id = $1 AND link_type IN ('script', 'scene')",
        )
        .bind(fixture.run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        3
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_steps WHERE run_id = $1 AND step_key = 'promote_script'",
        )
        .bind(fixture.run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "succeeded"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_steps WHERE run_id = $1 AND step_key = 'director'",
        )
        .bind(fixture.run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "queued"
    );
}

#[tokio::test]
async fn database_failure_rolls_back_every_promotion_side_effect() {
    let (_admin, pool, _guard) = database().await;
    let fixture = promotion_fixture(&pool, "rollback").await;
    sqlx::query(
        r#"
        CREATE FUNCTION fail_second_promoted_scene() RETURNS TRIGGER AS $$
        BEGIN
            IF NEW.sequence = 2 THEN
                RAISE EXCEPTION 'injected scene persistence failure';
            END IF;
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        CREATE TRIGGER fail_second_promoted_scene
            BEFORE INSERT ON scenes
            FOR EACH ROW EXECUTE FUNCTION fail_second_promoted_scene()
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let error = ScriptPackagePromotionService::new(pool.clone())
        .promote(promotion_command(&fixture, "promote-rollback"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "database_error");
    assert_no_promotion_side_effect(&pool, &fixture).await;
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_steps WHERE run_id = $1 AND step_key = 'promote_script'",
        )
        .bind(fixture.run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "queued"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_commands WHERE aggregate_id = $1 AND command_type = 'promote_script'",
        )
        .bind(fixture.run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn stale_package_is_rejected_when_a_constituent_has_a_newer_version() {
    let (_admin, pool, _guard) = database().await;
    let fixture = promotion_fixture(&pool, "stale").await;
    let newer_draft = json!({
        "title": "更新脚本",
        "hook": "新的组成产物使旧批准失效",
        "scenes": [{
            "sequence": 1,
            "narration": "新版旁白",
            "visual_description": "新版画面",
            "emotion": "清晰",
            "duration_sec": 8,
            "character_ids": ["engineer"]
        }]
    });
    sqlx::query(
        r#"
        INSERT INTO script_drafts (
            production_project_id, version, content, run_id, step_id, attempt,
            revision_epoch, content_digest, applied_suggestion_ids, audit_status
        ) VALUES ($1, 2, $2, $3, $4, 1, 0, $5, '[]'::jsonb, 'complete')
        "#,
    )
    .bind(fixture.intent_id)
    .bind(&newer_draft)
    .bind(fixture.run_id)
    .bind(fixture.screenwriter_step_id)
    .bind(canonical_digest(&newer_draft).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let error = ScriptPackagePromotionService::new(pool.clone())
        .promote(promotion_command(&fixture, "promote-stale"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "stale_package");
    assert_no_promotion_side_effect(&pool, &fixture).await;
}

#[tokio::test]
async fn cross_project_deleted_and_consumed_topics_are_rejected_without_partial_writes() {
    let (_admin, pool, _guard) = database().await;
    let cross_project = promotion_fixture(&pool, "cross-project").await;
    let (other_project_id, _) = insert_source(&pool, "other-project").await;
    sqlx::query("UPDATE production_projects SET project_id = $2 WHERE id = $1")
        .bind(cross_project.intent_id)
        .bind(other_project_id)
        .execute(&pool)
        .await
        .unwrap();
    let error = ScriptPackagePromotionService::new(pool.clone())
        .promote(promotion_command(&cross_project, "promote-cross-project"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "source_invalid");
    assert_no_promotion_side_effect(&pool, &cross_project).await;

    let deleted = promotion_fixture(&pool, "deleted").await;
    sqlx::query("UPDATE production_projects SET status = 'failed' WHERE id = $1")
        .bind(deleted.intent_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE content_topics SET deleted_at = NOW() WHERE id = $1")
        .bind(deleted.topic_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE production_projects SET status = 'active' WHERE id = $1")
        .bind(deleted.intent_id)
        .execute(&pool)
        .await
        .unwrap();
    let error = ScriptPackagePromotionService::new(pool.clone())
        .promote(promotion_command(&deleted, "promote-deleted"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "source_invalid");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scripts WHERE production_run_id = $1",)
            .bind(deleted.run_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );

    let consumed = promotion_fixture(&pool, "consumed").await;
    sqlx::query("UPDATE production_projects SET status = 'failed' WHERE id = $1")
        .bind(consumed.intent_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE content_topics SET status = 'scripted' WHERE id = $1")
        .bind(consumed.topic_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE production_projects SET status = 'active' WHERE id = $1")
        .bind(consumed.intent_id)
        .execute(&pool)
        .await
        .unwrap();
    let error = ScriptPackagePromotionService::new(pool.clone())
        .promote(promotion_command(&consumed, "promote-consumed"))
        .await
        .unwrap_err();
    assert_eq!(error.code(), "transition_conflict");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scripts WHERE production_run_id = $1",)
            .bind(consumed.run_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn repeated_and_concurrent_promotion_return_the_original_script() {
    let (_admin, pool, _guard) = database().await;
    let fixture = promotion_fixture(&pool, "concurrent").await;
    let service = ScriptPackagePromotionService::new(pool.clone());
    let command = promotion_command(&fixture, "promote-concurrent");

    let (left, right) = tokio::join!(
        service.promote(command.clone()),
        service.promote(command.clone())
    );
    let left = left.unwrap();
    let right = right.unwrap();
    assert_eq!(left.id, right.id);
    assert_eq!(left.scenes, right.scenes);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scripts WHERE production_run_id = $1",)
            .bind(fixture.run_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );

    let replay = service.promote(command.clone()).await.unwrap();
    assert_eq!(replay.id, left.id);
    let conflicting = ScriptPackagePromotionCommand {
        package_digest: "f".repeat(64),
        ..command
    };
    let error = service.promote(conflicting).await.unwrap_err();
    assert_eq!(error.code(), "idempotency_conflict");
}

async fn attach_director_suggestion(
    pool: &PgPool,
    fixture: &PromotionFixture,
    script_id: Uuid,
) -> Uuid {
    let repository = DurableProductionRepository::new(pool.clone());
    let director_step = repository
        .get_run(fixture.run_id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.revision_epoch == 0 && step.step_key == "director")
        .unwrap();
    let model_id = insert_enabled_text_model(pool).await;
    let agent_run_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO agent_runs (agent_type, status) VALUES ('production', 'running') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let model_call_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO model_calls (
            agent_run_id, node_key, attempt, status, agent_key, agent_version,
            prompt_key, prompt_version, registry_digest, prompt_snapshot,
            model_id, behavior_fingerprint, model_snapshot, completed_at
        ) VALUES (
            $1, 'production.director', 1, 'succeeded',
            'production.director', '3.0.0', 'production.director', '3.0.0',
            $2, '{"system":"fixture","user":"fixture"}', $3, $4,
            '{"provider":"test"}', NOW()
        ) RETURNING id
        "#,
    )
    .bind(agent_run_id)
    .bind("1".repeat(64))
    .bind(model_id)
    .bind("2".repeat(64))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        UPDATE production_steps
        SET status = 'succeeded', attempt = 1, model_call_id = $2,
            completed_at = NOW(), updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(director_step.id)
    .bind(model_call_id)
    .execute(pool)
    .await
    .unwrap();
    let script_digest = sqlx::query_scalar::<_, String>(
        r#"
        SELECT target_digest FROM production_domain_links
        WHERE run_id = $1 AND link_type = 'script' AND script_id = $2
        "#,
    )
    .bind(fixture.run_id)
    .bind(script_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let suggestion = repository
        .create_collaboration_suggestion(CreateCollaborationSuggestionCommand {
            run_id: fixture.run_id,
            source_step_id: director_step.id,
            source_attempt: 1,
            source_model_call_id: model_call_id,
            from_role: "director".into(),
            to_role: "screenwriter".into(),
            target_artifact_type: "script".into(),
            target_artifact_id: script_id,
            target_artifact_version: 1,
            target_content_digest: script_digest.trim().into(),
            suggestion_type: "revision".into(),
            content: json!({"semantic_change": "强化第二幕的核心叙事"}),
            blocking: true,
        })
        .await
        .unwrap();
    repository
        .respond_to_collaboration_suggestion(
            suggestion.id,
            SuggestionDecision::Accepted,
            None,
            ProductionActor::local_operator(),
        )
        .await
        .unwrap();
    suggestion.id
}

async fn save_old_production_package(pool: &PgPool, fixture: &PromotionFixture) -> Uuid {
    let repository = DurableProductionRepository::new(pool.clone());
    let steps = repository.get_run(fixture.run_id).await.unwrap().steps;
    let step_id = |key: &str| {
        steps
            .iter()
            .find(|step| step.revision_epoch == 0 && step.step_key == key)
            .unwrap()
            .id
    };
    let director = step_id("director");
    let performance = step_id("performance_director");
    let sound = step_id("sound_director");
    sqlx::query(
        "UPDATE production_steps SET status = 'succeeded', attempt = 1 WHERE id IN ($1, $2, $3)",
    )
    .bind(director)
    .bind(performance)
    .bind(sound)
    .execute(pool)
    .await
    .unwrap();
    let artifact = |artifact_type: &str, seed: i32, source_step_id: Uuid| ArtifactRef {
        run_id: fixture.run_id,
        artifact_type: artifact_type.into(),
        artifact_id: Uuid::new_v4(),
        version: 1,
        content_digest: format!("{seed:064x}"),
        source_step_id,
        source_attempt: 1,
    };
    let items = vec![
        artifact("directorial_treatment", 31, director),
        artifact("shot_contract", 32, director),
        artifact("performance_brief", 33, performance),
        artifact("sound_plan", 34, sound),
    ];
    let metadata = production_package_metadata(&items);
    let package = ArtifactPackageSnapshot::build(
        PackageType::Production,
        fixture.run_id,
        sound,
        1,
        0,
        1,
        items,
        metadata,
    )
    .unwrap();
    repository.save_package(&package).await.unwrap();
    package.id
}

async fn save_work_history(
    pool: &PgPool,
    fixture: &PromotionFixture,
    script_id: Uuid,
) -> (Uuid, Uuid) {
    let work_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO works (project_id, script_id, title, status)
        VALUES ($1, $2, '旧脚本作品', 'planned') RETURNING id
        "#,
    )
    .bind(fixture.project_id)
    .bind(script_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let draft_version_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_versions (
            work_id, version_no, source_manifest_version, input_snapshot,
            model_snapshot, parameter_snapshot, timeline_snapshot, prompt_snapshot, status
        ) VALUES (
            $1, 1, 'manifest-old-draft', '{}'::jsonb, '{}'::jsonb,
            '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, 'draft'
        ) RETURNING id
        "#,
    )
    .bind(work_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let draft_plan_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_plans (
            work_id, work_version_id, plan_version, status, input_fingerprint,
            capability_snapshot, output_snapshot, prompt_snapshot, timeline_snapshot
        ) VALUES (
            $1, $2, 1, 'ready', $3, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb
        ) RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(draft_version_id)
    .bind("a".repeat(64))
    .fetch_one(pool)
    .await
    .unwrap();
    let historical_version_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_versions (
            work_id, version_no, source_manifest_version, input_snapshot,
            model_snapshot, parameter_snapshot, timeline_snapshot, prompt_snapshot, status
        ) VALUES (
            $1, 2, 'manifest-old-confirmed', '{}'::jsonb, '{}'::jsonb,
            '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, 'confirmed'
        ) RETURNING id
        "#,
    )
    .bind(work_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let historical_plan_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_plans (
            work_id, work_version_id, plan_version, status, input_fingerprint,
            capability_snapshot, output_snapshot, prompt_snapshot, timeline_snapshot
        ) VALUES (
            $1, $2, 2, 'confirmed', $3, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb
        ) RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(historical_version_id)
    .bind("b".repeat(64))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO work_generation_runs (
            work_id, work_version_id, work_plan_id, idempotency_key, status,
            model_snapshot, capability_snapshot, prompt_snapshot,
            timeline_snapshot, parameter_snapshot
        ) VALUES (
            $1, $2, $3, 'historical-run', 'succeeded', '{}'::jsonb,
            '{}'::jsonb, '{}'::jsonb, '{}'::jsonb, '{}'::jsonb
        )
        "#,
    )
    .bind(work_id)
    .bind(historical_version_id)
    .bind(historical_plan_id)
    .execute(pool)
    .await
    .unwrap();
    (draft_plan_id, historical_plan_id)
}

async fn save_revised_script_package(pool: &PgPool, fixture: &PromotionFixture, epoch: i32) {
    let repository = DurableProductionRepository::new(pool.clone());
    let run_steps = repository.get_run(fixture.run_id).await.unwrap().steps;
    let screenwriter_step_id = run_steps
        .iter()
        .find(|step| step.revision_epoch == epoch && step.step_key == "screenwriter")
        .unwrap()
        .id;
    let suggestion_resolution_step_id = run_steps
        .iter()
        .find(|step| {
            step.revision_epoch == epoch && step.step_key == "character_suggestion_resolution"
        })
        .unwrap()
        .id;
    sqlx::query(
        "UPDATE production_steps SET status = 'succeeded', attempt = 1 WHERE id IN ($1, $2)",
    )
    .bind(screenwriter_step_id)
    .bind(suggestion_resolution_step_id)
    .execute(pool)
    .await
    .unwrap();
    let story = json!({"premise": "修订后的叙事", "themes": ["一致性"]});
    let character = json!({"name": "工程师", "visual_description": "工程师"});
    let draft = json!({
        "title": "一次完整提交（修订版）",
        "hook": "核心叙事已经明确修订",
        "scenes": [{
            "sequence": 1,
            "narration": "修订后的核心旁白。",
            "visual_description": "新的第一幕画面",
            "emotion": "坚定",
            "duration_sec": 9,
            "character_ids": ["engineer"]
        }]
    });
    let story_digest = canonical_digest(&story).unwrap();
    let character_digest = canonical_digest(&character).unwrap();
    let draft_digest = canonical_digest(&draft).unwrap();
    let story_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO story_bibles (
            production_project_id, version, content, run_id, step_id, attempt,
            revision_epoch, content_digest, applied_suggestion_ids, audit_status
        ) VALUES ($1, 2, $2, $3, $4, 1, $5, $6, '[]'::jsonb, 'complete') RETURNING id
        "#,
    )
    .bind(fixture.intent_id)
    .bind(&story)
    .bind(fixture.run_id)
    .bind(screenwriter_step_id)
    .bind(epoch)
    .bind(&story_digest)
    .fetch_one(pool)
    .await
    .unwrap();
    let character_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO character_bibles (
            production_project_id, character_id, version, content, run_id, step_id,
            attempt, revision_epoch, content_digest, applied_suggestion_ids, audit_status
        ) VALUES (
            $1, 'engineer', 2, $2, $3, $4, 1, $5, $6, '[]'::jsonb, 'complete'
        ) RETURNING id
        "#,
    )
    .bind(fixture.intent_id)
    .bind(&character)
    .bind(fixture.run_id)
    .bind(screenwriter_step_id)
    .bind(epoch)
    .bind(&character_digest)
    .fetch_one(pool)
    .await
    .unwrap();
    let draft_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO script_drafts (
            production_project_id, version, content, run_id, step_id, attempt,
            revision_epoch, content_digest, applied_suggestion_ids, audit_status
        ) VALUES ($1, 2, $2, $3, $4, 1, $5, $6, '[]'::jsonb, 'complete') RETURNING id
        "#,
    )
    .bind(fixture.intent_id)
    .bind(&draft)
    .bind(fixture.run_id)
    .bind(screenwriter_step_id)
    .bind(epoch)
    .bind(&draft_digest)
    .fetch_one(pool)
    .await
    .unwrap();
    let artifact = |artifact_type: &str, artifact_id: Uuid, digest: String| ArtifactRef {
        run_id: fixture.run_id,
        artifact_type: artifact_type.into(),
        artifact_id,
        version: 2,
        content_digest: digest,
        source_step_id: screenwriter_step_id,
        source_attempt: 1,
    };
    let package = ArtifactPackageSnapshot::build(
        PackageType::Script,
        fixture.run_id,
        screenwriter_step_id,
        1,
        epoch as u32,
        1,
        vec![
            artifact("story_bible", story_id, story_digest),
            artifact("character_bible", character_id, character_digest),
            artifact("script_draft", draft_id, draft_digest),
        ],
        json!({}),
    )
    .unwrap();
    repository.save_package(&package).await.unwrap();
    repository
        .decide_package(PackageDecisionCommand {
            run_id: fixture.run_id,
            package_digest: package.package_digest,
            decision: GateDecision::Approve,
            reason: None,
            affected_owners: Vec::new(),
            actor: ProductionActor::local_operator(),
            idempotency_key: format!("approve-revision-{epoch}"),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn director_scene_references_accept_only_current_formal_script_scenes() {
    let (_admin, pool, _guard) = database().await;
    let fixture = promotion_fixture(&pool, "scene-reference").await;
    let script = ScriptPackagePromotionService::new(pool.clone())
        .promote(promotion_command(&fixture, "promote-scene-reference"))
        .await
        .unwrap();
    let service = ScriptVersionGovernanceService::new(pool.clone());
    let references = script
        .scenes
        .iter()
        .map(|scene| DirectorSceneReference { scene_id: scene.id })
        .collect::<Vec<_>>();
    service
        .validate_director_scene_references(fixture.run_id, script.id, &references)
        .await
        .unwrap();

    let foreign_script_id = Uuid::new_v4();
    let foreign_scene_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO scripts (id, project_id, title, hook, content, status)
        VALUES ($1, $2, '外部脚本', '外部', '{}'::jsonb, 'approved')
        "#,
    )
    .bind(foreign_script_id)
    .bind(fixture.project_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO scenes (
            id, script_id, sequence, narration, visual_description, emotion, duration_sec
        ) VALUES ($1, $2, 1, '外部', '外部', '平静', 5)
        "#,
    )
    .bind(foreign_scene_id)
    .bind(foreign_script_id)
    .execute(&pool)
    .await
    .unwrap();
    let error = service
        .validate_director_scene_references(
            fixture.run_id,
            script.id,
            &[DirectorSceneReference {
                scene_id: foreign_scene_id,
            }],
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), "invalid_artifact_schema");
    assert!(serde_json::from_value::<DirectorSceneReference>(json!({
        "scene_id": "scene-1"
    }))
    .is_err());
    assert!(serde_json::from_value::<DirectorSceneReference>(json!({
        "scene_number": 1
    }))
    .is_err());
}

#[tokio::test]
async fn semantic_revision_creates_child_script_and_invalidates_only_unconfirmed_downstream() {
    let (_admin, pool, _guard) = database().await;
    let fixture = promotion_fixture(&pool, "semantic-revision").await;
    let promotion = ScriptPackagePromotionService::new(pool.clone());
    let original = promotion
        .promote(promotion_command(&fixture, "promote-original"))
        .await
        .unwrap();
    let old_production_package_id = save_old_production_package(&pool, &fixture).await;
    let suggestion_id = attach_director_suggestion(&pool, &fixture, original.id).await;
    let (draft_plan_id, historical_plan_id) = save_work_history(&pool, &fixture, original.id).await;
    let governance = ScriptVersionGovernanceService::new(pool.clone());
    let revision = governance
        .request_revision(ScriptRevisionCommand {
            run_id: fixture.run_id,
            current_script_id: original.id,
            scope: ScriptRevisionScope::ScriptSemantic {
                director_suggestion_id: suggestion_id,
            },
            reason: "核心旁白需要语义修订".into(),
            instruction: "强化第二幕的核心叙事，但保留已确认事实".into(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "semantic-revision".into(),
        })
        .await
        .unwrap();
    assert_eq!(revision.revision_epoch, 1);
    assert!(revision.requires_new_script_package);
    let instruction =
        sqlx::query_as::<_, (i32, String, String, String, String, String, String, String)>(
            r#"
        SELECT revision_epoch, owner_role, actor_type, actor_id, source, trust,
               instruction, instruction_digest
        FROM production_revision_instructions
        WHERE run_id = $1 AND revision_epoch = $2
        "#,
        )
        .bind(fixture.run_id)
        .bind(revision.revision_epoch)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(instruction.0, 1);
    assert_eq!(instruction.1, "screenwriter");
    assert_eq!(instruction.2, "local_operator");
    assert_eq!(instruction.3, "local_operator");
    assert_eq!(instruction.4, "script_revision_command");
    assert_eq!(instruction.5, "user_instruction");
    assert_eq!(instruction.6, "强化第二幕的核心叙事，但保留已确认事实");
    assert_eq!(
        instruction.7,
        canonical_digest(&"强化第二幕的核心叙事，但保留已确认事实").unwrap()
    );
    let old_package_digest = sqlx::query_scalar::<_, String>(
        "SELECT package_digest FROM artifact_package_snapshots WHERE id = $1",
    )
    .bind(old_production_package_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let stale_error = DurableProductionRepository::new(pool.clone())
        .decide_package(PackageDecisionCommand {
            run_id: fixture.run_id,
            package_digest: old_package_digest.trim().into(),
            decision: GateDecision::Approve,
            reason: None,
            affected_owners: vec![],
            actor: ProductionActor::local_operator(),
            idempotency_key: "stale-production-package-after-revision".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(stale_error.code(), "stale_package");
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_steps WHERE run_id = $1 AND revision_epoch = 1 AND step_key = 'screenwriter'",
        )
        .bind(fixture.run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "queued"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scripts WHERE production_run_id = $1",)
            .bind(fixture.run_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );

    save_revised_script_package(&pool, &fixture, 1).await;
    let package_digest = sqlx::query_scalar::<_, String>(
        r#"
        SELECT package_digest FROM artifact_package_snapshots
        WHERE run_id = $1 AND revision_epoch = 1 AND package_type = 'script'
        "#,
    )
    .bind(fixture.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let revised = promotion
        .promote(ScriptPackagePromotionCommand {
            run_id: fixture.run_id,
            package_digest: package_digest.trim().into(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "promote-revised".into(),
        })
        .await
        .unwrap();
    assert_eq!(revised.parent_id, Some(original.id));
    assert_ne!(revised.id, original.id);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT title FROM scripts WHERE id = $1")
            .bind(original.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        original.title
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_package_invalidations WHERE package_id = $1 AND replacement_script_id = $2",
        )
        .bind(old_production_package_id)
        .bind(revised.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_script_invalidations WHERE source_script_id = $1 AND replacement_script_id = $2",
        )
        .bind(original.id)
        .bind(revised.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    for mutation in [
        "UPDATE production_script_invalidations SET reason = reason WHERE source_script_id = $1 AND replacement_script_id = $2",
        "DELETE FROM production_script_invalidations WHERE source_script_id = $1 AND replacement_script_id = $2",
    ] {
        let error = sqlx::query(mutation)
            .bind(original.id)
            .bind(revised.id)
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("append-only"));
    }
    for mutation in [
        "UPDATE production_package_invalidations SET reason = reason WHERE package_id = $1 AND replacement_script_id = $2",
        "DELETE FROM production_package_invalidations WHERE package_id = $1 AND replacement_script_id = $2",
    ] {
        let error = sqlx::query(mutation)
            .bind(old_production_package_id)
            .bind(revised.id)
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("append-only"));
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM work_plans WHERE id = $1")
            .bind(draft_plan_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "invalidated"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM work_plans WHERE id = $1")
            .bind(historical_plan_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "confirmed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT version.status FROM work_versions version JOIN work_plans plan ON plan.work_version_id = version.id WHERE plan.id = $1",
        )
        .bind(draft_plan_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "invalidated"
    );
}

#[tokio::test]
async fn production_expression_revision_reopens_only_owner_without_creating_script_version() {
    let (_admin, pool, _guard) = database().await;
    let fixture = promotion_fixture(&pool, "expression-revision").await;
    let original = ScriptPackagePromotionService::new(pool.clone())
        .promote(promotion_command(&fixture, "promote-expression-base"))
        .await
        .unwrap();
    let director_step_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM production_steps WHERE run_id = $1 AND revision_epoch = 0 AND step_key = 'director'",
    )
    .bind(fixture.run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE production_steps SET status = 'succeeded', attempt = 1 WHERE id = $1")
        .bind(director_step_id)
        .execute(&pool)
        .await
        .unwrap();

    let revision = ScriptVersionGovernanceService::new(pool.clone())
        .request_revision(ScriptRevisionCommand {
            run_id: fixture.run_id,
            current_script_id: original.id,
            scope: ScriptRevisionScope::ProductionExpression {
                owner: ProductionArtifactOwner::Director,
            },
            reason: "只调整镜头语言".into(),
            instruction: "保持旁白与 Scene 结构不变，调整镜头运动".into(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "expression-revision".into(),
        })
        .await
        .unwrap();
    assert!(!revision.requires_new_script_package);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scripts WHERE production_run_id = $1",)
            .bind(fixture.run_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_steps WHERE run_id = $1 AND revision_epoch = 1 AND step_key = 'director'",
        )
        .bind(fixture.run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "queued"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_steps WHERE run_id = $1 AND revision_epoch = 1 AND step_key = 'screenwriter'",
        )
        .bind(fixture.run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "succeeded"
    );
}
