use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

mod support;

use support::test_database::TestDatabase;

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

async fn unique_partial_index_predicate(pool: &PgPool, index_name: &str) -> Option<String> {
    sqlx::query_as::<_, (bool, Option<String>)>(
        r#"
        SELECT index_info.indisunique,
               pg_get_expr(index_info.indpred, index_info.indrelid) AS predicate
        FROM pg_index index_info
        JOIN pg_class index_class ON index_class.oid = index_info.indexrelid
        JOIN pg_namespace namespace ON namespace.oid = index_class.relnamespace
        WHERE namespace.nspname = 'public'
          AND index_class.relname = $1
        "#,
    )
    .bind(index_name)
    .fetch_optional(pool)
    .await
    .expect("index metadata query should run")
    .and_then(|(is_unique, predicate)| if is_unique { predicate } else { None })
}

async fn column_exists(pool: &PgPool, table_name: &str, column_name: &str) -> bool {
    sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.columns
            WHERE table_schema = 'public'
              AND table_name = $1
              AND column_name = $2
        )
        "#,
    )
    .bind(table_name)
    .bind(column_name)
    .fetch_one(pool)
    .await
    .expect("column existence query should run")
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

async fn create_database(
    admin_pool: &PgPool,
    admin_url: &str,
    database_name: &str,
) -> TestDatabase {
    let query = format!(r#"CREATE DATABASE "{}""#, database_name);
    sqlx::query(&query)
        .execute(admin_pool)
        .await
        .expect("temporary migration database should be created");
    TestDatabase::new(admin_url, database_name)
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
    let suffix = Uuid::new_v4().simple().to_string();
    let database_name = format!("video_agent_migration_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");

    let database_name = create_database(&admin_pool, &admin_url, &database_name).await;

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
        "topic_quality_evaluations",
        "viral_videos",
        "content_strategies",
        "video_workspace_menus",
        "asset_generation_tasks",
        "asset_generation_task_requests",
        "scene_asset_candidates",
    ] {
        assert!(
            table_exists(&test_pool, table).await,
            "{table} table should exist"
        );
    }

    for index in [
        "idx_materials_project",
        "idx_materials_project_status_updated",
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
        "idx_topic_generation_batches_supplement_of",
        "idx_topic_quality_evaluations_project_batch_created",
        "idx_topic_quality_evaluations_source_run",
        "idx_topic_quality_evaluations_status",
        "idx_scripts_topic",
        "idx_video_workspace_menus_parent_sort",
        "idx_asset_generation_tasks_project_created",
        "idx_asset_generation_tasks_status",
        "idx_asset_generation_tasks_visible_script",
        "asset_generation_tasks_one_in_flight_image_per_scene",
        "idx_asset_generation_task_requests_task",
        "idx_scene_asset_candidates_script_scene_rank",
        "scene_asset_candidates_one_selected_per_scene",
    ] {
        assert!(
            index_exists(&test_pool, index).await,
            "{index} index should exist"
        );
    }

    assert!(
        column_exists(&test_pool, "materials", "status").await,
        "materials.status should exist"
    );
    assert!(
        column_exists(&test_pool, "asset_generation_tasks", "dismissed_at").await,
        "asset_generation_tasks.dismissed_at should preserve soft-dismiss audit state"
    );
    assert!(
        constraint_exists(&test_pool, "materials", "materials_status_check").await,
        "materials.status should be constrained"
    );
    assert!(
        constraint_exists(&test_pool, "materials", "materials_id_project_unique").await,
        "materials should expose id/project composite key for asset candidate integrity"
    );
    assert!(
        constraint_exists(&test_pool, "scripts", "scripts_id_project_unique").await,
        "scripts should expose id/project composite key for asset candidate integrity"
    );
    assert!(
        constraint_exists(&test_pool, "scenes", "scenes_id_script_unique").await,
        "scenes should expose id/script composite key for asset candidate integrity"
    );
    let selected_candidate_index_predicate =
        unique_partial_index_predicate(&test_pool, "scene_asset_candidates_one_selected_per_scene")
            .await
            .expect("selected candidate index should be unique and partial");
    assert!(
        selected_candidate_index_predicate.contains("selected"),
        "selected candidate unique index should only cover selected status, got {selected_candidate_index_predicate}"
    );
    let in_flight_scene_task_predicate = unique_partial_index_predicate(
        &test_pool,
        "asset_generation_tasks_one_in_flight_image_per_scene",
    )
    .await
    .expect("in-flight scene image task index should be unique and partial");
    assert!(
        in_flight_scene_task_predicate.contains("image_candidates")
            && in_flight_scene_task_predicate.contains("pending")
            && in_flight_scene_task_predicate.contains("processing"),
        "in-flight scene image task index should cover pending and processing image tasks, got {in_flight_scene_task_predicate}"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "asset_generation_tasks",
            "asset_generation_tasks_provider_check"
        )
        .await,
        "asset generation task provider should be constrained"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "asset_generation_tasks",
            "asset_generation_tasks_type_check"
        )
        .await,
        "asset generation task type should be constrained"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "asset_generation_tasks",
            "asset_generation_tasks_status_check"
        )
        .await,
        "asset generation task status should be constrained"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "asset_generation_tasks",
            "asset_generation_tasks_candidate_count_check"
        )
        .await,
        "asset generation task candidate count should be constrained"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "asset_generation_tasks",
            "asset_generation_tasks_retry_count_check"
        )
        .await,
        "asset generation task retry count should be constrained"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "scene_asset_candidates",
            "scene_asset_candidates_script_project_fk"
        )
        .await,
        "scene asset candidates should keep script and project consistent"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "scene_asset_candidates",
            "scene_asset_candidates_scene_script_fk"
        )
        .await,
        "scene asset candidates should keep scene and script consistent"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "scene_asset_candidates",
            "scene_asset_candidates_material_project_fk"
        )
        .await,
        "scene asset candidates should keep material and project consistent"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "scene_asset_candidates",
            "scene_asset_candidates_type_check"
        )
        .await,
        "scene asset candidate type should be constrained"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "scene_asset_candidates",
            "scene_asset_candidates_source_check"
        )
        .await,
        "scene asset candidate source should be constrained"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "scene_asset_candidates",
            "scene_asset_candidates_status_check"
        )
        .await,
        "scene asset candidate status should be constrained"
    );
    assert!(
        constraint_exists(
            &test_pool,
            "scene_asset_candidates",
            "scene_asset_candidates_rank_check"
        )
        .await,
        "scene asset candidate rank should be constrained"
    );
    let material_project_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    sqlx::query(
        r#"
        INSERT INTO projects (id, name, positioning, description)
        VALUES ($1, '测试账号', '', '')
        "#,
    )
    .bind(material_project_id)
    .execute(&test_pool)
    .await
    .expect("project fixture should be inserted");
    sqlx::query(
        r#"
        INSERT INTO materials (project_id, material_type, file_url, file_name)
        VALUES ($1, 'subtitle', 'https://cdn.example.com/subtitles/demo.vtt', 'demo.vtt')
        "#,
    )
    .bind(material_project_id)
    .execute(&test_pool)
    .await
    .expect("subtitle material should be accepted");
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
            "topic_quality_evaluations",
            "topic_quality_evaluations_status_check"
        )
        .await,
        "topic quality evaluation status should be constrained"
    );
    assert!(
        column_exists(
            &test_pool,
            "topic_generation_batches",
            "supplement_of_batch_id"
        )
        .await,
        "topic generation batches should expose supplement_of_batch_id"
    );
    assert!(
        column_exists(&test_pool, "projects", "strategy_profile").await,
        "projects should expose strategy_profile for account strategy context"
    );
    let strategy_profile_column = sqlx::query_as::<_, (String, String, String)>(
        r#"
        SELECT data_type, is_nullable, column_default
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'projects'
          AND column_name = 'strategy_profile'
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .expect("projects.strategy_profile column metadata should be readable");
    assert_eq!(
        strategy_profile_column,
        (
            "jsonb".to_string(),
            "NO".to_string(),
            "'{}'::jsonb".to_string()
        )
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

    let material_management = sqlx::query_as::<_, (bool, String)>(
        r#"
        SELECT is_enabled, status
        FROM video_workspace_menus
        WHERE menu_key = 'material-management'
        "#,
    )
    .fetch_one(&test_pool)
    .await
    .expect("material management seed query should run");
    assert_eq!(material_management, (true, "active".to_string()));

    let material_children = sqlx::query_as::<_, (String, String, bool, String, String)>(
        r#"
        SELECT child.menu_key, child.label, child.is_enabled, child.status, child.module_key
        FROM video_workspace_menus child
        JOIN video_workspace_menus parent ON parent.id = child.parent_id
        WHERE parent.menu_key = 'material-management'
          AND child.menu_key IN ('material-library', 'asset-generation')
        ORDER BY child.sort_order ASC
        "#,
    )
    .fetch_all(&test_pool)
    .await
    .expect("material child menu seed query should run");
    assert_eq!(
        material_children,
        vec![
            (
                "material-library".to_string(),
                "素材库".to_string(),
                true,
                "active".to_string(),
                "materials.library".to_string(),
            ),
            (
                "asset-generation".to_string(),
                "素材生成".to_string(),
                true,
                "active".to_string(),
                "materials.asset-generation".to_string(),
            ),
        ]
    );

    let content_strategy_children = sqlx::query_as::<_, (String, String, bool, String)>(
        r#"
        SELECT child.menu_key, child.label, child.is_enabled, child.status
        FROM video_workspace_menus child
        JOIN video_workspace_menus parent ON parent.id = child.parent_id
        WHERE parent.menu_key = 'content-strategy'
          AND child.menu_key IN ('account-strategy', 'topic-history', 'topic-generator')
        ORDER BY child.sort_order ASC
        "#,
    )
    .fetch_all(&test_pool)
    .await
    .expect("content strategy child menu seed query should run");
    assert_eq!(
        content_strategy_children,
        vec![
            (
                "account-strategy".to_string(),
                "账号策略".to_string(),
                true,
                "active".to_string()
            ),
            (
                "topic-history".to_string(),
                "历史生成".to_string(),
                true,
                "active".to_string()
            ),
            (
                "topic-generator".to_string(),
                "当前选题池".to_string(),
                true,
                "active".to_string()
            ),
        ]
    );

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
    assert_eq!(planned_top_level_count, 4);

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
async fn runtime_postgres_connection_applies_pending_migrations_before_serving() {
    let base_url = database_url();
    let suffix = Uuid::new_v4().simple().to_string();
    let database_name = format!("video_agent_runtime_migration_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    let database_name = create_database(&admin_pool, &admin_url, &database_name).await;

    let runtime_pool = novex_api::connect_runtime_pg_pool(&test_url, 1)
        .await
        .expect("runtime postgres connection should apply pending migrations");

    assert!(table_exists(&runtime_pool, "asset_generation_tasks").await);
    assert!(
        column_exists(&runtime_pool, "asset_generation_tasks", "dismissed_at").await,
        "runtime connection should apply the failed-task dismissal migration"
    );
    let dismissal_migration_applied = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM _sqlx_migrations WHERE version = 20260710030000 AND success)",
    )
    .fetch_one(&runtime_pool)
    .await
    .expect("runtime migration history should be readable");
    assert!(dismissal_migration_applied);

    runtime_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn runtime_postgres_connection_syncs_content_strategy_menu_state() {
    let base_url = database_url();
    let suffix = Uuid::new_v4().simple().to_string();
    let database_name = format!("video_agent_runtime_menu_sync_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    let database_name = create_database(&admin_pool, &admin_url, &database_name).await;

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
        WHERE menu_key IN ('content-strategy', 'account-strategy', 'topic-history', 'topic-generator')
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
        WHERE menu_key IN ('content-strategy', 'account-strategy', 'topic-history', 'topic-generator')
        ORDER BY CASE menu_key
            WHEN 'content-strategy' THEN 1
            WHEN 'account-strategy' THEN 2
            WHEN 'topic-history' THEN 3
            WHEN 'topic-generator' THEN 4
            ELSE 4
        END
        "#,
    )
    .fetch_all(&runtime_pool)
    .await
    .expect("content strategy menu state should be readable");
    assert_eq!(
        menu_states,
        vec![
            ("content-strategy".to_string(), true, "active".to_string()),
            ("account-strategy".to_string(), true, "active".to_string()),
            ("topic-history".to_string(), true, "active".to_string()),
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
