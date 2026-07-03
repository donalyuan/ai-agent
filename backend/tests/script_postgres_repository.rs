use chrono::Utc;
use novex_api::agents::models::{Scene, Script, ScriptListFilter, ScriptStatus};
use novex_api::repositories::{PostgresScriptRepository, ScriptRepository, ScriptRepositoryError};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

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

async fn create_database(admin_pool: &PgPool, database_name: &str) {
    let query = format!(r#"CREATE DATABASE "{}""#, database_name);
    sqlx::query(&query)
        .execute(admin_pool)
        .await
        .expect("temporary repository database should be created");
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

async fn migrated_pool() -> (PgPool, PgPool, String) {
    let base_url = database_url();
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let database_name = format!("video_agent_repo_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    create_database(&admin_pool, &database_name).await;

    let test_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&test_url)
        .await
        .expect("temporary repository database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for repository test database");

    (admin_pool, test_pool, database_name)
}

async fn insert_project(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('科技博主', '科技知识账号', '脚本仓储测试项目')
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("project fixture should be inserted")
}

fn sample_script(project_id: Uuid) -> Script {
    let now = Utc::now();
    Script::new(
        Uuid::new_v4(),
        project_id,
        "程序员必看：ChatGPT工作流".to_string(),
        "还在手写重复代码？".to_string(),
        json!({"topic": "ChatGPT如何改变程序员工作流"}),
        ScriptStatus::Draft,
        None,
        vec![
            Scene {
                id: Uuid::new_v4(),
                sequence: 2,
                narration: "第二个分镜旁白。".to_string(),
                visual_description: "第二个分镜画面。".to_string(),
                emotion: "好奇".to_string(),
                duration_sec: 9,
            },
            Scene {
                id: Uuid::new_v4(),
                sequence: 1,
                narration: "第一个分镜旁白。".to_string(),
                visual_description: "第一个分镜画面。".to_string(),
                emotion: "焦虑".to_string(),
                duration_sec: 8,
            },
        ],
        now,
        now,
    )
}

#[tokio::test]
async fn postgres_script_repository_persists_and_reads_script_aggregate() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let repository = PostgresScriptRepository::new(test_pool.clone());
    let script = sample_script(project_id);

    let saved = repository.save_script(script.clone()).await.unwrap();
    assert_eq!(saved.id, script.id);
    assert_eq!(saved.scenes.len(), 2);
    assert_eq!(saved.scenes[0].sequence, 1);

    let fetched = repository.get_script(script.id).await.unwrap();
    assert_eq!(fetched.id, script.id);
    assert_eq!(fetched.project_id, project_id);
    assert_eq!(fetched.scenes.len(), 2);
    assert_eq!(fetched.total_duration_sec(), 17);

    let listed = repository
        .list_scripts(
            project_id,
            ScriptListFilter {
                status: Some(ScriptStatus::Draft),
                limit: Some(20),
                offset: Some(0),
            },
        )
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, script.id);

    let summaries = repository
        .list_script_summaries(
            project_id,
            ScriptListFilter {
                status: Some(ScriptStatus::Draft),
                limit: Some(20),
                offset: Some(0),
            },
        )
        .await
        .unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].script_id, script.id);
    assert_eq!(summaries[0].scene_count, 2);

    assert_eq!(
        repository
            .count_scripts(project_id, Some(ScriptStatus::Draft))
            .await
            .unwrap(),
        1
    );

    let approved = repository
        .update_script_status(script.id, ScriptStatus::Approved)
        .await
        .unwrap();
    assert_eq!(approved.status, ScriptStatus::Approved);

    let patched = repository
        .update_scene(
            script.id,
            Scene {
                id: saved.scenes[0].id,
                sequence: 1,
                narration:
                    "深夜上线前，程序员发现 AI 建议和错误日志互相矛盾，只能重新验证每一步判断。"
                        .to_string(),
                visual_description: "凌晨办公室里，屏幕同时显示 AI 建议、红色日志和发布倒计时。"
                    .to_string(),
                emotion: "紧张".to_string(),
                duration_sec: 10,
            },
        )
        .await
        .unwrap();
    assert_eq!(patched.scenes[0].sequence, 1);
    assert_eq!(patched.scenes[0].emotion, "紧张");
    assert_eq!(patched.scenes[0].duration_sec, 10);

    let missing = repository.get_script(Uuid::new_v4()).await.unwrap_err();
    assert!(matches!(missing, ScriptRepositoryError::NotFound(_)));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
