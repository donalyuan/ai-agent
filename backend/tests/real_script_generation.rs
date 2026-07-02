use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use novex_api::{build_app_with_state, AppConfig, AppState};
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::{SystemTime, UNIX_EPOCH};
use tower::ServiceExt;
use uuid::Uuid;

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

async fn create_database(admin_pool: &PgPool, database_name: &str) {
    let query = format!(r#"CREATE DATABASE "{}""#, database_name);
    sqlx::query(&query)
        .execute(admin_pool)
        .await
        .expect("temporary real LLM database should be created");
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

async fn migrated_pool() -> (PgPool, PgPool, String, String) {
    let base_url = database_url();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let database_name = format!("video_agent_real_llm_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    create_database(&admin_pool, &database_name).await;

    let test_pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&test_url)
        .await
        .expect("temporary real LLM database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for real LLM test database");

    (admin_pool, test_pool, database_name, test_url)
}

async fn insert_project(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('真实 LLM 验证项目', 'AI 工具效率账号', 'extract-llm-client-to-novex-model 验证项目')
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("real LLM project fixture should be inserted")
}

async fn response_body(response: axum::response::Response) -> (StatusCode, String) {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

struct RealScriptSummary {
    script_id: String,
    title: String,
    scene_count: usize,
}

async fn run_real_script_generation(
    test_pool: PgPool,
    test_url: String,
    project_id: Uuid,
) -> Result<RealScriptSummary, String> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| "OPENAI_API_KEY must be set for real LLM generation".to_string())?;
    let base_url = std::env::var("OPENAI_BASE_URL")
        .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.4-mini".to_string());
    let reasoning_effort = std::env::var("OPENAI_REASONING_EFFORT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("none"))
        .or_else(|| Some("low".to_string()));
    let max_output_tokens = std::env::var("OPENAI_MAX_OUTPUT_TOKENS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(3000);

    let app = build_app_with_state(
        AppState::new(
            AppConfig {
                environment: "real-llm-test".to_string(),
                database_url: test_url,
                redis_url: "redis://127.0.0.1:6379/15".to_string(),
                openai_api_key: api_key,
                openai_base_url: base_url,
                openai_model: model,
                openai_timeout_seconds: 120,
                openai_reasoning_effort: reasoning_effort,
                openai_max_output_tokens: max_output_tokens,
            },
            test_pool,
            None,
        )
        .map_err(|error| error.to_string())?,
    );

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/scripts/generate")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "project_id": project_id,
                        "topic": "AI 如何提升程序员日常开发效率",
                        "style": "knowledge",
                        "scene_count": 5
                    })
                    .to_string(),
                ))
                .map_err(|error| error.to_string())?,
        )
        .await
        .map_err(|error| error.to_string())?;
    let (status, body) = response_body(response).await;
    if status != StatusCode::OK {
        return Err(format!("expected 200 OK, got {status}: {body}"));
    }

    let payload: Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    let scenes = payload["scenes"]
        .as_array()
        .ok_or_else(|| "response scenes must be an array".to_string())?;
    if scenes.len() != 5 {
        return Err(format!("expected 5 scenes, got {}", scenes.len()));
    }

    Ok(RealScriptSummary {
        script_id: payload["script_id"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        title: payload["title"].as_str().unwrap_or_default().to_string(),
        scene_count: scenes.len(),
    })
}

#[tokio::test]
#[ignore = "requires real OpenAI-compatible provider credentials"]
async fn real_responses_api_generates_and_persists_script() {
    let (admin_pool, test_pool, database_name, test_url) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;

    let result = run_real_script_generation(test_pool.clone(), test_url, project_id).await;

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;

    let summary = result.unwrap();
    println!("script_id={}", summary.script_id);
    println!("title={}", summary.title);
    println!("scene_count={}", summary.scene_count);
}
