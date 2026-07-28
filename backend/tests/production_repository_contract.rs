use async_trait::async_trait;
use novex_api::application::production_cancellation::{
    ProductionCancellationService, WorkCancellationPortError, WorkGenerationCancellationPort,
};
use novex_api::application::{
    asset_generation::AssetGenerationService,
    production_workflow_integration::{
        ProductionWorkVersionReworkService, ProductionWorkflowIntegrationService,
    },
    script_package_promotion::{ScriptPackagePromotionCommand, ScriptPackagePromotionService},
    work_generation::WorkGenerationService,
};
use novex_api::repositories::{
    PostgresAiModelRepository, PostgresAssetGenerationRepository, PostgresMaterialRepository,
    PostgresScriptRepository, PostgresVoiceCatalogRepository, PostgresWorkGenerationRepository,
    PostgresWorkLibraryRepository,
};
use novex_production_crew::durable::package::{
    ArtifactPackageSnapshot, ArtifactRef, GateDecision, PackageType, ProductionPackageMetadata,
};
use novex_production_crew::durable::production_input::ProductionPackageInput;
use novex_production_crew::durable::{
    canonical_digest,
    command_store::{
        ProductionAggregateType, ProductionCommandScope, ProductionCommandStore,
        ProductionCommandType,
    },
    media::{ComposeInput, FinalMediaAsset, MediaEvidenceSnapshot, RequiredTakeInventorySnapshot},
    plan::{FullCrewPlanRegistry, ResourceLimits},
    repository::{
        CreateCollaborationSuggestionCommand, CreateIntentCommand, DurableProductionRepository,
        ExternalCancellationState, PackageDecisionCommand, ProductionActor, RetryStepCommand,
        StartRunCommand, SuggestionDecision,
    },
    resource::ResourceRequest,
};
use novex_production_crew::orchestrator::application_port::{
    MediaEvidenceAnalysis, MediaEvidenceProvider, ProductionWorkPlanOverrides,
    ProductionWorkPlanRequest, ProductionWorkPlanSettings, SceneDurationOverride,
    ScenePromptOverride, SceneVisualManifestPort, TemporaryMediaAccess, WorkGenerationPlanningPort,
    WorkGenerationRunDisposition, WorkGenerationRunReference, WorkGenerationRunStatus,
    WorkVersionReworkKind, WorkVersionReworkPort, WorkVersionReworkReference,
    WorkVersionReworkRequest,
};
use novex_production_crew::{
    gates::{quality_gate::QualityGateOutcome, GateRegistry},
    orchestrator::ProductionOrchestrator,
    roles::RoleRegistry,
};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
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
        .map_or((database_url, ""), |(base, _query)| {
            (base, &database_url[base.len()..])
        });
    let slash = base.rfind('/').unwrap();
    format!("{}{}{}", &base[..=slash], database_name, query)
}

async fn database() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let database_name = format!("full_crew_repo_{}", Uuid::new_v4().simple());
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
        .max_connections(8)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin, pool, guard)
}

async fn insert_enabled_video_model(pool: &PgPool) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, protocol_version,
            auth_scheme, request_base_url, upstream_model, api_key, settings, status
        ) VALUES (
            'Full Crew 测试视频模型', 'video', 'test', 'volcengine_ark_video',
            'test-video-v1', 'bearer', 'https://example.invalid/v1', 'test-video',
            'test-key',
            '{"min_duration_seconds":4,"max_duration_seconds":15,"max_reference_images":9,"max_prompt_chars":500,"aspect_ratios":["9:16","16:9"],"resolutions":["1080p"],"generate_audio":true}'::jsonb,
            'enabled'
        ) RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_enabled_tos_staging_config(pool: &PgPool) {
    sqlx::query(
        r#"
        INSERT INTO tos_staging_tool_configs (
            version,is_current,is_enabled,storage_provider,endpoint,region,bucket,
            object_prefix,access_key,secret_key,signed_url_ttl_seconds,max_file_bytes,
            max_audio_duration_seconds,last_check_status,last_check_requested_at,last_checked_at
        ) VALUES (
            1,TRUE,TRUE,'volcengine_tos','https://tos.example.invalid','test-region',
            'private-test','novex/test','test-ak','test-sk',600,10485760,3600,
            'succeeded',NOW(),NOW()
        )
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

async fn insert_enabled_tts_voice(pool: &PgPool) -> (Uuid, String) {
    let model_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO ai_models (
            display_name,model_type,provider_name,api_protocol,protocol_version,
            auth_scheme,request_base_url,upstream_model,api_key,catalog_access_key,
            catalog_secret_key,settings,status
        ) VALUES (
            'Full Crew 测试 TTS','speech','test','volcengine_tts_v3','v3','api_key',
            'https://example.invalid/tts','doubao-seed-tts-2.0','runtime-key','catalog-ak','catalog-sk',
            '{"resource_id":"seed-tts-2.0","supported_audio_formats":["mp3","wav"],"default_audio_format":"mp3","supported_sample_rates":[24000],"default_sample_rate":24000,"max_input_characters":3000,"max_audio_duration_seconds":null,"supports_word_timestamps":true,"word_timestamp_languages":["zh-cn"],"catalog_sync_interval_minutes":1440,"parameters":{}}'::jsonb,
            'enabled'
        ) RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let sync_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO voice_catalog_syncs (
            model_id,trigger_source,status,page_count,speaker_count,started_at,completed_at
        ) VALUES ($1,'workspace','succeeded',1,1,NOW(),NOW()) RETURNING id
        "#,
    )
    .bind(model_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let voice_type = "zh_female_clear".to_string();
    sqlx::query(
        r#"
        INSERT INTO voice_catalog_entries (
            model_id,voice_type,resource_id,name,languages,first_seen_sync_id,last_seen_sync_id
        ) VALUES ($1,$2,'seed-tts-2.0','清晰女声','[{"Language":"zh-cn"}]',$3,$3)
        "#,
    )
    .bind(model_id)
    .bind(&voice_type)
    .bind(sync_id)
    .execute(pool)
    .await
    .unwrap();
    (model_id, voice_type)
}

async fn insert_selected_visuals(
    pool: &PgPool,
    project_id: Uuid,
    script_id: Uuid,
    scene_ids: &[Uuid],
    prefix: &str,
) {
    for scene_id in scene_ids {
        let material_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO materials (project_id,material_type,file_url,file_name,status)
            VALUES ($1,'image',$2,$3,'active') RETURNING id
            "#,
        )
        .bind(project_id)
        .bind(format!("/materials/{prefix}-{scene_id}.png"))
        .bind(format!("{prefix}-{scene_id}.png"))
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO scene_asset_candidates (
                project_id,script_id,scene_id,material_id,candidate_type,source,status,rank
            ) VALUES ($1,$2,$3,$4,'image','existing_material','selected',0)
            "#,
        )
        .bind(project_id)
        .bind(script_id)
        .bind(scene_id)
        .bind(material_id)
        .execute(pool)
        .await
        .unwrap();
    }
}

async fn source(pool: &PgPool) -> (Uuid, Uuid) {
    let project_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO projects (name, positioning, status) VALUES ('测试账号', '知识视频', 'active') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let topic_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO content_topics (project_id, title, angle, target_audience, status)
        VALUES ($1, '持久工作流', '工程审计', '开发者', 'approved') RETURNING id
        "#,
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (project_id, topic_id)
}

fn create_command(project_id: Uuid, topic_id: Uuid, key: &str) -> CreateIntentCommand {
    CreateIntentCommand {
        project_id,
        topic_id,
        title: "Full Crew 测试".into(),
        description: Some("repository contract".into()),
        initial_input: json!({"brief": "严格按来源执行"}),
        actor: ProductionActor::local_operator(),
        idempotency_key: key.into(),
    }
}

fn plan() -> novex_production_crew::durable::plan::PlanSnapshot {
    plan_with_limits(ResourceLimits::strict_default())
}

fn plan_with_limits(limits: ResourceLimits) -> novex_production_crew::durable::plan::PlanSnapshot {
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
    FullCrewPlanRegistry::snapshot_v1(false, serde_json::Value::Object(bindings), limits).unwrap()
}

#[tokio::test]
async fn unified_production_command_store_scopes_replay_and_digest_conflicts() {
    let (_admin, pool, _guard) = database().await;
    let actor = ProductionActor::local_operator();
    let run_id = Uuid::new_v4();
    let key = "same-client-key";
    let approve = ProductionCommandScope::new(
        actor.clone(),
        ProductionCommandType::ApprovePackage,
        ProductionAggregateType::ProductionRun,
        run_id,
        key,
    );
    let request = json!({"run_id": run_id, "package_digest": format!("{:064x}", 1)});
    let digest = ProductionCommandStore::canonical_request_digest(&request).unwrap();
    let result = json!({"decision_id": Uuid::new_v4()});

    let mut tx = pool.begin().await.unwrap();
    assert!(ProductionCommandStore::replay(&mut tx, &approve, &digest)
        .await
        .unwrap()
        .is_none());
    ProductionCommandStore::record(&mut tx, &approve, &digest, result.clone())
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        ProductionCommandStore::replay(&mut tx, &approve, &digest)
            .await
            .unwrap(),
        Some(result)
    );
    let changed = ProductionCommandStore::canonical_request_digest(
        &json!({"run_id": run_id, "package_digest": format!("{:064x}", 2)}),
    )
    .unwrap();
    assert_eq!(
        ProductionCommandStore::replay(&mut tx, &approve, &changed)
            .await
            .unwrap_err()
            .code(),
        "idempotency_conflict"
    );

    let reject = ProductionCommandScope::new(
        actor.clone(),
        ProductionCommandType::RejectPackage,
        ProductionAggregateType::ProductionRun,
        run_id,
        key,
    );
    assert!(ProductionCommandStore::replay(&mut tx, &reject, &changed)
        .await
        .unwrap()
        .is_none());
    let other_run = ProductionCommandScope::new(
        actor,
        ProductionCommandType::ApprovePackage,
        ProductionAggregateType::ProductionRun,
        Uuid::new_v4(),
        key,
    );
    assert!(
        ProductionCommandStore::replay(&mut tx, &other_run, &changed)
            .await
            .unwrap()
            .is_none()
    );
    tx.rollback().await.unwrap();
}

fn contains_json_key(value: &serde_json::Value, target: &str) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            object.contains_key(target)
                || object
                    .values()
                    .any(|child| contains_json_key(child, target))
        }
        serde_json::Value::Array(items) => {
            items.iter().any(|child| contains_json_key(child, target))
        }
        _ => false,
    }
}

fn production_package_metadata(
    items: &[ArtifactRef],
    suggestion_resolutions: Vec<serde_json::Value>,
) -> serde_json::Value {
    let run_id = items[0].run_id;
    let script_id = Uuid::new_v5(&run_id, b"fixture-script");
    let scene_id = Uuid::new_v5(&run_id, b"fixture-scene");
    let shot_items = items
        .iter()
        .filter(|item| item.artifact_type == "shot_contract")
        .collect::<Vec<_>>();
    let performance_items = items
        .iter()
        .filter(|item| item.artifact_type == "performance_brief")
        .collect::<Vec<_>>();
    let character_ids = performance_items
        .iter()
        .map(|item| Uuid::new_v5(&run_id, item.artifact_id.as_bytes()))
        .collect::<Vec<_>>();
    let sound_id = items
        .iter()
        .find(|item| item.artifact_type == "sound_plan")
        .unwrap()
        .artifact_id;
    json!({
        "script_id": script_id,
        "script_version": script_id.to_string(),
        "script_digest": format!("{:064x}", 900),
        "scenes": [{
            "scene_id": scene_id,
            "scene_version": scene_id.to_string(),
            "scene_digest": format!("{:064x}", 901),
            "sequence": 1,
            "duration_sec": shot_items.len(),
            "character_bible_ids": character_ids
        }],
        "characters": performance_items.iter().enumerate().map(|(index, item)| json!({
            "character_bible_id": Uuid::new_v5(&run_id, item.artifact_id.as_bytes()),
            "character_id": format!("character-{index}")
        })).collect::<Vec<_>>(),
        "shots": shot_items.iter().enumerate().map(|(index, item)| json!({
            "artifact_id": item.artifact_id,
            "shot_id": format!("shot-{index}"),
            "sequence": index + 1,
            "scene_id": scene_id,
            "duration_sec": 1,
            "character_bible_ids": character_ids
        })).collect::<Vec<_>>(),
        "performance_briefs": performance_items.iter().enumerate().map(|(index, item)| json!({
            "artifact_id": item.artifact_id,
            "script_id": script_id,
            "character_bible_id": Uuid::new_v5(&run_id, item.artifact_id.as_bytes()),
            "character_id": format!("character-{index}"),
            "scene_ids": [scene_id]
        })).collect::<Vec<_>>(),
        "sound_plan": {
            "artifact_id": sound_id,
            "script_id": script_id,
            "scene_ids": [scene_id]
        },
        "suggestion_resolutions": suggestion_resolutions
    })
}

async fn seed_production_package_scope(
    pool: &PgPool,
    run_id: Uuid,
    production_project_id: Uuid,
    project_id: Uuid,
    topic_id: Uuid,
) -> (Uuid, Vec<Uuid>) {
    let steps = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, step_key FROM production_steps
        WHERE run_id=$1 AND revision_epoch=0
          AND step_key IN ('screenwriter', 'promote_script', 'director',
                           'performance_director', 'sound_director')
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(id, key)| (key, id))
    .collect::<std::collections::BTreeMap<_, _>>();
    let screenwriter = steps["screenwriter"];
    let promote_script = steps["promote_script"];
    let director = steps["director"];
    let performance = steps["performance_director"];
    let sound = steps["sound_director"];
    sqlx::query("UPDATE production_steps SET status='succeeded', attempt=1 WHERE id = ANY($1)")
        .bind(vec![
            screenwriter,
            promote_script,
            director,
            performance,
            sound,
        ])
        .execute(pool)
        .await
        .unwrap();

    let character_id = Uuid::new_v4();
    let character_content = json!({
        "character_id": "lead",
        "name": "主角",
        "role": "lead",
        "personality": "果断",
        "motivation": "完成任务",
        "arc": "从犹豫到坚定"
    });
    let character_digest = canonical_digest(&character_content).unwrap();
    sqlx::query(
        r#"
        INSERT INTO character_bibles (
            id, production_project_id, character_id, version, status, content, created_by,
            run_id, step_id, attempt, revision_epoch, content_digest, audit_status
        ) VALUES ($1,$2,'lead',1,'draft',$3,'screenwriter',$4,$5,1,0,$6,'complete')
        "#,
    )
    .bind(character_id)
    .bind(production_project_id)
    .bind(&character_content)
    .bind(run_id)
    .bind(screenwriter)
    .bind(&character_digest)
    .execute(pool)
    .await
    .unwrap();
    let draft_id = Uuid::new_v4();
    let draft_content = json!({
        "title": "精确拼包",
        "hook": "拒绝跨 Run 最新产物",
        "scenes": [
            {"sequence": 1, "narration": "第一幕", "visual_description": "场景一", "emotion": "专注", "duration_sec": 5, "character_ids": ["lead"]},
            {"sequence": 2, "narration": "第二幕", "visual_description": "场景二", "emotion": "坚定", "duration_sec": 5, "character_ids": ["lead"]}
        ]
    });
    let draft_digest = canonical_digest(&draft_content).unwrap();
    sqlx::query(
        r#"
        INSERT INTO script_drafts (
            id, production_project_id, version, status, content, created_by,
            run_id, step_id, attempt, revision_epoch, content_digest, audit_status
        ) VALUES ($1,$2,1,'draft',$3,'screenwriter',$4,$5,1,0,$6,'complete')
        "#,
    )
    .bind(draft_id)
    .bind(production_project_id)
    .bind(&draft_content)
    .bind(run_id)
    .bind(screenwriter)
    .bind(&draft_digest)
    .execute(pool)
    .await
    .unwrap();

    let script_package_id = Uuid::new_v4();
    let script_package_digest = "1".repeat(64);
    sqlx::query(
        r#"
        INSERT INTO artifact_package_snapshots (
            id, run_id, source_step_id, source_attempt, revision_epoch,
            package_type, package_version, package_digest, schema_version, metadata
        ) VALUES ($1,$2,$3,1,0,'script',1,$4,'1.0.0','{}')
        "#,
    )
    .bind(script_package_id)
    .bind(run_id)
    .bind(screenwriter)
    .bind(&script_package_digest)
    .execute(pool)
    .await
    .unwrap();
    let script_id = Uuid::new_v4();
    let source_artifacts = json!([
        {"artifact_type": "character_bible", "artifact_id": character_id, "artifact_version": 1, "content_digest": character_digest, "source_step_id": screenwriter, "source_attempt": 1},
        {"artifact_type": "script_draft", "artifact_id": draft_id, "artifact_version": 1, "content_digest": draft_digest, "source_step_id": screenwriter, "source_attempt": 1}
    ]);
    sqlx::query(
        r#"
        INSERT INTO scripts (
            id, project_id, topic_id, title, hook, content, status,
            production_run_id, script_package_id, script_package_digest,
            topic_snapshot, source_artifacts, source_revision_epoch
        ) VALUES ($1,$2,$3,'精确拼包','拒绝跨 Run 最新产物','{}','approved',
                  $4,$5,$6,'{}',$7,0)
        "#,
    )
    .bind(script_id)
    .bind(project_id)
    .bind(topic_id)
    .bind(run_id)
    .bind(script_package_id)
    .bind(&script_package_digest)
    .bind(&source_artifacts)
    .execute(pool)
    .await
    .unwrap();

    let mut scene_ids = Vec::new();
    for sequence in 1..=2 {
        let scene_id = Uuid::new_v4();
        let scene_digest = canonical_digest(&json!({
            "sequence": sequence,
            "narration": format!("第{sequence}幕"),
            "visual_description": format!("场景{sequence}"),
            "emotion": if sequence == 1 { "专注" } else { "坚定" },
            "duration_sec": 5
        }))
        .unwrap();
        sqlx::query(
            "INSERT INTO scenes (id,script_id,sequence,narration,visual_description,emotion,duration_sec) VALUES ($1,$2,$3,$4,$5,$6,5)",
        )
        .bind(scene_id)
        .bind(script_id)
        .bind(sequence)
        .bind(format!("第{sequence}幕"))
        .bind(format!("场景{sequence}"))
        .bind(if sequence == 1 { "专注" } else { "坚定" })
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO production_domain_links (
                run_id, source_step_id, revision_epoch, link_type, scene_id,
                target_version, target_digest
            ) VALUES ($1,$2,0,'scene',$3,$4,$5)
            "#,
        )
        .bind(run_id)
        .bind(promote_script)
        .bind(scene_id)
        .bind(scene_id.to_string())
        .bind(scene_digest)
        .execute(pool)
        .await
        .unwrap();
        scene_ids.push(scene_id);
    }
    sqlx::query(
        r#"
        INSERT INTO production_domain_links (
            run_id, source_step_id, revision_epoch, link_type, script_id,
            target_version, target_digest
        ) VALUES ($1,$2,0,'script',$3,$4,$5)
        "#,
    )
    .bind(run_id)
    .bind(promote_script)
    .bind(script_id)
    .bind(script_id.to_string())
    .bind(canonical_digest(&json!({"script_id": script_id})).unwrap())
    .execute(pool)
    .await
    .unwrap();

    let treatment_content = json!({
        "visual_style": "纪实",
        "pacing": "紧凑",
        "emotional_arc": "坚定",
        "color_palette": ["neutral"],
        "reference_works": []
    });
    let treatment_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO directorial_treatments (
            id, production_project_id, version, status, content, created_by,
            run_id, step_id, attempt, revision_epoch, content_digest, audit_status
        ) VALUES ($1,$2,1,'draft',$3,'director',$4,$5,1,0,$6,'complete')
        "#,
    )
    .bind(treatment_id)
    .bind(production_project_id)
    .bind(&treatment_content)
    .bind(run_id)
    .bind(director)
    .bind(canonical_digest(&treatment_content).unwrap())
    .execute(pool)
    .await
    .unwrap();

    for (index, scene_id) in scene_ids.iter().enumerate() {
        let shot_id = Uuid::new_v4();
        let shot_content = json!({
            "shot_id": format!("shot-{}", index + 1),
            "sequence": index + 1,
            "scene_id": scene_id,
            "shot_type": "medium",
            "camera_movement": "static",
            "duration_sec": 5,
            "description": "正式镜头",
            "character_ids": ["lead"]
        });
        sqlx::query(
            r#"
            INSERT INTO shot_contracts (
                id, production_project_id, shot_id, scene_id, domain_scene_id, version,
                status, content, created_by, run_id, step_id, attempt, revision_epoch,
                content_digest, audit_status
            ) VALUES ($1,$2,$3,$4,$5,1,'draft',$6,'director',$7,$8,1,0,$9,'complete')
            "#,
        )
        .bind(shot_id)
        .bind(production_project_id)
        .bind(format!("shot-{}", index + 1))
        .bind(scene_id.to_string())
        .bind(scene_id)
        .bind(&shot_content)
        .bind(run_id)
        .bind(director)
        .bind(canonical_digest(&shot_content).unwrap())
        .execute(pool)
        .await
        .unwrap();
    }
    let performance_content = json!({
        "character_bible_id": character_id,
        "character_id": "lead",
        "script_id": script_id,
        "emotional_arc": [
            {"sequence": 1, "scene_id": scene_ids[0], "emotion": "专注", "intensity": 5, "notes": "保持克制"},
            {"sequence": 2, "scene_id": scene_ids[1], "emotion": "坚定", "intensity": 8, "notes": "明确收束"}
        ],
        "body_language": "稳定",
        "vocal_direction": "清晰"
    });
    sqlx::query(
        r#"
        INSERT INTO performance_briefs (
            production_project_id, character_id, character_bible_id, script_id,
            version, status, content, created_by, run_id, step_id, attempt,
            revision_epoch, content_digest, audit_status
        ) VALUES ($1,'lead',$2,$3,1,'draft',$4,'performance_director',$5,$6,1,0,$7,'complete')
        "#,
    )
    .bind(production_project_id)
    .bind(character_id)
    .bind(script_id)
    .bind(&performance_content)
    .bind(run_id)
    .bind(performance)
    .bind(canonical_digest(&performance_content).unwrap())
    .execute(pool)
    .await
    .unwrap();
    let sound_content = json!({
        "script_id": script_id,
        "music_style": "极简",
        "scene_sound_notes": [
            {"sequence": 1, "scene_id": scene_ids[0], "music_cue": "起", "sfx_notes": ["环境声"], "dialogue_direction": "平稳"},
            {"sequence": 2, "scene_id": scene_ids[1], "music_cue": "收", "sfx_notes": ["提示音"], "dialogue_direction": "坚定"}
        ]
    });
    sqlx::query(
        r#"
        INSERT INTO sound_plans (
            production_project_id, script_id, version, status, content, created_by,
            run_id, step_id, attempt, revision_epoch, content_digest, audit_status
        ) VALUES ($1,$2,1,'draft',$3,'sound_director',$4,$5,1,0,$6,'complete')
        "#,
    )
    .bind(production_project_id)
    .bind(script_id)
    .bind(&sound_content)
    .bind(run_id)
    .bind(sound)
    .bind(canonical_digest(&sound_content).unwrap())
    .execute(pool)
    .await
    .unwrap();

    // 旧查询若按 ProductionProject 取 MAX(version)，会错误选中这条 legacy 产物。
    sqlx::query(
        r#"
        INSERT INTO shot_contracts (
            production_project_id, shot_id, scene_id, version, status, content,
            created_by, audit_status
        ) VALUES ($1,'legacy-latest',$2,99,'draft','{}','director','legacy_partial_audit')
        "#,
    )
    .bind(production_project_id)
    .bind(scene_ids[0].to_string())
    .execute(pool)
    .await
    .unwrap();
    (script_id, scene_ids)
}

