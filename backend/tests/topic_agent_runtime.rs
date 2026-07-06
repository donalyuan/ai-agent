use async_trait::async_trait;
use novex_api::agents::conversation::{AgentMessageRole, CreateAgentConversationInput};
use novex_api::agents::conversational_runtime::{AgentRuntime, AgentTurnRequest};
use novex_api::agents::models::{ContentTopicFilter, ContentTopicSource, ContentTopicStatus};
use novex_api::agents::{LLMClient, LLMError};
use novex_api::repositories::{
    ConversationRepository, PostgresConversationRepository, PostgresProjectRepository,
    PostgresScriptRepository, PostgresTopicRepository, TopicRepository,
};
use novex_model::LLMPrompt;
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::{Arc, Mutex};
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
        .expect("temporary topic agent database should be created");
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
    let database_name = format!("video_agent_topic_agent_test_{}", suffix);
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
        .expect("temporary topic agent database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for topic agent test database");

    (admin_pool, test_pool, database_name)
}

async fn insert_project(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('科技博主', 'AI 工具和内容生产效率', '面向内容运营负责人的科技知识账号')
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("project fixture should be inserted")
}

struct ScriptedLLMClient {
    responses: Mutex<Vec<Result<String, LLMError>>>,
    prompts: Mutex<Vec<LLMPrompt>>,
}

impl ScriptedLLMClient {
    fn returning(response: serde_json::Value) -> Self {
        Self::from_result(Ok(response.to_string()))
    }

    fn returning_raw(response: &str) -> Self {
        Self::from_result(Ok(response.to_string()))
    }

    fn failing(error: LLMError) -> Self {
        Self::from_result(Err(error))
    }

