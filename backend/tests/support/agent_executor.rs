use async_trait::async_trait;
use novex_agent::{
    AgentInvocation, AgentRegistry, AuditedExecutionBinding, AuditedModelExecutor,
    BoundModelResolver, FixedModelBinding, ModelExecutionRef, ResolvedBoundModel,
};
use novex_ai_core::DefinitionRegistry;
use novex_api::application::agents::adapters::{AgentRuntimeError, AgentTurnResponse};
use novex_api::application::agents::kernel::AgentExecutor;
use novex_api::model_routing::model_behavior_evidence;
use novex_api::repositories::{PostgresConversationRepository, PostgresModelCallRepository};
use novex_model::{ApiProtocol, LLMClient, ModelExecutionSnapshot, ModelType};
use serde_json::json;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
struct FixedTestModelResolver {
    resolved: ResolvedBoundModel,
}

#[async_trait]
impl BoundModelResolver for FixedTestModelResolver {
    async fn resolve(&self, model_id: Uuid) -> Result<ResolvedBoundModel, novex_agent::BoxError> {
        if model_id != self.resolved.model_id {
            return Err(format!("unexpected test model: {model_id}").into());
        }
        Ok(self.resolved.clone())
    }
}

pub struct TestAgentExecutor {
    executor: AgentExecutor,
    model: ModelExecutionRef,
    #[allow(dead_code)]
    definitions: Arc<DefinitionRegistry>,
}

impl TestAgentExecutor {
    pub async fn new(
        registry: AgentRegistry,
        repository: PostgresConversationRepository,
        pool: PgPool,
        client: Arc<dyn LLMClient>,
        agent_key: &str,
    ) -> Self {
        let model_id = crate::support::test_database::insert_enabled_text_model(&pool).await;
        let snapshot = ModelExecutionSnapshot {
            model_id,
            display_name: "Audited test model".into(),
            model_type: ModelType::Text,
            provider_name: "test".into(),
            api_protocol: ApiProtocol::OpenAiResponses,
            protocol_version: "test".into(),
            request_base_url: "https://example.invalid/v1".into(),
            upstream_model: "test-model".into(),
            reasoning_effort: None,
            timeout_seconds: 5,
            max_output_tokens: Some(3000),
            settings: json!({"context_window": 128000}),
        };
        let evidence = model_behavior_evidence(&snapshot).unwrap();
        let definitions_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("agent-definitions");
        let definitions = Arc::new(DefinitionRegistry::load(definitions_path).unwrap());
        let agent = definitions.active_agent(agent_key).unwrap();
        let agent_key = agent.agent_key.clone();
        let agent_version = agent.version.clone();
        let model_snapshot = serde_json::to_value(&snapshot).unwrap();
        let resolver = Arc::new(FixedTestModelResolver {
            resolved: ResolvedBoundModel {
                client,
                model_id,
                behavior_fingerprint: evidence.behavior_fingerprint.clone(),
                capabilities: evidence.capabilities,
                model_snapshot,
                known_secrets: Vec::new(),
            },
        });
        let audited = Arc::new(AuditedModelExecutor::new(
            definitions.clone(),
            resolver,
            Arc::new(PostgresModelCallRepository::new(pool)),
        ));
        Self {
            executor: AgentExecutor::new(registry, repository),
            model: ModelExecutionRef {
                snapshot: Some(snapshot),
                audited: Some(AuditedExecutionBinding {
                    executor: audited,
                    agent_key,
                    agent_version,
                    binding: FixedModelBinding {
                        model_id,
                        behavior_fingerprint: evidence.behavior_fingerprint,
                    },
                }),
            },
            definitions,
        }
    }

    #[allow(dead_code)]
    pub fn model(&self) -> &ModelExecutionRef {
        &self.model
    }

    #[allow(dead_code)]
    pub fn definitions(&self) -> &DefinitionRegistry {
        &self.definitions
    }

    pub async fn execute(
        &self,
        invocation: AgentInvocation,
    ) -> Result<AgentTurnResponse, AgentRuntimeError> {
        self.executor.execute(invocation, self.model.clone()).await
    }
}
