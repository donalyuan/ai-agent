use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use novex_api::build_app;
use tower::ServiceExt;

#[tokio::test]
async fn cors_allows_admin_origin_for_api_gets() {
    let response = build_app()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/health")
                .header(header::ORIGIN, "http://127.0.0.1:18182")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        "*",
    );
}

#[tokio::test]
async fn cors_allows_admin_origin_preflight_for_json_writes() {
    let response = build_app()
        .oneshot(
            Request::builder()
                .method(Method::OPTIONS)
                .uri("/api/projects")
                .header(header::ORIGIN, "http://127.0.0.1:18182")
                .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .unwrap(),
        "*",
    );
}
