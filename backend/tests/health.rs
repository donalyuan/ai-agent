use axum::body::Body;
use axum::http::{Request, StatusCode};
use novex_api::application::health::HealthService;
use serde_json::Value;
use tower::ServiceExt;

#[test]
fn health_service_exposes_environment_for_liveness() {
    let status = HealthService::new("test".to_owned(), None, None).liveness();

    assert_eq!(status.environment, "test");
}

#[tokio::test]
async fn health_service_reports_missing_dependencies_as_not_ready() {
    let status = HealthService::new("test".to_owned(), None, None)
        .readiness()
        .await;

    assert!(!status.is_ready());
    assert!(!status.postgres);
    assert!(!status.redis);
}

#[tokio::test]
async fn health_returns_service_status() {
    let app = novex_api::build_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["service"], "novex-api");
    assert_eq!(payload["status"], "ok");
}

#[tokio::test]
async fn ready_reports_missing_dependencies_with_stable_payload() {
    let app = novex_api::build_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(payload["service"], "novex-api");
    assert_eq!(payload["status"], "not_ready");
    assert_eq!(payload["postgres"], "error");
    assert_eq!(payload["redis"], "error");
}