async fn seed_e2e_script_package(
    pool: &PgPool,
    repository: &DurableProductionRepository,
    production_project_id: Uuid,
    run_id: Uuid,
) -> ArtifactPackageSnapshot {
    let steps = repository.get_run(run_id).await.unwrap().steps;
    let screenwriter_step_id = steps
        .iter()
        .find(|step| step.step_key == "screenwriter")
        .unwrap()
        .id;
    let resolution_step_id = steps
        .iter()
        .find(|step| step.step_key == "character_suggestion_resolution")
        .unwrap()
        .id;
    sqlx::query("UPDATE production_steps SET status='succeeded',attempt=1 WHERE id IN ($1,$2)")
        .bind(screenwriter_step_id)
        .bind(resolution_step_id)
        .execute(pool)
        .await
        .unwrap();

    let story = json!({
        "premise": "可靠制作必须沿用同一组正式事实",
        "themes": ["一致性", "可恢复性"],
        "narrative_arc": {
            "setup": "选题获批", "conflict": "多阶段制作",
            "climax": "媒体质量检查", "resolution": "审计闭合"
        }
    });
    let character = json!({
        "character_id": "engineer", "name": "工程师", "role": "讲述者",
        "personality": "严谨", "motivation": "确保制作事实可恢复",
        "arc": "从检查流程到确认审计闭合"
    });
    let draft = json!({
        "title": "一条可恢复的制作链",
        "hook": "一次确认如何贯穿脚本、成片与质检？",
        "scenes": [
            {
                "sequence": 1, "narration": "流程从已批准选题开始。",
                "visual_description": "选题进入制作流水线", "emotion": "专注",
                "duration_sec": 5, "character_ids": ["engineer"]
            },
            {
                "sequence": 2, "narration": "全部事实最终闭合为质量证据。",
                "visual_description": "成片通过媒体质检", "emotion": "笃定",
                "duration_sec": 5, "character_ids": ["engineer"]
            }
        ]
    });
    let story_digest = canonical_digest(&story).unwrap();
    let character_digest = canonical_digest(&character).unwrap();
    let draft_digest = canonical_digest(&draft).unwrap();
    let story_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO story_bibles (
            production_project_id,version,content,run_id,step_id,attempt,
            revision_epoch,content_digest,applied_suggestion_ids,audit_status
        ) VALUES ($1,1,$2,$3,$4,1,0,$5,'[]','complete') RETURNING id
        "#,
    )
    .bind(production_project_id)
    .bind(&story)
    .bind(run_id)
    .bind(screenwriter_step_id)
    .bind(&story_digest)
    .fetch_one(pool)
    .await
    .unwrap();
    let character_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO character_bibles (
            production_project_id,character_id,version,content,run_id,step_id,attempt,
            revision_epoch,content_digest,applied_suggestion_ids,audit_status
        ) VALUES ($1,'engineer',1,$2,$3,$4,1,0,$5,'[]','complete') RETURNING id
        "#,
    )
    .bind(production_project_id)
    .bind(&character)
    .bind(run_id)
    .bind(screenwriter_step_id)
    .bind(&character_digest)
    .fetch_one(pool)
    .await
    .unwrap();
    let draft_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO script_drafts (
            production_project_id,version,content,run_id,step_id,attempt,
            revision_epoch,content_digest,applied_suggestion_ids,audit_status
        ) VALUES ($1,1,$2,$3,$4,1,0,$5,'[]','complete') RETURNING id
        "#,
    )
    .bind(production_project_id)
    .bind(&draft)
    .bind(run_id)
    .bind(screenwriter_step_id)
    .bind(&draft_digest)
    .fetch_one(pool)
    .await
    .unwrap();
    let artifact = |artifact_type: &str, artifact_id: Uuid, content_digest: String| ArtifactRef {
        run_id,
        artifact_type: artifact_type.into(),
        artifact_id,
        version: 1,
        content_digest,
        source_step_id: screenwriter_step_id,
        source_attempt: 1,
    };
    ArtifactPackageSnapshot::build(
        PackageType::Script,
        run_id,
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
    .unwrap()
}

async fn seed_e2e_production_artifacts(
    pool: &PgPool,
    repository: &DurableProductionRepository,
    production_project_id: Uuid,
    run_id: Uuid,
    script_id: Uuid,
    scene_ids: &[Uuid],
) -> Vec<(Uuid, Uuid)> {
    let steps = repository
        .get_run(run_id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .map(|step| (step.step_key, step.id))
        .collect::<std::collections::BTreeMap<_, _>>();
    let completed = [
        steps["director"],
        steps["cinematographer"],
        steps["suggestion_resolution"],
        steps["performance_director"],
        steps["sound_director"],
    ];
    sqlx::query("UPDATE production_steps SET status='succeeded',attempt=1 WHERE id=ANY($1)")
        .bind(completed.to_vec())
        .execute(pool)
        .await
        .unwrap();
    let character_bible_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM character_bibles WHERE run_id=$1 AND character_id='engineer'",
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let treatment = json!({
        "visual_style": "纪实科技感", "pacing": "紧凑",
        "emotional_arc": "专注到笃定", "color_palette": ["neutral"],
        "reference_works": []
    });
    sqlx::query(
        r#"
        INSERT INTO directorial_treatments (
            production_project_id,version,status,content,created_by,run_id,step_id,
            attempt,revision_epoch,content_digest,audit_status
        ) VALUES ($1,1,'draft',$2,'director',$3,$4,1,0,$5,'complete')
        "#,
    )
    .bind(production_project_id)
    .bind(&treatment)
    .bind(run_id)
    .bind(steps["director"])
    .bind(canonical_digest(&treatment).unwrap())
    .execute(pool)
    .await
    .unwrap();

    let mut scene_shots = Vec::new();
    for (index, scene_id) in scene_ids.iter().enumerate() {
        let artifact_id = Uuid::new_v4();
        let content = json!({
            "shot_id": format!("e2e-shot-{}", index + 1), "sequence": index + 1,
            "scene_id": scene_id, "shot_type": "medium", "camera_movement": "static",
            "duration_sec": 5, "description": "E2E 正式镜头",
            "character_ids": ["engineer"]
        });
        sqlx::query(
            r#"
            INSERT INTO shot_contracts (
                id,production_project_id,shot_id,scene_id,domain_scene_id,version,status,
                content,created_by,run_id,step_id,attempt,revision_epoch,content_digest,audit_status
            ) VALUES ($1,$2,$3,$4,$4,1,'draft',$5,'director',$6,$7,1,0,$8,'complete')
            "#,
        )
        .bind(artifact_id)
        .bind(production_project_id)
        .bind(format!("e2e-shot-{}", index + 1))
        .bind(scene_id)
        .bind(&content)
        .bind(run_id)
        .bind(steps["director"])
        .bind(canonical_digest(&content).unwrap())
        .execute(pool)
        .await
        .unwrap();
        scene_shots.push((*scene_id, artifact_id));
    }
    let performance = json!({
        "character_bible_id": character_bible_id, "character_id": "engineer",
        "script_id": script_id,
        "emotional_arc": scene_ids.iter().enumerate().map(|(index, scene_id)| json!({
            "sequence": index + 1, "scene_id": scene_id,
            "emotion": if index == 0 { "专注" } else { "笃定" },
            "intensity": 7, "notes": "保持自然"
        })).collect::<Vec<_>>(),
        "body_language": "稳定", "vocal_direction": "清晰"
    });
    sqlx::query(
        r#"
        INSERT INTO performance_briefs (
            production_project_id,character_id,character_bible_id,script_id,version,status,
            content,created_by,run_id,step_id,attempt,revision_epoch,content_digest,audit_status
        ) VALUES ($1,'engineer',$2,$3,1,'draft',$4,'performance_director',$5,$6,1,0,$7,'complete')
        "#,
    )
    .bind(production_project_id)
    .bind(character_bible_id)
    .bind(script_id)
    .bind(&performance)
    .bind(run_id)
    .bind(steps["performance_director"])
    .bind(canonical_digest(&performance).unwrap())
    .execute(pool)
    .await
    .unwrap();
    let sound = json!({
        "script_id": script_id, "music_style": "极简",
        "scene_sound_notes": scene_ids.iter().enumerate().map(|(index, scene_id)| json!({
            "sequence": index + 1, "scene_id": scene_id,
            "music_cue": format!("cue-{}", index + 1), "sfx_notes": ["环境声"],
            "dialogue_direction": "清晰"
        })).collect::<Vec<_>>()
    });
    sqlx::query(
        r#"
        INSERT INTO sound_plans (
            production_project_id,script_id,version,status,content,created_by,run_id,
            step_id,attempt,revision_epoch,content_digest,audit_status
        ) VALUES ($1,$2,1,'draft',$3,'sound_director',$4,$5,1,0,$6,'complete')
        "#,
    )
    .bind(production_project_id)
    .bind(script_id)
    .bind(&sound)
    .bind(run_id)
    .bind(steps["sound_director"])
    .bind(canonical_digest(&sound).unwrap())
    .execute(pool)
    .await
    .unwrap();
    scene_shots
}

async fn complete_e2e_fake_generation(
    pool: &PgPool,
    work_version_id: Uuid,
    generation_run_id: Uuid,
) {
    let segments = sqlx::query_as::<_, (Uuid, serde_json::Value)>(
        "SELECT id,input_snapshot FROM work_generation_steps WHERE run_id=$1 AND step_type='video_segment' ORDER BY step_no,id",
    )
    .bind(generation_run_id)
    .fetch_all(pool)
    .await
    .unwrap();
    assert!(!segments.is_empty());
    for (index, (step_id, input)) in segments.iter().enumerate() {
        sqlx::query("UPDATE work_generation_steps SET status='succeeded' WHERE id=$1")
            .bind(step_id)
            .execute(pool)
            .await
            .unwrap();
        let attempt_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO work_generation_attempts (step_id,attempt_no,status,model_snapshot,resource_usage) VALUES ($1,1,'succeeded','{}','{}') RETURNING id",
        )
        .bind(step_id)
        .fetch_one(pool)
        .await
        .unwrap();
        let duration_ms = input
            .get("duration_seconds")
            .or_else(|| input.get("duration_sec"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(5)
            * 1_000;
        sqlx::query(
            r#"
            INSERT INTO work_artifacts (
                work_version_id,role,generation_step_id,file_name,storage_path,mime_type,
                size_bytes,sha256,metadata
            ) VALUES ($1,'reusable_intermediate',$2,$3,$4,'video/mp4',100,$5,$6)
            "#,
        )
        .bind(work_version_id)
        .bind(step_id)
        .bind(format!("e2e-segment-{}.mp4", index + 1))
        .bind(format!("works/e2e-segment-{}.mp4", index + 1))
        .bind(format!("{:064x}", 500 + index))
        .bind(json!({"duration_ms": duration_ms, "generation_attempt_id": attempt_id}))
        .execute(pool)
        .await
        .unwrap();
    }
    let mix_step_id = sqlx::query_scalar::<_, Uuid>(
        "UPDATE work_generation_steps SET status='succeeded' WHERE run_id=$1 AND step_type='mix' RETURNING id",
    )
    .bind(generation_run_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let (compose_step_id, compose_dependencies) = sqlx::query_as::<_, (Uuid, serde_json::Value)>(
        "UPDATE work_generation_steps SET status='succeeded' WHERE run_id=$1 AND step_type='compose' RETURNING id,depends_on",
    )
    .bind(generation_run_id)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(compose_dependencies, json!([mix_step_id]));
    let compose_attempt_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO work_generation_attempts (step_id,attempt_no,status,model_snapshot,resource_usage) VALUES ($1,1,'succeeded','{}','{}') RETURNING id",
    )
    .bind(compose_step_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO work_artifacts (
            work_version_id,role,generation_step_id,file_name,storage_path,mime_type,
            size_bytes,sha256,metadata
        ) VALUES ($1,'final_video',$2,'e2e-final.mp4','works/e2e-final.mp4',
                  'video/mp4',200,$3,$4)
        "#,
    )
    .bind(work_version_id)
    .bind(compose_step_id)
    .bind("f".repeat(64))
    .bind(json!({"duration_ms": 10_000, "generation_attempt_id": compose_attempt_id}))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("UPDATE work_generation_runs SET status='succeeded' WHERE id=$1")
        .bind(generation_run_id)
        .execute(pool)
        .await
        .unwrap();
}

async fn persist_e2e_quality_artifacts(
    pool: &PgPool,
    production_project_id: Uuid,
    run_id: Uuid,
    inventory: &RequiredTakeInventorySnapshot,
    evidence: &MediaEvidenceSnapshot,
    scene_shots: &[(Uuid, Uuid)],
) {
    let steps = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        UPDATE production_steps SET status='succeeded',attempt=1
        WHERE run_id=$1 AND revision_epoch=0 AND step_key IN ('editor','qc')
        RETURNING id,step_key
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(id, key)| (key, id))
    .collect::<std::collections::BTreeMap<_, _>>();
    let mut ledgers = std::collections::BTreeMap::new();
    for (index, (_, shot_id)) in scene_shots.iter().enumerate() {
        let ledger_id = Uuid::new_v4();
        let content = json!({
            "order": index + 1, "shot_contract_id": shot_id,
            "work_version_id": inventory.work_version_id,
            "inventory_id": inventory.inventory_id,
            "evidence_snapshot_id": evidence.evidence_id,
            "visual_facts": ["fake media inspection passed"], "continuity_flags": []
        });
        let digest = canonical_digest(&content).unwrap();
        sqlx::query(
            r#"
            INSERT INTO continuity_ledgers (
                id,production_project_id,content,created_by,run_id,step_id,attempt,
                revision_epoch,work_version_id,inventory_id,evidence_snapshot_id,
                shot_contract_id,version,content_digest,audit_status
            ) VALUES ($1,$2,$3,'editor',$4,$5,1,0,$6,$7,$8,$9,1,$10,'complete')
            "#,
        )
        .bind(ledger_id)
        .bind(production_project_id)
        .bind(&content)
        .bind(run_id)
        .bind(steps["editor"])
        .bind(inventory.work_version_id)
        .bind(inventory.inventory_id)
        .bind(evidence.evidence_id)
        .bind(shot_id)
        .bind(&digest)
        .execute(pool)
        .await
        .unwrap();
        ledgers.insert(*shot_id, (ledger_id, digest));
    }
    for take in &inventory.takes {
        let applicable_shots = take
            .scene_ids
            .iter()
            .flat_map(|scene_id| take.scene_shot_map[scene_id].iter().copied())
            .collect::<Vec<_>>();
        let content = json!({
            "required_take_id": take.take_id, "work_version_id": inventory.work_version_id,
            "inventory_id": inventory.inventory_id, "evidence_snapshot_id": evidence.evidence_id,
            "applicable_shot_contract_ids": applicable_shots,
            "review_status": "approved",
            "quality_assessment": {"visual": 9, "narrative": 9, "technical": 9},
            "issues": [], "suggestions": []
        });
        let review_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO take_reviews (
                id,production_project_id,status,content,created_by,run_id,step_id,attempt,
                revision_epoch,work_version_id,inventory_id,evidence_snapshot_id,
                required_take_id,version,content_digest,audit_status
            ) VALUES ($1,$2,'approved',$3,'qc',$4,$5,1,0,$6,$7,$8,$9,1,$10,'complete')
            "#,
        )
        .bind(review_id)
        .bind(production_project_id)
        .bind(&content)
        .bind(run_id)
        .bind(steps["qc"])
        .bind(inventory.work_version_id)
        .bind(inventory.inventory_id)
        .bind(evidence.evidence_id)
        .bind(take.take_id)
        .bind(canonical_digest(&content).unwrap())
        .execute(pool)
        .await
        .unwrap();
        for (ordinal, shot_id) in applicable_shots.iter().enumerate() {
            let (ledger_id, digest) = &ledgers[shot_id];
            sqlx::query(
                r#"
                INSERT INTO take_review_ledger_versions (
                    take_review_id,ordinal,continuity_ledger_id,shot_contract_id,
                    ledger_version,content_digest
                ) VALUES ($1,$2,$3,$4,1,$5)
                "#,
            )
            .bind(review_id)
            .bind(ordinal as i32)
            .bind(ledger_id)
            .bind(shot_id)
            .bind(digest)
            .execute(pool)
            .await
            .unwrap();
        }
    }
}

