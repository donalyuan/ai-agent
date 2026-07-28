//! Full Crew 取消协调：先持久化 Production cancellation intent，再通过正式作品端口取消。

use crate::application::work_generation::WorkGenerationService;
use async_trait::async_trait;
use novex_production_crew::{
    durable::repository::{
        DurableProductionRepository, ExternalCancellationState, ProductionActor,
        ProductionRunRecord,
    },
    ProductionResult,
};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkCancellationPortError {
    pub code: &'static str,
}

#[async_trait]
pub trait WorkGenerationCancellationPort: Send + Sync {
    async fn cancel(
        &self,
        work_generation_run_id: Uuid,
    ) -> Result<ExternalCancellationState, WorkCancellationPortError>;
}

#[async_trait]
impl WorkGenerationCancellationPort for WorkGenerationService {
    async fn cancel(
        &self,
        work_generation_run_id: Uuid,
    ) -> Result<ExternalCancellationState, WorkCancellationPortError> {
        let details = self.cancel_run(work_generation_run_id).await.map_err(|_| {
            WorkCancellationPortError {
                code: "work_generation_cancel_port_error",
            }
        })?;
        match details.task.status.as_str() {
            "cancelled" => Ok(ExternalCancellationState::Cancelled),
            "cancelling" => Ok(ExternalCancellationState::Cancelling),
            "waiting_manual" => Ok(ExternalCancellationState::AttentionRequired),
            _ => Err(WorkCancellationPortError {
                code: "work_generation_cancel_unresolved",
            }),
        }
    }
}

#[derive(Clone)]
pub struct ProductionCancellationService {
    repository: DurableProductionRepository,
    work_generation: Arc<dyn WorkGenerationCancellationPort>,
}

impl ProductionCancellationService {
    pub fn new(
        repository: DurableProductionRepository,
        work_generation: Arc<dyn WorkGenerationCancellationPort>,
    ) -> Self {
        Self {
            repository,
            work_generation,
        }
    }

    pub async fn cancel(
        &self,
        run_id: Uuid,
        actor: ProductionActor,
        idempotency_key: &str,
        reason: &str,
    ) -> ProductionResult<ProductionRunRecord> {
        let mut run = self
            .repository
            .cancel_run(run_id, actor, idempotency_key, reason)
            .await?;
        if run.status == "cancelled" {
            return Ok(run);
        }

        let context = self.repository.cancellation_context(run_id).await?;
        for external_run_id in context.external_run_ids {
            if has_recorded_result(&context.external_results, external_run_id) {
                continue;
            }
            let (state, error_code) = match self.work_generation.cancel(external_run_id).await {
                Ok(state) => (state, None),
                Err(error) => (
                    ExternalCancellationState::AttentionRequired,
                    Some(error.code),
                ),
            };
            run = self
                .repository
                .reconcile_external_cancellation(run_id, external_run_id, state, error_code)
                .await?;
        }
        Ok(run)
    }
}

fn has_recorded_result(results: &Value, external_run_id: Uuid) -> bool {
    results
        .as_object()
        .is_some_and(|values| values.contains_key(&external_run_id.to_string()))
}
