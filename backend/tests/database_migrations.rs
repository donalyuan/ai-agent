use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::{SystemTime, UNIX_EPOCH};

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
    })
}

fn with_database_name(database_url: &str, database_name: &str) -> String {
    let query_start = database_url.find('?');
    let (base, query) = match query_start {
        Some(index) => (&database_url[..index], &database_url[index..]),
        None => (database_url, ""),
    };

    let slash_index = base
        .rfind('/')
        .expect("DATABASE_URL must include database name");
    format!("{}{}{}", &base[..=slash_index], database_name, query)
}

async fn table_exists(pool: &PgPool, table_name: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = 'public'
              AND table_name = $1
        )
        "#,
    )
    .bind(table_name)
    .fetch_one(pool)
    .await
    .expect("table existence query should run")
}

async fn index_exists(pool: &PgPool, index_name: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM pg_indexes
            WHERE schemaname = 'public'
              AND indexname = $1
        )
        "#,
    )
    .bind(index_name)
    .fetch_one(pool)
    .await
    .expect("index existence query should run")
}

async fn constraint_exists(pool: &PgPool, table_name: &str, constraint_name: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.table_constraints
            WHERE table_schema = 'public'
              AND table_name = $1
              AND constraint_name = $2
        )
        "#,
    )
    .bind(table_name)
    .bind(constraint_name)
    .fetch_one(pool)
    .await
    .expect("constraint existence query should run")
}

