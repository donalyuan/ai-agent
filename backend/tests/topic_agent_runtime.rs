use async_trait::async_trait;
use novex_api::agents::conversation::{
    AgentMessageRole, CreateAgentConversationInput, CreateAgentMessageInput,
};
use novex_api::agents::conversational_runtime::{
    AgentRuntime, AgentRuntimeError, AgentTurnRequest,
};
use novex_api::agents::models::{
    ContentTopicFilter, ContentTopicSource, ContentTopicStatus, TopicGenerationBatchStatus,
    TopicQualityEvaluationStatus, TopicReviewPriority, TopicReviewRiskFlag,
};
use novex_api::agents::{LLMClient, LLMError};
use novex_api::repositories::{
    ConversationRepository, CreateContentTopicInput, CreateTopicGenerationBatchInput,
    PostgresConversationRepository, PostgresProjectRepository, PostgresScriptRepository,
    PostgresTopicRepository, TopicRepository,
};
use novex_model::LLMPrompt;
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::{Arc, Mutex};
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

async fn create_database(
    admin_pool: &PgPool,
    admin_url: &str,
    database_name: &str,
) -> TestDatabase {
    let query = format!(r#"CREATE DATABASE "{}""#, database_name);
    sqlx::query(&query)
        .execute(admin_pool)
        .await
        .expect("temporary topic agent database should be created");
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

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let suffix = Uuid::new_v4().simple().to_string();
    let database_name = format!("video_agent_topic_agent_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    let database_name = create_database(&admin_pool, &admin_url, &database_name).await;

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

async fn insert_project_with_strategy_profile(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description, strategy_profile)
        VALUES (
            'AI 工具账号',
            'AI 工具和内容生产效率',
            '面向内容运营负责人的科技知识账号',
            $1
        )
        RETURNING id
        "#,
    )
    .bind(json!({
        "target_audience": "中小内容团队负责人",
        "content_pillars": ["AI 工具教程", "内容生产案例"],
        "tone_style": "直接清晰，少术语",
        "forbidden_topics": ["夸大收益", "灰产引流"],
        "reference_accounts": ["参考账号A"],
        "topic_preferences": "优先 60 秒内可讲清楚步骤的教程选题"
    }))
    .fetch_one(pool)
    .await
    .expect("project fixture with strategy profile should be inserted")
}

async fn latest_topic_generation_batch_id(pool: &PgPool, project_id: Uuid) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        SELECT id
        FROM topic_generation_batches
        WHERE project_id = $1
        ORDER BY created_at DESC, id DESC
        LIMIT 1
        "#,
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .expect("latest topic generation batch should exist")
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

    fn from_results(responses: Vec<Result<String, LLMError>>) -> Self {
        Self {
            responses: Mutex::new(responses),
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

fn topic_input(project_id: Uuid, batch_id: Uuid, title: &str) -> CreateContentTopicInput {
    CreateContentTopicInput {
        project_id,
        batch_id: Some(batch_id),
        title: title.to_string(),
        angle: "从内容生产流程角度解释 AI 工具落地".to_string(),
        target_audience: "内容运营负责人".to_string(),
        hook_points: vec!["低成本提效".to_string()],
        content_type: "knowledge".to_string(),
        score: Some(86.0),
        score_reason: "选题贴近当前项目定位".to_string(),
        tags: vec!["AI工具".to_string()],
        source: ContentTopicSource::Agent,
        metadata: json!({}),
    }
}

#[tokio::test]
async fn account_strategy_context_is_injected_into_topic_generation_and_quality_prompts() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project_with_strategy_profile(&test_pool).await;
    let generation = json!({
        "topics": [
            {
                "title": "AI 工具如何改变选题会",
                "angle": "用一个团队会议场景说明 AI 如何缩短选题周期",
                "target_audience": "中小内容团队负责人",
                "hook_points": ["减少重复劳动", "保留人工判断"],
                "content_type": "knowledge",
                "score": 90,
                "score_reason": "贴合账号策略资料中的目标受众和教程偏好",
                "tags": ["AI工具", "内容生产"]
            }
        ]
    });
    let quality = json!({
        "summary": "本批 1 条通过。",
        "items": [
            {
                "candidate_key": "candidate-1",
                "title": "AI 工具如何改变选题会",
                "decision": "pass",
                "quality_score": 90,
                "flags": [],
                "reason": "贴合账号策略资料，未触碰禁区。"
            }
        ]
    });
    let llm_client = Arc::new(ScriptedLLMClient::from_results(vec![
        Ok(quality.to_string()),
        Ok(generation.to_string()),
    ]));
    let (runtime, conversation_repository, _topic_repository) =
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

    runtime
        .handle_turn(AgentTurnRequest {
            conversation_id: conversation.id,
            user_message: "生成 1 个 AI 工具教程选题".to_string(),
            supplement_of_batch_id: None,
        })
        .await
        .unwrap();

    {
        let prompts = llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2);
        for prompt in [&prompts[0].user, &prompts[1].user] {
            assert!(prompt.contains("账号策略资料"));
            assert!(prompt.contains("中小内容团队负责人"));
            assert!(prompt.contains("AI 工具教程"));
            assert!(prompt.contains("内容生产案例"));
            assert!(prompt.contains("直接清晰，少术语"));
            assert!(prompt.contains("夸大收益"));
            assert!(prompt.contains("灰产引流"));
            assert!(prompt.contains("参考账号A"));
            assert!(prompt.contains("优先 60 秒内可讲清楚步骤的教程选题"));
        }
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn account_strategy_context_is_injected_into_topic_group_review_prompt() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project_with_strategy_profile(&test_pool).await;
    let llm_client = Arc::new(ScriptedLLMClient::returning_raw("{}"));
    let (_runtime, _conversation_repository, topic_repository) =
        build_topic_runtime(test_pool.clone(), llm_client);
    let original_batch = topic_repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id: None,
            prompt: "原始生成 AI 工具方向选题".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    let original_topic = topic_repository
        .create_topic(topic_input(
            project_id,
            original_batch.id,
            "AI 工具教程选题",
        ))
        .await
        .unwrap();
    let review_llm_client = Arc::new(ScriptedLLMClient::returning(json!({
        "review_summary": "优先推进贴合账号策略的教程选题。",
        "topic_reviews": [
            {
                "topic_id": original_topic.id,
                "priority": "priority",
                "reason": "符合目标受众和教程偏好。",
                "risk_flags": [],
                "similar_topic_ids": []
            }
        ]
    })));
    let (runtime, _conversation_repository, _topic_repository) =
        build_topic_runtime(test_pool.clone(), review_llm_client.clone());

    runtime
        .review_topic_group(project_id, original_batch.id)
        .await
        .unwrap();

    {
        let prompts = review_llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 1);
        let prompt = &prompts[0].user;
        assert!(prompt.contains("账号策略资料"));
        assert!(prompt.contains("中小内容团队负责人"));
        assert!(prompt.contains("AI 工具教程"));
        assert!(prompt.contains("内容生产案例"));
        assert!(prompt.contains("直接清晰，少术语"));
        assert!(prompt.contains("夸大收益"));
        assert!(prompt.contains("灰产引流"));
        assert!(prompt.contains("参考账号A"));
        assert!(prompt.contains("优先 60 秒内可讲清楚步骤的教程选题"));
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_group_review_persists_snapshot_records_steps_and_preserves_topic_status() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let (_runtime, _conversation_repository, topic_repository) = build_topic_runtime(
        test_pool.clone(),
        Arc::new(ScriptedLLMClient::returning_raw("{}")),
    );
    let original_batch = topic_repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id: None,
            prompt: "原始生成 AI 工具方向选题".to_string(),
            requested_count: 2,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    let original_topic = topic_repository
        .create_topic(topic_input(project_id, original_batch.id, "原始批次选题"))
        .await
        .unwrap();
    topic_repository
        .update_topic_status(original_topic.id, ContentTopicStatus::Approved)
        .await
        .unwrap();
    let supplement_batch = topic_repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id: Some(original_batch.id),
            prompt: "补充生成小团队案例".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    let supplement_topic = topic_repository
        .create_topic(topic_input(project_id, supplement_batch.id, "补充批次选题"))
        .await
        .unwrap();
    let llm_client = Arc::new(ScriptedLLMClient::returning(json!({
        "review_summary": "优先推进能直接脚本化的工具落地选题。",
        "topic_reviews": [
            {
                "topic_id": original_topic.id,
                "priority": "priority",
                "reason": "账号匹配度高，脚本化路径清晰。",
                "risk_flags": ["duplicate"],
                "similar_topic_ids": [supplement_topic.id]
            },
            {
                "topic_id": supplement_topic.id,
                "priority": "backup",
                "reason": "可作为同主题后续补充。",
                "risk_flags": ["hard_to_script"],
                "similar_topic_ids": [original_topic.id]
            }
        ]
    })));
    let (runtime, _conversation_repository, topic_repository) =
        build_topic_runtime(test_pool.clone(), llm_client.clone());

    let snapshot = runtime
        .review_topic_group(project_id, original_batch.id)
        .await
        .unwrap();

    assert_eq!(snapshot.project_id, project_id);
    assert_eq!(snapshot.root_batch_id, original_batch.id);
    assert!(snapshot.source_run_id.is_some());
    assert_eq!(
        snapshot.review_summary,
        "优先推进能直接脚本化的工具落地选题。"
    );
    assert_eq!(snapshot.result.topic_reviews.len(), 2);
    assert_eq!(
        snapshot.result.topic_reviews[0].priority,
        TopicReviewPriority::Priority
    );
    assert_eq!(
        snapshot.result.topic_reviews[0].risk_flags,
        vec![TopicReviewRiskFlag::Duplicate]
    );

    let latest = topic_repository
        .get_latest_topic_review_snapshot(project_id, original_batch.id)
        .await
        .unwrap()
        .expect("latest topic review snapshot should be saved");
    assert_eq!(latest.id, snapshot.id);

    let original_after = topic_repository.get_topic(original_topic.id).await.unwrap();
    let supplement_after = topic_repository
        .get_topic(supplement_topic.id)
        .await
        .unwrap();
    assert_eq!(original_after.status, ContentTopicStatus::Approved);
    assert_eq!(supplement_after.status, ContentTopicStatus::Idea);

    let step_types = sqlx::query_scalar::<_, String>(
        r#"
        SELECT step_type
        FROM agent_steps
        WHERE agent_run_id = $1
        ORDER BY step_order ASC
        "#,
    )
    .bind(snapshot.source_run_id.unwrap())
    .fetch_all(&test_pool)
    .await
    .unwrap();
    assert_eq!(
        step_types,
        vec![
            "read_topic_group",
            "review_topic_group",
            "persist_topic_review_snapshot"
        ]
    );

    {
        let prompts = llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 1);
        let prompt = &prompts[0].user;
        assert!(prompt.contains("AI 工具和内容生产效率"));
        assert!(prompt.contains("原始生成 AI 工具方向选题"));
        assert!(prompt.contains("原始批次选题"));
        assert!(prompt.contains("补充批次选题"));
        assert!(prompt.contains("原始生成"));
        assert!(prompt.contains("补充生成"));
        assert!(prompt.contains("只作为决策辅助"));
        let schema = prompts[0]
            .output_schema
            .as_ref()
            .expect("topic review should request structured output");
        assert_eq!(schema.name, "topic_group_review");
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_group_review_rejects_invalid_output_without_changing_topic_status() {
    let cases = vec![
        (
            "external_topic",
            json!({
                "review_summary": "包含组外选题",
                "topic_reviews": [
                    {
                        "topic_id": Uuid::new_v4(),
                        "priority": "priority",
                        "reason": "错误引用",
                        "risk_flags": [],
                        "similar_topic_ids": []
                    }
                ]
            }),
            "topic_id must belong to current topic group",
        ),
        (
            "invalid_priority",
            json!({
                "review_summary": "非法优先级",
                "topic_reviews": [
                    {
                        "topic_id": Uuid::nil(),
                        "priority": "must_do",
                        "reason": "非法 priority",
                        "risk_flags": [],
                        "similar_topic_ids": []
                    }
                ]
            }),
            "unknown variant",
        ),
        (
            "invalid_risk_flag",
            json!({
                "review_summary": "非法风险",
                "topic_reviews": [
                    {
                        "topic_id": Uuid::nil(),
                        "priority": "backup",
                        "reason": "非法 risk flag",
                        "risk_flags": ["not_a_risk"],
                        "similar_topic_ids": []
                    }
                ]
            }),
            "unknown variant",
        ),
    ];

    for (label, mut payload, expected_error) in cases {
        let (admin_pool, test_pool, database_name) = migrated_pool().await;
        let project_id = insert_project(&test_pool).await;
        let original_batch = {
            let repository = PostgresTopicRepository::new(test_pool.clone());
            repository
                .create_generation_batch(CreateTopicGenerationBatchInput {
                    project_id,
                    source_run_id: None,
                    supplement_of_batch_id: None,
                    prompt: format!("{label}: 原始生成 AI 工具方向选题"),
                    requested_count: 1,
                    status: TopicGenerationBatchStatus::Succeeded,
                    error_message: None,
                    metadata: json!({}),
                })
                .await
                .unwrap()
        };
        let topic_repository = Arc::new(PostgresTopicRepository::new(test_pool.clone()));
        let topic = topic_repository
            .create_topic(topic_input(project_id, original_batch.id, "待评审选题"))
            .await
            .unwrap();
        topic_repository
            .update_topic_status(topic.id, ContentTopicStatus::Approved)
            .await
            .unwrap();

        if label != "external_topic" {
            payload["topic_reviews"][0]["topic_id"] = json!(topic.id);
        }
        let llm_client = Arc::new(ScriptedLLMClient::returning(payload));
        let (runtime, _conversation_repository, topic_repository) =
            build_topic_runtime(test_pool.clone(), llm_client);

        let error = runtime
            .review_topic_group(project_id, original_batch.id)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains(expected_error),
            "{label} should contain {expected_error}, got {error}"
        );
        assert!(matches!(error, AgentRuntimeError::InvalidLlmOutput(_)));

        let latest = topic_repository
            .get_latest_topic_review_snapshot(project_id, original_batch.id)
            .await
            .unwrap();
        assert!(latest.is_none(), "{label} should not save succeeded review");
        let topic_after = topic_repository.get_topic(topic.id).await.unwrap();
        assert_eq!(topic_after.status, ContentTopicStatus::Approved);

        test_pool.close().await;
        drop_database(&admin_pool, &database_name).await;
        admin_pool.close().await;
    }
}

#[tokio::test]
async fn topic_group_review_rejects_invalid_json_missing_fields_and_llm_failure() {
    enum FailureKind {
        Raw(&'static str),
        EmptyReviews,
        MissingReason,
        LlmTimeout,
    }

    let cases = vec![
        (
            "invalid_json",
            FailureKind::Raw("not json"),
            "missing JSON object start",
        ),
        (
            "empty_reviews",
            FailureKind::EmptyReviews,
            "topic_reviews must not be empty",
        ),
        (
            "missing_reason",
            FailureKind::MissingReason,
            "missing field `reason`",
        ),
        (
            "llm_timeout",
            FailureKind::LlmTimeout,
            "llm request timeout",
        ),
    ];

    for (label, kind, expected_error) in cases {
        let (admin_pool, test_pool, database_name) = migrated_pool().await;
        let project_id = insert_project(&test_pool).await;
        let topic_repository = Arc::new(PostgresTopicRepository::new(test_pool.clone()));
        let original_batch = topic_repository
            .create_generation_batch(CreateTopicGenerationBatchInput {
                project_id,
                source_run_id: None,
                supplement_of_batch_id: None,
                prompt: format!("{label}: 原始生成 AI 工具方向选题"),
                requested_count: 1,
                status: TopicGenerationBatchStatus::Succeeded,
                error_message: None,
                metadata: json!({}),
            })
            .await
            .unwrap();
        let topic = topic_repository
            .create_topic(topic_input(project_id, original_batch.id, "待评审选题"))
            .await
            .unwrap();
        topic_repository
            .update_topic_status(topic.id, ContentTopicStatus::Approved)
            .await
            .unwrap();

        let llm_client = match kind {
            FailureKind::Raw(raw) => ScriptedLLMClient::returning_raw(raw),
            FailureKind::EmptyReviews => ScriptedLLMClient::returning(json!({
                "review_summary": "无结果",
                "topic_reviews": []
            })),
            FailureKind::MissingReason => ScriptedLLMClient::returning(json!({
                "review_summary": "缺字段",
                "topic_reviews": [
                    {
                        "topic_id": topic.id,
                        "priority": "backup",
                        "risk_flags": [],
                        "similar_topic_ids": []
                    }
                ]
            })),
            FailureKind::LlmTimeout => ScriptedLLMClient::failing(LLMError::Timeout),
        };
        let (runtime, _conversation_repository, topic_repository) =
            build_topic_runtime(test_pool.clone(), Arc::new(llm_client));

        let error = runtime
            .review_topic_group(project_id, original_batch.id)
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains(expected_error),
            "{label} should contain {expected_error}, got {error}"
        );

        let latest = topic_repository
            .get_latest_topic_review_snapshot(project_id, original_batch.id)
            .await
            .unwrap();
        assert!(latest.is_none(), "{label} should not save succeeded review");
        let topic_after = topic_repository.get_topic(topic.id).await.unwrap();
        assert_eq!(topic_after.status, ContentTopicStatus::Approved);

        test_pool.close().await;
        drop_database(&admin_pool, &database_name).await;
        admin_pool.close().await;
    }
}

#[tokio::test]
async fn topic_agent_generates_topics_persists_batch_and_records_steps() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let generation = json!({
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
    });
    let quality = json!({
        "summary": "本批 2 条全部通过。",
        "items": [
            {
                "candidate_key": "candidate-1",
                "title": "AI 工具如何改变选题会",
                "decision": "pass",
                "quality_score": 90,
                "flags": [],
                "reason": "贴合账号定位且脚本化路径清晰。"
            },
            {
                "candidate_key": "candidate-2",
                "title": "内容团队为什么需要 AI 工作流",
                "decision": "pass",
                "quality_score": 87,
                "flags": [],
                "reason": "受众明确，适合科普短视频。"
            }
        ]
    });
    let llm_client = Arc::new(ScriptedLLMClient::from_results(vec![
        Ok(quality.to_string()),
        Ok(generation.to_string()),
    ]));
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
            supplement_of_batch_id: None,
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
        vec![
            "read_project_context",
            "generate_topics",
            "evaluate_topic_quality",
            "persist_topics"
        ]
    );

    {
        let prompts = llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2);
        assert!(prompts[0].user.contains("AI 工具和内容生产效率"));
        assert!(prompts[0].user.contains("生成 2 个选题"));
        assert!(prompts[0].user.contains("只输出一个 JSON 对象"));
        let schema = prompts[0]
            .output_schema
            .as_ref()
            .expect("topic agent prompt should request structured output");
        assert_eq!(schema.name, "topic_generation_batch");
        assert_eq!(schema.schema["required"], json!(["topics"]));
        let quality_schema = prompts[1]
            .output_schema
            .as_ref()
            .expect("topic quality prompt should request structured output");
        assert_eq!(quality_schema.name, "topic_quality_gate");
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_agent_filters_candidates_through_quality_gate_before_persisting() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let generation = json!({
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
                "title": "人工智能是什么",
                "angle": "泛泛介绍人工智能概念",
                "target_audience": "所有人",
                "hook_points": ["基础概念"],
                "content_type": "knowledge",
                "score": 91,
                "score_reason": "模型给出的原始分数偏高",
                "tags": ["AI"]
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
    });
    let quality = json!({
        "summary": "本批 3 条中 2 条通过，1 条因泛化被淘汰。",
        "items": [
            {
                "candidate_key": "candidate-1",
                "title": "AI 工具如何改变选题会",
                "decision": "pass",
                "quality_score": 88,
                "flags": [],
                "reason": "贴合账号定位，脚本化路径清晰。"
            },
            {
                "candidate_key": "candidate-2",
                "title": "人工智能是什么",
                "decision": "reject",
                "quality_score": 52,
                "flags": ["too_generic", "score_untrusted"],
                "reason": "标题过于泛化，原始评分与理由不匹配。"
            },
            {
                "candidate_key": "candidate-3",
                "title": "内容团队为什么需要 AI 工作流",
                "decision": "pass",
                "quality_score": 84,
                "flags": [],
                "reason": "受众明确，适合科普短视频。"
            }
        ]
    });
    let llm_client = Arc::new(ScriptedLLMClient::from_results(vec![
        Ok(quality.to_string()),
        Ok(generation.to_string()),
    ]));
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
            user_message: "本周 AI 工具方向，生成 3 个选题".to_string(),
            supplement_of_batch_id: None,
        })
        .await
        .unwrap();

    assert_eq!(response.run.status, "succeeded");
    assert_eq!(response.agent_message.metadata["topic_count"], 2);
    assert_eq!(response.agent_message.metadata["quality_pass_count"], 2);
    assert_eq!(response.agent_message.metadata["quality_reject_count"], 1);
    assert_eq!(
        response.agent_message.metadata["quality_rewrite_triggered"],
        false
    );
    let batch_id: Uuid =
        serde_json::from_value(response.agent_message.metadata["batch_id"].clone()).unwrap();

    let topics = topic_repository
        .list_topics(
            project_id,
            ContentTopicFilter {
                status: Some(ContentTopicStatus::Idea),
                source: Some(ContentTopicSource::Agent),
                batch_id: Some(batch_id),
            },
        )
        .await
        .unwrap();
    assert_eq!(topics.len(), 2);
    assert_eq!(topics[0].title, "AI 工具如何改变选题会");
    assert_eq!(topics[0].metadata["quality_gate"]["quality_score"], 88);
    assert_eq!(
        topics[0].metadata["quality_gate"]["reason"],
        "贴合账号定位，脚本化路径清晰。"
    );

    let latest_quality = topic_repository
        .get_latest_topic_quality_evaluation(project_id, batch_id)
        .await
        .unwrap()
        .expect("quality evaluation should be saved");
    assert_eq!(latest_quality.pass_count, 2);
    assert_eq!(latest_quality.reject_count, 1);
    assert_eq!(latest_quality.result.items.len(), 3);

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
        vec![
            "read_project_context",
            "generate_topics",
            "evaluate_topic_quality",
            "persist_topics"
        ]
    );

    {
        let prompts = llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2);
        assert_eq!(
            prompts[1]
                .output_schema
                .as_ref()
                .expect("quality gate prompt should request structured output")
                .name,
            "topic_quality_gate"
        );
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_agent_rewrites_once_when_first_quality_pass_rate_is_low() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let first_generation = json!({
        "topics": [
            {
                "title": "人工智能是什么",
                "angle": "泛泛介绍人工智能",
                "target_audience": "所有人",
                "hook_points": ["基础概念"],
                "content_type": "knowledge",
                "score": 91,
                "score_reason": "原始评分虚高",
                "tags": ["AI"]
            },
            {
                "title": "AI 工具如何改变选题会",
                "angle": "对比传统选题会和 AI 辅助选题",
                "target_audience": "内容运营负责人",
                "hook_points": ["三分钟生成候选"],
                "content_type": "knowledge",
                "score": 92,
                "score_reason": "贴合账号定位",
                "tags": ["AI工具", "选题"]
            },
            {
                "title": "ChatGPT 是什么",
                "angle": "泛泛解释 ChatGPT",
                "target_audience": "所有人",
                "hook_points": ["基础概念"],
                "content_type": "knowledge",
                "score": 89,
                "score_reason": "原始评分虚高",
                "tags": ["AI"]
            }
        ]
    });
    let first_quality = json!({
        "summary": "首轮 3 条中 1 条通过，2 条泛化淘汰。",
        "items": [
            {
                "candidate_key": "candidate-1",
                "title": "人工智能是什么",
                "decision": "reject",
                "quality_score": 45,
                "flags": ["too_generic", "score_untrusted"],
                "reason": "标题过泛。"
            },
            {
                "candidate_key": "candidate-2",
                "title": "AI 工具如何改变选题会",
                "decision": "pass",
                "quality_score": 88,
                "flags": [],
                "reason": "贴合账号定位。"
            },
            {
                "candidate_key": "candidate-3",
                "title": "ChatGPT 是什么",
                "decision": "reject",
                "quality_score": 50,
                "flags": ["too_generic"],
                "reason": "缺少内容策略角度。"
            }
        ]
    });
    let rewritten_generation = json!({
        "topics": [
            {
                "title": "AI 工具如何把选题会从 2 小时压到 20 分钟",
                "angle": "用选题会流程改造案例解释 AI 工具价值",
                "target_audience": "内容运营负责人",
                "hook_points": ["时间对比", "流程改造"],
                "content_type": "knowledge",
                "score": 94,
                "score_reason": "具体且适合脚本化",
                "tags": ["AI工具", "选题"]
            },
            {
                "title": "内容团队怎样用 AI 建立选题质量复盘表",
                "angle": "从复盘指标和质量闸门切入",
                "target_audience": "中小内容团队",
                "hook_points": ["复盘表", "质量闸门"],
                "content_type": "tutorial",
                "score": 90,
                "score_reason": "与账号定位一致",
                "tags": ["内容运营", "AI工作流"]
            },
            {
                "title": "AI 工具清单大全",
                "angle": "罗列工具名称",
                "target_audience": "所有人",
                "hook_points": ["工具列表"],
                "content_type": "knowledge",
                "score": 80,
                "score_reason": "仍偏泛化",
                "tags": ["AI工具"]
            }
        ]
    });
    let rewritten_quality = json!({
        "summary": "重写后 3 条中 2 条通过，1 条因泛化被淘汰。",
        "items": [
            {
                "candidate_key": "candidate-1",
                "title": "AI 工具如何把选题会从 2 小时压到 20 分钟",
                "decision": "pass",
                "quality_score": 91,
                "flags": [],
                "reason": "具体、贴合账号定位且脚本化路径清晰。"
            },
            {
                "candidate_key": "candidate-2",
                "title": "内容团队怎样用 AI 建立选题质量复盘表",
                "decision": "pass",
                "quality_score": 87,
                "flags": [],
                "reason": "差异化明确，适合教程型短视频。"
            },
            {
                "candidate_key": "candidate-3",
                "title": "AI 工具清单大全",
                "decision": "reject",
                "quality_score": 62,
                "flags": ["too_generic"],
                "reason": "清单式标题过泛。"
            }
        ]
    });
    let llm_client = Arc::new(ScriptedLLMClient::from_results(vec![
        Ok(rewritten_quality.to_string()),
        Ok(rewritten_generation.to_string()),
        Ok(first_quality.to_string()),
        Ok(first_generation.to_string()),
    ]));
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
            user_message: "本周 AI 工具方向，生成 3 个选题".to_string(),
            supplement_of_batch_id: None,
        })
        .await
        .unwrap();

    assert_eq!(response.agent_message.metadata["topic_count"], 2);
    assert_eq!(response.agent_message.metadata["quality_pass_count"], 2);
    assert_eq!(response.agent_message.metadata["quality_reject_count"], 1);
    assert_eq!(
        response.agent_message.metadata["quality_rewrite_triggered"],
        true
    );
    let batch_id: Uuid =
        serde_json::from_value(response.agent_message.metadata["batch_id"].clone()).unwrap();
    let latest_quality = topic_repository
        .get_latest_topic_quality_evaluation(project_id, batch_id)
        .await
        .unwrap()
        .expect("latest quality evaluation should exist");
    assert!(latest_quality.rewrite_triggered);
    assert_eq!(latest_quality.pass_count, 2);
    assert_eq!(latest_quality.reject_count, 1);

    let topics = topic_repository
        .list_topics(
            project_id,
            ContentTopicFilter {
                status: Some(ContentTopicStatus::Idea),
                source: Some(ContentTopicSource::Agent),
                batch_id: Some(batch_id),
            },
        )
        .await
        .unwrap();
    assert_eq!(topics.len(), 2);
    assert!(topics
        .iter()
        .any(|topic| topic.title.contains("2 小时压到 20 分钟")));
    assert!(!topics.iter().any(|topic| topic.title == "人工智能是什么"));

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
        vec![
            "read_project_context",
            "generate_topics",
            "evaluate_topic_quality",
            "rewrite_topics",
            "evaluate_topic_quality",
            "persist_topics"
        ]
    );
    {
        let prompts = llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 4);
        assert!(prompts[2].user.contains("基于质量闸门淘汰原因重写"));
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_agent_quality_evaluation_failure_marks_batch_failed_without_topics() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let generation = json!({
        "topics": [
            {
                "title": "AI 工具如何改变选题会",
                "angle": "对比传统选题会和 AI 辅助选题",
                "target_audience": "内容运营负责人",
                "hook_points": ["三分钟生成候选"],
                "content_type": "knowledge",
                "score": 92,
                "score_reason": "贴合账号定位",
                "tags": ["AI工具", "选题"]
            }
        ]
    });
    let llm_client = Arc::new(ScriptedLLMClient::from_results(vec![
        Ok("not json".to_string()),
        Ok(generation.to_string()),
    ]));
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
            user_message: "本周 AI 工具方向，生成 1 个选题".to_string(),
            supplement_of_batch_id: None,
        })
        .await
        .unwrap_err();

    assert!(matches!(error, AgentRuntimeError::InvalidLlmOutput(_)));
    let batch_id = latest_topic_generation_batch_id(&test_pool, project_id).await;
    let batch = topic_repository
        .get_generation_batch(batch_id)
        .await
        .unwrap();
    assert_eq!(batch.status, TopicGenerationBatchStatus::Failed);
    assert!(batch
        .error_message
        .as_deref()
        .unwrap_or_default()
        .contains("missing JSON object start"));
    let topics = topic_repository
        .list_topics(
            project_id,
            ContentTopicFilter {
                status: None,
                source: Some(ContentTopicSource::Agent),
                batch_id: Some(batch_id),
            },
        )
        .await
        .unwrap();
    assert!(topics.is_empty());
    let quality = topic_repository
        .get_latest_topic_quality_evaluation(project_id, batch_id)
        .await
        .unwrap()
        .expect("failed quality evaluation should be saved");
    assert_eq!(quality.status, TopicQualityEvaluationStatus::Failed);
    assert_eq!(quality.pass_count, 0);
    assert_eq!(quality.reject_count, 0);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_agent_marks_batch_failed_when_rewrite_has_no_passed_candidates() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let first_generation = json!({
        "topics": [
            {
                "title": "人工智能是什么",
                "angle": "泛泛介绍人工智能",
                "target_audience": "所有人",
                "hook_points": ["基础概念"],
                "content_type": "knowledge",
                "score": 91,
                "score_reason": "原始评分虚高",
                "tags": ["AI"]
            }
        ]
    });
    let first_quality = json!({
        "summary": "首轮没有通过项。",
        "items": [
            {
                "candidate_key": "candidate-1",
                "title": "人工智能是什么",
                "decision": "reject",
                "quality_score": 40,
                "flags": ["too_generic"],
                "reason": "过于泛化。"
            }
        ]
    });
    let rewritten_generation = json!({
        "topics": [
            {
                "title": "AI 工具清单大全",
                "angle": "罗列工具名称",
                "target_audience": "所有人",
                "hook_points": ["工具列表"],
                "content_type": "knowledge",
                "score": 82,
                "score_reason": "仍偏泛化",
                "tags": ["AI工具"]
            }
        ]
    });
    let rewritten_quality = json!({
        "summary": "重写后仍没有通过项。",
        "items": [
            {
                "candidate_key": "candidate-1",
                "title": "AI 工具清单大全",
                "decision": "reject",
                "quality_score": 55,
                "flags": ["too_generic", "score_untrusted"],
                "reason": "缺少具体场景和脚本化路径。"
            }
        ]
    });
    let llm_client = Arc::new(ScriptedLLMClient::from_results(vec![
        Ok(rewritten_quality.to_string()),
        Ok(rewritten_generation.to_string()),
        Ok(first_quality.to_string()),
        Ok(first_generation.to_string()),
    ]));
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
            user_message: "本周 AI 工具方向，生成 1 个选题".to_string(),
            supplement_of_batch_id: None,
        })
        .await
        .unwrap_err();

    assert!(error.to_string().contains("质量闸门未产生可用选题"));
    let batch_id = latest_topic_generation_batch_id(&test_pool, project_id).await;
    let batch = topic_repository
        .get_generation_batch(batch_id)
        .await
        .unwrap();
    assert_eq!(batch.status, TopicGenerationBatchStatus::Failed);
    let topics = topic_repository
        .list_topics(
            project_id,
            ContentTopicFilter {
                status: None,
                source: Some(ContentTopicSource::Agent),
                batch_id: Some(batch_id),
            },
        )
        .await
        .unwrap();
    assert!(topics.is_empty());
    let latest_quality = topic_repository
        .get_latest_topic_quality_evaluation(project_id, batch_id)
        .await
        .unwrap()
        .expect("latest quality evaluation should be saved");
    assert!(latest_quality.rewrite_triggered);
    assert_eq!(latest_quality.pass_count, 0);
    assert_eq!(latest_quality.reject_count, 1);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_agent_generates_supplement_batch_without_mutating_original_batch() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let generation = json!({
        "topics": [
            {
                "title": "补充选题：AI 工作流复盘",
                "angle": "从复盘历史生成的遗漏角度补充",
                "target_audience": "内容运营负责人",
                "hook_points": ["补齐遗漏", "保持批次可追溯"],
                "content_type": "knowledge",
                "score": 90,
                "score_reason": "适合作为历史批次补充",
                "tags": ["AI工具", "补充选题"]
            }
        ]
    });
    let quality = json!({
        "summary": "补充批次 1 条通过。",
        "items": [
            {
                "candidate_key": "candidate-1",
                "title": "补充选题：AI 工作流复盘",
                "decision": "pass",
                "quality_score": 86,
                "flags": [],
                "reason": "延续原主题且补充复盘角度。"
            }
        ]
    });
    let llm_client = Arc::new(ScriptedLLMClient::from_results(vec![
        Ok(quality.to_string()),
        Ok(generation.to_string()),
    ]));
    let (runtime, conversation_repository, topic_repository) =
        build_topic_runtime(test_pool.clone(), llm_client);
    let original_batch = topic_repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id: None,
            prompt: "原始生成 AI 工具方向选题".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({ "original": true }),
        })
        .await
        .unwrap();
    topic_repository
        .create_topic(topic_input(project_id, original_batch.id, "原始批次选题"))
        .await
        .unwrap();
    let conversation = conversation_repository
        .create_conversation(CreateAgentConversationInput {
            project_id: Some(project_id),
            agent_type: "topic".to_string(),
            subject_type: None,
            subject_id: None,
            title: "选题补充".to_string(),
            metadata: json!({}),
        })
        .await
        .unwrap();

    let response = runtime
        .handle_turn(AgentTurnRequest {
            conversation_id: conversation.id,
            user_message: "围绕遗漏的 AI 工作流角度补充 1 个选题".to_string(),
            supplement_of_batch_id: Some(original_batch.id),
        })
        .await
        .unwrap();

    let supplement_batch_id: Uuid =
        serde_json::from_value(response.agent_message.metadata["batch_id"].clone()).unwrap();
    assert_ne!(supplement_batch_id, original_batch.id);
    assert_eq!(
        response.agent_message.metadata["supplement_of_batch_id"],
        json!(original_batch.id)
    );
    assert_eq!(response.agent_message.metadata["topic_count"], 1);

    let supplement_batch = topic_repository
        .get_generation_batch(supplement_batch_id)
        .await
        .unwrap();
    assert_eq!(
        supplement_batch.supplement_of_batch_id,
        Some(original_batch.id)
    );
    let original_after = topic_repository
        .get_generation_batch(original_batch.id)
        .await
        .unwrap();
    assert_eq!(original_after.prompt, "原始生成 AI 工具方向选题");
    assert_eq!(original_after.requested_count, 1);
    assert_eq!(original_after.source_run_id, None);
    assert_eq!(original_after.metadata, json!({ "original": true }));

    let supplement_topics = topic_repository
        .list_topics(
            project_id,
            ContentTopicFilter {
                status: Some(ContentTopicStatus::Idea),
                source: Some(ContentTopicSource::Agent),
                batch_id: Some(supplement_batch_id),
            },
        )
        .await
        .unwrap();
    assert_eq!(supplement_topics.len(), 1);
    assert_eq!(supplement_topics[0].batch_id, Some(supplement_batch_id));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_agent_includes_topic_context_when_generating_supplement_batch() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let generation = json!({
        "topics": [
            {
                "title": "补充选题：AI 工具落地复盘",
                "angle": "基于已有主题继续补充实操复盘角度",
                "target_audience": "内容运营负责人",
                "hook_points": ["延续原主题", "避开重复角度"],
                "content_type": "knowledge",
                "score": 90,
                "score_reason": "与原始主题相关且补充新角度",
                "tags": ["AI工具", "复盘"]
            }
        ]
    });
    let quality = json!({
        "summary": "补充批次 1 条通过。",
        "items": [
            {
                "candidate_key": "candidate-1",
                "title": "补充选题：AI 工具落地复盘",
                "decision": "pass",
                "quality_score": 88,
                "flags": [],
                "reason": "基于同主题组补充复盘角度，未重复已有选题。"
            }
        ]
    });
    let llm_client = Arc::new(ScriptedLLMClient::from_results(vec![
        Ok(quality.to_string()),
        Ok(generation.to_string()),
    ]));
    let (runtime, conversation_repository, topic_repository) =
        build_topic_runtime(test_pool.clone(), llm_client.clone());
    let original_batch = topic_repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id: None,
            prompt: "原始生成 AI 工具方向选题".to_string(),
            requested_count: 2,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({ "original": true }),
        })
        .await
        .unwrap();
    topic_repository
        .create_topic(topic_input(project_id, original_batch.id, "原始批次选题"))
        .await
        .unwrap();
    let existing_supplement_batch = topic_repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id: Some(original_batch.id),
            prompt: "第一次补充：加入工具清单方向".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    topic_repository
        .create_topic(topic_input(
            project_id,
            existing_supplement_batch.id,
            "既有补充批次选题",
        ))
        .await
        .unwrap();
    let unrelated_batch = topic_repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id: None,
            prompt: "无关原始批次".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    topic_repository
        .create_topic(topic_input(project_id, unrelated_batch.id, "无关批次选题"))
        .await
        .unwrap();

    let conversation = conversation_repository
        .create_conversation(CreateAgentConversationInput {
            project_id: Some(project_id),
            agent_type: "topic".to_string(),
            subject_type: None,
            subject_id: None,
            title: "选题补充".to_string(),
            metadata: json!({}),
        })
        .await
        .unwrap();
    conversation_repository
        .save_message(CreateAgentMessageInput {
            conversation_id: conversation.id,
            role: AgentMessageRole::User,
            content: "上一轮要求：更偏实操路线".to_string(),
            metadata: json!({}),
        })
        .await
        .unwrap();
    conversation_repository
        .save_message(CreateAgentMessageInput {
            conversation_id: conversation.id,
            role: AgentMessageRole::Assistant,
            content: "上一轮已生成了基础方向".to_string(),
            metadata: json!({}),
        })
        .await
        .unwrap();

    runtime
        .handle_turn(AgentTurnRequest {
            conversation_id: conversation.id,
            user_message: "继续补充 1 个复盘角度".to_string(),
            supplement_of_batch_id: Some(existing_supplement_batch.id),
        })
        .await
        .unwrap();

    {
        let prompts = llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2);
        let generation_prompt = &prompts[0].user;
        assert!(generation_prompt.contains("原始生成 AI 工具方向选题"));
        assert!(generation_prompt.contains("原始批次选题"));
        assert!(generation_prompt.contains("既有补充批次选题"));
        assert!(generation_prompt.contains("上一轮要求：更偏实操路线"));
        assert!(generation_prompt.contains("上一轮已生成了基础方向"));
        assert!(generation_prompt.contains("基于同一主题继续扩展"));
        assert!(generation_prompt.contains("避免重复已有选题"));
        assert!(!generation_prompt.contains("无关批次选题"));

        let quality_prompt = &prompts[1].user;
        assert!(quality_prompt.contains("同主题组已有选题"));
        assert!(quality_prompt.contains("原始批次选题"));
        assert!(quality_prompt.contains("既有补充批次选题"));
        assert!(!quality_prompt.contains("无关批次选题"));
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn topic_agent_rejects_unusable_supplement_targets_before_llm_call() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let other_project_id = insert_project(&test_pool).await;
    let llm_client = Arc::new(ScriptedLLMClient::returning(json!({
        "topics": [
            {
                "title": "不应生成的补充选题",
                "angle": "目标不可用时不应调用 LLM",
                "target_audience": "内容运营负责人",
                "hook_points": ["拒绝生成"],
                "content_type": "knowledge",
                "score": 80,
                "score_reason": "不应被使用",
                "tags": ["AI工具"]
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
            title: "选题补充".to_string(),
            metadata: json!({}),
        })
        .await
        .unwrap();

    let other_project_batch = topic_repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id: other_project_id,
            source_run_id: None,
            supplement_of_batch_id: None,
            prompt: "其他项目批次".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    topic_repository
        .create_topic(topic_input(
            other_project_id,
            other_project_batch.id,
            "其他项目选题",
        ))
        .await
        .unwrap();
    let failed_batch = topic_repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id: None,
            prompt: "失败批次".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Failed,
            error_message: Some("invalid topic JSON".to_string()),
            metadata: json!({}),
        })
        .await
        .unwrap();
    let empty_batch = topic_repository
        .create_generation_batch(CreateTopicGenerationBatchInput {
            project_id,
            source_run_id: None,
            supplement_of_batch_id: None,
            prompt: "空批次".to_string(),
            requested_count: 1,
            status: TopicGenerationBatchStatus::Succeeded,
            error_message: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    let cases = [
        (
            Uuid::new_v4(),
            "missing",
            "topic generation batch not found",
        ),
        (
            other_project_batch.id,
            "cross_project",
            "topic generation batch not found",
        ),
        (
            failed_batch.id,
            "failed",
            "topic generation batch cannot be supplemented",
        ),
        (
            empty_batch.id,
            "empty",
            "topic generation batch cannot be supplemented",
        ),
    ];

    let batch_count_before = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM topic_generation_batches WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    let topic_count_before =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM content_topics WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&test_pool)
            .await
            .unwrap();

    for (target_batch_id, label, expected_error) in cases {
        let error = runtime
            .handle_turn(AgentTurnRequest {
                conversation_id: conversation.id,
                user_message: format!("{label}: 补充 1 个选题"),
                supplement_of_batch_id: Some(target_batch_id),
            })
            .await
            .unwrap_err();
        assert!(
            error.to_string().contains(expected_error),
            "{label} should contain {expected_error}, got {error}"
        );
        assert!(matches!(error, AgentRuntimeError::TopicRepository(_)));
    }

    let batch_count_after = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM topic_generation_batches WHERE project_id = $1",
    )
    .bind(project_id)
    .fetch_one(&test_pool)
    .await
    .unwrap();
    let topic_count_after =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM content_topics WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&test_pool)
            .await
            .unwrap();

    assert_eq!(batch_count_after, batch_count_before);
    assert_eq!(topic_count_after, topic_count_before);
    assert!(llm_client.prompts.lock().unwrap().is_empty());

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
            supplement_of_batch_id: None,
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
                supplement_of_batch_id: None,
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