    fn from_result(response: Result<String, LLMError>) -> Self {
        Self {
            responses: Mutex::new(vec![response]),
            prompts: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl LLMClient for ScriptedLLMClient {
    async fn generate_script(&self, prompt: LLMPrompt) -> Result<String, LLMError> {
        self.prompts.lock().unwrap().push(prompt);
        self.responses
            .lock()
            .unwrap()
            .pop()
            .expect("scripted topic LLM response should exist")
    }
}

fn build_topic_runtime(
    pool: PgPool,
    llm_client: Arc<ScriptedLLMClient>,
) -> (
    AgentRuntime,
    Arc<PostgresConversationRepository>,
    Arc<PostgresTopicRepository>,
) {
    let conversation_repository = Arc::new(PostgresConversationRepository::new(pool.clone()));
    let script_repository = Arc::new(PostgresScriptRepository::new(pool.clone()));
    let project_repository = Arc::new(PostgresProjectRepository::new(pool.clone()));
    let topic_repository = Arc::new(PostgresTopicRepository::new(pool));
    let runtime = AgentRuntime::new(
        conversation_repository.clone(),
        script_repository,
        project_repository,
        llm_client,
    )
    .with_topic_repository(topic_repository.clone());

    (runtime, conversation_repository, topic_repository)
}

#[tokio::test]
async fn topic_agent_generates_topics_persists_batch_and_records_steps() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let llm_client = Arc::new(ScriptedLLMClient::returning(json!({
        "topics": [
            {
                "title": "AI 工具如何改变选题会",
                "angle": "对比传统选题会和 AI 辅助选题",
                "target_audience": "内容运营负责人",
                "hook_points": ["三分钟生成候选", "人工评估更聚焦"],
                "content_type": "knowledge",
                "score": 92,
                "score_reason": "贴合账号定位且容易脚本化",
                "tags": ["AI工具", "选题"]
            },
            {
                "title": "内容团队为什么需要 AI 工作流",
                "angle": "从协作和质量控制解释工作流改造",
                "target_audience": "中小内容团队",
                "hook_points": ["减少重复劳动", "保留人工判断"],
                "content_type": "knowledge",
                "score": 89,
                "score_reason": "受众明确，适合科普",
                "tags": ["内容运营", "工作流"]
            }
        ]
    })));
    let (runtime, conversation_repository, topic_repository) =
        build_topic_runtime(test_pool.clone(), llm_client.clone());
    let conversation = conversation_repository
        .create_conversation(CreateAgentConversationInput {
            project_id: Some(project_id),
            agent_type: "topic".to_string(),
            subject_type: None,
            subject_id: None,
            title: "选题生成".to_string(),
            metadata: json!({}),
        })
        .await
        .unwrap();

    let response = runtime
        .handle_turn(AgentTurnRequest {
            conversation_id: conversation.id,
            user_message: "本周 AI 工具方向，生成 2 个选题".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(response.agent_message.role, AgentMessageRole::Assistant);
    assert_eq!(response.agent_message.metadata["topic_count"], 2);
    assert!(response.agent_message.metadata["batch_id"].is_string());
    assert_eq!(response.run.status, "succeeded");

    let topics = topic_repository
        .list_topics(
            project_id,
            ContentTopicFilter {
                status: Some(ContentTopicStatus::Idea),
                source: Some(ContentTopicSource::Agent),
                batch_id: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(topics.len(), 2);
    assert!(topics
        .iter()
        .all(|topic| topic.batch_id.is_some() && topic.status == ContentTopicStatus::Idea));

    let step_types = sqlx::query_scalar::<_, String>(
        r#"
        SELECT step_type
        FROM agent_steps
        WHERE agent_run_id = $1
        ORDER BY step_order ASC
        "#,
    )
    .bind(response.run.id)
    .fetch_all(&test_pool)
    .await
    .unwrap();
    assert_eq!(
        step_types,
        vec!["read_project_context", "generate_topics", "persist_topics"]
    );

    {
        let prompts = llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 1);
        assert!(prompts[0].user.contains("AI 工具和内容生产效率"));
        assert!(prompts[0].user.contains("生成 2 个选题"));
        assert!(prompts[0].user.contains("只输出一个 JSON 对象"));
        let schema = prompts[0]
            .output_schema
            .as_ref()
            .expect("topic agent prompt should request structured output");
        assert_eq!(schema.name, "topic_generation_batch");
        assert_eq!(schema.schema["required"], json!(["topics"]));
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_agent_rejects_invalid_llm_output_without_partial_topics() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let llm_client = Arc::new(ScriptedLLMClient::returning(json!({
        "topics": [
            {
                "title": "缺少 score 的选题",
                "angle": "字段不完整",
                "target_audience": "内容运营负责人",
                "hook_points": ["字段缺失"],
                "content_type": "knowledge",
                "score_reason": "缺少评分",
                "tags": ["AI工具"]
            }
        ]
    })));
    let (runtime, conversation_repository, topic_repository) =
        build_topic_runtime(test_pool.clone(), llm_client);
    let conversation = conversation_repository
        .create_conversation(CreateAgentConversationInput {
            project_id: Some(project_id),
            agent_type: "topic".to_string(),
            subject_type: None,
            subject_id: None,
            title: "选题生成".to_string(),
            metadata: json!({}),
        })
        .await
        .unwrap();

    let error = runtime
        .handle_turn(AgentTurnRequest {
            conversation_id: conversation.id,
            user_message: "生成 1 个 AI 工具方向选题".to_string(),
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("score is required"));

    let topics = topic_repository
        .list_topics(project_id, ContentTopicFilter::default())
        .await
        .unwrap();
    assert!(topics.is_empty());

    let failed_run_status = sqlx::query_scalar::<_, String>(
        r#"
        SELECT status
        FROM agent_runs
        WHERE input->>'conversation_id' = $1
        ORDER BY started_at DESC
        LIMIT 1
        "#,
    )
    .bind(conversation.id.to_string())
    .fetch_one(&test_pool)
    .await
    .unwrap();
    assert_eq!(failed_run_status, "failed");

    let failed_batch_status = sqlx::query_scalar::<_, String>(
        r#"
        SELECT status
        FROM topic_generation_batches
        WHERE project_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(project_id)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    assert_eq!(failed_batch_status, "failed");

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_agent_rejects_empty_invalid_out_of_range_and_llm_failure_outputs() {
    let cases: Vec<(&str, ScriptedLLMClient, &str)> = vec![
        (
            "empty",
            ScriptedLLMClient::returning(json!({ "topics": [] })),
            "topic output must not be empty",
        ),
        (
            "string_array",
            ScriptedLLMClient::returning(json!(["只返回标题的错误输出"])),
            "missing JSON object start",
        ),
        (
            "invalid_json",
            ScriptedLLMClient::returning_raw("not json"),
            "missing JSON object start",
        ),
        (
            "score_out_of_range",
            ScriptedLLMClient::returning(json!({
                "topics": [
                    {
                        "title": "AI 工具选题",
                        "angle": "解释 AI 工具",
                        "target_audience": "内容运营负责人",
                        "hook_points": ["提效"],
                        "content_type": "knowledge",
                        "score": 101,
                        "score_reason": "越界评分",
                        "tags": ["AI工具"]
                    }
                ]
            })),
            "score must be between 0 and 100",
        ),
        (
            "llm_timeout",
            ScriptedLLMClient::failing(LLMError::Timeout),
            "llm request timeout",
        ),
    ];

    for (label, llm_client, expected_error) in cases {
        let (admin_pool, test_pool, database_name) = migrated_pool().await;
        let project_id = insert_project(&test_pool).await;
        let (runtime, conversation_repository, topic_repository) =
            build_topic_runtime(test_pool.clone(), Arc::new(llm_client));
        let conversation = conversation_repository
            .create_conversation(CreateAgentConversationInput {
                project_id: Some(project_id),
                agent_type: "topic".to_string(),
                subject_type: None,
                subject_id: None,
                title: format!("选题生成 {label}"),
                metadata: json!({}),
            })
            .await
            .unwrap();

        let error = runtime
            .handle_turn(AgentTurnRequest {
                conversation_id: conversation.id,
                user_message: "生成 1 个 AI 工具方向选题".to_string(),
            })
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains(expected_error),
            "{label} should contain {expected_error}, got {error}"
        );

        let topics = topic_repository
            .list_topics(project_id, ContentTopicFilter::default())
            .await
            .unwrap();
        assert!(
            topics.is_empty(),
            "{label} should not persist partial topics"
        );

        let failed_batch_status = sqlx::query_scalar::<_, String>(
            r#"
            SELECT status
            FROM topic_generation_batches
            WHERE project_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(project_id)
        .fetch_one(&test_pool)
        .await
        .unwrap();
        assert_eq!(failed_batch_status, "failed");

        test_pool.close().await;
        drop_database(&admin_pool, &database_name).await;
        admin_pool.close().await;
    }
}
