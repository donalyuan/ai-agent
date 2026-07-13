use novex_api::bootstrap::build_runtime_state;
use novex_api::build_app_with_state;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = build_runtime_state().await?;
    let app = build_app_with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;

    axum::serve(listener, app).await?;

    Ok(())
}