async fn create_database(admin_pool: &PgPool, database_name: &str) {
    let query = format!(r#"CREATE DATABASE "{}""#, database_name);
    sqlx::query(&query)
        .execute(admin_pool)
        .await
        .expect("temporary migration database should be created");
}

async fn drop_database(admin_pool: &PgPool, database_name: &str) {
    let disconnect = format!(
        r#"
        SELECT pg_terminate_backend(pid)
        FROM pg_stat_activity
        WHERE datname = '{}'
        "#,
        database_name
    );
    let drop = format!(r#"DROP DATABASE IF EXISTS "{}""#, database_name);

    let _ = sqlx::query(&disconnect).execute(admin_pool).await;
    let _ = sqlx::query(&drop).execute(admin_pool).await;
}

#[tokio::test]
async fn migrations_create_video_agent_core_schema() {
    let base_url = database_url();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let database_name = format!("video_agent_migration_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");

    create_database(&admin_pool, &database_name).await;

    let test_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&test_url)
        .await
        .expect("temporary migration database should be reachable");

    let migration_result = sqlx::migrate!("./migrations").run(&test_pool).await;
    assert!(
        migration_result.is_ok(),
        "migrations should run cleanly: {:?}",
        migration_result.err()
    );

    for table in [
        "projects",
        "accounts",
        "materials",
        "material_embeddings",
        "scripts",
        "scenes",
        "generation_tasks",
        "videos",
        "publish_tasks",
        "metrics",
        "revenues",
        "agent_runs",
        "agent_steps",
        "agent_conversations",
        "agent_messages",
        "content_topics",
        "topic_generation_batches",
        "viral_videos",
        "content_strategies",
        "video_workspace_menus",
    ] {
        assert!(
            table_exists(&test_pool, table).await,
            "{table} table should exist"
        );
    }

    for index in [
        "idx_materials_project",
        "idx_scripts_project",
        "idx_scenes_script",
        "idx_generation_tasks_status",
        "idx_publish_tasks_status",
        "idx_agent_runs_type",
        "idx_agent_conversations_project",
        "idx_agent_messages_conversation_created",
        "idx_content_topics_project",
        "idx_content_topics_status",
        "idx_content_topics_source",
        "idx_content_topics_batch",
        "idx_content_topics_created",
        "idx_topic_generation_batches_project",
        "idx_scripts_topic",
        "idx_video_workspace_menus_parent_sort",
    ] {
        assert!(
            index_exists(&test_pool, index).await,
            "{index} index should exist"
        );
    }

    assert!(
        constraint_exists(&test_pool, "scripts", "scripts_status_check").await,
        "scripts.status should be constrained to known states"
    );
    assert!(
        constraint_exists(&test_pool, "scenes", "scenes_script_sequence_unique").await,
        "scene sequence should be unique per script"
    );
    assert!(
        constraint_exists(&test_pool, "agent_messages", "agent_messages_role_check").await,
        "agent message role should be constrained"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "agent_conversations",
            "agent_conversations_agent_type_check"
        )
        .await,
        "conversation agent type should be constrained"
    );
    assert!(
        constraint_exists(&test_pool, "content_topics", "content_topics_status_check").await,
        "content topic status should be constrained"
    );
    assert!(
        constraint_exists(&test_pool, "content_topics", "content_topics_source_check").await,
        "content topic source should be constrained"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "topic_generation_batches",
            "topic_generation_batches_status_check"
        )
        .await,
        "topic generation batch status should be constrained"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "video_workspace_menus",
            "video_workspace_menus_menu_key_unique"
        )
        .await,
        "video workspace menu keys should be unique"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "video_workspace_menus",
            "video_workspace_menus_status_check"
        )
        .await,
        "video workspace menu status should be constrained"
    );

    let top_level_menu_labels = sqlx::query_scalar::<_, String>(
        r#"
        SELECT label
        FROM video_workspace_menus
        WHERE parent_id IS NULL
        ORDER BY sort_order ASC
        "#,
    )
    .fetch_all(&test_pool)
    .await
    .expect("top-level menu seed query should run");
    assert_eq!(
        top_level_menu_labels,
        vec![
            "内容策略",
            "脚本创作",
            "素材管理",
            "作品生产",
            "发布运营",
            "数据分析",
            "工作流任务",
        ]
    );

    let script_creation = sqlx::query_as::<_, (bool, String)>(
        r#"
        SELECT is_enabled, status
        FROM video_workspace_menus
        WHERE menu_key = 'script-creation'
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .expect("script creation seed query should run");
    assert_eq!(script_creation, (true, "active".to_string()));

    let content_strategy = sqlx::query_as::<_, (bool, String)>(
        r#"
        SELECT is_enabled, status
        FROM video_workspace_menus
        WHERE menu_key = 'content-strategy'
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .expect("content strategy seed query should run");
    assert_eq!(content_strategy, (true, "active".to_string()));

    let topic_generator = sqlx::query_as::<_, (bool, String)>(
        r#"
        SELECT child.is_enabled, child.status
        FROM video_workspace_menus child
        JOIN video_workspace_menus parent ON parent.id = child.parent_id
        WHERE parent.menu_key = 'content-strategy'
          AND child.menu_key = 'topic-generator'
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .expect("topic generator seed query should run");
    assert_eq!(topic_generator, (true, "active".to_string()));

    let planned_top_level_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM video_workspace_menus
        WHERE parent_id IS NULL
          AND menu_key <> 'script-creation'
          AND menu_key <> 'content-strategy'
          AND is_visible = true
          AND is_enabled = false
          AND status = 'planned'
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .expect("planned menu seed query should run");
    assert_eq!(planned_top_level_count, 5);

    let script_child_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM video_workspace_menus child
        JOIN video_workspace_menus parent ON parent.id = child.parent_id
        WHERE parent.menu_key = 'script-creation'
          AND child.menu_key = 'script-generator'
          AND child.agent_key = 'script-generation-agent'
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .expect("script child menu seed query should run");
    assert_eq!(script_child_count, 1);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn runtime_postgres_connection_syncs_content_strategy_menu_state() {
    let base_url = database_url();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let database_name = format!("video_agent_runtime_menu_sync_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    create_database(&admin_pool, &database_name).await;

    let setup_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&test_url)
        .await
        .expect("temporary runtime menu sync database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&setup_pool)
        .await
        .expect("migrations should run before stale menu fixture");
    sqlx::query(
        r#"
        UPDATE video_workspace_menus
        SET is_enabled = false,
            status = 'planned'
        WHERE menu_key IN ('content-strategy', 'topic-generator')
        "#,
    )
    .execute(&setup_pool)
    .await
    .expect("stale content strategy menu fixture should update");
    setup_pool.close().await;

    let runtime_pool = novex_api::connect_runtime_pg_pool(&test_url, 1)
        .await
        .expect("runtime postgres connection should sync menu state");

    let menu_states = sqlx::query_as::<_, (String, bool, String)>(
        r#"
        SELECT menu_key, is_enabled, status
        FROM video_workspace_menus
        WHERE menu_key IN ('content-strategy', 'topic-generator')
        ORDER BY menu_key
        "#,
    )
    .fetch_all(&runtime_pool)
    .await
    .expect("content strategy menu state should be readable");
    assert_eq!(
        menu_states,
        vec![
            ("content-strategy".to_string(), true, "active".to_string()),
            ("topic-generator".to_string(), true, "active".to_string()),
        ]
    );

    runtime_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
#[ignore = "runs migrations against the configured development database"]
async fn migrations_apply_to_configured_database() {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url())
        .await
        .expect("configured database should be reachable");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations should run against configured database");

    assert!(table_exists(&pool, "projects").await);
    assert!(table_exists(&pool, "scripts").await);
    assert!(table_exists(&pool, "scenes").await);

    pool.close().await;
}
