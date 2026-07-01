use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use sqlx::{postgres::PgPoolOptions, PgPool};

pub mod agents;
pub mod repositories;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub environment: String,
    pub database_url: String,
    pub redis_url: String,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            environment: std::env::var("NOVEX_ENV")
                .unwrap_or_else(|_| "development".to_string()),
            database_url: std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
            }),
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://bs-redis:6379/2".to_string()),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    config: AppConfig,
    pg_pool: Option<PgPool>,
    redis_client: Option<redis::Client>,
}

impl AppState {
    pub fn test() -> Self {
        Self {
            config: AppConfig::from_env(),
            pg_pool: None,
            redis_client: None,
        }
    }

    pub fn new(config: AppConfig, pg_pool: PgPool, redis_client: redis::Client) -> Self {
        Self {
            config,
            pg_pool: Some(pg_pool),
            redis_client: Some(redis_client),
        }
    }
}

#[derive(Serialize)]
struct HealthResponse {
    service: &'static str,
    status: &'static str,
    environment: String,
}

#[derive(Serialize)]
struct ReadyResponse {
    service: &'static str,
    status: &'static str,
    postgres: &'static str,
    redis: &'static str,
}

pub fn build_app() -> Router {
    build_app_with_state(AppState::test())
}

pub fn build_app_with_state(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .with_state(state)
}

pub async fn build_runtime_state() -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env();
    let pg_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_lazy(&config.database_url)?;
    let redis_client = redis::Client::open(config.redis_url.clone())?;

    Ok(AppState::new(config, pg_pool, redis_client))
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "novex-api",
        status: "ok",
        environment: state.config.environment,
    })
}

async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let postgres_ok = match state.pg_pool {
        Some(pool) => sqlx::query_scalar::<_, i32>("SELECT 1")
            .fetch_one(&pool)
            .await
            .is_ok(),
        None => false,
    };

    let redis_ok = match state.redis_client {
        Some(client) => match client.get_multiplexed_async_connection().await {
            Ok(mut connection) => redis::cmd("PING")
                .query_async::<String>(&mut connection)
                .await
                .map(|value| value == "PONG")
                .unwrap_or(false),
            Err(_) => false,
        },
        None => false,
    };

    let status_code = if postgres_ok && redis_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body = ReadyResponse {
        service: "novex-api",
        status: if postgres_ok && redis_ok { "ready" } else { "not_ready" },
        postgres: if postgres_ok { "ok" } else { "error" },
        redis: if redis_ok { "ok" } else { "error" },
    };

    (status_code, Json(body))
}
