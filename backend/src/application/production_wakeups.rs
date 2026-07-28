//! Full Crew Redis 唤醒适配器；Redis 只接收最小定位消息，权威状态始终留在 PostgreSQL。

use novex_production_crew::{durable::repository::DurableProductionRepository, ProductionResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionWakeupMessage {
    pub run_id: uuid::Uuid,
    pub step_id: uuid::Uuid,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WakeupDispatchReport {
    pub recovered: u64,
    pub delivered: u64,
    pub failed: u64,
}

#[derive(Clone)]
pub struct RedisProductionWakeupDispatcher {
    client: redis::Client,
    queue_key: String,
}

impl RedisProductionWakeupDispatcher {
    pub fn new(client: redis::Client, queue_key: impl Into<String>) -> Self {
        Self {
            client,
            queue_key: queue_key.into(),
        }
    }

    /// 先由 PostgreSQL 恢复扫描补 outbox，再逐条投递；Redis 失败只留下可重试事实。
    pub async fn recover_and_dispatch(
        &self,
        repository: &DurableProductionRepository,
        limit: i64,
    ) -> ProductionResult<WakeupDispatchReport> {
        let recovered = repository.enqueue_recoverable_wakeups(limit).await?;
        let wakeups = repository.pending_wakeups(limit).await?;
        let mut report = WakeupDispatchReport {
            recovered,
            ..WakeupDispatchReport::default()
        };
        for wakeup in wakeups {
            let message = ProductionWakeupMessage {
                run_id: wakeup.run_id,
                step_id: wakeup.step_id,
            };
            match self.publish(&message).await {
                Ok(()) => {
                    repository
                        .record_wakeup_delivery(wakeup.id, true, None)
                        .await?;
                    report.delivered += 1;
                }
                Err(error_kind) => {
                    repository
                        .record_wakeup_delivery(wakeup.id, false, Some(&error_kind))
                        .await?;
                    report.failed += 1;
                }
            }
        }
        Ok(report)
    }

    async fn publish(&self, message: &ProductionWakeupMessage) -> Result<(), String> {
        let payload = serde_json::to_string(message)
            .map_err(|_| "wakeup_serialization_failed".to_string())?;
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| format!("redis_connection_{:?}", error.kind()))?;
        redis::cmd("RPUSH")
            .arg(&self.queue_key)
            .arg(payload)
            .query_async::<i64>(&mut connection)
            .await
            .map(|_| ())
            .map_err(|error| format!("redis_publish_{:?}", error.kind()))
    }

    /// 消费一个最小唤醒消息。消息只用于定位 PostgreSQL 中的步骤，不能直接作为流程状态。
    pub async fn pop(&self) -> ProductionResult<Option<ProductionWakeupMessage>> {
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| {
                novex_production_crew::ProductionError::AgentExecution(error.to_string())
            })?;
        let payload = redis::cmd("LPOP")
            .arg(&self.queue_key)
            .query_async::<Option<String>>(&mut connection)
            .await
            .map_err(|error| {
                novex_production_crew::ProductionError::AgentExecution(error.to_string())
            })?;
        payload
            .map(|payload| {
                serde_json::from_str(&payload).map_err(|error| {
                    novex_production_crew::ProductionError::TransitionConflict {
                        reason: format!("invalid production wakeup payload: {error}"),
                    }
                })
            })
            .transpose()
    }
}
