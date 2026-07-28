use novex_api::{
    application::production_media_validation::{
        build_production_media_validation_plan, ProductionMediaValidationLimits,
    },
    bootstrap::AppConfig,
};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env();
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&config.database_url)
        .await?;
    // 当前部署没有正式 MediaEvidenceProvider 配置源；保持 None 才能如实输出 blocker。
    let plan = build_production_media_validation_plan(
        &pool,
        None,
        ProductionMediaValidationLimits::conservative_v3(),
    )
    .await?;
    println!("{}", serde_json::to_string_pretty(&plan)?);
    pool.close().await;
    Ok(())
}
