use axum::body::Body;
use axum::http::{Request, StatusCode};
use novex_api::{build_app_with_state, AppConfig, AppState};
use serde_json::{json, Value};
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
    let (base, query) = match query_start {
        Some(index) => (&database_url[..index], &database_url[index..]),
        None => (database_url, ""),
    };

    let slash_index = base
        .rfind('/')
        .expect("DATABASE_URL must include database name");
    format!("{}{}{}", &base[..=slash_index], database_name, query)
}

async fn create_database(
    admin_pool: &PgPool,
    admin_url: &str,
    database_name: &str,
) -> TestDatabase {
    let query = format!(r#"CREATE DATABASE "{}""#, database_name);
    sqlx::query(&query)
        .execute(admin_pool)
        .await
        .expect("temporary asset generation route database should be created");
    TestDatabase::new(admin_url, database_name)
}

async fn drop_database(admin_pool: &PgPool, database_name: &str) {
    let disconnect = format!(
        r#"
        SELECT pg_terminate_backend(pid)
        FROM pg_stat_activity
        WHERE datname = '{}'
        "#,
        database_name
    );
    let drop = format!(r#"DROP DATABASE IF EXISTS "{}""#, database_name);

    let _ = sqlx::query(&disconnect).execute(admin_pool).await;
    let _ = sqlx::query(&drop).execute(admin_pool).await;
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase, String) {
    let base_url = database_url();
    let suffix = Uuid::new_v4().simple().to_string();
    let database_name = format!("video_agent_asset_route_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    let database_name = create_database(&admin_pool, &admin_url, &database_name).await;

    let test_pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&test_url)
        .await
        .expect("temporary asset generation route database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for asset generation route test database");

    (admin_pool, test_pool, database_name, test_url)
}

fn app_state(test_url: String, pool: PgPool) -> AppState {
    app_state_with_asset_options(
        test_url,
        pool,
        "/app/storage/assets".to_string(),
        vec!["gpt-image-2".to_string(), "jimeng".to_string()],
    )
}

fn app_state_with_asset_options(
    test_url: String,
    pool: PgPool,
    asset_storage_root: String,
    asset_generation_providers: Vec<String>,
) -> AppState {
    AppState::new(
        AppConfig {
            environment: "test".to_string(),
            database_url: test_url,
            redis_url: "redis://127.0.0.1:6379/15".to_string(),
            openai_api_key: "".to_string(),
            openai_base_url: "https://example.invalid/v1".to_string(),
            openai_model: "test-model".to_string(),
            openai_timeout_seconds: 5,
            openai_reasoning_effort: Some("low".to_string()),
            openai_max_output_tokens: 3000,
            asset_storage_root,
            asset_generation_providers,
        },
        pool,
        None,
    )
    .unwrap()
}

async fn insert_project(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('素材生成账号', '', '')
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("project fixture should be inserted")
}

async fn insert_script_with_scenes(
    pool: &PgPool,
    project_id: Uuid,
    scene_count: i32,
) -> (Uuid, Vec<Uuid>) {
    let script_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO scripts (project_id, title, hook, content, status)
        VALUES ($1, 'AI 图片素材脚本', '开场钩子', $2, 'draft')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(json!({ "topic": "AI 图片素材生成" }))
    .fetch_one(pool)
    .await
    .expect("script fixture should be inserted");

    let mut scene_ids = Vec::new();
    for sequence in 1..=scene_count {
        let scene_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO scenes (
                script_id, sequence, narration, visual_description, emotion, duration_sec
            )
            VALUES ($1, $2, '旁白', '画面指令', '平静', 8)
            RETURNING id
            "#,
        )
        .bind(script_id)
        .bind(sequence)
        .fetch_one(pool)
        .await
        .expect("scene fixture should be inserted");
        scene_ids.push(scene_id);
    }

    (script_id, scene_ids)
}

async fn insert_material(pool: &PgPool, project_id: Uuid, file_name: &str) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO materials (project_id, material_type, file_url, file_name, status)
        VALUES ($1, 'image', $2, $3, 'active')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(format!("https://cdn.example.com/{file_name}"))
    .bind(file_name)
    .fetch_one(pool)
    .await
    .expect("material fixture should be inserted")
}

