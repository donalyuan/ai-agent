use axum::body::Body;
use axum::http::{Request, StatusCode};
use novex_api::bootstrap::{AppConfig, AppState};
use novex_api::build_app_with_state;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::path::PathBuf;
use tower::ServiceExt;
use uuid::Uuid;

mod support;

use support::test_database::TestDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
    })
}

fn with_database_name(database_url: &str, database_name: &str) -> String {
    let query_start = database_url.find('?');
    let (base, query) = query_start
        .map(|index| (&database_url[..index], &database_url[index..]))
        .unwrap_or((database_url, ""));
    let slash_index = base.rfind('/').unwrap();
    format!("{}{}{}", &base[..=slash_index], database_name, query)
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase, String) {
    let base_url = database_url();
    let database_name = format!("work_library_routes_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
    let database = TestDatabase::new(&admin_url, &database_name);
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin_pool, pool, database, test_url)
}

fn app_state(test_url: String, pool: PgPool, storage_root: PathBuf) -> AppState {
    AppState::new(
        AppConfig {
            environment: "test".into(),
            database_url: test_url,
            redis_url: "redis://127.0.0.1:6379/15".into(),
            openai_api_key: String::new(),
            openai_base_url: "https://example.invalid/v1".into(),
            openai_model: "unused".into(),
            openai_timeout_seconds: 5,
            openai_reasoning_effort: None,
            openai_max_output_tokens: 3000,
            asset_storage_root: storage_root.to_string_lossy().into_owned(),
            asset_generation_providers: vec![],
        },
        pool,
        None,
    )
    .unwrap()
}

async fn send_json(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Value,
    idempotency_key: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(key) = idempotency_key {
        builder = builder.header("idempotency-key", key);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn assert_no_amount_fields(value: &Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                let normalized = key.to_ascii_lowercase();
                assert!(
                    !["price", "currency", "cost", "amount", "budget"]
                        .iter()
                        .any(|word| normalized.contains(word)),
                    "作品库 API 不得返回金额字段: {key}"
                );
                assert_no_amount_fields(value);
            }
        }
        Value::Array(items) => items.iter().for_each(assert_no_amount_fields),
        _ => {}
    }
}

struct SeededWork {
    project_id: Uuid,
    work_id: Uuid,
    version_id: Uuid,
    final_artifact_id: Uuid,
    artifact_ids: Vec<Uuid>,
}