#[tokio::test]
async fn brief_and_script_package_builders_use_only_current_exact_role_attempt() {
    let (_admin, pool, _guard) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "role-package-scope"))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "role-package-scope-run".into(),
        })
        .await
        .unwrap();
    let old_steps = repo.get_run(run.id).await.unwrap().steps;
    let old_producer = old_steps
        .iter()
        .find(|step| step.step_key == "producer")
        .unwrap()
        .id;
    let old_screenwriter = old_steps
        .iter()
        .find(|step| step.step_key == "screenwriter")
        .unwrap()
        .id;
    sqlx::query(
        "UPDATE production_steps SET status='succeeded',attempt=1,side_effect_state='confirmed' WHERE id IN ($1,$2)",
    )
    .bind(old_producer)
    .bind(old_screenwriter)
    .execute(&pool)
    .await
    .unwrap();

    let old_brief = json!({"epoch":0,"kind":"brief"});
    let old_story = json!({"epoch":0,"kind":"story"});
    let old_character = json!({"epoch":0,"kind":"character"});
    let old_draft = json!({"epoch":0,"kind":"draft"});
    sqlx::query("INSERT INTO creative_briefs (production_project_id,version,content,run_id,step_id,attempt,revision_epoch,content_digest,audit_status) VALUES ($1,1,$2,$3,$4,1,0,$5,'complete')")
        .bind(intent.id).bind(&old_brief).bind(run.id).bind(old_producer)
        .bind(canonical_digest(&old_brief).unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO story_bibles (production_project_id,version,content,run_id,step_id,attempt,revision_epoch,content_digest,audit_status) VALUES ($1,1,$2,$3,$4,1,0,$5,'complete')")
        .bind(intent.id).bind(&old_story).bind(run.id).bind(old_screenwriter)
        .bind(canonical_digest(&old_story).unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO character_bibles (production_project_id,character_id,version,content,run_id,step_id,attempt,revision_epoch,content_digest,audit_status) VALUES ($1,'old-character',1,$2,$3,$4,1,0,$5,'complete')")
        .bind(intent.id).bind(&old_character).bind(run.id).bind(old_screenwriter)
        .bind(canonical_digest(&old_character).unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO script_drafts (production_project_id,version,content,run_id,step_id,attempt,revision_epoch,content_digest,audit_status) VALUES ($1,1,$2,$3,$4,1,0,$5,'complete')")
        .bind(intent.id).bind(&old_draft).bind(run.id).bind(old_screenwriter)
        .bind(canonical_digest(&old_draft).unwrap()).execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO production_revision_epochs (run_id,epoch,reason_type,reason,affected_owners,actor_type,actor_id) VALUES ($1,1,'script_semantic_revision','验证当前 epoch 精确拼包','[\"producer\",\"screenwriter\"]','local_operator','local_operator')")
        .bind(run.id).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO production_steps (run_id,revision_epoch,plan_order,step_key,step_type,role_key,dependencies,status,attempt,side_effect_state) SELECT run_id,1,plan_order,step_key,step_type,role_key,dependencies,'succeeded',2,'confirmed' FROM production_steps WHERE run_id=$1 AND revision_epoch=0 AND step_key IN ('producer','screenwriter')")
        .bind(run.id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE production_runs SET current_revision_epoch=1 WHERE id=$1")
        .bind(run.id)
        .execute(&pool)
        .await
        .unwrap();
    let current_steps = repo.get_run(run.id).await.unwrap().steps;
    let current_producer = current_steps
        .iter()
        .find(|step| step.revision_epoch == 1 && step.step_key == "producer")
        .unwrap()
        .id;
    let current_screenwriter = current_steps
        .iter()
        .find(|step| step.revision_epoch == 1 && step.step_key == "screenwriter")
        .unwrap()
        .id;

    let current_brief = json!({"epoch":1,"kind":"brief"});
    let current_story = json!({"epoch":1,"kind":"story"});
    let current_character = json!({"epoch":1,"kind":"character"});
    let current_draft = json!({"epoch":1,"kind":"draft"});
    let current_brief_id = Uuid::new_v4();
    let current_story_id = Uuid::new_v4();
    let current_character_id = Uuid::new_v4();
    let current_draft_id = Uuid::new_v4();
    sqlx::query("INSERT INTO creative_briefs (id,production_project_id,version,content,run_id,step_id,attempt,revision_epoch,content_digest,audit_status) VALUES ($1,$2,2,$3,$4,$5,2,1,$6,'complete')")
        .bind(current_brief_id).bind(intent.id).bind(&current_brief).bind(run.id)
        .bind(current_producer).bind(canonical_digest(&current_brief).unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO story_bibles (id,production_project_id,version,content,run_id,step_id,attempt,revision_epoch,content_digest,audit_status) VALUES ($1,$2,2,$3,$4,$5,2,1,$6,'complete')")
        .bind(current_story_id).bind(intent.id).bind(&current_story).bind(run.id)
        .bind(current_screenwriter).bind(canonical_digest(&current_story).unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO character_bibles (id,production_project_id,character_id,version,content,run_id,step_id,attempt,revision_epoch,content_digest,audit_status) VALUES ($1,$2,'current-character',2,$3,$4,$5,2,1,$6,'complete')")
        .bind(current_character_id).bind(intent.id).bind(&current_character).bind(run.id)
        .bind(current_screenwriter).bind(canonical_digest(&current_character).unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO script_drafts (id,production_project_id,version,content,run_id,step_id,attempt,revision_epoch,content_digest,audit_status) VALUES ($1,$2,2,$3,$4,$5,2,1,$6,'complete')")
        .bind(current_draft_id).bind(intent.id).bind(&current_draft).bind(run.id)
        .bind(current_screenwriter).bind(canonical_digest(&current_draft).unwrap()).execute(&pool).await.unwrap();

    // 另一 Run 中相同角色和版本的产物不得参与当前 Run 拼包。
    let (other_project_id, other_topic_id) = source(&pool).await;
    let other_intent = repo
        .create_intent(create_command(
            other_project_id,
            other_topic_id,
            "other-role-package-scope",
        ))
        .await
        .unwrap();
    let other_run = repo
        .start_run(StartRunCommand {
            intent_id: other_intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "other-role-package-scope-run".into(),
        })
        .await
        .unwrap();
    let other_producer = repo
        .get_run(other_run.id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.step_key == "producer")
        .unwrap()
        .id;
    sqlx::query("UPDATE production_steps SET status='succeeded',attempt=2 WHERE id=$1")
        .bind(other_producer)
        .execute(&pool)
        .await
        .unwrap();
    let other_brief = json!({"epoch":1,"kind":"cross-run-brief"});
    sqlx::query(
        r#"
        INSERT INTO creative_briefs (
            production_project_id,version,content,run_id,step_id,attempt,revision_epoch,
            content_digest,audit_status
        ) VALUES ($1,1,$2,$3,$4,2,0,$5,'complete')
        "#,
    )
    .bind(other_intent.id)
    .bind(&other_brief)
    .bind(other_run.id)
    .bind(other_producer)
    .bind(canonical_digest(&other_brief).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let brief_package = repo.build_brief_package(run.id, 1).await.unwrap();
    assert_eq!(brief_package.revision_epoch, 1);
    assert_eq!(brief_package.source_step_id, current_producer);
    assert_eq!(brief_package.source_attempt, 2);
    assert_eq!(brief_package.items.len(), 1);
    assert_eq!(brief_package.items[0].artifact_id, current_brief_id);

    let script_package = repo.build_script_package(run.id, 1).await.unwrap();
    assert_eq!(script_package.revision_epoch, 1);
    assert_eq!(script_package.source_step_id, current_screenwriter);
    assert_eq!(script_package.source_attempt, 2);
    assert_eq!(script_package.items.len(), 3);
    assert_eq!(
        script_package
            .items
            .iter()
            .map(|item| item.artifact_id)
            .collect::<std::collections::BTreeSet<_>>(),
        [current_story_id, current_character_id, current_draft_id]
            .into_iter()
            .collect()
    );

    let duplicate_brief = json!({"epoch":1,"kind":"duplicate-brief"});
    let duplicate_story = json!({"epoch":1,"kind":"duplicate-story"});
    sqlx::query("INSERT INTO creative_briefs (production_project_id,version,content,run_id,step_id,attempt,revision_epoch,content_digest,audit_status) VALUES ($1,3,$2,$3,$4,2,1,$5,'complete')")
        .bind(intent.id).bind(&duplicate_brief).bind(run.id).bind(current_producer)
        .bind(canonical_digest(&duplicate_brief).unwrap()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO story_bibles (production_project_id,version,content,run_id,step_id,attempt,revision_epoch,content_digest,audit_status) VALUES ($1,3,$2,$3,$4,2,1,$5,'complete')")
        .bind(intent.id).bind(&duplicate_story).bind(run.id).bind(current_screenwriter)
        .bind(canonical_digest(&duplicate_story).unwrap()).execute(&pool).await.unwrap();
    assert_eq!(
        repo.build_brief_package(run.id, 2)
            .await
            .unwrap_err()
            .code(),
        "transition_conflict"
    );
    assert_eq!(
        repo.build_script_package(run.id, 2)
            .await
            .unwrap_err()
            .code(),
        "transition_conflict"
    );
}

#[tokio::test]
async fn zero_cost_full_crew_e2e_reaches_completion_with_bounded_rework_policy() {
    let (_admin, pool, _guard) = database().await;
    let repository = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let mut e2e_limits = ResourceLimits::strict_default();
    e2e_limits.max_quality_reworks = 1;
    let intent = repository
        .create_intent(create_command(project_id, topic_id, "zero-cost-e2e-intent"))
        .await
        .unwrap();
    let run = repository
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan_with_limits(e2e_limits),
            actor: ProductionActor::local_operator(),
            idempotency_key: "zero-cost-e2e-run".into(),
        })
        .await
        .unwrap();

    let producer_step_id = repository
        .get_run(run.id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.step_key == "producer")
        .unwrap()
        .id;
    sqlx::query("UPDATE production_steps SET status='succeeded',attempt=1 WHERE id=$1")
        .bind(producer_step_id)
        .execute(&pool)
        .await
        .unwrap();
    let brief_content = json!({
        "target_audience": "开发者", "tone": ["严谨"],
        "key_messages": ["持久化", "可恢复"],
        "constraints": {"duration_seconds": 10, "platform": ["internal_fixture"]},
        "success_criteria": ["全部 Gate 与媒体证据闭合"]
    });
    let brief_digest = canonical_digest(&brief_content).unwrap();
    let brief_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO creative_briefs (
            production_project_id,version,status,content,created_by,run_id,step_id,
            attempt,revision_epoch,content_digest,audit_status
        ) VALUES ($1,1,'draft',$2,'producer',$3,$4,1,0,$5,'complete') RETURNING id
        "#,
    )
    .bind(intent.id)
    .bind(&brief_content)
    .bind(run.id)
    .bind(producer_step_id)
    .bind(&brief_digest)
    .fetch_one(&pool)
    .await
    .unwrap();
    let brief_package = ArtifactPackageSnapshot::build(
        PackageType::Brief,
        run.id,
        producer_step_id,
        1,
        0,
        1,
        vec![ArtifactRef {
            run_id: run.id,
            artifact_type: "creative_brief".into(),
            artifact_id: brief_id,
            version: 1,
            content_digest: brief_digest,
            source_step_id: producer_step_id,
            source_attempt: 1,
        }],
        json!({}),
    )
    .unwrap();
    repository.save_package(&brief_package).await.unwrap();
    repository
        .decide_package(PackageDecisionCommand {
            run_id: run.id,
            package_digest: brief_package.package_digest,
            decision: GateDecision::Approve,
            reason: None,
            affected_owners: vec![],
            actor: ProductionActor::local_operator(),
            idempotency_key: "zero-cost-e2e-approve-brief".into(),
        })
        .await
        .unwrap();

    let script_package = seed_e2e_script_package(&pool, &repository, intent.id, run.id).await;
    repository.save_package(&script_package).await.unwrap();
    repository
        .decide_package(PackageDecisionCommand {
            run_id: run.id,
            package_digest: script_package.package_digest.clone(),
            decision: GateDecision::Approve,
            reason: None,
            affected_owners: vec![],
            actor: ProductionActor::local_operator(),
            idempotency_key: "zero-cost-e2e-approve-script".into(),
        })
        .await
        .unwrap();
    let script = ScriptPackagePromotionService::new(pool.clone())
        .promote(ScriptPackagePromotionCommand {
            run_id: run.id,
            package_digest: script_package.package_digest,
            actor: ProductionActor::local_operator(),
            idempotency_key: "zero-cost-e2e-promote-script".into(),
        })
        .await
        .unwrap();
    let scene_ids = script
        .scenes
        .iter()
        .map(|scene| scene.id)
        .collect::<Vec<_>>();
    assert_eq!(scene_ids.len(), 2);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM content_topics WHERE id=$1")
            .bind(topic_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "scripted"
    );

    let scene_shots =
        seed_e2e_production_artifacts(&pool, &repository, intent.id, run.id, script.id, &scene_ids)
            .await;
    let production_package = repository
        .build_production_package(run.id, 1)
        .await
        .unwrap();
    repository.save_package(&production_package).await.unwrap();
    repository
        .decide_package(PackageDecisionCommand {
            run_id: run.id,
            package_digest: production_package.package_digest.clone(),
            decision: GateDecision::Approve,
            reason: None,
            affected_owners: vec![],
            actor: ProductionActor::local_operator(),
            idempotency_key: "zero-cost-e2e-approve-production".into(),
        })
        .await
        .unwrap();
    insert_selected_visuals(&pool, project_id, script.id, &scene_ids, "zero-cost-e2e").await;
    let llm_model_id = insert_enabled_text_model(&pool).await;
    let video_model_id = insert_enabled_video_model(&pool).await;
    insert_enabled_tos_staging_config(&pool).await;
    let asset_service = AssetGenerationService::new(
        pool.clone(),
        PostgresAiModelRepository::new(pool.clone()),
        PostgresAssetGenerationRepository::new(pool.clone()),
        PostgresMaterialRepository::new(pool.clone()),
        PostgresScriptRepository::new(pool.clone()),
    );
    let work_service = WorkGenerationService::new(
        PostgresWorkGenerationRepository::new(pool.clone()),
        PostgresAiModelRepository::new(pool.clone()),
        PostgresVoiceCatalogRepository::new(pool.clone()),
        asset_service.clone(),
    );
    let integration = Arc::new(ProductionWorkflowIntegrationService::new(
        asset_service,
        Some(work_service.clone()),
    ));
    let media_provider = Arc::new(InspectingMediaEvidenceProvider {
        calls: AtomicUsize::new(0),
    });
    let mut orchestrator = ProductionOrchestrator::new(
        pool.clone(),
        Arc::new(RoleRegistry::new()),
        Arc::new(GateRegistry::new()),
    );
    orchestrator.scene_visual_manifest_port = Some(integration.clone());
    orchestrator.work_generation_planning_port = Some(integration.clone());
    orchestrator.work_generation_run_port = Some(integration);
    orchestrator.media_evidence_provider = Some(media_provider.clone());

    let manifest = orchestrator
        .resume_scene_visual_manifest(run.id, &production_package.package_digest)
        .await
        .unwrap();
    let production_input = repository
        .load_approved_production_input(run.id, &production_package.package_digest)
        .await
        .unwrap();
    let work_plan = orchestrator
        .resume_create_work_plan(ProductionWorkPlanRequest {
            production: production_input,
            manifest,
            operator_settings: ProductionWorkPlanSettings {
                llm_model_id,
                video_model_id,
                tts_model_id: None,
                tts_voice_type: None,
                duration_strategy: "script_total".into(),
                duration_seconds: None,
                aspect_ratio: "9:16".into(),
                resolution: "1080p".into(),
                audio_mode: "silent".into(),
                narration_override: None,
                audio_material_ids: vec![],
                burn_subtitles: false,
                overrides: Default::default(),
            },
        })
        .await
        .unwrap();
    let confirmed = work_service
        .confirm(
            work_plan.work_plan_id,
            "zero-cost-e2e-confirm-work-plan".into(),
        )
        .await
        .unwrap();
    assert!(confirmed.created);
    let external = orchestrator
        .resume_work_plan_confirmation(run.id, work_plan.clone())
        .await
        .unwrap();
    assert_eq!(external.run_id, confirmed.run.id);

    complete_e2e_fake_generation(&pool, work_plan.work_version_id, external.run_id).await;
    assert_eq!(
        orchestrator
            .resume_work_generation(run.id, external.run_id)
            .await
            .unwrap(),
        WorkGenerationRunDisposition::ReadyForMediaReview
    );
    let resource_summary = repository.get_run(run.id).await.unwrap().resource_summary;
    assert!(
        resource_summary
            .iter()
            .any(|item| item["resource_key"] == "concurrency" && item["actual"].as_i64() == Some(1)),
        "released concurrency reservations must retain usage ledger actuals"
    );
    let inventory = orchestrator
        .build_required_take_inventory(run.id)
        .await
        .unwrap();
    let evidence = orchestrator
        .capture_media_evidence(
            inventory.clone(),
            TemporaryMediaAccess {
                asset_id: inventory.final_asset.artifact_id,
                access_url: "https://media.invalid/temporary-signature".into(),
                request_headers: std::collections::BTreeMap::from([(
                    "Authorization".into(),
                    "Bearer temporary-secret".into(),
                )]),
            },
        )
        .await
        .unwrap();
    persist_e2e_quality_artifacts(
        &pool,
        intent.id,
        run.id,
        &inventory,
        &evidence,
        &scene_shots,
    )
    .await;
    let quality = repository.build_quality_package(run.id, 1).await.unwrap();
    assert_eq!(quality.outcome, QualityGateOutcome::Approved);
    repository.save_package(&quality.package).await.unwrap();
    repository
        .decide_package(PackageDecisionCommand {
            run_id: run.id,
            package_digest: quality.package.package_digest,
            decision: GateDecision::Approve,
            reason: None,
            affected_owners: vec![],
            actor: ProductionActor::local_operator(),
            idempotency_key: "zero-cost-e2e-approve-quality".into(),
        })
        .await
        .unwrap();

    let completed = repository.get_run(run.id).await.unwrap();
    assert_eq!(completed.run.status, "completed");
    assert_eq!(completed.run.quality_status, "approved");
    assert_eq!(completed.run.resource_limits["max_quality_reworks"], 1);
    assert_eq!(media_provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        sqlx::query_as::<_, (String, i64, i64)>(
            "SELECT status,(SELECT COUNT(*) FROM model_calls),(SELECT COUNT(*) FROM asset_generation_tasks) FROM work_generation_runs WHERE id=$1",
        )
        .bind(external.run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("succeeded".into(), 0, 0),
        "E2E 只能使用 fake generation/media，不得触发真实模型或画面 provider"
    );
}

fn next_production_package(input: &ProductionPackageInput) -> ProductionPackageInput {
    let current = input.package_snapshot().unwrap();
    let next = ArtifactPackageSnapshot::build(
        current.package_type,
        current.run_id,
        current.source_step_id,
        current.source_attempt,
        current.revision_epoch,
        current.package_version + 1,
        current.items,
        current.metadata,
    )
    .unwrap();
    ProductionPackageInput::from_approved_package(&next, input.clone().into_content()).unwrap()
}

fn rebind_production_package_to_script(
    input: &ProductionPackageInput,
    script_id: Uuid,
    scene_ids: &[Uuid],
) -> ProductionPackageInput {
    let current = input.package_snapshot().unwrap();
    let mut metadata: ProductionPackageMetadata =
        serde_json::from_value(current.metadata.clone()).unwrap();
    let mut content = input.clone().into_content();
    let script_digest = canonical_digest(&json!({
        "script_id": script_id,
        "title": content.script.title,
        "hook": content.script.hook,
    }))
    .unwrap();
    content.script.script_id = script_id;
    content.script.script_version = script_id.to_string();
    content.script.script_digest = script_digest.clone();
    metadata.script_id = script_id;
    metadata.script_version = script_id.to_string();
    metadata.script_digest = script_digest;

    for ((scene, scene_ref), scene_id) in content
        .scenes
        .iter_mut()
        .zip(&mut metadata.scenes)
        .zip(scene_ids)
    {
        let scene_digest = canonical_digest(&json!({
            "scene_id": scene_id,
            "sequence": scene.sequence,
            "narration": scene.narration,
            "visual_description": scene.visual_description,
            "emotion": scene.emotion,
            "duration_sec": scene.duration_sec,
        }))
        .unwrap();
        scene.scene_id = *scene_id;
        scene.scene_version = scene_id.to_string();
        scene.scene_digest = scene_digest.clone();
        scene_ref.scene_id = *scene_id;
        scene_ref.scene_version = scene_id.to_string();
        scene_ref.scene_digest = scene_digest;
    }
    for shot in &mut content.shot_contracts {
        shot.content.scene_id = scene_ids[(shot.content.sequence - 1) as usize];
        shot.content_digest = canonical_digest(&shot.content).unwrap();
        let shot_ref = metadata
            .shots
            .iter_mut()
            .find(|item| item.artifact_id == shot.artifact_id)
            .unwrap();
        shot_ref.scene_id = shot.content.scene_id;
    }
    for brief in &mut content.performance_briefs {
        brief.content.script_id = script_id;
        for scene in &mut brief.content.emotional_arc {
            scene.scene_id = scene_ids[(scene.sequence - 1) as usize];
        }
        brief.content_digest = canonical_digest(&brief.content).unwrap();
        let brief_ref = metadata
            .performance_briefs
            .iter_mut()
            .find(|item| item.artifact_id == brief.artifact_id)
            .unwrap();
        brief_ref.script_id = script_id;
        brief_ref.scene_ids = brief
            .content
            .emotional_arc
            .iter()
            .map(|scene| scene.scene_id)
            .collect();
    }
    content.sound_plan.content.script_id = script_id;
    for scene in &mut content.sound_plan.content.scene_sound_notes {
        scene.scene_id = scene_ids[(scene.sequence - 1) as usize];
    }
    content.sound_plan.content_digest = canonical_digest(&content.sound_plan.content).unwrap();
    metadata.sound_plan.script_id = script_id;
    metadata.sound_plan.scene_ids = content
        .sound_plan
        .content
        .scene_sound_notes
        .iter()
        .map(|scene| scene.scene_id)
        .collect();

    let mut items = current.items;
    for item in &mut items {
        if let Some(shot) = content
            .shot_contracts
            .iter()
            .find(|shot| shot.artifact_id == item.artifact_id)
        {
            item.content_digest = shot.content_digest.clone();
        } else if let Some(brief) = content
            .performance_briefs
            .iter()
            .find(|brief| brief.artifact_id == item.artifact_id)
        {
            item.content_digest = brief.content_digest.clone();
        } else if content.sound_plan.artifact_id == item.artifact_id {
            item.content_digest = content.sound_plan.content_digest.clone();
        }
    }
    for suggestion in &mut content.applied_suggestions {
        if let Some(item) = items
            .iter()
            .find(|item| item.artifact_id == suggestion.artifact_id)
        {
            suggestion.content_digest = item.content_digest.clone();
        }
    }
    metadata.suggestion_resolutions = content
        .applied_suggestions
        .iter()
        .map(|item| serde_json::to_value(item).unwrap())
        .collect();
    let next = ArtifactPackageSnapshot::build(
        current.package_type,
        current.run_id,
        current.source_step_id,
        current.source_attempt,
        current.revision_epoch,
        current.package_version + 1,
        items,
        serde_json::to_value(metadata).unwrap(),
    )
    .unwrap();
    ProductionPackageInput::from_approved_package(&next, content).unwrap()
}

#[derive(Clone)]
struct PersistedQualityScope {
    work_version_id: Uuid,
    inventory: RequiredTakeInventorySnapshot,
    evidence: MediaEvidenceSnapshot,
    editor_step_id: Uuid,
    qc_step_id: Uuid,
    ledger_ids: Vec<Uuid>,
    review_id: Uuid,
}

#[allow(clippy::too_many_arguments)]
async fn seed_persisted_quality_scope(
    pool: &PgPool,
    repo: &DurableProductionRepository,
    run_id: Uuid,
    production_project_id: Uuid,
    work_id: Uuid,
    revision_epoch: i32,
    version_no: i32,
    scene_ids: &[Uuid],
    scene_shots: &[(Uuid, Uuid)],
    prefix: &str,
) -> PersistedQualityScope {
    let role_steps = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        UPDATE production_steps
        SET status='succeeded',attempt=1,waiting_reason=NULL
        WHERE run_id=$1 AND revision_epoch=$2
          AND step_key IN ('wait_work_generation','editor','qc')
        RETURNING id,step_key
        "#,
    )
    .bind(run_id)
    .bind(revision_epoch)
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|(id, key)| (key, id))
    .collect::<std::collections::BTreeMap<_, _>>();
    let source_step_id = role_steps["wait_work_generation"];
    let editor_step_id = role_steps["editor"];
    let qc_step_id = role_steps["qc"];
    let work_version_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_versions (
            work_id,version_no,source_manifest_version,input_snapshot,model_snapshot,
            parameter_snapshot,timeline_snapshot,prompt_snapshot,status
        ) VALUES ($1,$2,$3,$4,'{}','{}','{}','{}','running') RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(version_no)
    .bind(format!("{prefix}-manifest"))
    .bind(json!({
        "production_run_id": run_id,
        "revision_epoch": revision_epoch,
        "scenes": scene_ids,
    }))
    .fetch_one(pool)
    .await
    .unwrap();
    let work_plan_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_plans (
            work_id,work_version_id,plan_version,status,input_fingerprint,
            capability_snapshot,output_snapshot,prompt_snapshot,timeline_snapshot
        ) VALUES ($1,$2,1,'confirmed',$3,'{}','{}','{}','{}') RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(work_version_id)
    .bind(format!("{:0>64}", version_no))
    .fetch_one(pool)
    .await
    .unwrap();
    let generation_run_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_generation_runs (
            work_id,work_version_id,work_plan_id,idempotency_key,status,
            model_snapshot,capability_snapshot,prompt_snapshot,timeline_snapshot,parameter_snapshot
        ) VALUES ($1,$2,$3,$4,'succeeded','{}','{}','{}','{}','{}') RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(work_version_id)
    .bind(work_plan_id)
    .bind(format!("{prefix}-generation"))
    .fetch_one(pool)
    .await
    .unwrap();
    let generation_step_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO work_generation_steps (run_id,step_no,step_type,status,input_snapshot) VALUES ($1,1,'video_segment','succeeded',$2) RETURNING id",
    )
    .bind(generation_run_id)
    .bind(json!({"sequence": 1, "scene_ids": scene_ids}))
    .fetch_one(pool)
    .await
    .unwrap();
    let generation_attempt_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO work_generation_attempts (step_id,attempt_no,status,model_snapshot,resource_usage) VALUES ($1,1,'succeeded','{}','{}') RETURNING id",
    )
    .bind(generation_step_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let output_hash = format!("{:064x}", 100 + version_no);
    let output_artifact_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_artifacts (
            work_version_id,role,generation_step_id,file_name,storage_path,mime_type,
            size_bytes,sha256,metadata
        ) VALUES ($1,'reusable_intermediate',$2,$3,$4,'video/mp4',100,$5,$6)
        RETURNING id
        "#,
    )
    .bind(work_version_id)
    .bind(generation_step_id)
    .bind(format!("{prefix}-segment.mp4"))
    .bind(format!("works/{prefix}-segment.mp4"))
    .bind(&output_hash)
    .bind(json!({"duration_ms": 17_000, "generation_attempt_id": generation_attempt_id}))
    .fetch_one(pool)
    .await
    .unwrap();
    let compose_step_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO work_generation_steps (run_id,step_no,step_type,status,depends_on) VALUES ($1,2,'compose','succeeded',$2) RETURNING id",
    )
    .bind(generation_run_id)
    .bind(json!([generation_step_id]))
    .fetch_one(pool)
    .await
    .unwrap();
    let final_hash = format!("{:064x}", 200 + version_no);
    let final_artifact_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_artifacts (
            work_version_id,role,generation_step_id,file_name,storage_path,mime_type,
            size_bytes,sha256,metadata
        ) VALUES ($1,'final_video',$2,$3,$4,'video/mp4',200,$5,'{"duration_ms":17000}')
        RETURNING id
        "#,
    )
    .bind(work_version_id)
    .bind(compose_step_id)
    .bind(format!("{prefix}-final.mp4"))
    .bind(format!("works/{prefix}-final.mp4"))
    .bind(&final_hash)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO production_domain_links (
            run_id,source_step_id,revision_epoch,link_type,work_generation_run_id,
            target_version,target_digest
        ) VALUES ($1,$2,$3,'work_generation_run',$4,$5,$6)
        "#,
    )
    .bind(run_id)
    .bind(source_step_id)
    .bind(revision_epoch)
    .bind(generation_run_id)
    .bind(version_no.to_string())
    .bind(format!("{:064x}", 300 + version_no))
    .execute(pool)
    .await
    .unwrap();

    let inventory = RequiredTakeInventorySnapshot::build(
        Uuid::new_v4(),
        run_id,
        source_step_id,
        1,
        revision_epoch as u32,
        work_id,
        work_version_id,
        generation_run_id,
        FinalMediaAsset {
            artifact_id: final_artifact_id,
            sha256: final_hash,
            mime_type: "video/mp4".into(),
            duration_ms: 17_000,
        },
        repo.work_version_hash(work_version_id).await.unwrap(),
        vec![ComposeInput {
            generation_step_id,
            generation_attempt_id,
            output_artifact_id,
            segment_key: format!("{prefix}-segment"),
            scene_ids: scene_ids.to_vec(),
            shot_contracts: scene_ids
                .iter()
                .map(|scene_id| {
                    (
                        *scene_id,
                        scene_shots
                            .iter()
                            .filter_map(|(candidate_scene, shot_id)| {
                                (candidate_scene == scene_id).then_some(*shot_id)
                            })
                            .collect(),
                    )
                })
                .collect(),
            consumed_by_final_compose: true,
            generation_succeeded: true,
        }],
    )
    .unwrap();
    repo.save_required_take_inventory(&inventory).await.unwrap();
    let evidence = MediaEvidenceSnapshot::build(
        Uuid::new_v4(),
        run_id,
        source_step_id,
        1,
        revision_epoch as u32,
        work_version_id,
        inventory.inventory_id,
        inventory.inventory_digest.clone(),
        inventory.final_asset.clone(),
        "vision-fixture@1".into(),
        "audio-fixture@1".into(),
        json!({
            "final_media": {"result": "reviewed"},
            "takes": [{"take_id": inventory.takes[0].take_id}],
        }),
    )
    .unwrap();
    repo.save_media_evidence(&evidence).await.unwrap();

    let mut ledger_ids = Vec::new();
    for (index, (_, shot_id)) in scene_shots.iter().enumerate() {
        let ledger_id = Uuid::new_v4();
        let content = json!({
            "order": index + 1,
            "shot_contract_id": shot_id,
            "work_version_id": work_version_id,
            "inventory_id": inventory.inventory_id,
            "evidence_snapshot_id": evidence.evidence_id,
            "visual_facts": [format!("{prefix}-visual-fact")],
            "continuity_flags": [],
        });
        sqlx::query(
            r#"
            INSERT INTO continuity_ledgers (
                id,production_project_id,shot_id,content,created_by,run_id,step_id,
                attempt,revision_epoch,work_version_id,inventory_id,evidence_snapshot_id,
                shot_contract_id,version,content_digest,audit_status
            ) VALUES ($1,$2,NULL,$3,'editor',$4,$5,1,$6,$7,$8,$9,$10,1,$11,'complete')
            "#,
        )
        .bind(ledger_id)
        .bind(production_project_id)
        .bind(&content)
        .bind(run_id)
        .bind(editor_step_id)
        .bind(revision_epoch)
        .bind(work_version_id)
        .bind(inventory.inventory_id)
        .bind(evidence.evidence_id)
        .bind(shot_id)
        .bind(canonical_digest(&content).unwrap())
        .execute(pool)
        .await
        .unwrap();
        ledger_ids.push(ledger_id);
    }
    let applicable_shots = scene_ids
        .iter()
        .flat_map(|scene_id| inventory.takes[0].scene_shot_map[scene_id].iter().copied())
        .collect::<Vec<_>>();
    let review_id = Uuid::new_v4();
    let review_content = json!({
        "required_take_id": inventory.takes[0].take_id,
        "work_version_id": work_version_id,
        "inventory_id": inventory.inventory_id,
        "evidence_snapshot_id": evidence.evidence_id,
        "applicable_shot_contract_ids": applicable_shots,
        "review_status": "approved",
        "quality_assessment": {"visual": 9, "narrative": 9, "technical": 9},
        "issues": [],
        "suggestions": [],
    });
    sqlx::query(
        r#"
        INSERT INTO take_reviews (
            id,production_project_id,shot_id,take_number,status,content,created_by,
            run_id,step_id,attempt,revision_epoch,work_version_id,inventory_id,
            evidence_snapshot_id,required_take_id,version,content_digest,audit_status
        ) VALUES ($1,$2,NULL,NULL,'approved',$3,'qc',$4,$5,1,$6,$7,$8,$9,$10,1,$11,'complete')
        "#,
    )
    .bind(review_id)
    .bind(production_project_id)
    .bind(&review_content)
    .bind(run_id)
    .bind(qc_step_id)
    .bind(revision_epoch)
    .bind(work_version_id)
    .bind(inventory.inventory_id)
    .bind(evidence.evidence_id)
    .bind(inventory.takes[0].take_id)
    .bind(canonical_digest(&review_content).unwrap())
    .execute(pool)
    .await
    .unwrap();
    for (ordinal, shot_id) in applicable_shots.iter().enumerate() {
        let (ledger_id, digest) = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT id,content_digest FROM continuity_ledgers WHERE run_id=$1 AND work_version_id=$2 AND inventory_id=$3 AND shot_contract_id=$4 AND version=1",
        )
        .bind(run_id)
        .bind(work_version_id)
        .bind(inventory.inventory_id)
        .bind(shot_id)
        .fetch_one(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO take_review_ledger_versions (
                take_review_id,ordinal,continuity_ledger_id,shot_contract_id,
                ledger_version,content_digest
            ) VALUES ($1,$2,$3,$4,1,$5)
            "#,
        )
        .bind(review_id)
        .bind(ordinal as i32)
        .bind(ledger_id)
        .bind(shot_id)
        .bind(digest)
        .execute(pool)
        .await
        .unwrap();
    }
    PersistedQualityScope {
        work_version_id,
        inventory,
        evidence,
        editor_step_id,
        qc_step_id,
        ledger_ids,
        review_id,
    }
}

struct FakeWorkCancellationPort {
    pool: PgPool,
    result: ExternalCancellationState,
    calls: AtomicUsize,
}

struct InspectingMediaEvidenceProvider {
    calls: AtomicUsize,
}

struct RejectingReworkPort {
    calls: AtomicUsize,
}

#[async_trait]
impl WorkVersionReworkPort for RejectingReworkPort {
    async fn create_rework_draft(
        &self,
        _request: WorkVersionReworkRequest,
    ) -> novex_production_crew::ProductionResult<WorkVersionReworkReference> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        panic!("返工上限校验必须发生在 WorkVersionReworkPort 调用之前")
    }
}

#[async_trait]
impl MediaEvidenceProvider for InspectingMediaEvidenceProvider {
    async fn inspect_media(
        &self,
        inventory: RequiredTakeInventorySnapshot,
        access: TemporaryMediaAccess,
    ) -> novex_production_crew::ProductionResult<MediaEvidenceAnalysis> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(access.asset_id, inventory.final_asset.artifact_id);
        assert!(access.access_url.contains("temporary-signature"));
        assert_eq!(
            access
                .request_headers
                .get("Authorization")
                .map(String::as_str),
            Some("Bearer temporary-secret")
        );
        Ok(MediaEvidenceAnalysis {
            vision_capability_version: "vision-fixture@1".into(),
            audio_capability_version: "audio-fixture@1".into(),
            redacted_analysis: json!({
                "final_media": {"visual": "pass", "audio": "pass"},
                "takes": inventory.takes.iter().map(|take| json!({
                    "take_id": take.take_id,
                    "result": "pass",
                })).collect::<Vec<_>>(),
            }),
        })
    }
}

#[async_trait]
impl WorkGenerationCancellationPort for FakeWorkCancellationPort {
    async fn cancel(
        &self,
        work_generation_run_id: Uuid,
    ) -> Result<ExternalCancellationState, WorkCancellationPortError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let status = match self.result {
            ExternalCancellationState::Cancelling => "cancelling",
            ExternalCancellationState::Cancelled => "cancelled",
            ExternalCancellationState::AttentionRequired => "waiting_manual",
        };
        sqlx::query("UPDATE work_generation_runs SET status = $2 WHERE id = $1")
            .bind(work_generation_run_id)
            .bind(status)
            .execute(&self.pool)
            .await
            .unwrap();
        Ok(self.result)
    }
}

