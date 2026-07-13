//! 建立生产数据库与 Redis 依赖，并在 Router 启动前完成必要初始化。

use super::{AppConfig, AppState};
use sqlx::{postgres::PgPoolOptions, PgPool};

/// 构建生产运行时依赖，并在服务启动前完成数据库迁移和菜单状态同步。
pub async fn build_runtime_state() -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env();
    let pg_pool = connect_runtime_pg_pool(&config.database_url, 5).await?;
    let redis_client = redis::Client::open(config.redis_url.clone())?;

    AppState::new(config, pg_pool, Some(redis_client))
}

pub async fn connect_runtime_pg_pool(
    database_url: &str,
    max_connections: u32,
) -> Result<PgPool, Box<dyn std::error::Error + Send + Sync>> {
    let pg_pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(database_url)
        .await?;
    sqlx::migrate!("./migrations").run(&pg_pool).await?;
    sync_content_strategy_menu_state(&pg_pool).await?;

    Ok(pg_pool)
}

/// 启动同步保证数据库种子菜单与当前已开放能力一致，避免部署旧数据隐藏入口。
async fn sync_content_strategy_menu_state(pool: &PgPool) -> Result<(), sqlx::Error> {
    let menu_table_exists = sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('public.video_workspace_menus') IS NOT NULL",
    )
    .fetch_one(pool)
    .await?;

    if !menu_table_exists {
        return Ok(());
    }

    sqlx::query(
        r#"
        UPDATE video_workspace_menus
        SET
            is_enabled = true,
            status = 'active',
            metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{phase}', '2'::jsonb, true),
            updated_at = NOW()
        WHERE menu_key IN ('content-strategy', 'account-strategy', 'topic-history', 'topic-generator')
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
