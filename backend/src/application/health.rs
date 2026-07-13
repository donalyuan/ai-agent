//! 进程存活信息与基础设施就绪探测，避免 HTTP 层直接执行数据库或 Redis 命令。

use sqlx::PgPool;

#[derive(Clone)]
/// 汇总 API 进程环境以及 PostgreSQL、Redis 的就绪状态。
pub struct HealthService {
    environment: String,
    pg_pool: Option<PgPool>,
    redis_client: Option<redis::Client>,
}

impl HealthService {
    pub fn new(
        environment: String,
        pg_pool: Option<PgPool>,
        redis_client: Option<redis::Client>,
    ) -> Self {
        Self {
            environment,
            pg_pool,
            redis_client,
        }
    }

    pub fn liveness(&self) -> LivenessStatus {
        LivenessStatus {
            environment: self.environment.clone(),
        }
    }

    /// 分别探测两个运行依赖，任一缺失或不可访问时都保持“不就绪”语义。
    pub async fn readiness(&self) -> ReadinessStatus {
        let postgres = match &self.pg_pool {
            Some(pool) => sqlx::query_scalar::<_, i32>("SELECT 1")
                .fetch_one(pool)
                .await
                .is_ok(),
            None => false,
        };

        let redis = match &self.redis_client {
            Some(client) => match client.get_multiplexed_async_connection().await {
                Ok(mut connection) => redis::cmd("PING")
                    .query_async::<String>(&mut connection)
                    .await
                    .map(|value| value == "PONG")
                    .unwrap_or(false),
                Err(_) => false,
            },
            None => false,
        };

        ReadinessStatus { postgres, redis }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LivenessStatus {
    pub environment: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadinessStatus {
    pub postgres: bool,
    pub redis: bool,
}

impl ReadinessStatus {
    pub fn is_ready(&self) -> bool {
        self.postgres && self.redis
    }
}
