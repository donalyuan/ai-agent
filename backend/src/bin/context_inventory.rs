use novex_ai_core::DefinitionRegistry;
use novex_api::model_context_inventory::build_model_context_inventory;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
    });
    let definitions_dir = std::env::var("NOVEX_AGENT_DEFINITIONS_DIR")
        .unwrap_or_else(|_| "/app/agent-definitions".to_string());
    let definitions = DefinitionRegistry::load(definitions_dir)?;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await?;
    let report = build_model_context_inventory(&pool, &definitions).await?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    pool.close().await;
    Ok(())
}
