use async_trait::async_trait;
use novex_agent::{AgentInvocation, AgentRegistry};
use novex_api::agents::{LLMClient, LLMError};
use novex_api::application::agents::adapters::{AgentRuntimeError, WorkAgentAdapter};
use novex_api::domain::conversation::{AgentMessageRole, CreateAgentConversationInput};
use novex_api::repositories::{
    ConversationRepository, PostgresConversationRepository, PostgresProjectRepository,
    PostgresWorkLibraryRepository,
};
use novex_model::LLMPrompt;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[path = "support/agent_executor.rs"]
mod agent_executor;
mod support;

use agent_executor::TestAgentExecutor;
use support::test_database::TestDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
    })
}

fn with_database_name(database_url: &str, database_name: &str) -> String {
    let query_start = database_url.find('?');
    let (base, query) = query_start
        .map(|index| (&database_url[..index], &database_url[index..]))
        .unwrap_or((database_url, ""));
    let slash_index = base.rfind('/').unwrap();
    format!("{}{}{}", &base[..=slash_index], database_name, query)
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let database_name = format!("work_agent_runtime_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
    let database = TestDatabase::new(&admin_url, &database_name);
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin_pool, pool, database)
}

async fn seed_work(pool: &PgPool) -> (Uuid, Uuid, Uuid) {
    let project_id: Uuid =
        sqlx::query_scalar("INSERT INTO projects (name) VALUES ('作品 Agent 项目') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let script_id: Uuid = sqlx::query_scalar("INSERT INTO scripts (project_id,title,hook,content) VALUES ($1,'节奏测试','hook','{}') RETURNING id")
        .bind(project_id).fetch_one(pool).await.unwrap();
    let work_id: Uuid = sqlx::query_scalar(
        "INSERT INTO works (project_id,script_id,title) VALUES ($1,$2,'节奏测试成片') RETURNING id",
    )
    .bind(project_id)
    .bind(script_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let source_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_versions (work_id,version_no,status,derivation_kind,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot) VALUES ($1,5,'completed','initial','manifest-v5','{}','{}','{}',$2,$3) RETURNING id",
    )
    .bind(work_id)
    .bind(json!({"full_prompt": "原始节奏"}))
    .bind(json!({"duration_seconds": 30, "audio_mode": "independent_tts"}))
    .fetch_one(pool).await.unwrap();
    let draft_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_versions (work_id,version_no,status,source_version_id,derivation_kind,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot) VALUES ($1,11,'draft',$2,'edit','manifest-v5','{}','{}','{}',$3,$4) RETURNING id",
    )
    .bind(work_id).bind(source_id)
    .bind(json!({"full_prompt": "原始节奏"}))
    .bind(json!({"duration_seconds": 30, "audio_mode": "independent_tts"}))
    .fetch_one(pool).await.unwrap();
    sqlx::query("UPDATE works SET current_version_id=$2,status='draft' WHERE id=$1")
        .bind(work_id)
        .bind(draft_id)
        .execute(pool)
        .await
        .unwrap();
    (project_id, work_id, draft_id)
}

struct ScriptedLlm {
    response: Mutex<Option<String>>,
    prompts: Mutex<Vec<LLMPrompt>>,
}

impl ScriptedLlm {
    fn new(response: Value) -> Self {
        Self {
            response: Mutex::new(Some(response.to_string())),
            prompts: Mutex::new(Vec::new()),
        }
    }

