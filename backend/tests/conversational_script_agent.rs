use async_trait::async_trait;
use novex_api::agents::conversation::{
    AgentConversationStatus, AgentMessageRole, CreateAgentConversationInput,
};
use novex_api::agents::conversational_runtime::{AgentRuntime, AgentTurnRequest};
use novex_api::agents::models::ScriptListFilter;
use novex_api::agents::{LLMClient, LLMError};
use novex_api::repositories::{
    ConversationRepository, PostgresConversationRepository, PostgresProjectRepository,
    ScriptRepository,
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
        .expect("temporary conversational script database should be created");
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
    let database_name = format!("video_agent_conversational_script_test_{}", suffix);
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
        .expect("temporary conversational script database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for conversational script test database");

    (admin_pool, test_pool, database_name)
}

async fn insert_project(pool: &PgPool) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO projects (name, positioning, description)
        VALUES ('科技博主', '科技知识账号', '对话式脚本改稿测试项目')
        RETURNING id
        "#,
    )
    .fetch_one(pool)
    .await
    .expect("project fixture should be inserted")
}

async fn insert_script(pool: &PgPool, project_id: Uuid) -> Uuid {
    let script_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO scripts (project_id, title, hook, content, status)
        VALUES ($1, '程序员必看：ChatGPT工作流', '还在手写重复代码？', $2, 'draft')
        RETURNING id
        "#,
    )
    .bind(project_id)
    .bind(json!({"topic": "ChatGPT如何改变程序员工作流"}))
    .fetch_one(pool)
    .await
    .expect("script fixture should be inserted");

    for (sequence, narration, visual, emotion) in [
        (
            1,
            "传统程序员每天要写大量重复代码。",
            "程序员盯着屏幕，快速切换多个代码文件。",
            "焦虑",
        ),
        (
            2,
            "现在只要描述需求，AI 就能快速生成初稿。",
            "屏幕上弹出代码建议。",
            "惊喜",
        ),
        (
            3,
            "AI 可以帮助新人快速理解陌生项目。",
            "代码结构图展开，重点模块被高亮标注。",
            "好奇",
        ),
    ] {
        sqlx::query(
            r#"
            INSERT INTO scenes (script_id, sequence, narration, visual_description, emotion, duration_sec)
            VALUES ($1, $2, $3, $4, $5, 8)
            "#,
        )
        .bind(script_id)
        .bind(sequence)
        .bind(narration)
        .bind(visual)
        .bind(emotion)
        .execute(pool)
        .await
        .expect("scene fixture should be inserted");
    }

    script_id
}

struct ScriptedLLMClient {
    responses: Mutex<Vec<Result<String, LLMError>>>,
    prompts: Mutex<Vec<LLMPrompt>>,
}

impl ScriptedLLMClient {
    fn returning(response: serde_json::Value) -> Self {
        Self {
            responses: Mutex::new(vec![Ok(response.to_string())]),
            prompts: Mutex::new(Vec::new()),
        }
    }

    fn returning_many(responses: Vec<serde_json::Value>) -> Self {
        Self {
            responses: Mutex::new(
                responses
                    .into_iter()
                    .rev()
                    .map(|response| Ok(response.to_string()))
                    .collect(),
            ),
            prompts: Mutex::new(Vec::new()),
        }
    }

    fn failing(error: LLMError) -> Self {
        Self {
            responses: Mutex::new(vec![Err(error)]),
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
            .expect("scripted LLM response should be configured")
    }
}

#[tokio::test]
async fn script_agent_dialogue_updates_target_scene_and_records_messages() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let script_id = insert_script(&test_pool, project_id).await;
    let conversation_repository = Arc::new(PostgresConversationRepository::new(test_pool.clone()));
    let script_repository = Arc::new(novex_api::repositories::PostgresScriptRepository::new(
        test_pool.clone(),
    ));
    let project_repository = Arc::new(PostgresProjectRepository::new(test_pool.clone()));
    let llm_client = Arc::new(ScriptedLLMClient::returning(json!({
        "scene_sequence": 3,
        "narration": "深夜办公室里，主角面对即将上线的故障代码，发现 AI 给出的解释和线上日志互相矛盾，只能立刻重新验证每一步判断。",
        "visual_description": "凌晨两点的办公室只剩一盏灯，屏幕上同时显示 AI 建议、红色错误日志和倒计时发布窗口。",
        "emotion": "紧张",
        "duration_sec": 10,
        "reply": "已把第 3 镜改成深夜上线前的冲突场景，强化紧迫感和人工验证。"
    })));
    let runtime = AgentRuntime::new(
        conversation_repository.clone(),
        script_repository.clone(),
        project_repository,
        llm_client.clone(),
    );
    let conversation = conversation_repository
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
    assert_eq!(conversation.status, AgentConversationStatus::Active);

