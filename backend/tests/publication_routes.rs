use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use novex_api::{
    bootstrap::{AppConfig, AppState},
    build_app_with_state,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{io::Read, path::PathBuf};
use tower::ServiceExt;
use uuid::Uuid;

mod support;
use support::test_database::TestDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@biga-postgres:5432/video_agent".into())
}
fn with_database_name(url: &str, name: &str) -> String {
    let query_start = url.find('?');
    let (base, query) = query_start
        .map(|i| (&url[..i], &url[i..]))
        .unwrap_or((url, ""));
    let slash = base.rfind('/').unwrap();
    format!("{}{}{}", &base[..=slash], name, query)
}
async fn migrated_pool() -> (PgPool, PgPool, TestDatabase, String) {
    let base = database_url();
    let name = format!("publication_routes_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base, "postgres");
    let test_url = with_database_name(&base, &name);
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{name}""#))
        .execute(&admin)
        .await
        .unwrap();
    let database = TestDatabase::new(&admin_url, &name);
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin, pool, database, test_url)
}
fn app_state(url: String, pool: PgPool, root: PathBuf) -> AppState {
    AppState::new(
        AppConfig {
            environment: "test".into(),
            database_url: url,
            redis_url: "redis://127.0.0.1:6379/15".into(),
            openai_api_key: String::new(),
            openai_base_url: "https://example.invalid/v1".into(),
            openai_model: "unused".into(),
            openai_timeout_seconds: 5,
            openai_reasoning_effort: None,
            openai_max_output_tokens: 3000,
            asset_storage_root: root.to_string_lossy().into_owned(),
            asset_generation_providers: vec![],
        },
        pool,
        None,
    )
    .unwrap()
}
async fn send(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Value,
    key: Option<&str>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(key) = key {
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

async fn seed_handoff(pool: &PgPool, root: &std::path::Path) -> Uuid {
    let project: Uuid =
        sqlx::query_scalar("INSERT INTO projects(name)VALUES('发布路由')RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let script:Uuid=sqlx::query_scalar("INSERT INTO scripts(project_id,title,hook,content)VALUES($1,'脚本','hook','{}')RETURNING id").bind(project).fetch_one(pool).await.unwrap();
    let work:Uuid=sqlx::query_scalar("INSERT INTO works(project_id,script_id,title,status)VALUES($1,$2,'发布作品','succeeded')RETURNING id").bind(project).bind(script).fetch_one(pool).await.unwrap();
    let version:Uuid=sqlx::query_scalar("INSERT INTO work_versions(work_id,version_no,status,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot)VALUES($1,1,'completed','v1','{}','{}','{}','{}','{}')RETURNING id").bind(work).fetch_one(pool).await.unwrap();
    std::fs::create_dir_all(root.join("works")).unwrap();
    let bytes = b"video";
    std::fs::write(root.join("works/final.mp4"), bytes).unwrap();
    let artifact:Uuid=sqlx::query_scalar("INSERT INTO work_artifacts(work_version_id,role,file_name,storage_path,mime_type,size_bytes,sha256)VALUES($1,'final_video','final.mp4','works/final.mp4','video/mp4',$2,$3)RETURNING id").bind(version).bind(bytes.len() as i64).bind(format!("{:x}",Sha256::digest(bytes))).fetch_one(pool).await.unwrap();
    sqlx::query_scalar("INSERT INTO publication_handoffs(work_id,work_version_id,final_video_artifact_id,idempotency_key)VALUES($1,$2,$3,'route-handoff')RETURNING id").bind(work).bind(version).bind(artifact).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn manual_publication_api_is_idempotent_revision_safe_and_truthful() {
    let (admin, pool, _database, url) = migrated_pool().await;
    let root = std::env::temp_dir().join(format!("publication-routes-{}", Uuid::new_v4()));
    let handoff = seed_handoff(&pool, &root).await;
    let app = build_app_with_state(app_state(url, pool.clone(), root.clone()));
    let plan_uri = format!("/api/publication-handoffs/{handoff}/publication");
    assert_eq!(
        send(&app, "POST", &plan_uri, json!({}), None).await.0,
        StatusCode::BAD_REQUEST
    );
    let (status, plan) = send(&app, "POST", &plan_uri, json!({}), Some("plan-once")).await;
    assert_eq!(status, StatusCode::CREATED);
    let plan_id = plan["id"].as_str().unwrap();
    let repeated = send(&app, "POST", &plan_uri, json!({}), Some("plan-once")).await;
    assert_eq!(repeated.0, StatusCode::OK);
    assert_eq!(plan["id"], repeated.1["id"]);
    let target_uri = format!("/api/publications/{plan_id}/targets/douyin");
    let draft = json!({"title":"标题","body":"正文","tags":["标签"]});
    let (status, target) = send(
        &app,
        "PUT",
        &target_uri,
        draft.clone(),
        Some("target-create"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let target_id = target["id"].as_str().unwrap();
    let replay = send(&app, "PUT", &target_uri, draft, Some("target-create")).await;
    assert_eq!(replay.1["draft_revision"], 1);
    let updated = send(
        &app,
        "PUT",
        &target_uri,
        json!({"expected_revision":1,"title":"新版","body":"正文","tags":[]}),
        Some("target-update"),
    )
    .await;
    assert_eq!(updated.1["draft_revision"], 2);
    assert_eq!(
        send(
            &app,
            "PUT",
            &target_uri,
            json!({"expected_revision":1,"title":"过期","tags":[]}),
            Some("target-stale")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let package_uri = format!("/api/publication-targets/{target_id}/package");
    let package = send(
        &app,
        "POST",
        &package_uri,
        json!({"draft_revision":2}),
        Some("package-once"),
    )
    .await;
    assert_eq!(package.0, StatusCode::CREATED, "{}", package.1);
    let manifest_text = package.1["manifest"].to_string();
    assert!(!manifest_text.contains("token"));
    assert!(!manifest_text.contains("/server/") && !manifest_text.contains("?signature="));
    let archive_file =
        std::fs::File::open(root.join(package.1["package_storage_path"].as_str().unwrap()))
            .unwrap();
    let mut archive = zip::ZipArchive::new(archive_file).unwrap();
    assert!(archive.by_name("manifest.json").is_ok());
    assert!(archive.by_name("发布文案.txt").is_ok());
    assert_eq!(
        archive.by_index(0).unwrap().compression(),
        zip::CompressionMethod::Stored
    );
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        let content = String::from_utf8_lossy(&bytes).to_ascii_lowercase();
        for forbidden in [
            "authorization",
            "bearer ",
            "cookie",
            "secret",
            "token",
            "?signature=",
            "/server/",
            "/app/",
            "/root/",
        ] {
            assert!(
                !content.contains(forbidden),
                "发布包条目 {} 泄露禁用内容 {forbidden}",
                entry.name()
            );
        }
    }
    drop(archive);
    let repeated_package = send(
        &app,
        "POST",
        &package_uri,
        json!({"draft_revision":2}),
        Some("package-once"),
    )
    .await;
    assert_eq!(repeated_package.0, StatusCode::OK);
    assert_eq!(repeated_package.1["id"], package.1["id"]);
    assert_eq!(
        send(
            &app,
            "GET",
            &format!("/api/publication-targets/{target_id}/downloads"),
            json!({}),
            None,
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        send(
            &app,
            "POST",
            &format!("/api/publication-targets/{target_id}/download-audits"),
            json!({}),
            Some("download-audit")
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        send(
            &app,
            "POST",
            &format!("/api/publication-targets/{target_id}/copy-audits"),
            json!({}),
            Some("copy-audit")
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let handoff_result = send(
        &app,
        "POST",
        &format!("/api/publication-targets/{target_id}/handoff"),
        json!({}),
        Some("open-official"),
    )
    .await;
    assert_eq!(handoff_result.0, StatusCode::OK);
    assert_eq!(handoff_result.1["target"]["status"], "handed_off");
    assert_eq!(
        handoff_result.1["publication_confirmation"],
        "manual_required"
    );
    let attention = send(
        &app,
        "POST",
        &format!("/api/publication-targets/{target_id}/needs-attention"),
        json!({}),
        Some("needs-fix"),
    )
    .await;
    assert_eq!(attention.1["status"], "needs_attention");
    let revised = send(
        &app,
        "PUT",
        &target_uri,
        json!({"expected_revision":2,"title":"修正版","body":"正文","tags":[]}),
        Some("revise-after-attention"),
    )
    .await;
    assert_eq!(revised.1["draft_revision"], 3);
    assert_eq!(
        send(
            &app,
            "POST",
            &package_uri,
            json!({"draft_revision":3}),
            Some("package-three")
        )
        .await
        .0,
        StatusCode::CREATED
    );
    assert_eq!(
        send(
            &app,
            "POST",
            &format!("/api/publication-targets/{target_id}/handoff"),
            json!({}),
            Some("open-official-again")
        )
        .await
        .1["target"]["status"],
        "handed_off"
    );
    let publish_uri = format!("/api/publication-targets/{target_id}/published");
    assert_eq!(send(&app,"POST",&publish_uri,json!({"published_url":"https://evil.example/video/1","published_at":"2026-07-23T06:00:00Z"}),Some("bad-result")).await.0,StatusCode::BAD_REQUEST);
    let published=send(&app,"POST",&publish_uri,json!({"published_url":"https://www.douyin.com/video/1","published_at":"2026-07-23T06:00:00Z"}),Some("publish-result")).await;
    assert_eq!(published.1["status"], "published");
    assert_eq!(published.1["result_snapshot"]["confirmation"], "manual");
    let corrected=send(&app,"POST",&format!("/api/publication-targets/{target_id}/result-corrections"),json!({"published_url":"https://www.douyin.com/video/2","published_at":"2026-07-23T06:05:00Z"}),Some("correct-result")).await;
    assert_eq!(
        corrected.1["published_url"],
        "https://www.douyin.com/video/2"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM publish_tasks")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT COUNT(*) FROM publication_events WHERE publication_target_id=$1 AND event_type IN('published','result_corrected')").bind(Uuid::parse_str(target_id).unwrap()).fetch_one(&pool).await.unwrap(),2);
    let red_uri = format!("/api/publications/{plan_id}/targets/xiaohongshu");
    let red = send(
        &app,
        "PUT",
        &red_uri,
        json!({"title":"小红书","body":"正文","tags":[],"planned_at":"2026-07-22T06:00:00Z"}),
        Some("red-create"),
    )
    .await
    .1;
    let red_id = red["id"].as_str().unwrap();
    let details = send(
        &app,
        "GET",
        &format!("/api/publications/{plan_id}"),
        json!({}),
        None,
    )
    .await
    .1;
    let red_view = details["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|target| target["platform"] == "xiaohongshu")
        .unwrap();
    assert_eq!(red_view["overdue"], true);
    assert_eq!(red_view["status"], "draft");
    std::fs::write(root.join("works/final.mp4"), b"corrupt").unwrap();
    assert_eq!(
        send(
            &app,
            "POST",
            &format!("/api/publication-targets/{red_id}/package"),
            json!({"draft_revision":1}),
            Some("red-corrupt-package")
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    std::fs::write(root.join("works/final.mp4"), b"video").unwrap();
    assert_eq!(
        send(
            &app,
            "POST",
            &format!("/api/publication-targets/{red_id}/cancel"),
            json!({}),
            Some("red-cancel")
        )
        .await
        .1["status"],
        "cancelled"
    );
    pool.close().await;
    admin.close().await;
}
