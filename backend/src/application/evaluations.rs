use novex_agent::{
    AuditedCallOwner, AuditedModelError, AuditedModelExecutor, AuditedModelRequest,
    AuditedModelResponse,
};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalBudgetCharge {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_micros: u64,
    pub retry: bool,
}

pub struct RealEvalRunner {
    executor: Arc<AuditedModelExecutor>,
}

impl RealEvalRunner {
    pub fn new(executor: Arc<AuditedModelExecutor>) -> Self {
        Self { executor }
    }

    /// Executes only through the audited path; the audit repository atomically reserves budget.
    pub async fn execute_attempt(
        &self,
        eval_run_id: Uuid,
        mut request: AuditedModelRequest,
        charge: EvalBudgetCharge,
    ) -> Result<AuditedModelResponse, AuditedModelError> {
        request.owner = AuditedCallOwner::EvalRun(eval_run_id);
        let mut parameters = match request.parameters {
            Value::Object(parameters) => parameters,
            _ => {
                return Err(AuditedModelError::Compile(
                    "eval parameters must be a JSON object".into(),
                ))
            }
        };
        parameters.insert(
            "eval_budget_charge".into(),
            json!({
                "input_tokens": charge.input_tokens,
                "output_tokens": charge.output_tokens,
                "cost_micros": charge.cost_micros,
                "retry": charge.retry,
            }),
        );
        request.parameters = Value::Object(parameters);
        self.executor.execute(request).await
    }
}
