//! 组合各业务 Router、跨域策略和静态素材服务，不承载具体业务 handler。

use crate::api::{
    ai_models, asset_generation, conversations, health, materials, projects, scripts,
    sound_subtitle, topics, tos_staging_tool, work_generation, work_library, workspace,
};
use crate::bootstrap::AppState;
use axum::{
    http::{header, HeaderName, HeaderValue, Method},
    Router,
};
use tower_http::{cors::CorsLayer, services::ServeDir};

pub fn build_app() -> Router {
    build_app_with_state(AppState::test())
}

pub fn build_app_with_state(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin("*".parse::<HeaderValue>().unwrap())
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::ACCEPT,
            header::CONTENT_TYPE,
            HeaderName::from_static("idempotency-key"),
        ]);

    Router::new()
        .merge(health::router())
        .merge(ai_models::router())
        .merge(workspace::router())
        .merge(projects::router())
        .merge(topics::router())
        .merge(materials::router())
        .merge(conversations::router())
        .merge(scripts::router())
        .merge(asset_generation::router())
        .merge(sound_subtitle::router())
        .merge(work_generation::router())
        .merge(work_library::router())
        .merge(tos_staging_tool::router())
        .nest_service(
            "/assets",
            ServeDir::new(state.config.asset_storage_root.clone()),
        )
        .layer(cors)
        .with_state(state)
}