async fn link_active_work_generation_run(
    pool: &PgPool,
    production_run_id: Uuid,
    project_id: Uuid,
) -> Uuid {
    let script_id: Uuid = sqlx::query_scalar(
        "INSERT INTO scripts (project_id, title, hook, content) VALUES ($1, '取消协调脚本', 'hook', '{}') RETURNING id",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let work_id: Uuid = sqlx::query_scalar(
        "INSERT INTO works (project_id, script_id, title, status) VALUES ($1, $2, '取消协调作品', 'running') RETURNING id",
    )
    .bind(project_id)
    .bind(script_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let work_version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_versions (work_id, version_no, source_manifest_version, input_snapshot, model_snapshot, parameter_snapshot) VALUES ($1, 1, 'cancel-contract', '{}', '{}', '{}') RETURNING id",
    )
    .bind(work_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let work_plan_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_plans (work_id, work_version_id, plan_version, status, input_fingerprint, capability_snapshot, output_snapshot, prompt_snapshot, timeline_snapshot) VALUES ($1, $2, 1, 'confirmed', $3, '{}', '{}', '{}', '{}') RETURNING id",
    )
    .bind(work_id)
    .bind(work_version_id)
    .bind("a".repeat(64))
    .fetch_one(pool)
    .await
    .unwrap();
    let work_generation_run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_runs (work_id, work_version_id, work_plan_id, idempotency_key, status, model_snapshot, capability_snapshot, prompt_snapshot, timeline_snapshot, parameter_snapshot) VALUES ($1, $2, $3, $4, 'queued', '{}', '{}', '{}', '{}', '{}') RETURNING id",
    )
    .bind(work_id)
    .bind(work_version_id)
    .bind(work_plan_id)
    .bind(format!("cancel-contract-{production_run_id}"))
    .fetch_one(pool)
    .await
    .unwrap();
    let source_step_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM production_steps WHERE run_id = $1 AND step_key = 'wait_work_generation' AND revision_epoch = 0",
    )
    .bind(production_run_id)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO production_domain_links (
            run_id, source_step_id, revision_epoch, link_type,
            work_generation_run_id, target_version, target_digest
        ) VALUES ($1, $2, 0, 'work_generation_run', $3, '1', $4)
        "#,
    )
    .bind(production_run_id)
    .bind(source_step_id)
    .bind(work_generation_run_id)
    .bind("b".repeat(64))
    .execute(pool)
    .await
    .unwrap();
    work_generation_run_id
}

async fn attach_model_call(pool: &PgPool, step_id: Uuid, attempt: i32) -> Uuid {
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
            $1, 'production.cinematographer', $2, 'succeeded',
            'production.cinematographer', '3.0.0', 'production.cinematographer',
            '3.0.0', $3, '{"system":"fixture","user":"fixture"}',
            $4, $5, '{"provider":"test"}', NOW()
        ) RETURNING id
        "#,
    )
    .bind(agent_run_id)
    .bind(attempt)
    .bind("1".repeat(64))
    .bind(model_id)
    .bind("2".repeat(64))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE production_steps SET status = 'succeeded', attempt = $2, model_call_id = $3 WHERE id = $1",
    )
    .bind(step_id)
    .bind(attempt)
    .bind(model_call_id)
    .execute(pool)
    .await
    .unwrap();
    model_call_id
}

#[tokio::test]
async fn intent_run_and_steps_are_created_atomically_and_reconstructed_from_postgres() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());

    let intent = repo
        .create_intent(create_command(project_id, topic_id, "create-1"))
        .await
        .unwrap();
    assert_eq!(intent.project_id, project_id);
    assert_eq!(intent.topic_id, topic_id);
    assert_eq!(intent.status, "created");

    let replay = repo
        .create_intent(create_command(project_id, topic_id, "create-1"))
        .await
        .unwrap();
    assert_eq!(replay.id, intent.id);

    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start-1".into(),
        })
        .await
        .unwrap();
    assert_eq!(run.production_project_id, intent.id);
    assert_eq!(run.status, "queued");

    let changed_bindings = plan()
        .role_bindings
        .as_object()
        .unwrap()
        .iter()
        .map(|(role, binding)| {
            let mut binding = binding.clone();
            binding["definition_version"] = json!("2.0.0");
            (role.clone(), binding)
        })
        .collect::<serde_json::Map<_, _>>();
    let changed_plan = FullCrewPlanRegistry::snapshot_v1(
        false,
        Value::Object(changed_bindings),
        ResourceLimits::strict_default(),
    )
    .unwrap();
    let replay = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: changed_plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "start-1".into(),
        })
        .await
        .unwrap();
    assert_eq!(replay.id, run.id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_plan_snapshots WHERE id=$1",)
            .bind(run.plan_snapshot_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );

    let view = DurableProductionRepository::new(pool.clone())
        .get_run(run.id)
        .await
        .unwrap();
    assert_eq!(view.run.id, run.id);
    assert_eq!(view.steps.len(), plan().steps.len());
    assert_eq!(view.steps[0].step_key, "validate_source");
    assert_eq!(view.steps[0].status, "queued");
    assert!(view.steps[1..].iter().all(|step| step.status == "blocked"));
    assert!(view.packages.is_empty());
    assert!(view.gate_decisions.is_empty());
    assert!(view.domain_links.is_empty());
}

#[tokio::test]
async fn start_run_rolls_back_plan_run_steps_and_intent_status_on_mid_transaction_failure() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "create"))
        .await
        .unwrap();

    sqlx::query(
        r#"
        CREATE FUNCTION fail_producer_step_for_contract_test() RETURNS TRIGGER AS $$
        BEGIN
            IF NEW.step_key = 'producer' THEN
                RAISE EXCEPTION 'injected production step failure';
            END IF;
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        CREATE TRIGGER fail_producer_step_for_contract_test
        BEFORE INSERT ON production_steps
        FOR EACH ROW EXECUTE FUNCTION fail_producer_step_for_contract_test()
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    assert!(repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start-fails".into(),
        })
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_runs WHERE production_project_id = $1",
        )
        .bind(intent.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM production_plan_snapshots")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(repo.get_intent(intent.id).await.unwrap().status, "created");
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_commands WHERE command_type = 'start_run'",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );

    sqlx::query("DROP TRIGGER fail_producer_step_for_contract_test ON production_steps")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION fail_producer_step_for_contract_test()")
        .execute(&pool)
        .await
        .unwrap();

    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start-succeeds".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        repo.get_run(run.id).await.unwrap().steps.len(),
        plan().steps.len()
    );
}

#[tokio::test]
async fn command_digest_active_intent_and_single_run_constraints_reject_conflicts() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "same-key"))
        .await
        .unwrap();

    let mut conflicting = create_command(project_id, topic_id, "same-key");
    conflicting.title = "不同 payload".into();
    assert_eq!(
        repo.create_intent(conflicting).await.unwrap_err().code(),
        "idempotency_conflict"
    );
    assert_eq!(
        repo.create_intent(create_command(project_id, topic_id, "other-key"))
            .await
            .unwrap_err()
            .code(),
        "active_intent_conflict"
    );

    repo.start_run(StartRunCommand {
        intent_id: intent.id,
        plan: plan(),
        actor: ProductionActor::local_operator(),
        idempotency_key: "start-a".into(),
    })
    .await
    .unwrap();
    assert_eq!(
        repo.start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start-b".into(),
        })
        .await
        .unwrap_err()
        .code(),
        "run_already_exists"
    );
}

#[tokio::test]
async fn concurrent_intent_and_step_claim_have_single_winners() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let left = DurableProductionRepository::new(pool.clone());
    let right = DurableProductionRepository::new(pool.clone());
    let (first, second) = tokio::join!(
        left.create_intent(create_command(project_id, topic_id, "left")),
        right.create_intent(create_command(project_id, topic_id, "right")),
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let intent = first.or(second).unwrap();
    let run = left
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start".into(),
        })
        .await
        .unwrap();
    let step_id = left.get_run(run.id).await.unwrap().steps[0].id;
    let request_digest = "a".repeat(64);
    let (claim_a, claim_b) = tokio::join!(
        left.claim_step(
            step_id,
            "worker-a",
            Duration::from_secs(30),
            &request_digest,
            "claim-a"
        ),
        right.claim_step(
            step_id,
            "worker-b",
            Duration::from_secs(30),
            &request_digest,
            "claim-b"
        ),
    );
    assert_eq!(
        usize::from(claim_a.is_ok()) + usize::from(claim_b.is_ok()),
        1
    );
    let claimed = claim_a.or(claim_b).unwrap();
    assert_eq!(claimed.status, "running");
    assert_eq!(claimed.attempt, 1);
}

#[tokio::test]
async fn leases_can_be_renewed_released_and_safely_reclaimed_without_live_attempt_leaks() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "create"))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start".into(),
        })
        .await
        .unwrap();
    let step_id = repo.get_run(run.id).await.unwrap().steps[0].id;
    let digest = "a".repeat(64);
    let first = repo
        .claim_step(
            step_id,
            "worker-a",
            Duration::from_secs(30),
            &digest,
            "claim-1",
        )
        .await
        .unwrap();
    let renewed = repo
        .renew_step_lease(step_id, "worker-a", first.attempt, Duration::from_secs(60))
        .await
        .unwrap();
    assert!(renewed.lease_expires_at > first.lease_expires_at);

    repo.release_step_lease(step_id, "worker-a", first.attempt)
        .await
        .unwrap();
    let second = repo
        .claim_step(
            step_id,
            "worker-b",
            Duration::from_secs(30),
            &digest,
            "claim-2",
        )
        .await
        .unwrap();
    assert_eq!(second.attempt, 2);
    sqlx::query(
        "UPDATE production_steps SET lease_expires_at = NOW() - INTERVAL '1 second' WHERE id = $1",
    )
    .bind(step_id)
    .execute(&pool)
    .await
    .unwrap();
    let third = repo
        .claim_step(
            step_id,
            "worker-c",
            Duration::from_secs(30),
            &digest,
            "claim-3",
        )
        .await
        .unwrap();
    assert_eq!(third.attempt, 3);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_step_attempts WHERE step_id = $1 AND status IN ('prepared', 'running', 'attention_required')",
        )
        .bind(step_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
}

#[tokio::test]
async fn resource_reservations_are_atomic_bounded_and_hold_unknown_results() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "create"))
        .await
        .unwrap();
    let mut limits = ResourceLimits::strict_default();
    limits.max_role_calls = 1;
    limits.max_input_tokens = 100;
    limits.max_output_tokens = 50;
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan_with_limits(limits),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start".into(),
        })
        .await
        .unwrap();
    let steps = repo.get_run(run.id).await.unwrap().steps;
    let dependency_step = steps
        .iter()
        .find(|step| step.step_key == "director_revision")
        .unwrap()
        .id;
    let first_step = steps
        .iter()
        .find(|step| step.step_key == "performance_director")
        .unwrap()
        .id;
    let second_step = steps
        .iter()
        .find(|step| step.step_key == "sound_director")
        .unwrap()
        .id;
    sqlx::query("UPDATE production_steps SET status = 'succeeded' WHERE id = $1")
        .bind(dependency_step)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE production_steps SET status = 'queued' WHERE id IN ($1, $2)")
        .bind(first_step)
        .bind(second_step)
        .execute(&pool)
        .await
        .unwrap();
    let request_digest = "b".repeat(64);
    let first_claim = repo
        .claim_step(
            first_step,
            "worker-a",
            Duration::from_secs(30),
            &request_digest,
            "claim-a",
        )
        .await
        .unwrap();
    let second_claim = repo
        .claim_step(
            second_step,
            "worker-b",
            Duration::from_secs(30),
            &request_digest,
            "claim-b",
        )
        .await
        .unwrap();
    let request = ResourceRequest::role_call(60, 20);
    let (first, second) = tokio::join!(
        repo.reserve_resources(
            first_step,
            "worker-a",
            first_claim.attempt,
            request.clone(),
            &request_digest,
        ),
        repo.reserve_resources(
            second_step,
            "worker-b",
            second_claim.attempt,
            request,
            &request_digest,
        ),
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    let (winning_step, winning_owner, winning_attempt, reservations) = match (first, second) {
        (Ok(reservations), Err(error)) => {
            assert_eq!(error.code(), "resource_limit");
            (first_step, "worker-a", first_claim.attempt, reservations)
        }
        (Err(error), Ok(reservations)) => {
            assert_eq!(error.code(), "resource_limit");
            (second_step, "worker-b", second_claim.attempt, reservations)
        }
        _ => panic!("exactly one atomic reservation must succeed"),
    };
    assert_eq!(reservations.len(), 3);

    repo.settle_resources(
        winning_step,
        winning_owner,
        winning_attempt,
        [
            ("role_calls".into(), 1),
            ("input_tokens".into(), 55),
            ("output_tokens".into(), 18),
        ]
        .into_iter()
        .collect(),
        &"c".repeat(64),
        true,
    )
    .await
    .unwrap();
    let summary = repo.get_run(run.id).await.unwrap().resource_summary;
    assert!(summary
        .iter()
        .any(|item| { item["resource_key"] == "role_calls" && item["held_uncertain"] == 1 }));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM production_steps WHERE id = $1")
            .bind(winning_step)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "attention_required"
    );
}

#[tokio::test]
async fn role_retry_is_atomically_metered_idempotent_and_bounded() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "role-retry-metering"))
        .await
        .unwrap();
    let mut limits = ResourceLimits::strict_default();
    limits.max_role_retries = 1;
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan_with_limits(limits),
            actor: ProductionActor::local_operator(),
            idempotency_key: "role-retry-run".into(),
        })
        .await
        .unwrap();
    let producer = repo
        .get_run(run.id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.step_key == "producer")
        .unwrap();
    sqlx::query(
        "UPDATE production_steps SET status='failed',attempt=1,retryable=TRUE,side_effect_state='none' WHERE id=$1",
    )
    .bind(producer.id)
    .execute(&pool)
    .await
    .unwrap();

    let command = RetryStepCommand {
        run_id: run.id,
        step_id: producer.id,
        actor: ProductionActor::local_operator(),
        idempotency_key: "role-retry-once".into(),
    };
    repo.retry_step(command.clone()).await.unwrap();
    repo.retry_step(command).await.unwrap();
    assert_eq!(
        sqlx::query_as::<_, (i64, i64, String)>(
            r#"
            SELECT reserved_value,actual_value,status
            FROM production_resource_reservations
            WHERE run_id=$1 AND step_id=$2 AND attempt_no=2 AND resource_key='role_retries'
            "#,
        )
        .bind(run.id)
        .bind(producer.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (1, 1, "settled".into())
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_resource_usage WHERE run_id=$1 AND resource_key='role_retries' AND used_value=1",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );

    sqlx::query(
        "UPDATE production_steps SET status='failed',attempt=2,retryable=TRUE,side_effect_state='none' WHERE id=$1",
    )
    .bind(producer.id)
    .execute(&pool)
    .await
    .unwrap();
    let error = repo
        .retry_step(RetryStepCommand {
            run_id: run.id,
            step_id: producer.id,
            actor: ProductionActor::local_operator(),
            idempotency_key: "role-retry-over-limit".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code(), "resource_limit");
    assert!(error.to_string().contains("role_retries"));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM production_steps WHERE id=$1")
            .bind(producer.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "failed"
    );
}

#[tokio::test]
async fn trusted_usage_is_settled_and_unused_reservations_are_released() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "create"))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start".into(),
        })
        .await
        .unwrap();
    let first_step = repo.get_run(run.id).await.unwrap().steps[0].id;
    let request_digest = "d".repeat(64);
    let claim = repo
        .claim_step(
            first_step,
            "worker-a",
            Duration::from_secs(30),
            &request_digest,
            "claim-a",
        )
        .await
        .unwrap();
    repo.reserve_resources(
        first_step,
        "worker-a",
        claim.attempt,
        ResourceRequest::role_call(60, 20),
        &request_digest,
    )
    .await
    .unwrap();
    repo.settle_resources(
        first_step,
        "worker-a",
        claim.attempt,
        [
            ("role_calls".into(), 1),
            ("input_tokens".into(), 55),
            ("output_tokens".into(), 18),
        ]
        .into_iter()
        .collect(),
        &"e".repeat(64),
        false,
    )
    .await
    .unwrap();
    let summary = repo.get_run(run.id).await.unwrap().resource_summary;
    assert!(summary
        .iter()
        .any(|item| { item["resource_key"] == "input_tokens" && item["actual"] == 55 }));

    sqlx::query(
        r#"
        UPDATE production_step_attempts
        SET status = 'succeeded', completed_at = NOW()
        WHERE step_id = $1 AND attempt_no = $2
        "#,
    )
    .bind(first_step)
    .bind(claim.attempt)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        UPDATE production_steps
        SET status = 'succeeded', lease_owner = NULL, lease_expires_at = NULL
        WHERE id = $1
        "#,
    )
    .bind(first_step)
    .execute(&pool)
    .await
    .unwrap();
    let second_step = repo
        .get_run(run.id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.step_key == "producer")
        .unwrap()
        .id;
    sqlx::query("UPDATE production_steps SET status = 'queued' WHERE id = $1")
        .bind(second_step)
        .execute(&pool)
        .await
        .unwrap();
    let second_claim = repo
        .claim_step(
            second_step,
            "worker-b",
            Duration::from_secs(30),
            &request_digest,
            "claim-b",
        )
        .await
        .unwrap();
    repo.reserve_resources(
        second_step,
        "worker-b",
        second_claim.attempt,
        ResourceRequest::video_generation(1, 10),
        &request_digest,
    )
    .await
    .unwrap();
    repo.release_resources(second_step, "worker-b", second_claim.attempt)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_resource_reservations WHERE step_id = $1 AND status = 'released'",
        )
        .bind(second_step)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
}

#[tokio::test]
async fn resource_and_audit_snapshots_reject_pricing_and_credentials() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "create"))
        .await
        .unwrap();
    let mut unsafe_bindings = plan().role_bindings;
    unsafe_bindings.get_mut("producer").unwrap()["api_key"] = json!("must-not-persist");
    let unsafe_plan =
        FullCrewPlanRegistry::snapshot_v1(false, unsafe_bindings, ResourceLimits::strict_default())
            .unwrap();
    assert_eq!(
        repo.start_run(StartRunCommand {
            intent_id: intent.id,
            plan: unsafe_plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "unsafe-start".into(),
        })
        .await
        .unwrap_err()
        .code(),
        "transition_conflict"
    );

    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "safe-start".into(),
        })
        .await
        .unwrap();
    let response = serde_json::to_value(repo.get_run(run.id).await.unwrap()).unwrap();
    for forbidden in [
        "price",
        "currency",
        "amount_limit",
        "api_key",
        "api_secret",
        "authorization",
        "credential",
    ] {
        assert!(!contains_json_key(&response, forbidden));
    }
}

#[tokio::test]
async fn active_source_is_locked_and_safe_cancellation_releases_the_topic() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "create"))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start".into(),
        })
        .await
        .unwrap();

    assert!(
        sqlx::query("UPDATE content_topics SET title = '绕过修改' WHERE id = $1")
            .bind(topic_id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE projects SET status = 'archived' WHERE id = $1")
            .bind(project_id)
            .execute(&pool)
            .await
            .is_err()
    );

    let cancelled = repo
        .cancel_run(
            run.id,
            ProductionActor::local_operator(),
            "cancel",
            "操作者终止测试流程",
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status, "cancelled");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM content_topics WHERE id = $1")
            .bind(topic_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "approved"
    );

    let next = repo
        .create_intent(create_command(project_id, topic_id, "new-intent"))
        .await
        .unwrap();
    assert_ne!(next.id, intent.id);
}

#[tokio::test]
async fn source_lifecycle_matrix_is_fail_closed_and_safe_failure_releases_the_lock() {
    let (_admin, pool, _guard) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());

    let (archived_project_id, archived_topic_id) = source(&pool).await;
    sqlx::query("UPDATE projects SET status = 'archived' WHERE id = $1")
        .bind(archived_project_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        repo.create_intent(create_command(
            archived_project_id,
            archived_topic_id,
            "archived-project",
        ))
        .await
        .unwrap_err()
        .code(),
        "source_invalid"
    );

    let (active_project_id, approved_topic_id) = source(&pool).await;
    let idea_topic_id: Uuid = sqlx::query_scalar(
        "INSERT INTO content_topics (project_id, title, status) VALUES ($1, '未确认选题', 'idea') RETURNING id",
    )
    .bind(active_project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        repo.create_intent(create_command(
            active_project_id,
            idea_topic_id,
            "idea-topic",
        ))
        .await
        .unwrap_err()
        .code(),
        "source_invalid"
    );
    let deleted_topic_id: Uuid = sqlx::query_scalar(
        "INSERT INTO content_topics (project_id, title, status, deleted_at) VALUES ($1, '已删除选题', 'approved', NOW()) RETURNING id",
    )
    .bind(active_project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        repo.create_intent(create_command(
            active_project_id,
            deleted_topic_id,
            "deleted-topic",
        ))
        .await
        .unwrap_err()
        .code(),
        "source_invalid"
    );
    let other_project_id: Uuid = sqlx::query_scalar(
        "INSERT INTO projects (name, status) VALUES ('其他账号', 'active') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        repo.create_intent(create_command(
            other_project_id,
            approved_topic_id,
            "cross-project",
        ))
        .await
        .unwrap_err()
        .code(),
        "source_invalid"
    );

    let intent = repo
        .create_intent(create_command(
            active_project_id,
            approved_topic_id,
            "locked-source",
        ))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "locked-start".into(),
        })
        .await
        .unwrap();
    let original_snapshot = run.source_snapshot.clone();
    let mutations = [
        "UPDATE content_topics SET title = '修改标题' WHERE id = $1",
        "UPDATE content_topics SET angle = '修改角度' WHERE id = $1",
        "UPDATE content_topics SET target_audience = '修改受众' WHERE id = $1",
        "UPDATE content_topics SET hook_points = ARRAY['修改看点'] WHERE id = $1",
        "UPDATE content_topics SET content_type = '修改类型' WHERE id = $1",
        "UPDATE content_topics SET tags = ARRAY['修改标签'] WHERE id = $1",
        "UPDATE content_topics SET status = 'archived' WHERE id = $1",
        "UPDATE content_topics SET deleted_at = NOW() WHERE id = $1",
    ];
    for mutation in mutations {
        let error = sqlx::query(mutation)
            .bind(approved_topic_id)
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains("source_locked"),
            "mutation `{mutation}` returned unexpected error: {error}"
        );
    }
    let ownership_error = sqlx::query("UPDATE content_topics SET project_id = $2 WHERE id = $1")
        .bind(approved_topic_id)
        .bind(other_project_id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(ownership_error.to_string().contains("source_locked"));
    let archive_error = sqlx::query("UPDATE projects SET status = 'archived' WHERE id = $1")
        .bind(active_project_id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(archive_error.to_string().contains("source_locked"));

    sqlx::query("UPDATE projects SET positioning = '后续策略变化' WHERE id = $1")
        .bind(active_project_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        repo.get_run(run.id).await.unwrap().run.source_snapshot,
        original_snapshot
    );

    let failed = repo
        .fail_run(
            run.id,
            ProductionActor::local_operator(),
            "safe-failure",
            "deterministic_role_failure",
        )
        .await
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM content_topics WHERE id = $1")
            .bind(approved_topic_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "approved"
    );
    assert!(repo
        .get_run(run.id)
        .await
        .unwrap()
        .steps
        .iter()
        .all(|step| !matches!(step.status.as_str(), "queued" | "running" | "blocked")));
    let next = repo
        .create_intent(create_command(
            active_project_id,
            approved_topic_id,
            "after-safe-failure",
        ))
        .await
        .unwrap();
    assert_ne!(next.id, intent.id);
}

#[tokio::test]
async fn cancellation_with_unknown_external_result_never_reports_cancelled() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "create"))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start".into(),
        })
        .await
        .unwrap();
    let step = repo.get_run(run.id).await.unwrap().steps[0].clone();
    sqlx::query(
        "UPDATE production_steps SET status = 'running', side_effect_state = 'unknown', lease_owner = 'worker', lease_expires_at = NOW() + INTERVAL '30 seconds' WHERE id = $1",
    )
    .bind(step.id)
    .execute(&pool)
    .await
    .unwrap();

    let result = repo
        .cancel_run(
            run.id,
            ProductionActor::local_operator(),
            "cancel",
            "外部结果未知",
        )
        .await
        .unwrap();
    assert_eq!(result.status, "attention_required");
    assert!(result.cancellation_intent.is_some());
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM production_projects WHERE id = $1")
            .bind(intent.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "attention_required"
    );
}

