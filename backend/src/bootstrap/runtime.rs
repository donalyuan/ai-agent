//! 建立生产数据库与 Redis 依赖，并在 Router 启动前完成必要初始化。

use super::{AppConfig, AppState};
use crate::repositories::PostgresDefinitionReleaseRepository;
use novex_ai_core::{DefinitionRegistry, ExecutorOwner};
use sqlx::{postgres::PgPoolOptions, PgPool};

/// 构建生产运行时依赖，并在服务启动前完成数据库迁移和菜单状态同步。
pub async fn build_runtime_state() -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::from_env();
    let pg_pool = connect_runtime_pg_pool(&config.database_url, 5).await?;
    let redis_client = redis::Client::open(config.redis_url.clone())?;

    let state = AppState::new(config, pg_pool.clone(), Some(redis_client))?;
    let definitions = state.definition_registry()?;
    validate_production_execution_integrity(definitions.as_ref())?;
    PostgresDefinitionReleaseRepository::new(pg_pool)
        .publish_registry(definitions.as_ref())
        .await?;
    Ok(state)
}

/// 启动时固定核对全部 Rust 生产节点，防止仅靠测试 inventory 而在部署产物中漏装定义。
fn validate_production_execution_integrity(
    registry: &DefinitionRegistry,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    const INVENTORY: &[(&str, &[&str])] = &[
        ("video.project-strategy", &["project.strategy_draft"]),
        (
            "video.script",
            &[
                "script.complete",
                "script.metadata",
                "script.single_scene",
                "script.generation_intent",
                "script.scene_patch",
            ],
        ),
        (
            "video.topic",
            &[
                "topic.generate",
                "topic.supplement",
                "topic.quality_review",
                "topic.rewrite",
                "topic.group_review",
            ],
        ),
        ("video.sound", &["sound.recommend"]),
        ("video.work", &["work.plan", "work.patch"]),
    ];

    for (agent_key, node_keys) in INVENTORY {
        let agent = registry.active_agent(agent_key)?;
        if agent.executor_owner != ExecutorOwner::Rust {
            return Err(format!("production definition {agent_key} is not owned by Rust").into());
        }
        for node_key in *node_keys {
            if !agent.nodes.contains_key(*node_key) {
                return Err(format!(
                    "production definition {agent_key}@{} is missing node {node_key}",
                    agent.version
                )
                .into());
            }
        }
    }
    Ok(())
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
