use novex_agent::AuditedModelExecutor;
use novex_ai_core::DefinitionRegistry;
use novex_api::{
    application::evaluations::{
        build_production_crew_eval_authorization_plan, ProductionCrewEvalAuthorizationLimits,
    },
    bootstrap::AppConfig,
    model_routing::PostgresModelClientResolver,
    repositories::{
        PostgresAiModelRepository, PostgresContextAuditRepository, PostgresModelCallRepository,
    },
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&config.database_url)
        .await?;
    let registry = Arc::new(DefinitionRegistry::load(config.agent_definitions_dir())?);
    let model_id = selected_model_id(&pool).await?;
    let resolver = Arc::new(PostgresModelClientResolver::new(
        PostgresAiModelRepository::new(pool.clone()),
        registry.clone(),
    ));
    let executor = AuditedModelExecutor::new(
        registry.clone(),
        resolver,
        Arc::new(PostgresModelCallRepository::new(pool.clone())),
        Arc::new(PostgresContextAuditRepository::new(pool.clone())),
    );
    let plan = build_production_crew_eval_authorization_plan(
        &registry,
        &executor,
        model_id,
        ProductionCrewEvalAuthorizationLimits::conservative_v3(),
    )
    .await?;
    println!("{}", serde_json::to_string_pretty(&plan)?);
    pool.close().await;
    Ok(())
}

async fn selected_model_id(pool: &PgPool) -> Result<Uuid, sqlx::Error> {
    if let Some(configured) = std::env::var("PRODUCTION_CREW_EVAL_MODEL_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return Uuid::parse_str(&configured).map_err(|error| sqlx::Error::Decode(Box::new(error)));
    }
    sqlx::query_scalar(
        r#"
        SELECT id
        FROM ai_models
        WHERE model_type = 'text' AND status = 'enabled' AND deleted_at IS NULL
        ORDER BY is_default DESC, sort_order, created_at, id
        LIMIT 1
        "#,
    )
    .fetch_one(pool)
    .await
}
