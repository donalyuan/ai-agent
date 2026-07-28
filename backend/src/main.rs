use novex_api::bootstrap::build_runtime_state;
use novex_api::build_app_with_state;
use tokio::sync::watch;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = build_runtime_state().await?;
    let runner = state.production_runner()?;
    let app = build_app_with_state(state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let runner_task = tokio::spawn(async move {
        if let Err(error) = runner.run(shutdown_rx).await {
            eprintln!("Full Crew Runner 已停止: {error}");
        }
    });

    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;

    let _ = shutdown_tx.send(true);
    let _ = runner_task.await;
    Ok(())
}