#[tokio::test]
async fn cancellation_coordinator_calls_work_port_once_and_waits_for_a_true_terminal_result() {
    let (_admin, pool, _guard) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());

    let (project_id, topic_id) = source(&pool).await;
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "cancel-create"))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "cancel-start".into(),
        })
        .await
        .unwrap();
    let first_step = repo.get_run(run.id).await.unwrap().steps[0].clone();
    let claimed = repo
        .claim_step(
            first_step.id,
            "cancel-worker",
            Duration::from_secs(30),
            &"c".repeat(64),
            "cancel-claim",
        )
        .await
        .unwrap();
    repo.reserve_resources(
        claimed.id,
        "cancel-worker",
        claimed.attempt,
        ResourceRequest::role_call(10, 5),
        &"d".repeat(64),
    )
    .await
    .unwrap();
    let external_run_id = link_active_work_generation_run(&pool, run.id, project_id).await;
    let cancelled_port = Arc::new(FakeWorkCancellationPort {
        pool: pool.clone(),
        result: ExternalCancellationState::Cancelled,
        calls: AtomicUsize::new(0),
    });
    let service = ProductionCancellationService::new(repo.clone(), cancelled_port.clone());
    let cancelled = service
        .cancel(
            run.id,
            ProductionActor::local_operator(),
            "coordinated-cancel",
            "停止作品生产",
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status, "cancelled");
    assert_eq!(cancelled_port.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM work_generation_runs WHERE id = $1")
            .bind(external_run_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "cancelled"
    );
    let external_identity = sqlx::query_as::<_, (Uuid, Uuid, Uuid)>(
        "SELECT work_id,work_version_id,work_plan_id FROM work_generation_runs WHERE id=$1",
    )
    .bind(external_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let observed_cancelled = WorkGenerationRunReference::build(
        external_run_id,
        external_identity.0,
        external_identity.1,
        external_identity.2,
        WorkGenerationRunStatus::Cancelled,
        None,
        None,
        None,
        false,
        false,
        false,
    )
    .unwrap();
    assert_eq!(
        repo.sync_work_generation_state(run.id, &observed_cancelled)
            .await
            .unwrap(),
        WorkGenerationRunDisposition::Cancelled
    );
    assert_eq!(
        repo.get_run(run.id).await.unwrap().run.status,
        "cancelled",
        "观察已确认的外部取消不得让 ProductionRun 回退到 cancelling"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_resource_reservations WHERE run_id = $1"
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "released"
    );
    assert_eq!(
        service
            .cancel(
                run.id,
                ProductionActor::local_operator(),
                "coordinated-cancel",
                "停止作品生产",
            )
            .await
            .unwrap()
            .status,
        "cancelled"
    );
    assert_eq!(cancelled_port.calls.load(Ordering::SeqCst), 1);
    repo.create_intent(create_command(project_id, topic_id, "after-terminal"))
        .await
        .unwrap();

    let (attention_project_id, attention_topic_id) = source(&pool).await;
    let attention_intent = repo
        .create_intent(create_command(
            attention_project_id,
            attention_topic_id,
            "attention-create",
        ))
        .await
        .unwrap();
    let attention_run = repo
        .start_run(StartRunCommand {
            intent_id: attention_intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "attention-start".into(),
        })
        .await
        .unwrap();
    link_active_work_generation_run(&pool, attention_run.id, attention_project_id).await;
    let attention_port = Arc::new(FakeWorkCancellationPort {
        pool: pool.clone(),
        result: ExternalCancellationState::AttentionRequired,
        calls: AtomicUsize::new(0),
    });
    let attention_service =
        ProductionCancellationService::new(repo.clone(), attention_port.clone());
    let attention = attention_service
        .cancel(
            attention_run.id,
            ProductionActor::local_operator(),
            "attention-cancel",
            "等待上游人工确认",
        )
        .await
        .unwrap();
    assert_eq!(attention.status, "attention_required");
    assert_eq!(
        attention.error_code.as_deref(),
        Some("external_cancellation_attention_required")
    );
    assert_eq!(attention_port.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        repo.create_intent(create_command(
            attention_project_id,
            attention_topic_id,
            "attention-still-locked",
        ))
        .await
        .unwrap_err()
        .code(),
        "active_intent_conflict"
    );
}

#[tokio::test]
async fn package_gate_is_digest_bound_idempotent_and_unlocks_only_exact_successor() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "create"))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start".into(),
        })
        .await
        .unwrap();
    let view = repo.get_run(run.id).await.unwrap();
    let producer = view
        .steps
        .iter()
        .find(|step| step.step_key == "producer")
        .unwrap();
    sqlx::query(
        "UPDATE production_steps SET status = 'succeeded', attempt = 1, output_digest = $2, completed_at = NOW() WHERE id = $1",
    )
    .bind(producer.id)
    .bind("b".repeat(64))
    .execute(&pool)
    .await
    .unwrap();
    let approved_artifact_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO creative_briefs (
            production_project_id, version, status, content, run_id, step_id,
            attempt, revision_epoch, content_digest, audit_status
        ) VALUES (
            $1, 1, 'approved', '{"target_audience":"开发者"}', $2, $3,
            1, 0, $4, 'complete'
        ) RETURNING id
        "#,
    )
    .bind(intent.id)
    .bind(run.id)
    .bind(producer.id)
    .bind("b".repeat(64))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        repo.get_run(run.id)
            .await
            .unwrap()
            .steps
            .iter()
            .find(|step| step.step_key == "brief_approval")
            .unwrap()
            .status,
        "blocked"
    );
    let package = ArtifactPackageSnapshot::build(
        PackageType::Brief,
        run.id,
        producer.id,
        1,
        0,
        1,
        vec![ArtifactRef {
            run_id: run.id,
            artifact_type: "creative_brief".into(),
            artifact_id: approved_artifact_id,
            version: 1,
            content_digest: "b".repeat(64),
            source_step_id: producer.id,
            source_attempt: 1,
        }],
        json!({}),
    )
    .unwrap();
    repo.save_package(&package).await.unwrap();
    assert_eq!(
        repo.get_run(run.id)
            .await
            .unwrap()
            .steps
            .iter()
            .find(|step| step.step_key == "brief_approval")
            .unwrap()
            .status,
        "waiting_approval"
    );

    let stale = PackageDecisionCommand {
        run_id: run.id,
        package_digest: "c".repeat(64),
        decision: GateDecision::Approve,
        reason: None,
        affected_owners: vec![],
        actor: ProductionActor::local_operator(),
        idempotency_key: "approve-stale".into(),
    };
    assert_eq!(
        repo.decide_package(stale).await.unwrap_err().code(),
        "stale_package"
    );

    let approve = PackageDecisionCommand {
        run_id: run.id,
        package_digest: package.package_digest.clone(),
        decision: GateDecision::Approve,
        reason: None,
        affected_owners: vec![],
        actor: ProductionActor::local_operator(),
        idempotency_key: "approve".into(),
    };
    let decision = repo.decide_package(approve.clone()).await.unwrap();
    assert_eq!(decision.actor_type, "local_operator");
    assert_eq!(decision.actor_id, "local_operator");
    assert!(decision.decided_at <= chrono::Utc::now());
    assert_eq!(repo.decide_package(approve).await.unwrap().id, decision.id);
    let after = repo.get_run(run.id).await.unwrap();
    assert_eq!(
        after
            .steps
            .iter()
            .find(|step| step.step_key == "brief_approval")
            .unwrap()
            .status,
        "succeeded"
    );
    assert_eq!(
        after
            .steps
            .iter()
            .find(|step| step.step_key == "screenwriter")
            .unwrap()
            .status,
        "queued"
    );
    let opposite = PackageDecisionCommand {
        run_id: run.id,
        package_digest: package.package_digest,
        decision: GateDecision::Reject,
        reason: Some("相反决策".into()),
        affected_owners: vec!["producer".into()],
        actor: ProductionActor::local_operator(),
        idempotency_key: "opposite".into(),
    };
    assert_eq!(
        repo.decide_package(opposite).await.unwrap_err().code(),
        "transition_conflict"
    );
}

#[tokio::test]
async fn package_reject_creates_append_only_bounded_revision_epoch() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "create"))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start".into(),
        })
        .await
        .unwrap();
    let producer = repo
        .get_run(run.id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.step_key == "producer")
        .unwrap();
    sqlx::query("UPDATE production_steps SET status = 'succeeded', attempt = 1 WHERE id = $1")
        .bind(producer.id)
        .execute(&pool)
        .await
        .unwrap();
    let package = ArtifactPackageSnapshot::build(
        PackageType::Brief,
        run.id,
        producer.id,
        1,
        0,
        1,
        vec![ArtifactRef {
            run_id: run.id,
            artifact_type: "creative_brief".into(),
            artifact_id: Uuid::new_v4(),
            version: 1,
            content_digest: "d".repeat(64),
            source_step_id: producer.id,
            source_attempt: 1,
        }],
        json!({}),
    )
    .unwrap();
    repo.save_package(&package).await.unwrap();
    repo.decide_package(PackageDecisionCommand {
        run_id: run.id,
        package_digest: package.package_digest,
        decision: GateDecision::Reject,
        reason: Some("受众和核心信息不够明确".into()),
        affected_owners: vec!["producer".into()],
        actor: ProductionActor::local_operator(),
        idempotency_key: "reject".into(),
    })
    .await
    .unwrap();

    let after = repo.get_run(run.id).await.unwrap();
    assert_eq!(after.run.current_revision_epoch, 1);
    assert!(after.steps.iter().any(|step| {
        step.revision_epoch == 0 && step.step_key == "producer" && step.status == "succeeded"
    }));
    assert!(after.steps.iter().any(|step| {
        step.revision_epoch == 1 && step.step_key == "producer" && step.status == "queued"
    }));
    assert_eq!(after.gate_decisions.len(), 1);
    assert_eq!(after.packages.len(), 1);
}

#[tokio::test]
async fn collaboration_suggestions_bind_source_audit_and_have_immutable_responses() {
    let (_admin, pool, _guard) = database().await;
    let (project_id, topic_id) = source(&pool).await;
    let repo = DurableProductionRepository::new(pool.clone());
    let intent = repo
        .create_intent(create_command(project_id, topic_id, "create"))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "start".into(),
        })
        .await
        .unwrap();
    let source_step = repo
        .get_run(run.id)
        .await
        .unwrap()
        .steps
        .into_iter()
        .find(|step| step.step_key == "cinematographer")
        .unwrap();
    let model_call_id = attach_model_call(&pool, source_step.id, 1).await;
    let command = CreateCollaborationSuggestionCommand {
        run_id: run.id,
        source_step_id: source_step.id,
        source_attempt: 1,
        source_model_call_id: model_call_id,
        from_role: "cinematographer".into(),
        to_role: "director".into(),
        target_artifact_type: "shot_contract".into(),
        target_artifact_id: Uuid::new_v4(),
        target_artifact_version: 1,
        target_content_digest: "3".repeat(64),
        suggestion_type: "revision".into(),
        content: json!({"reason": "轴线需要统一", "specific_change": "调整反打镜头"}),
        blocking: true,
    };
    let suggestion = repo
        .create_collaboration_suggestion(command.clone())
        .await
        .unwrap();
    assert_eq!(suggestion.source_model_call_id, model_call_id);
    assert_eq!(suggestion.to_role, "director");
    assert!(suggestion.blocking);

    let accepted = repo
        .respond_to_collaboration_suggestion(
            suggestion.id,
            SuggestionDecision::Accepted,
            None,
            ProductionActor::local_operator(),
        )
        .await
        .unwrap();
    assert_eq!(
        repo.respond_to_collaboration_suggestion(
            suggestion.id,
            SuggestionDecision::Accepted,
            None,
            ProductionActor::local_operator(),
        )
        .await
        .unwrap()
        .id,
        accepted.id
    );
    assert_eq!(
        repo.respond_to_collaboration_suggestion(
            suggestion.id,
            SuggestionDecision::Rejected,
            Some("相反决定".into()),
            ProductionActor::local_operator(),
        )
        .await
        .unwrap_err()
        .code(),
        "transition_conflict"
    );
    assert!(sqlx::query(
        "UPDATE collaboration_suggestion_responses SET reason = '篡改' WHERE id = $1",
    )
    .bind(accepted.id)
    .execute(&pool)
    .await
    .is_err());

    let current_steps = repo.get_run(run.id).await.unwrap().steps;
    let director_step = current_steps
        .iter()
        .find(|step| step.step_key == "director")
        .unwrap()
        .id;
    let sound_step = current_steps
        .iter()
        .find(|step| step.step_key == "sound_director")
        .unwrap()
        .id;
    let performance_step = current_steps
        .iter()
        .find(|step| step.step_key == "performance_director")
        .unwrap()
        .id;
    sqlx::query(
        "UPDATE production_steps SET status = 'succeeded', attempt = 1 WHERE id IN ($1, $2, $3)",
    )
    .bind(director_step)
    .bind(sound_step)
    .bind(performance_step)
    .execute(&pool)
    .await
    .unwrap();
    let revised_artifact_id = Uuid::new_v4();
    let revised_digest = "5".repeat(64);
    let revised_item = ArtifactRef {
        run_id: run.id,
        artifact_type: "shot_contract".into(),
        artifact_id: revised_artifact_id,
        version: 2,
        content_digest: revised_digest.clone(),
        source_step_id: director_step,
        source_attempt: 1,
    };
    let production_items = vec![
        revised_item,
        ArtifactRef {
            run_id: run.id,
            artifact_type: "directorial_treatment".into(),
            artifact_id: Uuid::new_v4(),
            version: 2,
            content_digest: "6".repeat(64),
            source_step_id: director_step,
            source_attempt: 1,
        },
        ArtifactRef {
            run_id: run.id,
            artifact_type: "performance_brief".into(),
            artifact_id: Uuid::new_v4(),
            version: 1,
            content_digest: "7".repeat(64),
            source_step_id: director_step,
            source_attempt: 1,
        },
        ArtifactRef {
            run_id: run.id,
            artifact_type: "sound_plan".into(),
            artifact_id: Uuid::new_v4(),
            version: 1,
            content_digest: "8".repeat(64),
            source_step_id: director_step,
            source_attempt: 1,
        },
    ];
    let unresolved_metadata = production_package_metadata(&production_items, vec![]);
    let unresolved = ArtifactPackageSnapshot::build(
        PackageType::Production,
        run.id,
        sound_step,
        1,
        0,
        1,
        production_items.clone(),
        unresolved_metadata,
    )
    .unwrap();
    assert_eq!(
        repo.save_package(&unresolved).await.unwrap_err().code(),
        "transition_conflict"
    );
    let resolved_metadata = production_package_metadata(
        &production_items,
        vec![json!({
            "suggestion_id": suggestion.id,
            "owner_role": "director",
            "artifact_id": revised_artifact_id,
            "artifact_version": 2,
            "content_digest": revised_digest
        })],
    );
    let resolved = ArtifactPackageSnapshot::build(
        PackageType::Production,
        run.id,
        sound_step,
        1,
        0,
        1,
        production_items,
        resolved_metadata,
    )
    .unwrap();
    repo.save_package(&resolved).await.unwrap();

    let rejected_target = repo
        .create_collaboration_suggestion(CreateCollaborationSuggestionCommand {
            target_artifact_id: Uuid::new_v4(),
            target_content_digest: "4".repeat(64),
            blocking: false,
            ..command
        })
        .await
        .unwrap();
    assert_eq!(
        repo.respond_to_collaboration_suggestion(
            rejected_target.id,
            SuggestionDecision::Rejected,
            Some("   ".into()),
            ProductionActor::local_operator(),
        )
        .await
        .unwrap_err()
        .code(),
        "transition_conflict"
    );
    let rejected = repo
        .respond_to_collaboration_suggestion(
            rejected_target.id,
            SuggestionDecision::Rejected,
            Some("不符合当前导演意图".into()),
            ProductionActor::local_operator(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.decision, "rejected");
}

#[tokio::test]
async fn production_package_builder_uses_exact_run_epoch_step_attempt_and_formal_script() {
    let (_admin, pool, _database) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = repo
        .create_intent(create_command(
            project_id,
            topic_id,
            "package-builder-intent",
        ))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "package-builder-run".into(),
        })
        .await
        .unwrap();
    let (script_id, scene_ids) =
        seed_production_package_scope(&pool, run.id, intent.id, project_id, topic_id).await;

    let (other_project_id, other_topic_id) = source(&pool).await;
    let other_intent = repo
        .create_intent(create_command(
            other_project_id,
            other_topic_id,
            "package-builder-other-intent",
        ))
        .await
        .unwrap();
    let other_run = repo
        .start_run(StartRunCommand {
            intent_id: other_intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "package-builder-other-run".into(),
        })
        .await
        .unwrap();
    let other_director = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM production_steps WHERE run_id=$1 AND revision_epoch=0 AND step_key='director'",
    )
    .bind(other_run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE production_steps SET status='succeeded', attempt=1 WHERE id=$1")
        .bind(other_director)
        .execute(&pool)
        .await
        .unwrap();
    let cross_run_content = json!({
        "shot_id": "cross-run-latest",
        "sequence": 99,
        "scene_id": scene_ids[0],
        "shot_type": "invalid",
        "camera_movement": "invalid",
        "duration_sec": 30,
        "description": "不得进入当前 Run",
        "character_ids": []
    });
    sqlx::query(
        r#"
        INSERT INTO shot_contracts (
            production_project_id, shot_id, scene_id, domain_scene_id, version,
            status, content, created_by, run_id, step_id, attempt, revision_epoch,
            content_digest, audit_status
        ) VALUES ($1,'cross-run-latest',$2,$3,100,'draft',$4,'director',$5,$6,1,0,$7,'complete')
        "#,
    )
    .bind(intent.id)
    .bind(scene_ids[0].to_string())
    .bind(scene_ids[0])
    .bind(&cross_run_content)
    .bind(other_run.id)
    .bind(other_director)
    .bind(canonical_digest(&cross_run_content).unwrap())
    .execute(&pool)
    .await
    .unwrap();

    let package = repo.build_production_package(run.id, 1).await.unwrap();
    let metadata: ProductionPackageMetadata =
        serde_json::from_value(package.metadata.clone()).unwrap();
    assert_eq!(metadata.script_id, script_id);
    assert_eq!(
        metadata
            .scenes
            .iter()
            .map(|scene| scene.scene_id)
            .collect::<Vec<_>>(),
        scene_ids
    );
    assert_eq!(metadata.shots.len(), 2);
    assert_eq!(metadata.performance_briefs.len(), 1);
    assert_eq!(metadata.sound_plan.scene_ids, scene_ids);
    assert!(package.items.iter().all(|item| {
        item.run_id == run.id
            && item.source_attempt == 1
            && item.version == 1
            && item.content_digest.len() == 64
    }));
    assert!(!package.items.iter().any(|item| item.version >= 99));
}

#[tokio::test]
async fn missing_scene_visual_manifest_blocks_before_work_plan_run_or_provider_tasks() {
    let (_admin, pool, _database) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = repo
        .create_intent(create_command(
            project_id,
            topic_id,
            "manifest-blocker-intent",
        ))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "manifest-blocker-run".into(),
        })
        .await
        .unwrap();
    let (_script_id, scene_ids) =
        seed_production_package_scope(&pool, run.id, intent.id, project_id, topic_id).await;
    let package = repo.build_production_package(run.id, 1).await.unwrap();
    repo.save_package(&package).await.unwrap();
    repo.decide_package(PackageDecisionCommand {
        run_id: run.id,
        package_digest: package.package_digest.clone(),
        decision: GateDecision::Approve,
        reason: None,
        affected_owners: vec![],
        actor: ProductionActor::local_operator(),
        idempotency_key: "approve-manifest-blocker-package".into(),
    })
    .await
    .unwrap();
    let input = repo
        .load_approved_production_input(run.id, &package.package_digest)
        .await
        .unwrap();
    let asset_service = AssetGenerationService::new(
        pool.clone(),
        PostgresAiModelRepository::new(pool.clone()),
        PostgresAssetGenerationRepository::new(pool.clone()),
        PostgresMaterialRepository::new(pool.clone()),
        PostgresScriptRepository::new(pool.clone()),
    );
    let integration = ProductionWorkflowIntegrationService::new(asset_service, None);

    let error = integration
        .prepare_scene_visual_manifest(input)
        .await
        .unwrap_err();
    assert_eq!(error.code(), "external_wait");
    let details = error.details().expect("external wait must retain blockers");
    let blockers = details["blockers"].as_array().unwrap();
    assert_eq!(blockers.len(), scene_ids.len());
    assert!(blockers.iter().all(|blocker| {
        blocker["reason"] == "selected_image_missing"
            && scene_ids.contains(&Uuid::parse_str(blocker["scene_id"].as_str().unwrap()).unwrap())
    }));
    for table in [
        "work_plans",
        "work_generation_runs",
        "asset_generation_tasks",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0,
            "table={table}"
        );
    }
}