    fn raw(response: &str) -> Self {
        Self {
            response: Mutex::new(Some(response.to_string())),
            prompts: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl LLMClient for ScriptedLlm {
    async fn generate_script(&self, prompt: LLMPrompt) -> Result<String, LLMError> {
        self.prompts.lock().unwrap().push(prompt);
        Ok(self
            .response
            .lock()
            .unwrap()
            .take()
            .expect("每轮只允许一次模型调用"))
    }
}

async fn runtime(
    pool: PgPool,
    llm: Arc<ScriptedLlm>,
) -> (TestAgentExecutor, Arc<PostgresConversationRepository>) {
    let conversations = Arc::new(PostgresConversationRepository::new(pool.clone()));
    let mut registry = AgentRegistry::new();
    registry
        .register(Arc::new(WorkAgentAdapter::new(
            Arc::new(PostgresProjectRepository::new(pool.clone())),
            Arc::new(PostgresWorkLibraryRepository::new(pool.clone())),
        )))
        .unwrap();
    let runtime =
        TestAgentExecutor::new(registry, (*conversations).clone(), pool, llm, "video.work").await;
    (runtime, conversations)
}

async fn conversation(
    repository: &PostgresConversationRepository,
    project_id: Uuid,
    work_id: Uuid,
) -> Uuid {
    repository
        .create_conversation(CreateAgentConversationInput {
            project_id: Some(project_id),
            agent_type: "work".to_string(),
            subject_type: Some("work".to_string()),
            subject_id: Some(work_id),
            title: "作品修改".to_string(),
            metadata: json!({}),
        })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn work_agent_reuses_current_draft_and_returns_confirmable_diff() {
    let (_admin_pool, pool, _database) = migrated_pool().await;
    let (project_id, work_id, draft_id) = seed_work(&pool).await;
    let llm = Arc::new(ScriptedLlm::new(json!({
        "assistant_message": "已保留配音并收紧画面节奏，请确认影响范围。",
        "prompt_snapshot_patch": {"full_prompt": "保留配音，画面切换更紧凑"}
    })));
    let (runtime, conversations) = runtime(pool.clone(), llm.clone()).await;
    let conversation_id = conversation(&conversations, project_id, work_id).await;

    let response = runtime
        .execute(AgentInvocation {
            session_id: conversation_id,
            user_message: "保留配音，让画面节奏更紧凑".to_string(),
            user_metadata: json!({}),
            run_input: json!({}),
            payload: json!({}),
        })
        .await
        .unwrap();

    assert_eq!(response.agent_message.role, AgentMessageRole::Assistant);
    assert_eq!(
        response.agent_message.metadata["draft_version_id"],
        draft_id.to_string()
    );
    assert_eq!(response.agent_message.metadata["version_no"], 11);
    assert_eq!(
        response.agent_message.metadata["requires_confirmation"],
        true
    );
    assert!(response.agent_message.metadata["diff"]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|change| change["path"] == "prompt_snapshot.full_prompt"));
    assert_eq!(llm.prompts.lock().unwrap().len(), 1);
    let (node_key, context_snapshot_id, prompt_snapshot): (String, Option<Uuid>, Value) =
        sqlx::query_as(
            "SELECT node_key, context_snapshot_id, prompt_snapshot FROM model_calls WHERE agent_run_id=$1",
        )
        .bind(response.run.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(node_key, "work.patch");
    let context_snapshot_id = context_snapshot_id.expect("作品调用必须引用 ContextSnapshot");
    let (decisions, selected_order): (Value, Value) =
        sqlx::query_as("SELECT decisions, selected_order FROM context_snapshots WHERE id=$1")
            .bind(context_snapshot_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let selected = decisions
        .as_array()
        .unwrap()
        .iter()
        .filter(|decision| decision["decision"] == "selected")
        .collect::<Vec<_>>();
    assert!(selected.iter().any(|decision| {
        decision["source_kind"] == "current_work"
            && decision["trust"] == "confirmed_fact"
            && decision["priority"] == "p1"
    }));
    assert!(selected.iter().any(|decision| {
        decision["source_kind"] == "user_instruction"
            && decision["trust"] == "user_instruction"
            && decision["priority"] == "p0"
    }));
    assert_eq!(selected_order.as_array().unwrap().len(), 2);
    assert!(prompt_snapshot["user"]
        .as_str()
        .unwrap()
        .contains("当前作品和草稿："));
    assert!(prompt_snapshot["user"]
        .as_str()
        .unwrap()
        .contains("用户修改要求：\n保留配音，让画面节奏更紧凑"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_versions WHERE work_id=$1")
            .bind(work_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        sqlx::query_scalar::<_, Value>("SELECT prompt_snapshot FROM work_versions WHERE id=$1")
            .bind(draft_id)
            .fetch_one(&pool)
            .await
            .unwrap()["full_prompt"],
        "保留配音，画面切换更紧凑"
    );
}

#[tokio::test]
async fn work_agent_rejects_invalid_unknown_and_empty_patches_without_writing() {
    for response in [
        "not-json",
        r#"{"assistant_message":"越权","video_generation_patch":{"provider":"other"}}"#,
        r#"{"assistant_message":"没有修改"}"#,
    ] {
        let (_admin_pool, pool, _database) = migrated_pool().await;
        let (project_id, work_id, draft_id) = seed_work(&pool).await;
        let llm = Arc::new(ScriptedLlm::raw(response));
        let (runtime, conversations) = runtime(pool.clone(), llm).await;
        let conversation_id = conversation(&conversations, project_id, work_id).await;

        let result = runtime
            .execute(AgentInvocation {
                session_id: conversation_id,
                user_message: "修改作品".to_string(),
                user_metadata: json!({}),
                run_input: json!({}),
                payload: json!({}),
            })
            .await;

        assert!(matches!(
            result,
            Err(AgentRuntimeError::InvalidLlmOutput(_))
        ));
        assert_eq!(
            sqlx::query_scalar::<_, Value>("SELECT prompt_snapshot FROM work_versions WHERE id=$1")
                .bind(draft_id)
                .fetch_one(&pool)
                .await
                .unwrap()["full_prompt"],
            "原始节奏"
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM work_version_diff_plans WHERE work_id=$1"
            )
            .bind(work_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
            0
        );
    }
}

#[tokio::test]
async fn work_agent_rejects_work_from_another_project_before_model_call() {
    let (_admin_pool, pool, _database) = migrated_pool().await;
    let (_project_id, work_id, _draft_id) = seed_work(&pool).await;
    let other_project_id: Uuid =
        sqlx::query_scalar("INSERT INTO projects (name) VALUES ('其他项目') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let llm = Arc::new(ScriptedLlm::new(json!({
        "assistant_message": "不应执行",
        "prompt_snapshot_patch": {"full_prompt": "不应写入"}
    })));
    let (runtime, conversations) = runtime(pool, llm.clone()).await;
    let conversation_id = conversation(&conversations, other_project_id, work_id).await;

    let result = runtime
        .execute(AgentInvocation {
            session_id: conversation_id,
            user_message: "修改作品".to_string(),
            user_metadata: json!({}),
            run_input: json!({}),
            payload: json!({}),
        })
        .await;

    assert!(matches!(result, Err(AgentRuntimeError::Validation(_))));
    assert!(llm.prompts.lock().unwrap().is_empty());
}
