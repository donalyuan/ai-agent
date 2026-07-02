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
        "viral_videos",
        "content_strategies",
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

    test_pool.close().await;
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
