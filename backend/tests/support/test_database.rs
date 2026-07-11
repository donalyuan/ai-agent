use sqlx::{postgres::PgPoolOptions, PgPool};
use std::ops::Deref;
use std::thread;
use uuid::Uuid;

#[allow(dead_code)]
pub async fn insert_enabled_text_model(pool: &PgPool) -> Uuid {
    insert_enabled_text_model_with_base_url(pool, "https://example.invalid/v1").await
}

#[allow(dead_code)]
pub async fn insert_enabled_text_model_with_base_url(
    pool: &PgPool,
    request_base_url: &str,
) -> Uuid {
    sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, auth_scheme,
            request_base_url, upstream_model, api_key, timeout_seconds, status
        )
        VALUES (
            '测试文本模型', 'text', 'test', 'openai_chat_completions', 'bearer',
            $1, 'test-model', 'test-key', 5, 'enabled'
        )
        RETURNING id
        "#,
    )
    .bind(request_base_url)
    .fetch_one(pool)
    .await
    .expect("enabled text model fixture should be inserted")
}

pub struct TestDatabase {
    admin_url: String,
    name: String,
}

impl TestDatabase {
    pub fn new(admin_url: &str, name: &str) -> Self {
        Self {
            admin_url: admin_url.to_string(),
            name: name.to_string(),
        }
    }

    async fn drop_database(admin_url: &str, name: &str) {
        let admin_pool = match PgPoolOptions::new()
            .max_connections(1)
            .connect(admin_url)
            .await
        {
            Ok(pool) => pool,
            Err(_) => return,
        };
        let disconnect = format!(
            r#"
            SELECT pg_terminate_backend(pid)
            FROM pg_stat_activity
            WHERE datname = '{}'
            "#,
            name
        );
        let drop = format!(r#"DROP DATABASE IF EXISTS "{}""#, name);

        let _ = sqlx::query(&disconnect).execute(&admin_pool).await;
        let _ = sqlx::query(&drop).execute(&admin_pool).await;
        admin_pool.close().await;
    }
}

impl Deref for TestDatabase {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.name
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let admin_url = self.admin_url.clone();
        let name = self.name.clone();
        let _ = thread::spawn(move || {
            let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            runtime.block_on(Self::drop_database(&admin_url, &name));
        })
        .join();
    }
}
