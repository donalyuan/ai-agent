use novex_api::model_config_import::{import_legacy_model_config, LegacyModelImportConfig};
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() {
    if !std::env::args().any(|arg| arg == "--confirm-plaintext-credentials") {
        eprintln!(
            "拒绝导入：必须显式传入 --confirm-plaintext-credentials，确认凭据将按原文写入数据库"
        );
        std::process::exit(2);
    }

    if let Err(error) = run().await {
        eprintln!("模型配置导入失败: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
    });
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    let outcome = import_legacy_model_config(&pool, LegacyModelImportConfig::from_env()).await?;

    for model in &outcome.created {
        println!("已创建：{} ({})", model.display_name, model.model_id);
    }
    for source_key in &outcome.skipped {
        println!("已跳过：{source_key}");
    }
    if outcome.created.is_empty() && outcome.skipped.is_empty() {
        println!("未发现凭据完整的旧模型配置，未创建模型");
    }
    pool.close().await;
    Ok(())
}
