use novex_api::agents::conversation::{
    AgentConversationStatus, AgentMessageRole, CreateAgentConversationInput,
    CreateAgentMessageInput, CreateAgentRunInput, CreateAgentStepInput, FinishAgentRunInput,
};
use novex_api::repositories::{ConversationRepository, PostgresConversationRepository};
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
        .expect("temporary conversation database should be created");
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
    let database_name = format!("video_agent_conversation_repo_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    create_database(&admin_pool, &database_name).await;

    let test_pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&test_url)
        .await
        .expect("temporary conversation database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for conversation repository test database");

    (admin_pool, test_pool, database_name)
}

async fn insert_project(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('科技博主', '科技知识账号', '对话仓储测试项目')
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("project fixture should be inserted")
}

#[tokio::test]
async fn conversation_repository_persists_conversations_messages_runs_and_steps() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let script_id = Uuid::new_v4();
    let repository = PostgresConversationRepository::new(test_pool.clone());

    let conversation = repository
        .create_conversation(CreateAgentConversationInput {
            project_id: Some(project_id),
            agent_type: "script".to_string(),
            subject_type: Some("script".to_string()),
            subject_id: Some(script_id),
            title: "脚本改稿".to_string(),
            metadata: json!({"skill_keys": ["script.rewrite_scene"]}),
        })
        .await
        .unwrap();

    assert_eq!(conversation.project_id, Some(project_id));
    assert_eq!(conversation.subject_id, Some(script_id));
    assert_eq!(conversation.status, AgentConversationStatus::Active);

    let user_message = repository
        .save_message(CreateAgentMessageInput {
            conversation_id: conversation.id,
            role: AgentMessageRole::User,
            content: "把第 1 镜改得更有冲突感".to_string(),
            metadata: json!({"source": "test"}),
        })
        .await
        .unwrap();
    let assistant_message = repository
        .save_message(CreateAgentMessageInput {
            conversation_id: conversation.id,
            role: AgentMessageRole::Assistant,
            content: "已修改第 1 镜".to_string(),
            metadata: json!({"scene_sequence": 1}),
        })
        .await
        .unwrap();

    let messages = repository
        .list_messages(conversation.id)
        .await
        .expect("messages should be listed");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].id, user_message.id);
    assert_eq!(messages[0].role, AgentMessageRole::User);
    assert_eq!(messages[1].id, assistant_message.id);
    assert_eq!(messages[1].metadata["scene_sequence"], 1);

    let run = repository
        .create_run(CreateAgentRunInput {
            conversation_id: conversation.id,
            project_id: Some(project_id),
            agent_type: "script".to_string(),
            input: json!({"user_message_id": user_message.id}),
        })
        .await
        .unwrap();
    assert_eq!(run.status, "running");

    repository
        .add_step(CreateAgentStepInput {
            agent_run_id: run.id,
            step_order: 1,
            step_type: "read_script".to_string(),
            status: "succeeded".to_string(),
            input: json!({"script_id": script_id}),
            output: Some(json!({"scene_count": 1})),
            error_message: None,
        })
        .await
        .unwrap();

    let finished = repository
        .finish_run(FinishAgentRunInput {
            agent_run_id: run.id,
            status: "succeeded".to_string(),
            output: Some(json!({"assistant_message_id": assistant_message.id})),
            error_message: None,
        })
        .await
        .unwrap();
    assert_eq!(finished.status, "succeeded");
    assert!(finished.ended_at.is_some());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