async fn seed_completed_work(pool: &PgPool, storage_root: &std::path::Path) -> SeededWork {
    let project_id: Uuid =
        sqlx::query_scalar("INSERT INTO projects (name) VALUES ('作品库路由项目') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let script_id: Uuid = sqlx::query_scalar("INSERT INTO scripts (project_id,title,hook,content) VALUES ($1,'作品库脚本','hook','{}') RETURNING id").bind(project_id).fetch_one(pool).await.unwrap();
    let work_id: Uuid = sqlx::query_scalar("INSERT INTO works (project_id,script_id,title,status) VALUES ($1,$2,'作品库演示','succeeded') RETURNING id").bind(project_id).bind(script_id).fetch_one(pool).await.unwrap();
    let version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_versions (work_id,version_no,status,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot,completed_at) VALUES ($1,1,'completed','manifest-v1',$2,$3,$4,$5,$6,NOW()) RETURNING id",
    )
    .bind(work_id)
    .bind(json!({"scenes":[{"id":"scene-1","visual_description":"旧画面","narration":"测试旁白"},{"id":"scene-2","visual_description":"保留画面","narration":"继续"}]}))
    .bind(json!({"video_model_id":Uuid::new_v4(),"api_key":"must-not-enter-package"}))
    .bind(json!({"aspect_ratio":"16:9","resolution":"1080p"}))
    .bind(json!({"segments":[{"sequence":1,"duration_seconds":8},{"sequence":2,"duration_seconds":7}]}))
    .bind(json!({"duration_seconds":15,"subtitle":{"text":"旧字幕","style":"default","burn":true}}))
    .fetch_one(pool).await.unwrap();
    sqlx::query("UPDATE works SET current_version_id=$2 WHERE id=$1")
        .bind(work_id)
        .bind(version_id)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO work_timelines (work_version_id,video_tracks,audio_tracks,subtitle_tracks) VALUES ($1,$2,$3,$4)")
        .bind(version_id).bind(json!([{"scene_id":"scene-1"},{"scene_id":"scene-2"}])).bind(json!([])).bind(json!([{"text":"旧字幕"}])).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO work_plans (work_id,work_version_id,plan_version,status,input_fingerprint,capability_snapshot,output_snapshot,prompt_snapshot,timeline_snapshot,resource_usage) VALUES ($1,$2,1,'confirmed',$3,'{}',$4,$5,$6,$7)")
        .bind(work_id).bind(version_id).bind("0".repeat(64)).bind(json!({"aspect_ratio":"16:9","resolution":"1080p"})).bind(json!({"segments":[{"sequence":1,"duration_seconds":8},{"sequence":2,"duration_seconds":7}]})).bind(json!({"duration_seconds":15})).bind(json!({"video_task_count":2,"video_seconds":15,"tts_characters":6,"asr_seconds":0})).execute(pool).await.unwrap();

    std::fs::create_dir_all(storage_root.join("works")).unwrap();
    let bytes = b"valid-mp4-content";
    std::fs::write(storage_root.join("works/final.mp4"), bytes).unwrap();
    let sha256 = format!("{:x}", Sha256::digest(bytes));
    let final_artifact_id: Uuid = sqlx::query_scalar("INSERT INTO work_artifacts (work_version_id,role,file_name,storage_path,mime_type,size_bytes,sha256,metadata) VALUES ($1,'final_video','final.mp4','works/final.mp4','video/mp4',$2,$3,$4) RETURNING id")
        .bind(version_id).bind(bytes.len() as i64).bind(sha256).bind(json!({"dag_node":"compose"})).fetch_one(pool).await.unwrap();
    let mut artifact_ids = vec![final_artifact_id];
    for (role, name, mime, content, node) in [
        (
            "subtitle",
            "final.srt",
            "application/x-subrip",
            b"1\n00:00:00,000 --> 00:00:01,000\nsubtitle\n".as_slice(),
            "subtitle",
        ),
        (
            "mix",
            "mix.wav",
            "audio/wav",
            b"mixed-audio".as_slice(),
            "mix",
        ),
        (
            "audio_track",
            "tts.wav",
            "audio/wav",
            b"tts-track".as_slice(),
            "tts",
        ),
    ] {
        std::fs::write(storage_root.join(format!("works/{name}")), content).unwrap();
        let digest = format!("{:x}", Sha256::digest(content));
        let id: Uuid = sqlx::query_scalar("INSERT INTO work_artifacts (work_version_id,role,file_name,storage_path,mime_type,size_bytes,sha256,metadata) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING id")
            .bind(version_id).bind(role).bind(name).bind(format!("works/{name}")).bind(mime).bind(content.len() as i64).bind(digest).bind(json!({"dag_node":node})).fetch_one(pool).await.unwrap();
        artifact_ids.push(id);
    }
    SeededWork {
        project_id,
        work_id,
        version_id,
        final_artifact_id,
        artifact_ids,
    }
}

