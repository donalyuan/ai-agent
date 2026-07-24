use novex_agent::{AgentInvocation, AgentRegistry, ModelExecutionRef};
use novex_api::application::agents::adapters::{AgentRuntimeError, AgentTurnResponse};
use novex_api::application::agents::kernel::AgentExecutor;
use novex_api::repositories::PostgresConversationRepository;
use novex_model::{LLMClient, ModelExecutionSnapshot};
use std::sync::Arc;

pub struct TestAgentExecutor {
    executor: AgentExecutor,
    model: ModelExecutionRef,
}

impl TestAgentExecutor {
    pub fn new(
        registry: AgentRegistry,
        repository: PostgresConversationRepository,
        client: Arc<dyn LLMClient>,
        snapshot: Option<ModelExecutionSnapshot>,
    ) -> Self {
        Self {
            executor: AgentExecutor::new(registry, repository),
            model: ModelExecutionRef { client, snapshot },
        }
    }

    pub async fn execute(
        &self,
        invocation: AgentInvocation,
    ) -> Result<AgentTurnResponse, AgentRuntimeError> {
        self.executor.execute(invocation, self.model.clone()).await
    }
}