#[tokio::test]
async fn scene_visual_manifest_external_wait_recovers_and_unlocks_only_work_planning() {
    let (_admin, pool, _database) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = repo
        .create_intent(create_command(
            project_id,
            topic_id,
            "manifest-recovery-intent",
        ))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "manifest-recovery-run".into(),
        })
        .await
        .unwrap();
    let (script_id, scene_ids) =
        seed_production_package_scope(&pool, run.id, intent.id, project_id, topic_id).await;
    let package = repo.build_production_package(run.id, 1).await.unwrap();
    repo.save_package(&package).await.unwrap();
    repo.decide_package(PackageDecisionCommand {
        run_id: run.id,
        package_digest: package.package_digest.clone(),
        decision: GateDecision::Approve,
        reason: None,
        affected_owners: vec![],
        actor: ProductionActor::local_operator(),
        idempotency_key: "approve-manifest-recovery-package".into(),
    })
    .await
    .unwrap();
    let integration = || {
        ProductionWorkflowIntegrationService::new(
            AssetGenerationService::new(
                pool.clone(),
                PostgresAiModelRepository::new(pool.clone()),
                PostgresAssetGenerationRepository::new(pool.clone()),
                PostgresMaterialRepository::new(pool.clone()),
                PostgresScriptRepository::new(pool.clone()),
            ),
            None,
        )
    };
    let orchestrator = || {
        let mut orchestrator = ProductionOrchestrator::new(
            pool.clone(),
            Arc::new(RoleRegistry::new()),
            Arc::new(GateRegistry::new()),
        );
        orchestrator.scene_visual_manifest_port = Some(Arc::new(integration()));
        orchestrator
    };

    let blocked = orchestrator()
        .resume_scene_visual_manifest(run.id, &package.package_digest)
        .await
        .unwrap_err();
    assert_eq!(blocked.code(), "external_wait");
    assert_eq!(
        sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT status, waiting_reason FROM production_steps WHERE run_id=$1 AND revision_epoch=0 AND step_key='wait_scene_visual_manifest'",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("external_wait".into(), Some("scene_visual_manifest".into()))
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_steps WHERE run_id=$1 AND revision_epoch=0 AND step_key='create_work_plan'",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "blocked"
    );

    for scene_id in &scene_ids {
        let material_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO materials (
                project_id, material_type, file_url, file_name, status
            ) VALUES ($1, 'image', $2, $3, 'active') RETURNING id
            "#,
        )
        .bind(project_id)
        .bind(format!("/materials/{scene_id}.png"))
        .bind(format!("{scene_id}.png"))
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO scene_asset_candidates (
                project_id, script_id, scene_id, material_id,
                candidate_type, source, status, rank
            ) VALUES ($1, $2, $3, $4, 'image', 'existing_material', 'selected', 0)
            "#,
        )
        .bind(project_id)
        .bind(script_id)
        .bind(scene_id)
        .bind(material_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let manifest = orchestrator()
        .resume_scene_visual_manifest(run.id, &package.package_digest)
        .await
        .unwrap();
    assert_eq!(manifest.script_id, script_id);
    assert_eq!(manifest.scenes.len(), scene_ids.len());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_scene_visual_manifests WHERE run_id=$1 AND package_id=$2 AND manifest_digest=$3",
        )
        .bind(run.id)
        .bind(package.id)
        .bind(&manifest.manifest_digest)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_as::<_, (String, String)>(
            "SELECT status, output_digest FROM production_steps WHERE run_id=$1 AND revision_epoch=0 AND step_key='wait_scene_visual_manifest'",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("succeeded".into(), manifest.manifest_digest)
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_steps WHERE run_id=$1 AND revision_epoch=0 AND step_key='create_work_plan'",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "queued"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_plans")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
async fn typed_production_package_plans_existing_work_with_auditable_sources_and_invalidation() {
    let (_admin, pool, _database) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = repo
        .create_intent(create_command(
            project_id,
            topic_id,
            "typed-work-plan-intent",
        ))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "typed-work-plan-run".into(),
        })
        .await
        .unwrap();
    let (script_id, scene_ids) =
        seed_production_package_scope(&pool, run.id, intent.id, project_id, topic_id).await;
    let package = repo.build_production_package(run.id, 1).await.unwrap();
    repo.save_package(&package).await.unwrap();
    repo.decide_package(PackageDecisionCommand {
        run_id: run.id,
        package_digest: package.package_digest.clone(),
        decision: GateDecision::Approve,
        reason: None,
        affected_owners: vec![],
        actor: ProductionActor::local_operator(),
        idempotency_key: "approve-typed-work-plan-package".into(),
    })
    .await
    .unwrap();
    let input = repo
        .load_approved_production_input(run.id, &package.package_digest)
        .await
        .unwrap();

    for scene_id in &scene_ids {
        let material_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO materials (project_id, material_type, file_url, file_name, status)
            VALUES ($1, 'image', $2, $3, 'active') RETURNING id
            "#,
        )
        .bind(project_id)
        .bind(format!("/materials/work-plan-{scene_id}.png"))
        .bind(format!("work-plan-{scene_id}.png"))
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO scene_asset_candidates (
                project_id, script_id, scene_id, material_id,
                candidate_type, source, status, rank
            ) VALUES ($1, $2, $3, $4, 'image', 'existing_material', 'selected', 0)
            "#,
        )
        .bind(project_id)
        .bind(script_id)
        .bind(scene_id)
        .bind(material_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    let llm_model_id = insert_enabled_text_model(&pool).await;
    let video_model_id = insert_enabled_video_model(&pool).await;
    let asset_service = AssetGenerationService::new(
        pool.clone(),
        PostgresAiModelRepository::new(pool.clone()),
        PostgresAssetGenerationRepository::new(pool.clone()),
        PostgresMaterialRepository::new(pool.clone()),
        PostgresScriptRepository::new(pool.clone()),
    );
    let work_service = WorkGenerationService::new(
        PostgresWorkGenerationRepository::new(pool.clone()),
        PostgresAiModelRepository::new(pool.clone()),
        PostgresVoiceCatalogRepository::new(pool.clone()),
        asset_service.clone(),
    );
    let integration =
        ProductionWorkflowIntegrationService::new(asset_service, Some(work_service.clone()));
    let manifest = integration
        .prepare_scene_visual_manifest(input.clone())
        .await
        .unwrap();
    let settings = ProductionWorkPlanSettings {
        llm_model_id,
        video_model_id,
        tts_model_id: None,
        tts_voice_type: None,
        duration_strategy: "script_total".into(),
        duration_seconds: None,
        aspect_ratio: "9:16".into(),
        resolution: "1080p".into(),
        audio_mode: "silent".into(),
        narration_override: None,
        audio_material_ids: vec![],
        burn_subtitles: false,
        overrides: Default::default(),
    };
    let first = integration
        .create_work_plan(ProductionWorkPlanRequest {
            production: input.clone(),
            manifest: manifest.clone(),
            operator_settings: settings.clone(),
        })
        .await
        .unwrap();

    let saved: (
        i32,
        String,
        serde_json::Value,
        serde_json::Value,
        serde_json::Value,
    ) = sqlx::query_as(
        r#"
            SELECT version_no, source_manifest_version, input_snapshot,
                   prompt_snapshot, timeline_snapshot
            FROM work_versions WHERE id=$1
            "#,
    )
    .bind(first.work_version_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(first.work_version, 1);
    assert_eq!(saved.0, 1);
    assert_eq!(saved.1, manifest.manifest_version);
    assert_eq!(
        saved.2["production_crew"]["production_run_id"],
        run.id.to_string()
    );
    assert_eq!(
        saved.2["production_crew"]["production_package"]["digest"],
        package.package_digest
    );
    assert_eq!(
        saved.2["production_crew"]["script"]["script_id"],
        script_id.to_string()
    );
    assert_eq!(
        saved.2["production_crew"]["scene_visual_manifest"]["manifest_digest"],
        manifest.manifest_digest
    );
    assert_eq!(
        saved.3["production_crew"]["directorial_treatment"]["content"]["visual_style"],
        "纪实"
    );
    assert_eq!(
        saved.3["production_crew"]["shot_contracts"]
            .as_array()
            .unwrap()
            .len(),
        scene_ids.len()
    );
    assert_eq!(
        saved.3["production_crew"]["performance_briefs"][0]["content"]["vocal_direction"],
        "清晰"
    );
    assert_eq!(
        saved.4["production_crew"]["sound_plan"]["content"]["music_style"],
        "极简"
    );

    let next_input = next_production_package(&input);
    let second = integration
        .create_work_plan(ProductionWorkPlanRequest {
            production: next_input.clone(),
            manifest: manifest.clone(),
            operator_settings: settings.clone(),
        })
        .await
        .unwrap();
    assert_eq!(second.work_id, first.work_id);
    assert_eq!(second.work_version_id, first.work_version_id);
    assert_ne!(second.input_fingerprint, first.input_fingerprint);

    let mut changed_settings = settings;
    changed_settings.aspect_ratio = "16:9".into();
    let third = integration
        .create_work_plan(ProductionWorkPlanRequest {
            production: next_input.clone(),
            manifest: manifest.clone(),
            operator_settings: changed_settings.clone(),
        })
        .await
        .unwrap();
    assert_eq!(third.work_id, first.work_id);
    assert_eq!(third.work_version_id, first.work_version_id);
    assert_ne!(third.input_fingerprint, second.input_fingerprint);

    sqlx::query(
        "UPDATE scene_asset_candidates SET status='rejected' WHERE script_id=$1 AND status='selected'",
    )
    .bind(script_id)
    .execute(&pool)
    .await
    .unwrap();
    for scene_id in &scene_ids {
        let material_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO materials (project_id, material_type, file_url, file_name, status)
            VALUES ($1, 'image', $2, $3, 'active') RETURNING id
            "#,
        )
        .bind(project_id)
        .bind(format!("/materials/reselected-{scene_id}.png"))
        .bind(format!("reselected-{scene_id}.png"))
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO scene_asset_candidates (
                project_id, script_id, scene_id, material_id,
                candidate_type, source, status, rank
            ) VALUES ($1,$2,$3,$4,'image','existing_material','selected',0)
            "#,
        )
        .bind(project_id)
        .bind(script_id)
        .bind(scene_id)
        .bind(material_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    let changed_manifest = integration
        .prepare_scene_visual_manifest(next_input.clone())
        .await
        .unwrap();
    assert_ne!(changed_manifest.manifest_digest, manifest.manifest_digest);
    let fourth = integration
        .create_work_plan(ProductionWorkPlanRequest {
            production: next_input.clone(),
            manifest: changed_manifest,
            operator_settings: changed_settings.clone(),
        })
        .await
        .unwrap();
    assert_ne!(fourth.input_fingerprint, third.input_fingerprint);

    let replacement_script_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO scripts (project_id,title,hook,content,status,parent_id)
        VALUES ($1,$2,$3,$4,'approved',$5) RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(&next_input.script.title)
    .bind(&next_input.script.hook)
    .bind(json!({"production_run_id": run.id, "revision": "test"}))
    .bind(script_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut replacement_scene_ids = Vec::new();
    for scene in &next_input.scenes {
        let scene_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO scenes (
                script_id,sequence,narration,visual_description,emotion,duration_sec
            ) VALUES ($1,$2,$3,$4,$5,$6) RETURNING id
            "#,
        )
        .bind(replacement_script_id)
        .bind(scene.sequence as i32)
        .bind(&scene.narration)
        .bind(&scene.visual_description)
        .bind(&scene.emotion)
        .bind(scene.duration_sec as i32)
        .fetch_one(&pool)
        .await
        .unwrap();
        replacement_scene_ids.push(scene_id);
    }
    let replacement_input = rebind_production_package_to_script(
        &next_input,
        replacement_script_id,
        &replacement_scene_ids,
    );
    for scene_id in &replacement_scene_ids {
        let material_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO materials (project_id, material_type, file_url, file_name, status)
            VALUES ($1, 'image', $2, $3, 'active') RETURNING id
            "#,
        )
        .bind(project_id)
        .bind(format!("/materials/replacement-{scene_id}.png"))
        .bind(format!("replacement-{scene_id}.png"))
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
            INSERT INTO scene_asset_candidates (
                project_id, script_id, scene_id, material_id,
                candidate_type, source, status, rank
            ) VALUES ($1,$2,$3,$4,'image','existing_material','selected',0)
            "#,
        )
        .bind(project_id)
        .bind(replacement_script_id)
        .bind(scene_id)
        .bind(material_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    let replacement_manifest = integration
        .prepare_scene_visual_manifest(replacement_input.clone())
        .await
        .unwrap();
    let fifth = integration
        .create_work_plan(ProductionWorkPlanRequest {
            production: replacement_input.clone(),
            manifest: replacement_manifest.clone(),
            operator_settings: changed_settings.clone(),
        })
        .await
        .unwrap();
    assert_ne!(fifth.work_id, fourth.work_id);
    assert_ne!(fifth.input_fingerprint, fourth.input_fingerprint);
    let override_llm_model_id = insert_enabled_text_model(&pool).await;
    let override_video_model_id = insert_enabled_video_model(&pool).await;
    let (tts_model_id, tts_voice_type) = insert_enabled_tts_voice(&pool).await;
    let first_scene_id = replacement_scene_ids[0];
    let mut override_settings = changed_settings;
    override_settings.llm_model_id = override_llm_model_id;
    override_settings.video_model_id = override_video_model_id;
    override_settings.tts_model_id = Some(tts_model_id);
    override_settings.tts_voice_type = Some(tts_voice_type.clone());
    override_settings.audio_mode = "independent_tts".into();
    override_settings.narration_override = Some("人工调整后的完整旁白".into());
    override_settings.burn_subtitles = true;
    override_settings.aspect_ratio = "9:16".into();
    override_settings.overrides = ProductionWorkPlanOverrides {
        full_prompt: Some("人工确认的全片视觉 Prompt".into()),
        scene_prompts: vec![ScenePromptOverride {
            scene_id: first_scene_id,
            prompt: "人工确认的第一幕 Prompt".into(),
        }],
        segment_prompts: Some(vec!["人工确认的分段 Prompt".into()]),
        scene_durations: vec![SceneDurationOverride {
            scene_id: first_scene_id,
            duration_sec: 6,
        }],
    };
    let sixth = integration
        .create_work_plan(ProductionWorkPlanRequest {
            production: replacement_input,
            manifest: replacement_manifest,
            operator_settings: override_settings,
        })
        .await
        .unwrap();
    assert_eq!(sixth.work_id, fifth.work_id);
    assert_eq!(sixth.work_version_id, fifth.work_version_id);
    assert_ne!(sixth.input_fingerprint, fifth.input_fingerprint);
    let override_snapshots = sqlx::query_as::<_, (serde_json::Value, serde_json::Value, serde_json::Value, serde_json::Value)>(
        r#"
        SELECT version.input_snapshot,plan.output_snapshot,plan.prompt_snapshot,plan.timeline_snapshot
        FROM work_plans plan JOIN work_versions version ON version.id=plan.work_version_id
        WHERE plan.id=$1
        "#,
    )
    .bind(sixth.work_plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let override_diff = override_snapshots.0["production_crew"]["override_diff"]
        .as_array()
        .unwrap();
    for field in [
        "models.llm_model_id",
        "models.video_model_id",
        "models.tts_model_id",
        "voice.tts_voice_type",
        "sound.audio_mode",
        "subtitles.burn_subtitles",
        "output.aspect_ratio",
        "prompts.full_prompt",
        "prompts.segments",
    ] {
        assert!(override_diff.iter().any(|item| item["field"] == field));
    }
    assert!(override_diff
        .iter()
        .any(|item| { item["field"] == format!("main_images.{first_scene_id}") }));
    assert!(override_diff
        .iter()
        .any(|item| { item["field"] == format!("timeline.scenes.{first_scene_id}.duration_sec") }));
    assert_eq!(
        override_snapshots.0["models"]["tts_model_id"],
        tts_model_id.to_string()
    );
    assert_eq!(
        override_snapshots.2["full_prompt"],
        "人工确认的全片视觉 Prompt"
    );
    assert_eq!(override_snapshots.3["burn_subtitles"], true);
    assert_eq!(
        override_snapshots.1["production_override_diff"],
        override_snapshots.2["production_crew"]["override_diff"]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_generation_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0,
        "重新规划不得自动确认"
    );
    insert_enabled_tos_staging_config(&pool).await;
    let confirmed_override = work_service
        .confirm(sixth.work_plan_id, "confirm-full-crew-overrides".into())
        .await
        .unwrap();
    assert!(confirmed_override.created);
    assert_eq!(
        confirmed_override.run.parameter_snapshot["production_override_diff"],
        override_snapshots.1["production_override_diff"]
    );
    assert_eq!(
        confirmed_override.run.prompt_snapshot["production_crew"]["override_diff"],
        override_snapshots.2["production_crew"]["override_diff"]
    );
    assert_eq!(
        confirmed_override.run.timeline_snapshot["production_crew"]["override_diff"],
        override_snapshots.3["production_crew"]["override_diff"]
    );
    assert_eq!(
        sqlx::query_as::<_, (String, String, String, String, String, String)>(
            "SELECT p1.status,p2.status,p3.status,p4.status,p5.status,p6.status FROM work_plans p1,work_plans p2,work_plans p3,work_plans p4,work_plans p5,work_plans p6 WHERE p1.id=$1 AND p2.id=$2 AND p3.id=$3 AND p4.id=$4 AND p5.id=$5 AND p6.id=$6",
        )
        .bind(first.work_plan_id)
        .bind(second.work_plan_id)
        .bind(third.work_plan_id)
        .bind(fourth.work_plan_id)
        .bind(fifth.work_plan_id)
        .bind(sixth.work_plan_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (
            "invalidated".into(),
            "invalidated".into(),
            "invalidated".into(),
            "invalidated".into(),
            "invalidated".into(),
            "confirmed".into()
        )
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT (SELECT COUNT(*) FROM works),(SELECT COUNT(*) FROM work_versions),(SELECT COUNT(*) FROM work_plans)",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (2, 2, 6)
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT (SELECT COUNT(*) FROM work_generation_runs),(SELECT COUNT(*) FROM asset_generation_tasks)",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (1, 0)
    );
}

#[tokio::test]
async fn existing_manual_confirmation_is_idempotent_and_enters_production_external_wait() {
    let (_admin, pool, _database) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = repo
        .create_intent(create_command(
            project_id,
            topic_id,
            "manual-work-confirmation-intent",
        ))
        .await
        .unwrap();
    let mut resource_limits = ResourceLimits::strict_default();
    resource_limits.max_provider_retries = 2;
    let resource_plan = plan_with_limits(resource_limits);
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: resource_plan,
            actor: ProductionActor::local_operator(),
            idempotency_key: "manual-work-confirmation-run".into(),
        })
        .await
        .unwrap();
    let (script_id, scene_ids) =
        seed_production_package_scope(&pool, run.id, intent.id, project_id, topic_id).await;
    let package = repo.build_production_package(run.id, 1).await.unwrap();
    repo.save_package(&package).await.unwrap();
    repo.decide_package(PackageDecisionCommand {
        run_id: run.id,
        package_digest: package.package_digest.clone(),
        decision: GateDecision::Approve,
        reason: None,
        affected_owners: vec![],
        actor: ProductionActor::local_operator(),
        idempotency_key: "approve-manual-work-confirmation-package".into(),
    })
    .await
    .unwrap();
    insert_selected_visuals(&pool, project_id, script_id, &scene_ids, "confirm").await;
    let llm_model_id = insert_enabled_text_model(&pool).await;
    let video_model_id = insert_enabled_video_model(&pool).await;
    let (tts_model_id, tts_voice_type) = insert_enabled_tts_voice(&pool).await;
    insert_enabled_tos_staging_config(&pool).await;
    let asset_service = AssetGenerationService::new(
        pool.clone(),
        PostgresAiModelRepository::new(pool.clone()),
        PostgresAssetGenerationRepository::new(pool.clone()),
        PostgresMaterialRepository::new(pool.clone()),
        PostgresScriptRepository::new(pool.clone()),
    );
    let work_service = WorkGenerationService::new(
        PostgresWorkGenerationRepository::new(pool.clone()),
        PostgresAiModelRepository::new(pool.clone()),
        PostgresVoiceCatalogRepository::new(pool.clone()),
        asset_service.clone(),
    );
    let integration = Arc::new(ProductionWorkflowIntegrationService::new(
        asset_service,
        Some(work_service.clone()),
    ));
    let mut orchestrator = ProductionOrchestrator::new(
        pool.clone(),
        Arc::new(RoleRegistry::new()),
        Arc::new(GateRegistry::new()),
    );
    orchestrator.scene_visual_manifest_port = Some(integration.clone());
    orchestrator.work_generation_planning_port = Some(integration.clone());
    orchestrator.work_generation_run_port = Some(integration);

    let manifest = orchestrator
        .resume_scene_visual_manifest(run.id, &package.package_digest)
        .await
        .unwrap();
    let input = repo
        .load_approved_production_input(run.id, &package.package_digest)
        .await
        .unwrap();
    let plan_reference = orchestrator
        .resume_create_work_plan(ProductionWorkPlanRequest {
            production: input,
            manifest,
            operator_settings: ProductionWorkPlanSettings {
                llm_model_id,
                video_model_id,
                tts_model_id: Some(tts_model_id),
                tts_voice_type: Some(tts_voice_type),
                duration_strategy: "script_total".into(),
                duration_seconds: None,
                aspect_ratio: "9:16".into(),
                resolution: "1080p".into(),
                audio_mode: "independent_tts".into(),
                narration_override: None,
                audio_material_ids: vec![],
                burn_subtitles: false,
                overrides: Default::default(),
            },
        })
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_domain_links WHERE run_id=$1 AND link_type IN ('work','work_version','work_plan')",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        3
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT status FROM production_steps WHERE run_id=$1 AND step_key='work_plan_confirmation' AND revision_epoch=0",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        "queued"
    );

    let first = work_service
        .confirm(
            plan_reference.work_plan_id,
            "manual-confirm-same-key".into(),
        )
        .await
        .unwrap();
    let replay = work_service
        .confirm(
            plan_reference.work_plan_id,
            "manual-confirm-same-key".into(),
        )
        .await
        .unwrap();
    assert!(first.created);
    assert!(!replay.created);
    assert_eq!(replay.run.id, first.run.id);
    assert_eq!(replay.run.resource_usage, first.run.resource_usage);
    for forbidden in ["price", "cost", "currency", "amount", "api_key"] {
        assert!(!first
            .run
            .resource_usage
            .to_string()
            .to_ascii_lowercase()
            .contains(forbidden));
    }
    assert_eq!(first.run.resource_usage["video_task_count"], 1);
    assert_eq!(first.run.resource_usage["video_seconds"], 10);
    assert!(
        first.run.resource_usage["tts_characters"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, i64, i64, i64, i64)>(
            r#"
            SELECT
              MAX(reserved_value) FILTER (WHERE resource_key='video_tasks'),
              MAX(reserved_value) FILTER (WHERE resource_key='video_duration_sec'),
              MAX(reserved_value) FILTER (WHERE resource_key='tts_characters'),
              MAX(reserved_value) FILTER (WHERE resource_key='asr_tasks'),
              MAX(reserved_value) FILTER (WHERE resource_key='concurrency')
            FROM production_resource_reservations
            WHERE run_id=$1 AND status='reserved'
            "#,
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (
            1,
            10,
            first.run.resource_usage["tts_characters"].as_i64().unwrap(),
            0,
            1
        ),
        "作品确认创建运行前必须原子预占全部编排层媒体资源"
    );
    let generation_step_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_generation_steps WHERE run_id=$1")
            .bind(first.run.id)
            .fetch_one(&pool)
            .await
            .unwrap();

    let external = orchestrator
        .resume_work_plan_confirmation(run.id, plan_reference)
        .await
        .unwrap();
    assert_eq!(external.run_id, first.run.id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_domain_links WHERE run_id=$1 AND link_type='work_generation_run' AND work_generation_run_id=$2",
        )
        .bind(run.id)
        .bind(first.run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_as::<_, (String, Option<String>, i32)>(
            "SELECT status,waiting_reason,attempt FROM production_steps WHERE run_id=$1 AND step_key='wait_work_generation' AND revision_epoch=0",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("external_wait".into(), Some("work_generation".into()), 1)
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM production_runs WHERE id=$1")
            .bind(run.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "external_wait"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_generation_steps WHERE run_id=$1",)
            .bind(first.run.id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        generation_step_count
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, i64)>(
            "SELECT (SELECT COUNT(*) FROM work_generation_attempts),(SELECT COUNT(*) FROM asset_generation_tasks)",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (0, 0)
    );

    let assert_wait_state =
        |expected_step_status: &'static str,
         expected_run_status: &'static str,
         expected_error_code: Option<&'static str>| {
            let pool = pool.clone();
            async move {
                assert_eq!(
                sqlx::query_as::<_, (String, Option<String>)>(
                    "SELECT status,error_code FROM production_steps WHERE run_id=$1 AND step_key='wait_work_generation' AND revision_epoch=0",
                )
                .bind(run.id)
                .fetch_one(&pool)
                .await
                .unwrap(),
                (expected_step_status.into(), expected_error_code.map(str::to_string))
            );
                assert_eq!(
                    sqlx::query_as::<_, (String, Option<String>)>(
                        "SELECT status,error_code FROM production_runs WHERE id=$1",
                    )
                    .bind(run.id)
                    .fetch_one(&pool)
                    .await
                    .unwrap(),
                    (
                        expected_run_status.into(),
                        expected_error_code.map(str::to_string)
                    )
                );
                assert_eq!(
                sqlx::query_scalar::<_, String>(
                    "SELECT status FROM production_steps WHERE run_id=$1 AND step_key='editor' AND revision_epoch=0",
                )
                .bind(run.id)
                .fetch_one(&pool)
                .await
                .unwrap(),
                "blocked"
            );
                assert_eq!(
                    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_generation_attempts")
                        .fetch_one(&pool)
                        .await
                        .unwrap(),
                    0,
                    "观察外部状态不得自动创建 retry attempt"
                );
            }
        };

    sqlx::query(
        "UPDATE work_generation_runs SET status='failed',error_category='provider',error_code='provider_failed',error_summary='脱敏失败摘要' WHERE id=$1",
    )
    .bind(first.run.id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        orchestrator
            .resume_work_generation(run.id, first.run.id)
            .await
            .unwrap(),
        WorkGenerationRunDisposition::FailedBlocker
    );
    assert_wait_state("blocked", "blocked", Some("work_generation_failed")).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_resource_reservations WHERE run_id=$1 AND resource_key IN ('video_tasks','video_duration_sec','tts_characters','asr_tasks','concurrency') AND status='released'",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        5,
        "作品运行未创建任何 attempt 即失败时必须释放全部预占"
    );

    sqlx::query(
        "UPDATE work_generation_runs SET status='waiting_manual',error_category='provider',error_code='unknown_submission',error_summary='提交结果不确定' WHERE id=$1",
    )
    .bind(first.run.id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        orchestrator
            .resume_work_generation(run.id, first.run.id)
            .await
            .unwrap(),
        WorkGenerationRunDisposition::AttentionRequired
    );
    assert_wait_state(
        "attention_required",
        "attention_required",
        Some("unknown_submission"),
    )
    .await;

    sqlx::query(
        "UPDATE work_generation_runs SET status='cancelling',error_category=NULL,error_code=NULL,error_summary=NULL WHERE id=$1",
    )
    .bind(first.run.id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(
        orchestrator
            .resume_work_generation(run.id, first.run.id)
            .await
            .unwrap(),
        WorkGenerationRunDisposition::Cancelling
    );
    assert_wait_state(
        "cancelling",
        "cancelling",
        Some("work_generation_cancelling"),
    )
    .await;

    sqlx::query("UPDATE work_generation_runs SET status='cancelled' WHERE id=$1")
        .bind(first.run.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        orchestrator
            .resume_work_generation(run.id, first.run.id)
            .await
            .unwrap(),
        WorkGenerationRunDisposition::ExternalCancelConflict
    );
    assert_wait_state("blocked", "blocked", Some("external_cancel_conflict")).await;

    sqlx::query("UPDATE work_generation_runs SET status='succeeded' WHERE id=$1")
        .bind(first.run.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        orchestrator
            .resume_work_generation(run.id, first.run.id)
            .await
            .unwrap(),
        WorkGenerationRunDisposition::EvidenceBlocker
    );
    assert_wait_state("external_wait", "external_wait", Some("evidence_blocker")).await;

    let video_step_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM work_generation_steps WHERE run_id=$1 AND step_type='video_segment' ORDER BY step_no LIMIT 1",
    )
    .bind(first.run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE work_generation_steps SET status='failed' WHERE id=$1")
        .bind(video_step_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE work_generation_attempts SET status='failed' WHERE step_id=$1 AND status='queued'",
    )
    .bind(video_step_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO work_generation_attempts (step_id,attempt_no,status) VALUES ($1,1,'failed')",
    )
    .bind(video_step_id)
    .execute(&pool)
    .await
    .unwrap();
    let provider_retry = work_service
        .retry_step(video_step_id, "full-crew-provider-retry".into())
        .await
        .unwrap();
    assert_eq!(
        work_service
            .retry_step(video_step_id, "full-crew-provider-retry".into())
            .await
            .unwrap()
            .id,
        provider_retry.id
    );

    // 将非必需 ASR 节点置为失败只用于零费用资源合同；retry 仍通过正式 WorkGeneration API。
    let asr_step_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM work_generation_steps WHERE run_id=$1 AND step_type='asr' LIMIT 1",
    )
    .bind(first.run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE work_generation_steps SET status='failed',resource_usage='{\"asr_tasks\":1,\"asr_seconds\":10}'::jsonb WHERE id=$1",
    )
    .bind(asr_step_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO work_generation_attempts (step_id,attempt_no,status,resource_usage) VALUES ($1,1,'failed','{\"asr_tasks\":1,\"asr_seconds\":10}'::jsonb)",
    )
    .bind(asr_step_id)
    .execute(&pool)
    .await
    .unwrap();
    let asr_retry = work_service
        .retry_step(asr_step_id, "full-crew-asr-retry".into())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT reserved_value FROM production_resource_reservations WHERE run_id=$1 AND resource_key='asr_tasks' ORDER BY created_at DESC LIMIT 1",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1,
        "ASR retry must reserve one ASR task"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT reserved_value FROM production_resource_reservations WHERE run_id=$1 AND resource_key='provider_retries' ORDER BY created_at DESC LIMIT 1",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1,
        "ASR retry must reserve one provider retry"
    );
    assert!(asr_retry.attempt_no > 1);
    sqlx::query("UPDATE work_generation_attempts SET status='failed' WHERE id=$1")
        .bind(provider_retry.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE work_generation_steps SET status='failed' WHERE id=$1")
        .bind(video_step_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE work_generation_runs SET status='failed' WHERE id=$1")
        .bind(first.run.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        orchestrator
            .resume_work_generation(run.id, first.run.id)
            .await
            .unwrap(),
        WorkGenerationRunDisposition::FailedBlocker
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, String)>(
            "SELECT actual_value,status FROM production_resource_reservations WHERE run_id=$1 AND resource_key='provider_retries' ORDER BY actual_value DESC LIMIT 1",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (1, "settled".into())
    );
    sqlx::query("UPDATE work_generation_steps SET status='failed' WHERE id=$1")
        .bind(video_step_id)
        .execute(&pool)
        .await
        .unwrap();
    let second_video_retry = work_service
        .retry_step(video_step_id, "full-crew-provider-retry-second".into())
        .await
        .unwrap();
    assert!(second_video_retry.attempt_no > provider_retry.attempt_no);
    sqlx::query("UPDATE work_generation_steps SET status='failed' WHERE id=$1")
        .bind(video_step_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE work_generation_attempts SET status='failed' WHERE step_id=$1 AND status='queued'",
    )
    .bind(video_step_id)
    .execute(&pool)
    .await
    .unwrap();
    let retry_error = work_service
        .retry_step(video_step_id, "full-crew-provider-retry-over-limit".into())
        .await
        .unwrap_err();
    assert!(
        retry_error.to_string().contains("provider_retries")
            || retry_error.to_string().contains("资源限制"),
        "third provider retry must be blocked by the fixed resource limit: {retry_error}"
    );
}

#[tokio::test]
async fn controlled_media_provider_persists_only_immutable_redacted_snapshots() {
    let (_admin, pool, _database) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = repo
        .create_intent(create_command(
            project_id,
            topic_id,
            "media-evidence-intent",
        ))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "media-evidence-run".into(),
        })
        .await
        .unwrap();
    let (script_id, scene_ids) =
        seed_production_package_scope(&pool, run.id, intent.id, project_id, topic_id).await;
    let scene_shots = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT domain_scene_id,id FROM shot_contracts WHERE run_id=$1 ORDER BY domain_scene_id,id",
    )
    .bind(run.id)
    .fetch_all(&pool)
    .await
    .unwrap();

    let work_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO works (project_id,script_id,title,status) VALUES ($1,$2,'媒体证据作品','running') RETURNING id",
    )
    .bind(project_id)
    .bind(script_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let work_version_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_versions (
            work_id,version_no,source_manifest_version,input_snapshot,model_snapshot,
            parameter_snapshot,timeline_snapshot,prompt_snapshot,status
        ) VALUES ($1,1,'media-evidence-v1',$2,'{}','{}','{}','{}','running') RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(json!({
        "scenes": scene_ids.iter().map(|scene_id| json!({
            "id": scene_id,
            "visual_description": "原始正式画面",
            "narration": "原始正式旁白",
        })).collect::<Vec<_>>()
    }))
    .fetch_one(&pool)
    .await
    .unwrap();
    let work_plan_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_plans (
            work_id,work_version_id,plan_version,status,input_fingerprint,
            capability_snapshot,output_snapshot,prompt_snapshot,timeline_snapshot
        ) VALUES ($1,$2,1,'confirmed',$3,'{}','{}','{}','{}') RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(work_version_id)
    .bind("4".repeat(64))
    .fetch_one(&pool)
    .await
    .unwrap();
    let generation_run_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_generation_runs (
            work_id,work_version_id,work_plan_id,idempotency_key,status,
            model_snapshot,capability_snapshot,prompt_snapshot,timeline_snapshot,parameter_snapshot
        ) VALUES ($1,$2,$3,'media-evidence-generation','queued','{}','{}','{}','{}','{}') RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(work_version_id)
    .bind(work_plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let generation_step_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO work_generation_steps (run_id,step_no,step_type,status) VALUES ($1,1,'video_segment','succeeded') RETURNING id",
    )
    .bind(generation_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let generation_attempt_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO work_generation_attempts (step_id,attempt_no,status,model_snapshot,resource_usage) VALUES ($1,1,'succeeded','{}','{}') RETURNING id",
    )
    .bind(generation_step_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let output_artifact_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_artifacts (
            work_version_id,role,generation_step_id,file_name,storage_path,mime_type,
            size_bytes,sha256,metadata
        ) VALUES ($1,'reusable_intermediate',$2,'segment-1.mp4','works/segment-1.mp4',
                  'video/mp4',100,$3,'{"duration_ms":17000}') RETURNING id
        "#,
    )
    .bind(work_version_id)
    .bind(generation_step_id)
    .bind("5".repeat(64))
    .fetch_one(&pool)
    .await
    .unwrap();
    let compose_step_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO work_generation_steps (run_id,step_no,step_type,status,depends_on) VALUES ($1,2,'compose','succeeded',$2) RETURNING id",
    )
    .bind(generation_run_id)
    .bind(json!([generation_step_id]))
    .fetch_one(&pool)
    .await
    .unwrap();
    let final_artifact_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_artifacts (
            work_version_id,role,generation_step_id,file_name,storage_path,mime_type,
            size_bytes,sha256,metadata
        ) VALUES ($1,'final_video',$2,'final.mp4','works/final.mp4','video/mp4',200,$3,
                  '{"duration_ms":17000,"analysis_source":"fixture"}') RETURNING id
        "#,
    )
    .bind(work_version_id)
    .bind(compose_step_id)
    .bind("6".repeat(64))
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE work_generation_runs SET status='succeeded' WHERE id=$1")
        .bind(generation_run_id)
        .execute(&pool)
        .await
        .unwrap();
    let source_step_id = sqlx::query_scalar::<_, Uuid>(
        "UPDATE production_steps SET status='succeeded',attempt=1 WHERE run_id=$1 AND revision_epoch=0 AND step_key='wait_work_generation' RETURNING id",
    )
    .bind(run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let limit_generation_run_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_generation_runs (
            work_id,work_version_id,work_plan_id,idempotency_key,status,
            model_snapshot,capability_snapshot,prompt_snapshot,timeline_snapshot,parameter_snapshot
        ) VALUES ($1,$2,$3,'quality-rework-limit-generation','succeeded','{}','{}','{}','{}','{}')
        RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(work_version_id)
    .bind(work_plan_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let limit_generation_step_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO work_generation_steps (run_id,step_no,step_type,status) VALUES ($1,1,'video_segment','succeeded') RETURNING id",
    )
    .bind(limit_generation_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let limit_generation_attempt_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO work_generation_attempts (step_id,attempt_no,status,model_snapshot,resource_usage) VALUES ($1,1,'succeeded','{}','{}') RETURNING id",
    )
    .bind(limit_generation_step_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let limit_output_artifact_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO work_artifacts (
            work_version_id,role,generation_step_id,file_name,storage_path,mime_type,
            size_bytes,sha256,metadata
        ) VALUES ($1,'reusable_intermediate',$2,'limit-segment.mp4',
                  'works/limit-segment.mp4','video/mp4',100,$3,
                  '{"duration_ms":17000}') RETURNING id
        "#,
    )
    .bind(work_version_id)
    .bind(limit_generation_step_id)
    .bind("9".repeat(64))
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO production_domain_links (
            run_id,source_step_id,revision_epoch,link_type,work_generation_run_id,
            target_version,target_digest
        ) VALUES ($1,$2,0,'work_generation_run',$3,'1',$4)
        "#,
    )
    .bind(run.id)
    .bind(source_step_id)
    .bind(generation_run_id)
    .bind("7".repeat(64))
    .execute(&pool)
    .await
    .unwrap();

    let work_version_hash = repo.work_version_hash(work_version_id).await.unwrap();
    let inventory = RequiredTakeInventorySnapshot::build(
        Uuid::new_v4(),
        run.id,
        source_step_id,
        1,
        0,
        work_id,
        work_version_id,
        generation_run_id,
        FinalMediaAsset {
            artifact_id: final_artifact_id,
            sha256: "6".repeat(64),
            mime_type: "video/mp4".into(),
            duration_ms: 17_000,
        },
        work_version_hash,
        vec![ComposeInput {
            generation_step_id,
            generation_attempt_id,
            output_artifact_id,
            segment_key: "segment-1".into(),
            scene_ids: scene_ids.clone(),
            shot_contracts: scene_ids
                .iter()
                .map(|scene_id| {
                    (
                        *scene_id,
                        scene_shots
                            .iter()
                            .filter_map(|(actual_scene_id, shot_id)| {
                                (actual_scene_id == scene_id).then_some(*shot_id)
                            })
                            .collect(),
                    )
                })
                .collect(),
            consumed_by_final_compose: true,
            generation_succeeded: true,
        }],
    )
    .unwrap();
    let provider = Arc::new(InspectingMediaEvidenceProvider {
        calls: AtomicUsize::new(0),
    });
    assert_eq!(
        repo.media_review_input(run.id, 0).await.unwrap_err().code(),
        "evidence_blocker"
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    let mut orchestrator = ProductionOrchestrator::new(
        pool.clone(),
        Arc::new(RoleRegistry::new()),
        Arc::new(GateRegistry::new()),
    );
    orchestrator.media_evidence_provider = Some(provider.clone());
    let evidence = orchestrator
        .capture_media_evidence(
            inventory.clone(),
            TemporaryMediaAccess {
                asset_id: final_artifact_id,
                access_url: "https://example.invalid/final.mp4?temporary-signature=secret".into(),
                request_headers: std::collections::BTreeMap::from([(
                    "Authorization".into(),
                    "Bearer temporary-secret".into(),
                )]),
            },
        )
        .await
        .unwrap();

    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    assert_eq!(evidence.inventory_digest, inventory.inventory_digest);
    let media_review = repo.media_review_input(run.id, 0).await.unwrap();
    assert_eq!(media_review.inventory.inventory_id, inventory.inventory_id);
    assert_eq!(
        media_review.inventory.inventory_digest,
        inventory.inventory_digest
    );
    assert_eq!(media_review.evidence.evidence_id, evidence.evidence_id);
    assert_eq!(
        media_review.evidence.evidence_digest,
        evidence.evidence_digest
    );
    let ready = WorkGenerationRunReference::build(
        generation_run_id,
        work_id,
        work_version_id,
        work_plan_id,
        WorkGenerationRunStatus::Succeeded,
        None,
        None,
        None,
        false,
        true,
        true,
    )
    .unwrap();
    assert_eq!(
        repo.sync_work_generation_state(run.id, &ready)
            .await
            .unwrap(),
        WorkGenerationRunDisposition::ReadyForMediaReview
    );
    assert_eq!(
        sqlx::query_as::<_, (String, String, String)>(
            r#"
            SELECT production.status,production.quality_status,generation.status
            FROM production_runs production
            JOIN work_generation_runs generation ON generation.id=$2
            WHERE production.id=$1
            "#,
        )
        .bind(run.id)
        .bind(generation_run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("queued".into(), "reviewing".into(), "succeeded".into())
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, i64, i64)>(
            "SELECT (SELECT COUNT(*) FROM required_take_inventories),(SELECT COUNT(*) FROM required_takes),(SELECT COUNT(*) FROM media_evidence_snapshots)",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (1, 1, 1)
    );
    let persisted = sqlx::query_scalar::<_, String>(
        "SELECT redacted_analysis::text FROM media_evidence_snapshots WHERE id=$1",
    )
    .bind(evidence.evidence_id)
    .fetch_one(&pool)
    .await
    .unwrap()
    .to_ascii_lowercase();
    for forbidden in [
        "temporary-signature",
        "temporary-secret",
        "authorization",
        "signed_url",
        "base64",
    ] {
        assert!(!persisted.contains(forbidden));
        assert!(!serde_json::to_string(&evidence)
            .unwrap()
            .to_ascii_lowercase()
            .contains(forbidden));
    }

    sqlx::query(
        "UPDATE production_runs SET status='queued',quality_status='needs_revision' WHERE id=$1",
    )
    .bind(run.id)
    .execute(&pool)
    .await
    .unwrap();
    let request = WorkVersionReworkRequest {
        production_run_id: run.id,
        revision_epoch: 0,
        work_id,
        source_work_version_id: work_version_id,
        inventory_id: inventory.inventory_id,
        inventory_digest: inventory.inventory_digest.clone(),
        evidence_snapshot_id: evidence.evidence_id,
        evidence_digest: evidence.evidence_digest.clone(),
        kind: WorkVersionReworkKind::Edit,
        rejected_take_ids: inventory.takes.iter().map(|take| take.take_id).collect(),
        affected_shot_contract_ids: scene_shots.iter().map(|(_, shot_id)| *shot_id).collect(),
        reason: "局部镜头连续性未达到质量要求".into(),
        actor: ProductionActor::local_operator(),
        idempotency_key: "quality-rework-edit".into(),
    };
    let generation_run_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_generation_runs")
            .fetch_one(&pool)
            .await
            .unwrap();
    let mut rework_orchestrator = ProductionOrchestrator::new(
        pool.clone(),
        Arc::new(RoleRegistry::new()),
        Arc::new(GateRegistry::new()),
    );
    rework_orchestrator.work_version_rework_port = Some(Arc::new(
        ProductionWorkVersionReworkService::new(PostgresWorkLibraryRepository::new(pool.clone())),
    ));
    let rework = rework_orchestrator
        .resume_quality_rework(request.clone())
        .await
        .unwrap();

    assert_eq!(rework.kind, WorkVersionReworkKind::Edit);
    assert_eq!(rework.source_work_version_id, work_version_id);
    assert!(rework.requires_confirmation);
    assert!(!rework.reused_artifact_ids.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_generation_runs")
            .fetch_one(&pool)
            .await
            .unwrap(),
        generation_run_count,
        "质量返工草稿必须等待人工确认，不得自动创建作品运行"
    );
    assert_eq!(
        sqlx::query_as::<_, (String, String, String)>(
            r#"
            SELECT version.status,plan.status,diff.status
            FROM work_versions version
            JOIN work_plans plan ON plan.id=$2 AND plan.work_version_id=version.id
            JOIN work_version_diff_plans diff ON diff.id=$3 AND diff.draft_version_id=version.id
            WHERE version.id=$1
            "#,
        )
        .bind(rework.draft_work_version_id)
        .bind(rework.work_plan_id)
        .bind(rework.diff_plan_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        ("draft".into(), "ready".into(), "analyzed".into())
    );
    assert_eq!(
        sqlx::query_as::<_, (i32, String, String)>(
            "SELECT current_revision_epoch,status,quality_status FROM production_runs WHERE id=$1",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (1, "external_wait".into(), "not_started".into())
    );
    assert_eq!(
        sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT status,waiting_reason FROM production_steps WHERE run_id=$1 AND revision_epoch=1 AND step_key='work_plan_confirmation'",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (
            "external_wait".into(),
            Some("quality_rework_confirmation".into())
        )
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_domain_links WHERE run_id=$1 AND revision_epoch=1 AND link_type IN ('work_version','work_plan')",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        2
    );
    assert_eq!(
        rework_orchestrator
            .resume_quality_rework(request)
            .await
            .unwrap(),
        rework,
        "同 key 同 digest 必须在调用 Work Library 前返回原返工结果"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_revision_epochs WHERE run_id=$1 AND reason_type='quality_rework'",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1,
        "重复旧 scope 不得再次创建返工 epoch"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_commands WHERE aggregate_id=$1 AND command_type='quality_rework'",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, i64, String)>(
            "SELECT reserved_value,actual_value,status FROM production_resource_reservations WHERE run_id=$1 AND resource_key='quality_reworks'",
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (1, 1, "settled".into())
    );

    let (limit_project_id, limit_topic_id) = source(&pool).await;
    let limit_intent = repo
        .create_intent(create_command(
            limit_project_id,
            limit_topic_id,
            "quality-rework-limit-intent",
        ))
        .await
        .unwrap();
    let mut limit_resources = ResourceLimits::strict_default();
    limit_resources.max_quality_reworks = 0;
    let limit_run = repo
        .start_run(StartRunCommand {
            intent_id: limit_intent.id,
            plan: plan_with_limits(limit_resources),
            actor: ProductionActor::local_operator(),
            idempotency_key: "quality-rework-limit-run".into(),
        })
        .await
        .unwrap();
    let limit_director_step = sqlx::query_scalar::<_, Uuid>(
        "UPDATE production_steps SET status='succeeded',attempt=1 WHERE run_id=$1 AND revision_epoch=0 AND step_key='director' RETURNING id",
    )
    .bind(limit_run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut limit_scene_shots = Vec::new();
    for (index, scene_id) in scene_ids.iter().enumerate() {
        let shot_id = Uuid::new_v4();
        let content = json!({
            "shot_id": format!("limit-shot-{}", index + 1),
            "sequence": index + 1,
            "scene_id": scene_id,
            "shot_type": "medium",
            "camera_movement": "static",
            "duration_sec": 5,
            "description": "返工上限正式镜头",
            "character_ids": ["lead"],
        });
        sqlx::query(
            r#"
            INSERT INTO shot_contracts (
                id,production_project_id,shot_id,scene_id,domain_scene_id,version,status,
                content,created_by,run_id,step_id,attempt,revision_epoch,content_digest,audit_status
            ) VALUES ($1,$2,$3,$4,$4,1,'draft',$5,'director',$6,$7,1,0,$8,'complete')
            "#,
        )
        .bind(shot_id)
        .bind(limit_intent.id)
        .bind(format!("limit-shot-{}", index + 1))
        .bind(scene_id)
        .bind(&content)
        .bind(limit_run.id)
        .bind(limit_director_step)
        .bind(canonical_digest(&content).unwrap())
        .execute(&pool)
        .await
        .unwrap();
        limit_scene_shots.push((*scene_id, shot_id));
    }
    let limit_source_step = sqlx::query_scalar::<_, Uuid>(
        "UPDATE production_steps SET status='succeeded',attempt=1 WHERE run_id=$1 AND revision_epoch=0 AND step_key='wait_work_generation' RETURNING id",
    )
    .bind(limit_run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO production_domain_links (
            run_id,source_step_id,revision_epoch,link_type,work_generation_run_id,
            target_version,target_digest
        ) VALUES ($1,$2,0,'work_generation_run',$3,'1',$4)
        "#,
    )
    .bind(limit_run.id)
    .bind(limit_source_step)
    .bind(limit_generation_run_id)
    .bind("8".repeat(64))
    .execute(&pool)
    .await
    .unwrap();
    let limit_inventory = RequiredTakeInventorySnapshot::build(
        Uuid::new_v4(),
        limit_run.id,
        limit_source_step,
        1,
        0,
        work_id,
        work_version_id,
        limit_generation_run_id,
        FinalMediaAsset {
            artifact_id: final_artifact_id,
            sha256: "6".repeat(64),
            mime_type: "video/mp4".into(),
            duration_ms: 17_000,
        },
        repo.work_version_hash(work_version_id).await.unwrap(),
        vec![ComposeInput {
            generation_step_id: limit_generation_step_id,
            generation_attempt_id: limit_generation_attempt_id,
            output_artifact_id: limit_output_artifact_id,
            segment_key: "limit-segment".into(),
            scene_ids: scene_ids.clone(),
            shot_contracts: scene_ids
                .iter()
                .map(|scene_id| {
                    (
                        *scene_id,
                        limit_scene_shots
                            .iter()
                            .filter_map(|(actual_scene_id, shot_id)| {
                                (actual_scene_id == scene_id).then_some(*shot_id)
                            })
                            .collect(),
                    )
                })
                .collect(),
            consumed_by_final_compose: true,
            generation_succeeded: true,
        }],
    )
    .unwrap();
    repo.save_required_take_inventory(&limit_inventory)
        .await
        .unwrap();
    let limit_evidence = MediaEvidenceSnapshot::build(
        Uuid::new_v4(),
        limit_run.id,
        limit_source_step,
        1,
        0,
        work_version_id,
        limit_inventory.inventory_id,
        limit_inventory.inventory_digest.clone(),
        limit_inventory.final_asset.clone(),
        "vision-fixture@1".into(),
        "audio-fixture@1".into(),
        json!({
            "final_media": {"result": "reviewed"},
            "takes": [{"take_id": limit_inventory.takes[0].take_id}],
        }),
    )
    .unwrap();
    repo.save_media_evidence(&limit_evidence).await.unwrap();
    sqlx::query("UPDATE production_runs SET status='queued',quality_status='rejected' WHERE id=$1")
        .bind(limit_run.id)
        .execute(&pool)
        .await
        .unwrap();
    let rejecting_port = Arc::new(RejectingReworkPort {
        calls: AtomicUsize::new(0),
    });
    let mut limited_orchestrator = ProductionOrchestrator::new(
        pool.clone(),
        Arc::new(RoleRegistry::new()),
        Arc::new(GateRegistry::new()),
    );
    limited_orchestrator.work_version_rework_port = Some(rejecting_port.clone());
    let limit_error = limited_orchestrator
        .resume_quality_rework(WorkVersionReworkRequest {
            production_run_id: limit_run.id,
            revision_epoch: 0,
            work_id,
            source_work_version_id: work_version_id,
            inventory_id: limit_inventory.inventory_id,
            inventory_digest: limit_inventory.inventory_digest,
            evidence_snapshot_id: limit_evidence.evidence_id,
            evidence_digest: limit_evidence.evidence_digest,
            kind: WorkVersionReworkKind::FullRegeneration,
            rejected_take_ids: limit_inventory
                .takes
                .iter()
                .map(|take| take.take_id)
                .collect(),
            affected_shot_contract_ids: limit_scene_shots
                .iter()
                .map(|(_, shot_id)| *shot_id)
                .collect(),
            reason: "全局质量问题已达到固定返工上限".into(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "quality-rework-limit".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(limit_error.code(), "resource_limit");
    assert_eq!(rejecting_port.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        sqlx::query_as::<_, (String, String, Option<String>)>(
            r#"
            SELECT run.status,intent.status,run.error_code
            FROM production_runs run
            JOIN production_projects intent ON intent.id=run.production_project_id
            WHERE run.id=$1
            "#,
        )
        .bind(limit_run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (
            "attention_required".into(),
            "attention_required".into(),
            Some("quality_rework_limit_reached".into())
        )
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM production_revision_epochs WHERE run_id=$1 AND reason_type='quality_rework'",
        )
        .bind(limit_run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn all_package_reject_paths_are_append_only_owner_scoped_and_bounded() {
    let (_admin, pool, _guard) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());

    let (brief_project_id, brief_topic_id) = source(&pool).await;
    let brief_intent = repo
        .create_intent(create_command(
            brief_project_id,
            brief_topic_id,
            "brief-create",
        ))
        .await
        .unwrap();
    let brief_run = repo
        .start_run(StartRunCommand {
            intent_id: brief_intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "brief-start".into(),
        })
        .await
        .unwrap();
    for epoch in 0..=2 {
        let producer = repo
            .get_run(brief_run.id)
            .await
            .unwrap()
            .steps
            .into_iter()
            .find(|step| step.revision_epoch == epoch && step.step_key == "producer")
            .unwrap();
        sqlx::query("UPDATE production_steps SET status = 'succeeded', attempt = 1 WHERE id = $1")
            .bind(producer.id)
            .execute(&pool)
            .await
            .unwrap();
        let package = ArtifactPackageSnapshot::build(
            PackageType::Brief,
            brief_run.id,
            producer.id,
            1,
            epoch as u32,
            1,
            vec![ArtifactRef {
                run_id: brief_run.id,
                artifact_type: "creative_brief".into(),
                artifact_id: Uuid::new_v4(),
                version: epoch as u32 + 1,
                content_digest: format!("{:064x}", epoch + 10),
                source_step_id: producer.id,
                source_attempt: 1,
            }],
            json!({}),
        )
        .unwrap();
        repo.save_package(&package).await.unwrap();
        if epoch == 0 {
            assert_eq!(
                repo.decide_package(PackageDecisionCommand {
                    run_id: brief_run.id,
                    package_digest: package.package_digest.clone(),
                    decision: GateDecision::Reject,
                    reason: Some("   ".into()),
                    affected_owners: vec!["producer".into()],
                    actor: ProductionActor::local_operator(),
                    idempotency_key: "blank-reason".into(),
                })
                .await
                .unwrap_err()
                .code(),
                "transition_conflict"
            );
        }
        repo.decide_package(PackageDecisionCommand {
            run_id: brief_run.id,
            package_digest: package.package_digest,
            decision: GateDecision::Reject,
            reason: Some(format!("第 {} 次修订", epoch + 1)),
            affected_owners: vec!["producer".into()],
            actor: ProductionActor::local_operator(),
            idempotency_key: format!("brief-reject-{epoch}"),
        })
        .await
        .unwrap();
    }
    let bounded = repo.get_run(brief_run.id).await.unwrap();
    assert_eq!(bounded.run.status, "attention_required");
    assert_eq!(
        bounded.run.error_code.as_deref(),
        Some("revision_limit_reached")
    );
    assert_eq!(bounded.run.current_revision_epoch, 2);
    assert_eq!(bounded.packages.len(), 3);
    assert_eq!(bounded.gate_decisions.len(), 3);

    let (script_project_id, script_topic_id) = source(&pool).await;
    let script_intent = repo
        .create_intent(create_command(
            script_project_id,
            script_topic_id,
            "script-create",
        ))
        .await
        .unwrap();
    let script_run = repo
        .start_run(StartRunCommand {
            intent_id: script_intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "script-start".into(),
        })
        .await
        .unwrap();
    let script_steps = repo.get_run(script_run.id).await.unwrap().steps;
    let screenwriter = script_steps
        .iter()
        .find(|step| step.step_key == "screenwriter")
        .unwrap()
        .clone();
    let suggestion_resolution = script_steps
        .iter()
        .find(|step| step.step_key == "character_suggestion_resolution")
        .unwrap()
        .id;
    sqlx::query(
        "UPDATE production_steps SET status = 'succeeded', attempt = 1 WHERE id IN ($1, $2)",
    )
    .bind(screenwriter.id)
    .bind(suggestion_resolution)
    .execute(&pool)
    .await
    .unwrap();
    let script_item = |artifact_type: &str, seed: i32| ArtifactRef {
        run_id: script_run.id,
        artifact_type: artifact_type.into(),
        artifact_id: Uuid::new_v4(),
        version: 1,
        content_digest: format!("{:064x}", seed),
        source_step_id: screenwriter.id,
        source_attempt: 1,
    };
    let script_package = ArtifactPackageSnapshot::build(
        PackageType::Script,
        script_run.id,
        screenwriter.id,
        1,
        0,
        1,
        vec![
            script_item("story_bible", 21),
            script_item("character_bible", 22),
            script_item("script_draft", 23),
        ],
        json!({}),
    )
    .unwrap();
    repo.save_package(&script_package).await.unwrap();
    repo.decide_package(PackageDecisionCommand {
        run_id: script_run.id,
        package_digest: script_package.package_digest,
        decision: GateDecision::Reject,
        reason: Some("叙事结构需要重写".into()),
        affected_owners: vec!["screenwriter".into()],
        actor: ProductionActor::local_operator(),
        idempotency_key: "script-reject".into(),
    })
    .await
    .unwrap();
    let script_after = repo.get_run(script_run.id).await.unwrap();
    assert!(script_after.steps.iter().any(|step| {
        step.revision_epoch == 1 && step.step_key == "screenwriter" && step.status == "queued"
    }));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scripts WHERE topic_id = $1")
            .bind(script_topic_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );

    let (production_project_id, production_topic_id) = source(&pool).await;
    let production_intent = repo
        .create_intent(create_command(
            production_project_id,
            production_topic_id,
            "production-create",
        ))
        .await
        .unwrap();
    let production_run = repo
        .start_run(StartRunCommand {
            intent_id: production_intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "production-start".into(),
        })
        .await
        .unwrap();
    let production_steps = repo.get_run(production_run.id).await.unwrap().steps;
    let role_step = |key: &str| {
        production_steps
            .iter()
            .find(|step| step.step_key == key)
            .unwrap()
            .id
    };
    let director = role_step("director");
    let performance = role_step("performance_director");
    let sound = role_step("sound_director");
    sqlx::query(
        "UPDATE production_steps SET status = 'succeeded', attempt = 1 WHERE id IN ($1, $2, $3)",
    )
    .bind(director)
    .bind(performance)
    .bind(sound)
    .execute(&pool)
    .await
    .unwrap();
    let production_item = |artifact_type: &str, seed: i32, source_step_id: Uuid| ArtifactRef {
        run_id: production_run.id,
        artifact_type: artifact_type.into(),
        artifact_id: Uuid::new_v4(),
        version: 1,
        content_digest: format!("{:064x}", seed),
        source_step_id,
        source_attempt: 1,
    };
    let production_items = vec![
        production_item("directorial_treatment", 31, director),
        production_item("shot_contract", 32, director),
        production_item("performance_brief", 33, performance),
        production_item("sound_plan", 34, sound),
    ];
    let production_metadata = production_package_metadata(&production_items, vec![]);
    let production_package = ArtifactPackageSnapshot::build(
        PackageType::Production,
        production_run.id,
        sound,
        1,
        0,
        1,
        production_items,
        production_metadata,
    )
    .unwrap();
    repo.save_package(&production_package).await.unwrap();
    repo.decide_package(PackageDecisionCommand {
        run_id: production_run.id,
        package_digest: production_package.package_digest,
        decision: GateDecision::Reject,
        reason: Some("镜头与声音需要定向修订".into()),
        affected_owners: vec!["director".into(), "sound_director".into()],
        actor: ProductionActor::local_operator(),
        idempotency_key: "production-reject".into(),
    })
    .await
    .unwrap();
    let production_after = repo.get_run(production_run.id).await.unwrap();
    assert!(production_after.steps.iter().any(|step| {
        step.revision_epoch == 1 && step.step_key == "director" && step.status == "queued"
    }));
    assert!(production_after.steps.iter().any(|step| {
        step.revision_epoch == 1 && step.step_key == "sound_director" && step.status == "queued"
    }));
    assert!(production_after.steps.iter().any(|step| {
        step.revision_epoch == 1
            && step.step_key == "performance_director"
            && step.status == "blocked"
    }));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM content_topics WHERE id = $1")
            .bind(production_topic_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "approved"
    );
}

#[tokio::test]
async fn required_take_inventory_builder_uses_exact_compose_chain_plan_order_and_package_shots() {
    let (_admin, pool, _database) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = repo
        .create_intent(create_command(
            project_id,
            topic_id,
            "required-take-builder-intent",
        ))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "required-take-builder-run".into(),
        })
        .await
        .unwrap();
    let (script_id, scene_ids) =
        seed_production_package_scope(&pool, run.id, intent.id, project_id, topic_id).await;
    let package = repo.build_production_package(run.id, 1).await.unwrap();
    repo.save_package(&package).await.unwrap();
    repo.decide_package(PackageDecisionCommand {
        run_id: run.id,
        package_digest: package.package_digest.clone(),
        decision: GateDecision::Approve,
        reason: None,
        affected_owners: vec![],
        actor: ProductionActor::local_operator(),
        idempotency_key: "approve-required-take-builder-package".into(),
    })
    .await
    .unwrap();
    let package_metadata: ProductionPackageMetadata =
        serde_json::from_value(package.metadata.clone()).unwrap();

    let segments = json!([
        {"sequence": 1, "scene_ids": [scene_ids[0]], "duration_seconds": 8},
        {"sequence": 2, "scene_ids": [scene_ids[1]], "duration_seconds": 9}
    ]);
    let work_id: Uuid = sqlx::query_scalar(
        "INSERT INTO works (project_id,script_id,title,status) VALUES ($1,$2,'确定性清单作品','running') RETURNING id",
    )
    .bind(project_id)
    .bind(script_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let work_version_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO work_versions (
            work_id,version_no,source_manifest_version,input_snapshot,model_snapshot,
            parameter_snapshot,timeline_snapshot,prompt_snapshot,status
        ) VALUES ($1,1,'required-take-builder',$2,'{}','{}','{}','{}','running')
        RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(json!({
        "production_crew": {
            "production_run_id": run.id,
            "revision_epoch": 0,
            "production_package": {
                "id": package.id,
                "version": package.package_version,
                "digest": package.package_digest,
                "source_step_id": package.source_step_id,
                "source_attempt": package.source_attempt
            },
            "script": {"script_id": script_id}
        },
        "segments": segments
    }))
    .fetch_one(&pool)
    .await
    .unwrap();
    let work_plan_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO work_plans (
            work_id,work_version_id,plan_version,status,input_fingerprint,
            capability_snapshot,output_snapshot,prompt_snapshot,timeline_snapshot
        ) VALUES ($1,$2,1,'confirmed',$3,'{}','{}',$4,'{}') RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(work_version_id)
    .bind("a".repeat(64))
    .bind(json!({"segments": segments}))
    .fetch_one(&pool)
    .await
    .unwrap();
    let generation_run_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO work_generation_runs (
            work_id,work_version_id,work_plan_id,idempotency_key,status,
            model_snapshot,capability_snapshot,prompt_snapshot,timeline_snapshot,parameter_snapshot
        ) VALUES ($1,$2,$3,'required-take-builder-generation','succeeded','{}','{}',$4,'{}','{}')
        RETURNING id
        "#,
    )
    .bind(work_id)
    .bind(work_version_id)
    .bind(work_plan_id)
    .bind(json!({"segments": segments}))
    .fetch_one(&pool)
    .await
    .unwrap();

    let mut generation_steps = Vec::new();
    for (index, segment) in segments.as_array().unwrap().iter().enumerate() {
        let step_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO work_generation_steps (
                run_id,step_no,step_type,status,input_snapshot
            ) VALUES ($1,$2,'video_segment','succeeded',$3) RETURNING id
            "#,
        )
        .bind(generation_run_id)
        .bind(index as i32 + 1)
        .bind(segment)
        .fetch_one(&pool)
        .await
        .unwrap();
        let attempt_id: Uuid = sqlx::query_scalar(
            "INSERT INTO work_generation_attempts (step_id,attempt_no,status,model_snapshot,resource_usage) VALUES ($1,1,'succeeded','{}','{}') RETURNING id",
        )
        .bind(step_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let artifact_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO work_artifacts (
                work_version_id,role,generation_step_id,file_name,storage_path,mime_type,
                size_bytes,sha256,metadata
            ) VALUES ($1,'reusable_intermediate',$2,$3,$4,'video/mp4',100,$5,$6)
            RETURNING id
            "#,
        )
        .bind(work_version_id)
        .bind(step_id)
        .bind(format!("segment-{}.mp4", index + 1))
        .bind(format!("works/segment-{}.mp4", index + 1))
        .bind(format!("{:064x}", index + 1))
        .bind(json!({
            "duration_ms": (index as u64 + 8) * 1000,
            "generation_attempt_id": attempt_id
        }))
        .fetch_one(&pool)
        .await
        .unwrap();
        generation_steps.push((step_id, attempt_id, artifact_id));
    }

    // 未被 mix 消费的失败输出不得进入 required take inventory。
    let unused_step_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_steps (run_id,step_no,step_type,status,is_required,input_snapshot) VALUES ($1,3,'video_segment','failed',FALSE,$2) RETURNING id",
    )
    .bind(generation_run_id)
    .bind(json!({"sequence": 99, "scene_ids": [scene_ids[0]]}))
    .fetch_one(&pool)
    .await
    .unwrap();
    let unused_attempt_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_attempts (step_id,attempt_no,status,model_snapshot,resource_usage) VALUES ($1,1,'failed','{}','{}') RETURNING id",
    )
    .bind(unused_step_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO work_artifacts (
            work_version_id,role,generation_step_id,file_name,storage_path,mime_type,
            size_bytes,sha256,metadata
        ) VALUES ($1,'reusable_intermediate',$2,'unused.mp4','works/unused.mp4','video/mp4',
                  100,$3,$4)
        "#,
    )
    .bind(work_version_id)
    .bind(unused_step_id)
    .bind("f".repeat(64))
    .bind(json!({"duration_ms": 8000, "attempt_id": unused_attempt_id}))
    .execute(&pool)
    .await
    .unwrap();

    let mix_step_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_steps (run_id,step_no,step_type,status,depends_on) VALUES ($1,4,'mix','succeeded',$2) RETURNING id",
    )
    .bind(generation_run_id)
    .bind(json!(generation_steps.iter().map(|item| item.0).collect::<Vec<_>>()))
    .fetch_one(&pool)
    .await
    .unwrap();
    let compose_step_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_steps (run_id,step_no,step_type,status,depends_on) VALUES ($1,5,'compose','succeeded',$2) RETURNING id",
    )
    .bind(generation_run_id)
    .bind(json!([mix_step_id]))
    .fetch_one(&pool)
    .await
    .unwrap();
    let compose_attempt_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_attempts (step_id,attempt_no,status,model_snapshot,resource_usage) VALUES ($1,1,'succeeded','{}','{}') RETURNING id",
    )
    .bind(compose_step_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let final_artifact_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO work_artifacts (
            work_version_id,role,generation_step_id,file_name,storage_path,mime_type,
            size_bytes,sha256,metadata
        ) VALUES ($1,'final_video',$2,'final.mp4','works/final.mp4','video/mp4',200,$3,$4)
        RETURNING id
        "#,
    )
    .bind(work_version_id)
    .bind(compose_step_id)
    .bind("e".repeat(64))
    .bind(json!({
        "duration_ms": 17000,
        "generation_attempt_id": compose_attempt_id
    }))
    .fetch_one(&pool)
    .await
    .unwrap();
    let source_step_id: Uuid = sqlx::query_scalar(
        r#"
        UPDATE production_steps
        SET status='external_wait',waiting_reason='work_generation_evidence',attempt=1
        WHERE run_id=$1 AND revision_epoch=0 AND step_key='wait_work_generation'
        RETURNING id
        "#,
    )
    .bind(run.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO production_domain_links (
            run_id,source_step_id,revision_epoch,link_type,work_generation_run_id,
            target_version,target_digest
        ) VALUES ($1,$2,0,'work_generation_run',$3,'1',$4)
        "#,
    )
    .bind(run.id)
    .bind(source_step_id)
    .bind(generation_run_id)
    .bind("d".repeat(64))
    .execute(&pool)
    .await
    .unwrap();

    let first = repo.build_required_take_inventory(run.id).await.unwrap();
    let replay = repo.build_required_take_inventory(run.id).await.unwrap();
    assert_eq!(first.inventory_id, replay.inventory_id);
    assert_eq!(first.inventory_digest, replay.inventory_digest);
    assert_eq!(first.final_asset.artifact_id, final_artifact_id);
    assert_eq!(first.takes.len(), 2);
    assert_eq!(
        first
            .takes
            .iter()
            .map(|take| take.scene_ids.clone())
            .collect::<Vec<_>>(),
        vec![vec![scene_ids[0]], vec![scene_ids[1]]]
    );
    for take in &first.takes {
        let expected = package_metadata
            .shots
            .iter()
            .filter_map(|shot| {
                take.scene_ids
                    .contains(&shot.scene_id)
                    .then_some(shot.artifact_id)
            })
            .collect::<Vec<_>>();
        assert_eq!(take.scene_shot_map[&take.scene_ids[0]], expected);
    }
    assert!(!first
        .takes
        .iter()
        .any(|take| take.generation_step_id == unused_step_id));
    let orchestrator = ProductionOrchestrator::new(
        pool.clone(),
        Arc::new(RoleRegistry::new()),
        Arc::new(GateRegistry::new()),
    );
    let persisted = orchestrator
        .build_required_take_inventory(run.id)
        .await
        .unwrap();
    assert_eq!(persisted.inventory_id, first.inventory_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM required_takes WHERE inventory_id=$1",)
            .bind(first.inventory_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );

    sqlx::query("UPDATE work_generation_steps SET depends_on=$2 WHERE id=$1")
        .bind(mix_step_id)
        .bind(json!([generation_steps[0].0]))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        repo.build_required_take_inventory(run.id)
            .await
            .unwrap_err()
            .code(),
        "evidence_blocker"
    );
    sqlx::query("UPDATE work_generation_steps SET depends_on=$2 WHERE id=$1")
        .bind(mix_step_id)
        .bind(json!(generation_steps
            .iter()
            .map(|item| item.0)
            .collect::<Vec<_>>()))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE work_generation_steps SET input_snapshot=$2 WHERE id=$1")
        .bind(generation_steps[1].0)
        .bind(&segments[0])
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        repo.build_required_take_inventory(run.id)
            .await
            .unwrap_err()
            .code(),
        "evidence_blocker"
    );
    sqlx::query("UPDATE work_generation_steps SET input_snapshot=$2 WHERE id=$1")
        .bind(generation_steps[1].0)
        .bind(&segments[1])
        .execute(&pool)
        .await
        .unwrap();

    let other_script_id: Uuid = sqlx::query_scalar(
        "INSERT INTO scripts (project_id,title,hook,content,status) VALUES ($1,'跨脚本','hook','{}','approved') RETURNING id",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE works SET script_id=$2 WHERE id=$1")
        .bind(work_id)
        .bind(other_script_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        repo.build_required_take_inventory(run.id)
            .await
            .unwrap_err()
            .code(),
        "evidence_blocker"
    );
}

