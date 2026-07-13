use crate::bootstrap::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use serde::Serialize;

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

pub(crate) fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let liveness = state.health_service().liveness();
    Json(HealthResponse {
        service: "novex-api",
        status: "ok",
        environment: liveness.environment,
    })
}

async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let readiness = state.health_service().readiness().await;
    let status_code = if readiness.is_ready() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let body = ReadyResponse {
        service: "novex-api",
        status: if readiness.is_ready() {
            "ready"
        } else {
            "not_ready"
        },
        postgres: if readiness.postgres { "ok" } else { "error" },
        redis: if readiness.redis { "ok" } else { "error" },
    };

    (status_code, Json(body))
}