#[tokio::test]
async fn version_derivation_diff_and_confirmation_are_immutable_stale_safe_and_idempotent() {
    let (admin_pool, pool, _database, test_url) = migrated_pool().await;
    let storage_root = std::env::temp_dir().join(format!("work-library-{}", Uuid::new_v4()));
    let seeded = seed_completed_work(&pool, &storage_root).await;
    let source_plan_id: Uuid =
        sqlx::query_scalar("SELECT id FROM work_plans WHERE work_version_id=$1")
            .bind(seeded.version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let source_run_id: Uuid = sqlx::query_scalar("INSERT INTO work_generation_runs (work_id,work_version_id,work_plan_id,idempotency_key,status,model_snapshot,capability_snapshot,prompt_snapshot,timeline_snapshot,parameter_snapshot) VALUES ($1,$2,$3,'source-running','running','{}','{}','{}','{}','{}') RETURNING id")
        .bind(seeded.work_id).bind(seeded.version_id).bind(source_plan_id).fetch_one(&pool).await.unwrap();
    let app = build_app_with_state(app_state(test_url, pool.clone(), storage_root));

    let (status, draft) = send_json(
        &app,
        "POST",
        &format!("/api/work-versions/{}/derive", seeded.version_id),
        json!({"input_snapshot_patch":{"scenes":{"0":{"visual_description":"新画面"}}}}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let draft_id = draft["id"].as_str().unwrap();
    assert_eq!(draft["source_version_id"], json!(seeded.version_id));
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT work_version_id FROM work_generation_runs WHERE id=$1"
        )
        .bind(source_run_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        seeded.version_id
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM work_versions WHERE id=$1")
            .bind(seeded.version_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "running"
    );

    let (status, diff) = send_json(
        &app,
        "POST",
        &format!("/api/work-versions/{draft_id}/diff"),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_no_amount_fields(&diff);
    assert_eq!(diff["resource_usage"]["video_task_count"], 1);
    let diff_id = diff["id"].as_str().unwrap();

    send_json(
        &app,
        "POST",
        &format!("/api/work-versions/{draft_id}/derive"),
        json!({"timeline_snapshot_patch":{"subtitle":{"text":"更新后使计划过期"}}}),
        None,
    )
    .await;
    let (status, stale) = send_json(
        &app,
        "POST",
        &format!("/api/work-version-diffs/{diff_id}/confirm"),
        json!({}),
        Some("stale-confirm"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(stale["code"], "work_diff_stale");

    let (_, fresh_diff) = send_json(
        &app,
        "POST",
        &format!("/api/work-versions/{draft_id}/diff"),
        json!({}),
        None,
    )
    .await;
    let fresh_diff_id = fresh_diff["id"].as_str().unwrap();
    let (first_status, first) = send_json(
        &app,
        "POST",
        &format!("/api/work-version-diffs/{fresh_diff_id}/confirm"),
        json!({}),
        Some("confirm-once"),
    )
    .await;
    let (second_status, second) = send_json(
        &app,
        "POST",
        &format!("/api/work-version-diffs/{fresh_diff_id}/confirm"),
        json!({}),
        Some("confirm-once"),
    )
    .await;
    assert_eq!(first_status, StatusCode::CREATED);
    assert_eq!(second_status, StatusCode::OK);
    assert_eq!(first["run_id"], second["run_id"]);

    let (status, regenerated) = send_json(
        &app,
        "POST",
        &format!("/api/work-versions/{}/regenerate", seeded.version_id),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_ne!(regenerated["id"], json!(seeded.version_id));
    assert_eq!(regenerated["source_version_id"], json!(seeded.version_id));
    assert_eq!(regenerated["derivation_kind"], "full_regeneration");
    let (repeat_status, repeated_regeneration) = send_json(
        &app,
        "POST",
        &format!("/api/work-versions/{}/regenerate", seeded.version_id),
        json!({}),
        None,
    )
    .await;
    assert_eq!(repeat_status, StatusCode::CREATED);
    assert_eq!(repeated_regeneration["id"], regenerated["id"]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_versions WHERE source_version_id=$1 AND derivation_kind='full_regeneration' AND status='draft'",
        )
        .bind(seeded.version_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    admin_pool.close().await;
}

#[tokio::test]
async fn lifecycle_download_and_publication_handoff_follow_work_library_rules() {
    let (admin_pool, pool, _database, test_url) = migrated_pool().await;
    let storage_root = std::env::temp_dir().join(format!("work-library-{}", Uuid::new_v4()));
    let seeded = seed_completed_work(&pool, &storage_root).await;
    let app = build_app_with_state(app_state(test_url, pool.clone(), storage_root.clone()));

    let (status, list) = send_json(
        &app,
        "GET",
        &format!("/api/projects/{}/works", seeded.project_id),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_no_amount_fields(&list);
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    let (status, details) = send_json(
        &app,
        "GET",
        &format!("/api/works/{}", seeded.work_id),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_no_amount_fields(&details);

    let (status, _) = send_json(
        &app,
        "DELETE",
        &format!("/api/works/{}", seeded.work_id),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        send_json(
            &app,
            "POST",
            &format!("/api/works/{}/archive", seeded.work_id),
            json!({}),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        send_json(
            &app,
            "GET",
            &format!("/api/projects/{}/works", seeded.project_id),
            json!({}),
            None
        )
        .await
        .1["items"]
            .as_array()
            .unwrap()
            .len(),
        0
    );
    assert_eq!(
        send_json(
            &app,
            "GET",
            &format!("/api/projects/{}/works?archived=true", seeded.project_id),
            json!({}),
            None
        )
        .await
        .1["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        send_json(
            &app,
            "POST",
            &format!("/api/works/{}/restore", seeded.work_id),
            json!({}),
            None
        )
        .await
        .0,
        StatusCode::OK
    );

    let (status, downloads) = send_json(
        &app,
        "GET",
        &format!("/api/work-versions/{}/downloads", seeded.version_id),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_no_amount_fields(&downloads);
    assert_eq!(downloads["artifacts"].as_array().unwrap().len(), 4);
    assert!(downloads["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["integrity_status"] == "available"));
    for artifact_id in &seeded.artifact_ids {
        assert_eq!(
            send_json(
                &app,
                "GET",
                &format!("/api/work-artifacts/{artifact_id}/download"),
                json!({}),
                None
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    let (status, package) = send_json(
        &app,
        "GET",
        &format!(
            "/api/work-versions/{}/production-package",
            seeded.version_id
        ),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(package["schema"], "novex-work-package/v1");
    assert!(package["version"]["prompt_snapshot"].is_object());
    assert!(package["timeline"].is_object());
    assert!(!package.to_string().contains("must-not-enter-package"));

    let (status, _) = send_json(
        &app,
        "GET",
        &format!("/api/work-artifacts/{}/download", seeded.final_artifact_id),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    std::fs::write(storage_root.join("works/final.mp4"), b"corrupt").unwrap();
    let (status, error) = send_json(
        &app,
        "GET",
        &format!("/api/work-artifacts/{}/download", seeded.final_artifact_id),
        json!({}),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "work_artifact_integrity_failed");

    std::fs::write(storage_root.join("works/final.mp4"), b"valid-mp4-content").unwrap();
    let (status, handoff) = send_json(
        &app,
        "POST",
        &format!(
            "/api/work-versions/{}/publication-handoffs",
            seeded.version_id
        ),
        json!({}),
        Some("publish-draft-once"),
    )
    .await;
    let (second_status, repeated) = send_json(
        &app,
        "POST",
        &format!(
            "/api/work-versions/{}/publication-handoffs",
            seeded.version_id
        ),
        json!({}),
        Some("publish-draft-once"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(second_status, StatusCode::OK);
    assert_eq!(handoff["id"], repeated["id"]);
    assert_eq!(handoff["status"], "draft");
    assert_no_amount_fields(&handoff);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM publish_tasks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );

    let blank_script_id: Uuid = sqlx::query_scalar("INSERT INTO scripts (project_id,title,hook,content) VALUES ($1,'空白作品脚本','hook','{}') RETURNING id").bind(seeded.project_id).fetch_one(&pool).await.unwrap();
    let blank_work_id: Uuid = sqlx::query_scalar("INSERT INTO works (project_id,script_id,title,status) VALUES ($1,$2,'空白作品','draft') RETURNING id").bind(seeded.project_id).bind(blank_script_id).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO work_versions (work_id,version_no,status,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot) VALUES ($1,1,'draft','blank','{}','{}','{}','{}','{}')").bind(blank_work_id).execute(&pool).await.unwrap();
    assert_eq!(
        send_json(
            &app,
            "DELETE",
            &format!("/api/works/{blank_work_id}"),
            json!({}),
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scripts WHERE id=$1")
            .bind(blank_script_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1,
        "删除空白作品不得删除原脚本"
    );
    admin_pool.close().await;
}
