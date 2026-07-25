use async_trait::async_trait;
use chrono::Utc;
use novex_agent::{
    AgentAdapter, AgentExecutionContext, AgentInvocation, AgentRegistry, AgentSession, AgentStep,
    BoxError, MessageRole, ModelExecutionRef, StepRecorder, StoredMessage,
};
use novex_ai_core::AgentKey;
use novex_api::application::agents::adapters::{
    ScriptAgentAdapter, SoundAgentAdapter, TopicAgentAdapter, WorkAgentAdapter,
};
use novex_api::application::agents::kernel::{assemble_registry, AgentBootstrapError};
use novex_api::repositories::{
    PostgresConversationRepository, PostgresProjectRepository, PostgresScriptRepository,
    PostgresTopicRepository, PostgresVoiceCatalogRepository, PostgresWorkLibraryRepository,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use uuid::Uuid;

struct NoopSteps;

#[async_trait]
impl StepRecorder for NoopSteps {
    async fn record_step(&self, _: AgentStep) -> Result<Uuid, BoxError> {
        panic!("payload validation must happen before step recording")
    }
}

fn adapters() -> Vec<Arc<dyn AgentAdapter>> {
    let pool = PgPoolOptions::new()
        .connect_lazy("postgres://postgres:postgres@127.0.0.1:1/unused")
        .unwrap();
    let conversations = Arc::new(PostgresConversationRepository::new(pool.clone()));
    let projects = Arc::new(PostgresProjectRepository::new(pool.clone()));
    vec![
        Arc::new(ScriptAgentAdapter::new(
            conversations.clone(),
            Arc::new(PostgresScriptRepository::new(pool.clone())),
            projects.clone(),
        )),
        Arc::new(TopicAgentAdapter::new(
            conversations,
            projects.clone(),
            Arc::new(PostgresTopicRepository::new(pool.clone())),
        )),
        Arc::new(SoundAgentAdapter::new(Arc::new(
            PostgresVoiceCatalogRepository::new(pool.clone()),
        ))),
        Arc::new(WorkAgentAdapter::new(
            projects,
            Arc::new(PostgresWorkLibraryRepository::new(pool)),
        )),
    ]
}

#[tokio::test]
async fn all_business_adapters_declare_stable_keys_and_reject_unknown_payload_fields() {
    let adapters = adapters();
    let keys = adapters
        .iter()
        .map(|adapter| adapter.key().as_str())
        .collect::<Vec<_>>();
    assert_eq!(keys, vec!["script", "topic", "sound", "work"]);

    for adapter in adapters {
        let session_id = Uuid::new_v4();
        let context = AgentExecutionContext {
            session: AgentSession {
                id: session_id,
                project_id: Some(Uuid::new_v4()),
                agent_key: adapter.key().clone(),
                subject_type: None,
                subject_id: None,
                metadata: json!({}),
            },
            user_message: StoredMessage {
                id: Uuid::new_v4(),
                session_id,
                role: MessageRole::User,
                content: "fixture".into(),
                metadata: json!({}),
                created_at: Utc::now(),
            },
            run_id: Uuid::new_v4(),
            model: ModelExecutionRef {
                snapshot: None,
                audited: None,
            },
            steps: Arc::new(NoopSteps),
        };
        let error = adapter
            .execute(
                &AgentInvocation {
                    session_id,
                    user_message: "fixture".into(),
                    user_metadata: json!({}),
                    run_input: json!({}),
                    payload: json!({"unknown_field": true}),
                },
                &context,
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("Agent payload 无效"));
    }
}

#[tokio::test]
async fn bootstrap_rejects_invalid_duplicate_and_missing_adapter_dependencies() {
    assert!(AgentKey::new("Invalid Key").is_err());
    let adapters = adapters();
    let duplicate = assemble_registry(vec![
        ("first", Some(adapters[0].clone())),
        ("duplicate", Some(adapters[0].clone())),
    ]);
    assert!(matches!(
        duplicate,
        Err(AgentBootstrapError::Registry(
            novex_agent::RegistryError::DuplicateKey(_)
        ))
    ));

    let missing = assemble_registry(vec![("topic repository", None)]);
    assert!(matches!(
        missing,
        Err(AgentBootstrapError::MissingDependency("topic repository"))
    ));

    let mut registry = AgentRegistry::new();
    registry.register(adapters[1].clone()).unwrap();
    assert!(registry.resolve(&AgentKey::new("topic").unwrap()).is_some());
}
