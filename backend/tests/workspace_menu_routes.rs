use axum::body::Body;
use axum::http::{Request, StatusCode};
use novex_api::bootstrap::{AppConfig, AppState};
use novex_api::build_app_with_state;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
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
        .expect("temporary workspace menu route database should be created");
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
    let database_name = format!("video_agent_workspace_menu_route_test_{}", suffix);
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
        .expect("temporary workspace menu route database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for workspace menu route test database");

    (admin_pool, test_pool, database_name, test_url)
}

fn app_state(test_url: String, pool: PgPool) -> AppState {
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
            asset_storage_root: "/app/storage/assets".to_string(),
            asset_generation_providers: vec!["gpt-image-2".to_string(), "jimeng".to_string()],
        },
        pool,
        None,
    )
    .unwrap()
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}

#[tokio::test]
async fn workspace_menu_route_returns_visible_sorted_tree() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;

    let app = build_app_with_state(app_state(test_url, test_pool.clone()));
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/video-workspace/menus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    let menus = body["menus"].as_array().expect("menus should be an array");
    let labels = menus
        .iter()
        .map(|menu| menu["label"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        labels,
        vec![
            "内容策略",
            "脚本创作",
            "素材管理",
            "作品生产",
            "发布运营",
            "数据分析",
            "工作流任务",
        ]
    );

    let script_menu = menus
        .iter()
        .find(|menu| menu["menu_key"] == "script-creation")
        .expect("script creation menu should exist");
    assert_eq!(script_menu["is_enabled"], true);
    assert_eq!(script_menu["status"], "active");
    assert_eq!(script_menu["route_path"], "/scripts");
    assert_eq!(script_menu["metadata"], json!({ "phase": 1 }));
    assert_eq!(script_menu["children"][0]["menu_key"], "script-generator");
    assert_eq!(
        script_menu["children"][0]["agent_key"],
        "script-generation-agent"
    );

    let content_strategy = menus
        .iter()
        .find(|menu| menu["menu_key"] == "content-strategy")
        .expect("content strategy menu should exist");
    assert_eq!(content_strategy["is_enabled"], true);
    assert_eq!(content_strategy["status"], "active");
    assert_eq!(content_strategy["metadata"], json!({ "phase": 2 }));
    assert_eq!(
        content_strategy["children"][0]["menu_key"],
        "account-strategy"
    );
    assert_eq!(content_strategy["children"][0]["label"], "账号策略");
    assert_eq!(content_strategy["children"][0]["is_enabled"], true);
    assert_eq!(content_strategy["children"][0]["status"], "active");
    assert_eq!(
        content_strategy["children"][0]["agent_key"],
        "topic-generation-agent"
    );
    assert_eq!(
        content_strategy["children"][0]["module_key"],
        "strategy.account"
    );
    assert_eq!(content_strategy["children"][1]["menu_key"], "topic-history");
    assert_eq!(content_strategy["children"][1]["label"], "历史生成");
    assert_eq!(content_strategy["children"][1]["is_enabled"], true);
    assert_eq!(content_strategy["children"][1]["status"], "active");
    assert_eq!(
        content_strategy["children"][1]["agent_key"],
        "topic-generation-agent"
    );
    assert_eq!(
        content_strategy["children"][2]["menu_key"],
        "topic-generator"
    );
    assert_eq!(content_strategy["children"][2]["label"], "当前选题池");
    assert_eq!(content_strategy["children"][2]["is_enabled"], true);
    assert_eq!(content_strategy["children"][2]["status"], "active");

    let material_menu = menus
        .iter()
        .find(|menu| menu["menu_key"] == "material-management")
        .expect("material management menu should exist");
    assert_eq!(material_menu["is_enabled"], true);
    assert_eq!(material_menu["status"], "active");
    assert_eq!(material_menu["metadata"], json!({ "phase": 3 }));
    assert_eq!(material_menu["children"][0]["menu_key"], "material-library");
    assert_eq!(material_menu["children"][0]["label"], "素材库");
    assert_eq!(
        material_menu["children"][0]["route_path"],
        "/materials/library"
    );
    assert_eq!(material_menu["children"][0]["menu_type"], "page");
    assert_eq!(
        material_menu["children"][0]["module_key"],
        "materials.library"
    );
    assert_eq!(material_menu["children"][0]["is_enabled"], true);
    assert_eq!(material_menu["children"][0]["status"], "active");
    assert_eq!(material_menu["children"][1]["menu_key"], "asset-generation");
    assert_eq!(material_menu["children"][1]["label"], "画面生成");
    assert_eq!(
        material_menu["children"][1]["route_path"],
        "/materials/generation"
    );
    assert_eq!(material_menu["children"][1]["menu_type"], "page");
    assert_eq!(
        material_menu["children"][1]["module_key"],
        "materials.asset-generation"
    );
    assert_eq!(
        material_menu["children"][1]["agent_key"],
        "material-generation-agent"
    );
    assert_eq!(material_menu["children"][1]["is_enabled"], true);
    assert_eq!(material_menu["children"][1]["status"], "active");
    assert_eq!(
        material_menu["children"][2]["menu_key"],
        "sound-subtitle-generation"
    );
    assert_eq!(material_menu["children"][2]["label"], "声音与字幕生成");
    assert_eq!(material_menu["children"][2]["is_enabled"], true);
    assert_eq!(material_menu["children"][2]["status"], "active");
    assert_eq!(
        material_menu["children"][2]["module_key"],
        "materials.sound-subtitle-generation"
    );

    let body_text = body.to_string();
    assert!(!body_text.contains("material-search"));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