#[tokio::test]
async fn quality_package_isolates_work_versions_inventory_reviews_and_old_decisions() {
    let (_admin, pool, _database) = database().await;
    let repo = DurableProductionRepository::new(pool.clone());
    let (project_id, topic_id) = source(&pool).await;
    let intent = repo
        .create_intent(create_command(
            project_id,
            topic_id,
            "quality-version-isolation-intent",
        ))
        .await
        .unwrap();
    let run = repo
        .start_run(StartRunCommand {
            intent_id: intent.id,
            plan: plan(),
            actor: ProductionActor::local_operator(),
            idempotency_key: "quality-version-isolation-run".into(),
        })
        .await
        .unwrap();
    let (script_id, scene_ids) =
        seed_production_package_scope(&pool, run.id, intent.id, project_id, topic_id).await;
    let scene_shots = sqlx::query_as::<_, (Uuid, Uuid)>(
        "SELECT domain_scene_id,id FROM shot_contracts WHERE run_id=$1 ORDER BY domain_scene_id,id",
    )
    .bind(run.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let work_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO works (project_id,script_id,title,status) VALUES ($1,$2,'质量版本隔离','running') RETURNING id",
    )
    .bind(project_id)
    .bind(script_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let v1 = seed_persisted_quality_scope(
        &pool,
        &repo,
        run.id,
        intent.id,
        work_id,
        0,
        1,
        &scene_ids,
        &scene_shots,
        "quality-v1",
    )
    .await;
    let quality_v1 = repo.build_quality_package(run.id, 1).await.unwrap();
    assert_eq!(quality_v1.outcome, QualityGateOutcome::Approved);
    assert_eq!(
        quality_v1.gate_input.media_review.inventory.work_version_id,
        v1.work_version_id
    );

    // 保存并批准旧版本 QualityPackage，后续返工版本不得继承这个决策。
    sqlx::query(
        "UPDATE production_steps SET status='succeeded',attempt=GREATEST(attempt,1) WHERE run_id=$1 AND revision_epoch=0",
    )
    .bind(run.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE production_steps SET status='queued' WHERE run_id=$1 AND revision_epoch=0 AND step_key='quality_gate'",
    )
    .bind(run.id)
    .execute(&pool)
    .await
    .unwrap();
    repo.save_package(&quality_v1.package).await.unwrap();
    let old_decision = repo
        .decide_package(PackageDecisionCommand {
            run_id: run.id,
            package_digest: quality_v1.package.package_digest.clone(),
            decision: GateDecision::Approve,
            reason: None,
            affected_owners: vec![],
            actor: ProductionActor::local_operator(),
            idempotency_key: "approve-quality-v1".into(),
        })
        .await
        .unwrap();

    sqlx::query(
        r#"
        INSERT INTO production_revision_epochs (
            run_id,epoch,reason_type,reason,affected_owners,actor_type,actor_id
        ) VALUES ($1,1,'quality_rework','v1 质量返工','["editor","qc"]','local_operator','local_operator')
        "#,
    )
    .bind(run.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO production_steps (
            run_id,revision_epoch,plan_order,step_key,step_type,role_key,
            dependencies,status,attempt,side_effect_state
        )
        SELECT run_id,1,plan_order,step_key,step_type,role_key,dependencies,
               CASE WHEN step_key IN ('wait_work_generation','editor','qc')
                    THEN 'succeeded' ELSE 'blocked' END,
               CASE WHEN step_key IN ('wait_work_generation','editor','qc') THEN 1 ELSE 0 END,
               CASE WHEN step_key IN ('wait_work_generation','editor','qc')
                    THEN 'confirmed' ELSE 'none' END
        FROM production_steps
        WHERE run_id=$1 AND revision_epoch=0
          AND step_key IN ('wait_work_generation','editor','qc','quality_gate')
        "#,
    )
    .bind(run.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE production_runs SET current_revision_epoch=1,status='queued',quality_status='reviewing',completed_at=NULL WHERE id=$1",
    )
    .bind(run.id)
    .execute(&pool)
    .await
    .unwrap();
    let v2 = seed_persisted_quality_scope(
        &pool,
        &repo,
        run.id,
        intent.id,
        work_id,
        1,
        2,
        &scene_ids,
        &scene_shots,
        "quality-v2",
    )
    .await;

    // 即使当前 step attempt 下混入旧 WorkVersion/旧 inventory 行，builder 也不得读取它们。
    let stale_ledger_id = Uuid::new_v4();
    let stale_ledger_content = json!({
        "order": 1,
        "shot_contract_id": scene_shots[0].1,
        "work_version_id": v1.work_version_id,
        "inventory_id": v1.inventory.inventory_id,
        "evidence_snapshot_id": v1.evidence.evidence_id,
        "visual_facts": ["旧版本事实"],
        "continuity_flags": [],
    });
    sqlx::query(
        r#"
        INSERT INTO continuity_ledgers (
            id,production_project_id,shot_id,content,created_by,run_id,step_id,
            attempt,revision_epoch,work_version_id,inventory_id,evidence_snapshot_id,
            shot_contract_id,version,content_digest,audit_status
        ) VALUES ($1,$2,NULL,$3,'editor',$4,$5,1,1,$6,$7,$8,$9,2,$10,'complete')
        "#,
    )
    .bind(stale_ledger_id)
    .bind(intent.id)
    .bind(&stale_ledger_content)
    .bind(run.id)
    .bind(v2.editor_step_id)
    .bind(v1.work_version_id)
    .bind(v1.inventory.inventory_id)
    .bind(v1.evidence.evidence_id)
    .bind(scene_shots[0].1)
    .bind(canonical_digest(&stale_ledger_content).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let stale_review_id = Uuid::new_v4();
    let stale_review_content = json!({
        "required_take_id": v1.inventory.takes[0].take_id,
        "work_version_id": v1.work_version_id,
        "inventory_id": v1.inventory.inventory_id,
        "evidence_snapshot_id": v1.evidence.evidence_id,
        "applicable_shot_contract_ids": scene_shots.iter().map(|(_, shot)| *shot).collect::<Vec<_>>(),
        "review_status": "approved",
        "quality_assessment": {"visual": 8, "narrative": 8, "technical": 8},
        "issues": [],
        "suggestions": [],
    });
    sqlx::query(
        r#"
        INSERT INTO take_reviews (
            id,production_project_id,shot_id,take_number,status,content,created_by,
            run_id,step_id,attempt,revision_epoch,work_version_id,inventory_id,
            evidence_snapshot_id,required_take_id,version,content_digest,audit_status
        ) VALUES ($1,$2,NULL,NULL,'approved',$3,'qc',$4,$5,1,1,$6,$7,$8,$9,2,$10,'complete')
        "#,
    )
    .bind(stale_review_id)
    .bind(intent.id)
    .bind(&stale_review_content)
    .bind(run.id)
    .bind(v2.qc_step_id)
    .bind(v1.work_version_id)
    .bind(v1.inventory.inventory_id)
    .bind(v1.evidence.evidence_id)
    .bind(v1.inventory.takes[0].take_id)
    .bind(canonical_digest(&stale_review_content).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    for (ordinal, ledger_id) in v1.ledger_ids.iter().enumerate() {
        let (shot_id, version, digest) = sqlx::query_as::<_, (Uuid, i32, String)>(
            "SELECT shot_contract_id,version,content_digest FROM continuity_ledgers WHERE id=$1",
        )
        .bind(ledger_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO take_review_ledger_versions (take_review_id,ordinal,continuity_ledger_id,shot_contract_id,ledger_version,content_digest) VALUES ($1,$2,$3,$4,$5,$6)",
        )
        .bind(stale_review_id)
        .bind(ordinal as i32)
        .bind(ledger_id)
        .bind(shot_id)
        .bind(version)
        .bind(digest)
        .execute(&pool)
        .await
        .unwrap();
    }

    let quality_v2 = repo.build_quality_package(run.id, 1).await.unwrap();
    assert_eq!(quality_v2.outcome, QualityGateOutcome::Approved);
    assert_eq!(
        quality_v2.gate_input.media_review.inventory.work_version_id,
        v2.work_version_id
    );
    assert_eq!(
        quality_v2.package.metadata["inventory_digest"],
        v2.inventory.inventory_digest
    );
    assert_ne!(
        quality_v2.package.metadata["inventory_digest"],
        v1.inventory.inventory_digest
    );
    assert!(!quality_v2
        .package
        .items
        .iter()
        .any(|item| item.artifact_id == stale_ledger_id || item.artifact_id == stale_review_id));
    assert_eq!(
        sqlx::query_as::<_, (Uuid, String)>(
            "SELECT package_id,package_digest FROM production_gate_decisions WHERE id=$1",
        )
        .bind(old_decision.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (
            quality_v1.package.id,
            quality_v1.package.package_digest.clone()
        )
    );
    assert_ne!(
        old_decision.package_digest,
        quality_v2.package.package_digest
    );
    assert_eq!(
        sqlx::query_as::<_, (i64, i64, i64, i64)>(
            r#"
            SELECT COUNT(DISTINCT work_version_id),COUNT(DISTINCT inventory_id),
                   COUNT(DISTINCT evidence_snapshot_id),COUNT(DISTINCT required_take_id)
            FROM take_reviews WHERE run_id=$1 AND audit_status='complete'
            "#,
        )
        .bind(run.id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        (2, 2, 2, 2)
    );

    // 同一当前 required take 出现第二条当前 review 时必须拒绝，不能任选一条。
    let duplicate_review_id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO take_reviews (
            id,production_project_id,shot_id,take_number,status,content,created_by,
            run_id,step_id,attempt,revision_epoch,work_version_id,inventory_id,
            evidence_snapshot_id,required_take_id,version,content_digest,audit_status
        )
        SELECT $1,production_project_id,NULL,NULL,status,content,created_by,run_id,step_id,
               attempt,revision_epoch,work_version_id,inventory_id,evidence_snapshot_id,
               required_take_id,2,content_digest,'complete'
        FROM take_reviews WHERE id=$2
        "#,
    )
    .bind(duplicate_review_id)
    .bind(v2.review_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO take_review_ledger_versions (
            take_review_id,ordinal,continuity_ledger_id,shot_contract_id,
            ledger_version,content_digest
        )
        SELECT $1,ordinal,continuity_ledger_id,shot_contract_id,ledger_version,content_digest
        FROM take_review_ledger_versions WHERE take_review_id=$2
        "#,
    )
    .bind(duplicate_review_id)
    .bind(v2.review_id)
    .execute(&pool)
    .await
    .unwrap();
    for mutation in [
        "UPDATE take_review_ledger_versions SET content_digest = content_digest WHERE take_review_id = $1",
        "DELETE FROM take_review_ledger_versions WHERE take_review_id = $1",
    ] {
        let error = sqlx::query(mutation)
            .bind(duplicate_review_id)
            .execute(&pool)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("append-only"));
    }
    let error = repo.build_quality_package(run.id, 1).await.unwrap_err();
    assert_eq!(error.code(), "evidence_blocker");
    assert!(error.to_string().contains("take_review_current_ambiguous"));
}