    let response = runtime
        .handle_turn(AgentTurnRequest {
            conversation_id: conversation.id,
            user_message: "把第 3 镜改得更有冲突感，画面换成办公室深夜加班".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(response.agent_message.role, AgentMessageRole::Assistant);
    assert!(response.agent_message.content.contains("第 3 镜"));
    assert_eq!(
        response.agent_message.metadata["script_id"],
        script_id.to_string()
    );
    assert_eq!(response.agent_message.metadata["scene_sequence"], 3);
    assert_eq!(response.run.status, "succeeded");

    let updated_script = script_repository.get_script(script_id).await.unwrap();
    let updated_scene = updated_script
        .scenes
        .iter()
        .find(|scene| scene.sequence == 3)
        .unwrap();
    assert!(updated_scene.narration.contains("线上日志互相矛盾"));
    assert!(updated_scene.visual_description.contains("凌晨两点"));
    assert_eq!(updated_scene.emotion, "紧张");
    assert_eq!(updated_scene.duration_sec, 10);

    let messages = conversation_repository
        .list_messages(conversation.id)
        .await
        .unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, AgentMessageRole::User);
    assert_eq!(messages[1].role, AgentMessageRole::Assistant);

    {
        let prompts = llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 1);
        assert!(prompts[0].user.contains("第 3 镜"));
        assert!(prompts[0].user.contains("当前脚本"));
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn script_agent_dialogue_generates_script_for_unbound_conversation() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let conversation_repository = Arc::new(PostgresConversationRepository::new(test_pool.clone()));
    let script_repository = Arc::new(novex_api::repositories::PostgresScriptRepository::new(
        test_pool.clone(),
    ));
    let project_repository = Arc::new(PostgresProjectRepository::new(test_pool.clone()));
    let llm_client = Arc::new(ScriptedLLMClient::returning_many(vec![
        json!({
            "intent": "generate_script",
            "topic": "ChatGPT 如何改变程序员工作流",
            "style": "knowledge",
            "scene_count": 3,
            "reply": "我会生成一个 3 镜知识科普脚本。",
            "missing_fields": []
        }),
        json!({
            "title": "ChatGPT 工作流",
            "hook": "三个镜头看懂 AI 如何改变程序员日常。",
            "scenes": [
                {
                    "sequence": 1,
                    "narration": "很多程序员每天都被重复代码和文档检索消耗精力，真正需要判断架构、风险和边界的时间反而被压缩到最后阶段。",
                    "visual_description": "程序员在多个代码窗口和文档之间快速切换，屏幕角落出现时间流逝提示。",
                    "emotion": "焦虑",
                    "duration_sec": 8
                },
                {
                    "sequence": 2,
                    "narration": "把需求拆成清晰指令交给 ChatGPT 后，它能快速给出初稿、测试思路和改写方向，让人从空白页起步变成审稿。",
                    "visual_description": "聊天窗口生成代码片段、测试清单和重构建议，用户逐条勾选验证。",
                    "emotion": "惊喜",
                    "duration_sec": 9
                },
                {
                    "sequence": 3,
                    "narration": "真正高效的工作流不是盲目照搬 AI，而是让 AI 负责铺路，人负责验证事实、取舍方案并守住上线质量。",
                    "visual_description": "左侧是 AI 建议，右侧是人工评审清单，最后汇合到通过的发布流水线。",
                    "emotion": "笃定",
                    "duration_sec": 10
                }
            ]
        }),
    ]));
    let runtime = AgentRuntime::new(
        conversation_repository.clone(),
        script_repository.clone(),
        project_repository,
        llm_client.clone(),
    );
    let conversation = conversation_repository
        .create_conversation(CreateAgentConversationInput {
            project_id: Some(project_id),
            agent_type: "script".to_string(),
            subject_type: None,
            subject_id: None,
            title: "脚本生成".to_string(),
            metadata: json!({}),
        })
        .await
        .unwrap();

    let response = runtime
        .handle_turn(AgentTurnRequest {
            conversation_id: conversation.id,
            user_message: "帮我生成一个关于 ChatGPT 工作流的 3 镜知识科普脚本".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(response.agent_message.role, AgentMessageRole::Assistant);
    assert_eq!(response.agent_message.metadata["intent"], "generate_script");
    assert_eq!(response.agent_message.metadata["script_created"], true);
    assert_eq!(response.agent_message.metadata["needs_input"], false);
    assert_eq!(response.agent_message.metadata["missing_fields"], json!([]));
    let script_id = response.agent_message.metadata["script_id"]
        .as_str()
        .expect("script id should be returned");
    assert_eq!(response.run.status, "succeeded");

    let updated_conversation = conversation_repository
        .get_conversation(conversation.id)
        .await
        .unwrap();
    assert_eq!(updated_conversation.subject_type.as_deref(), Some("script"));
    assert_eq!(
        updated_conversation.subject_id.unwrap().to_string(),
        script_id
    );

    let script = script_repository
        .get_script(Uuid::parse_str(script_id).unwrap())
        .await
        .unwrap();
    assert_eq!(script.project_id, project_id);
    assert_eq!(script.scenes.len(), 3);
    assert!(script.content["topic"]
        .as_str()
        .unwrap()
        .contains("ChatGPT"));

    let messages = conversation_repository
        .list_messages(conversation.id)
        .await
        .unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, AgentMessageRole::User);
    assert_eq!(messages[1].role, AgentMessageRole::Assistant);

    {
        let prompts = llm_client.prompts.lock().unwrap();
        assert_eq!(prompts.len(), 2);
        assert!(prompts[0].user.contains("生成脚本参数"));
        assert!(prompts[1].user.contains("请根据以下选题生成3个分镜"));
    }

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn script_agent_dialogue_asks_for_missing_generation_fields_without_creating_script() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let conversation_repository = Arc::new(PostgresConversationRepository::new(test_pool.clone()));
    let script_repository = Arc::new(novex_api::repositories::PostgresScriptRepository::new(
        test_pool.clone(),
    ));
    let project_repository = Arc::new(PostgresProjectRepository::new(test_pool.clone()));
    let llm_client = Arc::new(ScriptedLLMClient::returning(json!({
        "intent": "generate_script",
        "topic": null,
        "style": null,
        "scene_count": null,
        "reply": "请补充选题、风格和分镜数，例如：生成一个关于 AI 工作流的 6 镜知识科普脚本。",
        "missing_fields": ["topic", "style", "scene_count"]
    })));
    let runtime = AgentRuntime::new(
        conversation_repository.clone(),
        script_repository.clone(),
        project_repository,
        llm_client.clone(),
    );
    let conversation = conversation_repository
        .create_conversation(CreateAgentConversationInput {
            project_id: Some(project_id),
            agent_type: "script".to_string(),
            subject_type: None,
            subject_id: None,
            title: "脚本生成".to_string(),
            metadata: json!({}),
        })
        .await
        .unwrap();

    let response = runtime
        .handle_turn(AgentTurnRequest {
            conversation_id: conversation.id,
            user_message: "帮我生成脚本".to_string(),
        })
        .await
        .unwrap();

    assert_eq!(response.agent_message.metadata["intent"], "generate_script");
    assert_eq!(response.agent_message.metadata["script_created"], false);
    assert_eq!(response.agent_message.metadata["needs_input"], true);
    assert_eq!(
        response.agent_message.metadata["missing_fields"],
        json!(["topic", "style", "scene_count"])
    );
    assert!(response.agent_message.content.contains("请补充"));
    assert_eq!(response.run.status, "succeeded");

    let updated_conversation = conversation_repository
        .get_conversation(conversation.id)
        .await
        .unwrap();
    assert!(updated_conversation.subject_type.is_none());
    assert!(updated_conversation.subject_id.is_none());
    let scripts = script_repository
        .list_scripts(
            project_id,
            ScriptListFilter {
                status: None,
                limit: None,
                offset: None,
            },
        )
        .await
        .unwrap();
    assert!(scripts.is_empty());

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn script_agent_dialogue_records_failed_run_when_generation_llm_fails() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = insert_project(&test_pool).await;
    let conversation_repository = Arc::new(PostgresConversationRepository::new(test_pool.clone()));
    let script_repository = Arc::new(novex_api::repositories::PostgresScriptRepository::new(
        test_pool.clone(),
    ));
    let project_repository = Arc::new(PostgresProjectRepository::new(test_pool.clone()));
    let llm_client = Arc::new(ScriptedLLMClient::failing(LLMError::Timeout));
    let runtime = AgentRuntime::new(
        conversation_repository.clone(),
        script_repository.clone(),
        project_repository,
        llm_client,
    );
    let conversation = conversation_repository
        .create_conversation(CreateAgentConversationInput {
            project_id: Some(project_id),
            agent_type: "script".to_string(),
            subject_type: None,
            subject_id: None,
            title: "脚本生成".to_string(),
            metadata: json!({}),
        })
        .await
        .unwrap();

    let error = runtime
        .handle_turn(AgentTurnRequest {
            conversation_id: conversation.id,
            user_message: "帮我生成一个关于 ChatGPT 工作流的 3 镜知识科普脚本".to_string(),
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("llm request timeout"));

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

    let messages = conversation_repository
        .list_messages(conversation.id)
        .await
        .unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, AgentMessageRole::User);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