async fn insert_candidate(
    pool: &PgPool,
    project_id: Uuid,
    script_id: Uuid,
    scene_id: Uuid,
    material_id: Uuid,
    rank: i32,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO scene_asset_candidates (
            project_id, script_id, scene_id, material_id, candidate_type, source, status, rank
        )
        VALUES ($1, $2, $3, $4, 'image', 'existing_material', 'candidate', $5)
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(script_id)
    .bind(scene_id)
    .bind(material_id)
    .bind(rank)
    .fetch_one(pool)
    .await
    .expect("candidate fixture should be inserted")
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

fn scene_generation_request(scene_id: Uuid, idempotency_key: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(format!("/api/scenes/{scene_id}/asset-generation-tasks"))
        .header("content-type", "application/json");
    if let Some(idempotency_key) = idempotency_key {
        builder = builder.header("idempotency-key", idempotency_key);
    }
    builder
        .body(Body::from(
            json!({
                "provider": "gpt-image-2",
                "image_candidates_per_scene": 2,
                "use_reference_materials": false
            })
            .to_string(),
        ))
        .unwrap()
}

#[tokio::test]
async fn assets_route_serves_generated_files_from_configured_storage_root() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let asset_root =
        std::env::temp_dir().join(format!("video-agent-assets-{}", Uuid::new_v4().simple()));
    let image_path = asset_root
        .join("generated")
        .join("images")
        .join("task-1")
        .join("scene-1.png");
    std::fs::create_dir_all(image_path.parent().unwrap()).unwrap();
    std::fs::write(&image_path, b"png").unwrap();

    let app = build_app_with_state(app_state_with_asset_options(
        test_url,
        test_pool.clone(),
        path_to_string(asset_root.clone()),
        vec!["gpt-image-2".to_string(), "jimeng".to_string()],
    ));
    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/assets/generated/images/task-1/scene-1.png")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&body[..], b"png");

    let _ = std::fs::remove_dir_all(asset_root);
    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

fn path_to_string(path: PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn asset_generation_plan_rejects_more_than_48_images() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (script_id, _) = insert_script_with_scenes(&test_pool, project_id, 13).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scripts/{script_id}/asset-generation-plan"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "gpt-image-2",
                        "image_candidates_per_scene": 4,
                        "use_reference_materials": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["can_create"], false);
    assert_eq!(body["image_candidate_count"], 52);
    assert_eq!(body["max_image_candidate_count"], 48);
    assert_eq!(body["reference_material_count"], 0);
    assert!(body["warnings"][0].as_str().unwrap().contains("48"));

    let create_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scripts/{script_id}/asset-generation-tasks"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "gpt-image-2",
                        "image_candidates_per_scene": 4,
                        "use_reference_materials": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::BAD_REQUEST);
    assert!(response_json(create_response).await["error"]
        .as_str()
        .unwrap()
        .contains("48"));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn create_asset_generation_tasks_does_not_wait_for_worker() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (script_id, _) = insert_script_with_scenes(&test_pool, project_id, 2).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scripts/{script_id}/asset-generation-tasks"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "gpt-image-2",
                        "image_candidates_per_scene": 3,
                        "use_reference_materials": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = response_json(response).await;
    let tasks = body["tasks"].as_array().expect("tasks should be returned");
    assert!(tasks.iter().any(|task| {
        task["task_type"] == "image_candidates"
            && task["status"] == "pending"
            && task["candidate_count"] == 6
    }));
    assert_eq!(
        tasks
            .iter()
            .filter(|task| task["task_type"] == "video_draft" && task["status"] == "draft")
            .count(),
        2
    );

    let persisted = sqlx::query_as::<_, (String, String, i32)>(
        r#"
        SELECT task_type, status, candidate_count
        FROM asset_generation_tasks
        WHERE script_id = $1
        ORDER BY task_type ASC, scene_id ASC NULLS FIRST
        "#,
    )
    .bind(script_id)
    .fetch_all(&test_pool)
    .await
    .expect("asset generation tasks should persist without waiting for worker");
    assert!(persisted
        .iter()
        .any(|(task_type, status, candidate_count)| {
            task_type == "image_candidates" && status == "pending" && *candidate_count == 6
        }));
    assert_eq!(
        persisted
            .iter()
            .filter(|(task_type, status, _)| task_type == "video_draft" && status == "draft")
            .count(),
        2
    );

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn create_asset_generation_tasks_is_idempotent_and_tasks_are_listable() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (script_id, _) = insert_script_with_scenes(&test_pool, project_id, 2).await;
    insert_material(&test_pool, project_id, "character.png").await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));
    let payload = json!({
        "provider": "gpt-image-2",
        "image_candidates_per_scene": 3,
        "use_reference_materials": true
    })
    .to_string();

    let first_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scripts/{script_id}/asset-generation-tasks"))
                .header("content-type", "application/json")
                .body(Body::from(payload.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_response.status(), StatusCode::CREATED);

    let second_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scripts/{script_id}/asset-generation-tasks"))
                .header("content-type", "application/json")
                .body(Body::from(payload))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_response.status(), StatusCode::OK);

    let list_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/scripts/{script_id}/asset-generation-tasks"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert_eq!(listed["tasks"].as_array().unwrap().len(), 3);

    let task_counts = sqlx::query_as::<_, (String, i64)>(
        r#"
        SELECT task_type, COUNT(*)
        FROM asset_generation_tasks
        WHERE script_id = $1
        GROUP BY task_type
        ORDER BY task_type
        "#,
    )
    .bind(script_id)
    .fetch_all(&test_pool)
    .await
    .expect("task counts should be queryable");
    assert!(task_counts.contains(&("image_candidates".to_string(), 1)));
    assert!(task_counts.contains(&("video_draft".to_string(), 2)));

    let existing_candidate_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM scene_asset_candidates
        WHERE script_id = $1
          AND source = 'existing_material'
        "#,
    )
    .bind(script_id)
    .fetch_one(&test_pool)
    .await
    .expect("existing candidate count should be queryable");
    assert_eq!(existing_candidate_count, 2);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn asset_generation_plan_uses_enabled_provider_configuration() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (script_id, _) = insert_script_with_scenes(&test_pool, project_id, 1).await;
    let app = build_app_with_state(app_state_with_asset_options(
        test_url,
        test_pool.clone(),
        "/app/storage/assets".to_string(),
        vec!["jimeng".to_string()],
    ));

    let jimeng_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scripts/{script_id}/asset-generation-plan"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "jimeng",
                        "image_candidates_per_scene": 3,
                        "use_reference_materials": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(jimeng_response.status(), StatusCode::OK);
    let jimeng_plan = response_json(jimeng_response).await;
    assert_eq!(jimeng_plan["enabled_providers"], json!(["jimeng"]));

    let disabled_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scripts/{script_id}/asset-generation-plan"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "gpt-image-2",
                        "image_candidates_per_scene": 3,
                        "use_reference_materials": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disabled_response.status(), StatusCode::BAD_REQUEST);
    assert!(response_json(disabled_response).await["error"]
        .as_str()
        .unwrap()
        .contains("未启用"));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn asset_generation_routes_validate_provider_reject_and_scene_regenerate() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (script_id, scene_ids) = insert_script_with_scenes(&test_pool, project_id, 1).await;
    let material_id = insert_material(&test_pool, project_id, "candidate.png").await;
    let candidate_id = insert_candidate(
        &test_pool,
        project_id,
        script_id,
        scene_ids[0],
        material_id,
        1,
    )
    .await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let invalid_provider = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scripts/{script_id}/asset-generation-plan"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "unknown-provider",
                        "image_candidates_per_scene": 3,
                        "use_reference_materials": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_provider.status(), StatusCode::BAD_REQUEST);
    assert!(response_json(invalid_provider).await["error"]
        .as_str()
        .unwrap()
        .contains("供应商"));

    let reject_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/scenes/{}/asset-candidates/{candidate_id}/reject",
                    scene_ids[0]
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reject_response.status(), StatusCode::OK);
    assert_eq!(response_json(reject_response).await["status"], "rejected");

    let scene_task_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/scenes/{}/asset-generation-tasks",
                    scene_ids[0]
                ))
                .header("content-type", "application/json")
                .header("idempotency-key", Uuid::new_v4().to_string())
                .body(Body::from(
                    json!({
                        "provider": "jimeng",
                        "image_candidates_per_scene": 2,
                        "use_reference_materials": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(scene_task_response.status(), StatusCode::CREATED);
    let scene_task = response_json(scene_task_response).await;
    assert_eq!(scene_task["provider"], "jimeng");
    assert_eq!(scene_task["task_type"], "image_candidates");
    assert_eq!(scene_task["status"], "pending");
    assert_eq!(scene_task["candidate_count"], 2);
    assert_eq!(scene_task["scene_id"], scene_ids[0].to_string());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn scene_asset_generation_requires_a_uuid_idempotency_key() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (_, scene_ids) = insert_script_with_scenes(&test_pool, project_id, 1).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let missing_response = app
        .clone()
        .oneshot(scene_generation_request(scene_ids[0], None))
        .await
        .unwrap();
    assert_eq!(missing_response.status(), StatusCode::BAD_REQUEST);
    assert!(response_json(missing_response).await["error"]
        .as_str()
        .unwrap()
        .contains("Idempotency-Key"));

    let invalid_response = app
        .oneshot(scene_generation_request(scene_ids[0], Some("not-a-uuid")))
        .await
        .unwrap();
    assert_eq!(invalid_response.status(), StatusCode::BAD_REQUEST);
    assert!(response_json(invalid_response).await["error"]
        .as_str()
        .unwrap()
        .contains("Idempotency-Key"));

    let task_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM asset_generation_tasks WHERE scene_id = $1",
    )
    .bind(scene_ids[0])
    .fetch_one(&test_pool)
    .await
    .unwrap();
    assert_eq!(task_count, 0);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn scene_asset_generation_reuses_retries_and_one_in_flight_task() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (_, scene_ids) = insert_script_with_scenes(&test_pool, project_id, 2).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));
    let first_key = Uuid::new_v4();

    let first_response = app
        .clone()
        .oneshot(scene_generation_request(
            scene_ids[0],
            Some(&first_key.to_string()),
        ))
        .await
        .unwrap();
    assert_eq!(first_response.status(), StatusCode::CREATED);
    let first_task = response_json(first_response).await;

    let retry_response = app
        .clone()
        .oneshot(scene_generation_request(
            scene_ids[0],
            Some(&first_key.to_string()),
        ))
        .await
        .unwrap();
    assert_eq!(retry_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(retry_response).await["task_id"],
        first_task["task_id"]
    );

    let first_task_id = Uuid::parse_str(first_task["task_id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE asset_generation_tasks SET status = 'completed' WHERE id = $1")
        .bind(first_task_id)
        .execute(&test_pool)
        .await
        .unwrap();

    let next_response = app
        .clone()
        .oneshot(scene_generation_request(
            scene_ids[0],
            Some(&Uuid::new_v4().to_string()),
        ))
        .await
        .unwrap();
    assert_eq!(next_response.status(), StatusCode::CREATED);
    let next_task = response_json(next_response).await;
    assert_ne!(next_task["task_id"], first_task["task_id"]);

    let late_retry_response = app
        .clone()
        .oneshot(scene_generation_request(
            scene_ids[0],
            Some(&first_key.to_string()),
        ))
        .await
        .unwrap();
    assert_eq!(late_retry_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(late_retry_response).await["task_id"],
        first_task["task_id"]
    );

    let concurrent_first_key = Uuid::new_v4();
    let concurrent_second_key = Uuid::new_v4();
    let concurrent_first = app.clone().oneshot(scene_generation_request(
        scene_ids[1],
        Some(&concurrent_first_key.to_string()),
    ));
    let concurrent_second = app.clone().oneshot(scene_generation_request(
        scene_ids[1],
        Some(&concurrent_second_key.to_string()),
    ));
    let (concurrent_first, concurrent_second) = tokio::join!(concurrent_first, concurrent_second);
    let concurrent_first = concurrent_first.unwrap();
    let concurrent_second = concurrent_second.unwrap();
    assert!(matches!(
        (concurrent_first.status(), concurrent_second.status()),
        (StatusCode::CREATED, StatusCode::OK) | (StatusCode::OK, StatusCode::CREATED)
    ));
    let concurrent_first = response_json(concurrent_first).await;
    let concurrent_second = response_json(concurrent_second).await;
    assert_eq!(concurrent_first["task_id"], concurrent_second["task_id"]);
    let concurrent_task_id =
        Uuid::parse_str(concurrent_first["task_id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE asset_generation_tasks SET status = 'completed' WHERE id = $1")
        .bind(concurrent_task_id)
        .execute(&test_pool)
        .await
        .unwrap();

    for request_key in [concurrent_first_key, concurrent_second_key] {
        let late_retry = app
            .clone()
            .oneshot(scene_generation_request(
                scene_ids[1],
                Some(&request_key.to_string()),
            ))
            .await
            .unwrap();
        assert_eq!(late_retry.status(), StatusCode::OK);
        assert_eq!(
            response_json(late_retry).await["task_id"],
            concurrent_first["task_id"]
        );
    }

    let task_counts = sqlx::query_as::<_, (Uuid, i64)>(
        r#"
        SELECT scene_id, COUNT(*)
        FROM asset_generation_tasks
        WHERE scene_id = ANY($1)
        GROUP BY scene_id
        ORDER BY scene_id
        "#,
    )
    .bind(&scene_ids)
    .fetch_all(&test_pool)
    .await
    .unwrap();
    assert!(task_counts.contains(&(scene_ids[0], 2)));
    assert!(task_counts.contains(&(scene_ids[1], 1)));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn cors_allows_scene_generation_idempotency_header() {
    let app = build_app_with_state(AppState::test());
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri(format!(
                    "/api/scenes/{}/asset-generation-tasks",
                    Uuid::new_v4()
                ))
                .header("origin", "http://localhost:18183")
                .header("access-control-request-method", "POST")
                .header(
                    "access-control-request-headers",
                    "content-type,idempotency-key",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let allowed_headers = response
        .headers()
        .get("access-control-allow-headers")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    assert!(allowed_headers.contains("idempotency-key"));
}

#[tokio::test]
async fn confirm_asset_generation_task_only_allows_draft_video_tasks() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (script_id, _) = insert_script_with_scenes(&test_pool, project_id, 1).await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/scripts/{script_id}/asset-generation-tasks"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "gpt-image-2",
                        "image_candidates_per_scene": 3,
                        "use_reference_materials": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = response_json(create_response).await;
    let image_task_id = created["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["task_type"] == "image_candidates")
        .unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_string();
    let video_task_id = created["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["task_type"] == "video_draft")
        .unwrap()["task_id"]
        .as_str()
        .unwrap()
        .to_string();

    let image_confirm = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/asset-generation-tasks/{image_task_id}/confirm"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(image_confirm.status(), StatusCode::BAD_REQUEST);

    let video_confirm = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/asset-generation-tasks/{video_task_id}/confirm"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(video_confirm.status(), StatusCode::OK);
    let confirmed = response_json(video_confirm).await;
    assert_eq!(confirmed["task_id"], video_task_id);
    assert_eq!(confirmed["task_type"], "video_generation");
    assert_eq!(confirmed["status"], "pending");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn selecting_candidate_replaces_existing_selected_candidate() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (script_id, scene_ids) = insert_script_with_scenes(&test_pool, project_id, 1).await;
    let first_material_id = insert_material(&test_pool, project_id, "first.png").await;
    let second_material_id = insert_material(&test_pool, project_id, "second.png").await;
    let first_candidate_id = insert_candidate(
        &test_pool,
        project_id,
        script_id,
        scene_ids[0],
        first_material_id,
        1,
    )
    .await;
    let second_candidate_id = insert_candidate(
        &test_pool,
        project_id,
        script_id,
        scene_ids[0],
        second_material_id,
        2,
    )
    .await;
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let first_select = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/scenes/{}/asset-candidates/{first_candidate_id}/select",
                    scene_ids[0]
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_select.status(), StatusCode::OK);
    assert_eq!(response_json(first_select).await["status"], "selected");

    let second_select = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/scenes/{}/asset-candidates/{second_candidate_id}/select",
                    scene_ids[0]
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_select.status(), StatusCode::OK);
    assert_eq!(
        response_json(second_select).await["candidate_id"],
        second_candidate_id.to_string()
    );

    let list_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/scripts/{script_id}/asset-candidates"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    let candidates = listed["candidates"].as_array().unwrap();
    let selected_candidates: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate["status"] == "selected")
        .collect();
    assert_eq!(selected_candidates.len(), 1);
    assert_eq!(
        selected_candidates[0]["candidate_id"],
        second_candidate_id.to_string()
    );
    assert!(candidates.iter().any(|candidate| {
        candidate["candidate_id"] == first_candidate_id.to_string()
            && candidate["status"] == "candidate"
    }));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn selecting_failed_candidate_returns_specific_error() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (script_id, scene_ids) = insert_script_with_scenes(&test_pool, project_id, 1).await;
    let candidate_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO scene_asset_candidates (
            project_id, script_id, scene_id, candidate_type, source, status, rank
        )
        VALUES ($1, $2, $3, 'image', 'ai_generated', 'failed', 1)
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(script_id)
    .bind(scene_ids[0])
    .fetch_one(&test_pool)
    .await
    .expect("failed candidate fixture should be inserted");
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/scenes/{}/asset-candidates/{candidate_id}/select",
                    scene_ids[0]
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(response).await["error"],
        "失败候选不可绑定分镜"
    );

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn dismiss_asset_generation_task_is_idempotent_and_rejects_non_failed_tasks() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (script_id, _) = insert_script_with_scenes(&test_pool, project_id, 1).await;
    let failed_task_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO asset_generation_tasks (
            project_id, script_id, provider, task_type, status, candidate_count,
            params, result, error_message
        )
        VALUES ($1, $2, 'gpt-image-2', 'image_candidates', 'failed', 3,
                '{"image_candidates_per_scene":3}',
                '{"generated_count":0,"failed_count":3}',
                'Image generation is not enabled for this group')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(script_id)
    .fetch_one(&test_pool)
    .await
    .expect("failed task fixture should be inserted");
    let completed_task_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO asset_generation_tasks (
            project_id, script_id, provider, task_type, status, candidate_count
        )
        VALUES ($1, $2, 'gpt-image-2', 'image_candidates', 'completed', 3)
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(script_id)
    .fetch_one(&test_pool)
    .await
    .expect("completed task fixture should be inserted");
    let app = build_app_with_state(app_state(test_url, test_pool.clone()));

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/asset-generation-tasks/{failed_task_id}/dismiss"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = response_json(first).await;
    assert_eq!(first_body["task_id"], failed_task_id.to_string());
    assert_eq!(first_body["status"], "failed");
    let first_dismissed_at = first_body["dismissed_at"]
        .as_str()
        .expect("dismiss response should include audit timestamp")
        .to_string();

    let repeated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/asset-generation-tasks/{failed_task_id}/dismiss"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(repeated.status(), StatusCode::OK);
    assert_eq!(
        response_json(repeated).await["dismissed_at"],
        first_dismissed_at
    );

    let conflict = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/asset-generation-tasks/{completed_task_id}/dismiss"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(conflict).await["error"],
        "只有失败的素材生成任务可以清理"
    );

    let missing = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/asset-generation-tasks/{}/dismiss",
                    Uuid::new_v4()
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let persisted = sqlx::query_as::<_, (String, i32, Value, Value, String)>(
        r#"
        SELECT status, candidate_count, params, result, error_message
        FROM asset_generation_tasks
        WHERE id = $1 AND dismissed_at IS NOT NULL
        "#,
    )
    .bind(failed_task_id)
    .fetch_one(&test_pool)
    .await
    .expect("dismissed task should retain audit fields");
    assert_eq!(persisted.0, "failed");
    assert_eq!(persisted.1, 3);
    assert_eq!(persisted.2["image_candidates_per_scene"], 3);
    assert_eq!(persisted.3["failed_count"], 3);
    assert!(persisted.4.contains("not enabled"));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
