use novex_ai_core::DefinitionRegistry;
use novex_api::application::ai_models::AiModelService;
use novex_api::repositories::{
    AiModelListFilter, AiModelRepository, AiModelRepositoryError, AiModelStatus,
    ChangeAiModelStatusInput, CreateAiModelInput, DeleteAiModelInput, DeleteAiModelOutcome,
    PostgresAiModelRepository, UpdateAiModelInput,
};
use novex_model::{ApiProtocol, AuthScheme, ModelType};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

mod support;

use support::test_database::TestDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
    })
}

fn definition_registry() -> Arc<DefinitionRegistry> {
    Arc::new(
        DefinitionRegistry::load(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../agent-definitions"),
        )
        .unwrap(),
    )
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

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let suffix = Uuid::new_v4().simple().to_string();
    let database_name = format!("video_agent_ai_model_repo_test_{suffix}");
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .expect("temporary model repository database should be created");
    let database_name = TestDatabase::new(&admin_url, &database_name);
    let test_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&test_url)
        .await
        .expect("temporary model repository database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for model repository tests");
    (admin_pool, test_pool, database_name)
}

async fn drop_database(admin_pool: &PgPool, database_name: &str) {
    let _ = sqlx::query(&format!(
        "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{database_name}'"
    ))
    .execute(admin_pool)
    .await;
    let _ = sqlx::query(&format!(r#"DROP DATABASE IF EXISTS "{database_name}""#))
        .execute(admin_pool)
        .await;
}

fn text_model(name: &str) -> CreateAiModelInput {
    CreateAiModelInput {
        display_name: name.to_string(),
        model_type: ModelType::Text,
        provider_name: "OpenAI".to_string(),
        api_protocol: ApiProtocol::OpenAiResponses,
        protocol_version: "v1".to_string(),
        auth_scheme: AuthScheme::Bearer,
        request_base_url: "https://api.example.com/v1".to_string(),
        upstream_model: format!("{}-upstream", name.to_lowercase()),
        api_key: "test-key-original".to_string(),
        api_secret: None,
        catalog_access_key: None,
        catalog_secret_key: None,
        voice_catalog_source_model_id: None,
        timeout_seconds: 120,
        reasoning_effort: Some("high".to_string()),
        max_output_tokens: Some(4096),
        context_window: Some(128000),
        tokenizer_profile_key: Some("openai.o200k".to_string()),
        tokenizer_profile_version: Some("1.0.0".to_string()),
        settings: json!({}),
        sort_order: 10,
        remark: String::new(),
        status: AiModelStatus::Enabled,
        source: "admin".to_string(),
        source_key: None,
    }
}

fn image_model(name: &str) -> CreateAiModelInput {
    CreateAiModelInput {
        display_name: name.to_string(),
        model_type: ModelType::Image,
        provider_name: "OpenAI".to_string(),
        api_protocol: ApiProtocol::OpenAiImages,
        protocol_version: "v1".to_string(),
        auth_scheme: AuthScheme::Bearer,
        request_base_url: "https://images.example.com/v1".to_string(),
        upstream_model: "gpt-image-test".to_string(),
        api_key: "image-test-key".to_string(),
        api_secret: None,
        catalog_access_key: None,
        catalog_secret_key: None,
        voice_catalog_source_model_id: None,
        timeout_seconds: 180,
        reasoning_effort: None,
        max_output_tokens: None,
        context_window: None,
        tokenizer_profile_key: None,
        tokenizer_profile_version: None,
        settings: json!({
            "supported_sizes": ["1024x1024"],
            "default_size": "1024x1024",
            "max_images_per_request": 4
        }),
        sort_order: 20,
        remark: String::new(),
        status: AiModelStatus::Enabled,
        source: "admin".to_string(),
        source_key: None,
    }
}

fn update_from(model: &novex_api::repositories::AiModel) -> UpdateAiModelInput {
    UpdateAiModelInput {
        version: model.version,
        model_type: model.model_type,
        display_name: model.display_name.clone(),
        provider_name: model.provider_name.clone(),
        api_protocol: model.api_protocol,
        protocol_version: model.protocol_version.clone(),
        auth_scheme: model.auth_scheme,
        request_base_url: model.request_base_url.clone(),
        upstream_model: model.upstream_model.clone(),
        api_key: None,
        api_secret: None,
        catalog_access_key: None,
        catalog_secret_key: None,
        voice_catalog_source_model_id: None,
        timeout_seconds: model.timeout_seconds,
        reasoning_effort: model.reasoning_effort.clone(),
        max_output_tokens: model.max_output_tokens,
        context_window: model.context_window,
        tokenizer_profile_key: model.tokenizer_profile_key.clone(),
        tokenizer_profile_version: model.tokenizer_profile_version.clone(),
        settings: model.settings.clone(),
        sort_order: model.sort_order,
        remark: model.remark.clone(),
        replacement_model_id: None,
        allow_no_default: false,
    }
}

#[tokio::test]
async fn governed_context_fields_are_required_only_for_enabled_text_models() {
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let repository = PostgresAiModelRepository::new(pool.clone());

    let mut missing = text_model("Missing Context");
    missing.context_window = None;
    missing.tokenizer_profile_key = None;
    missing.tokenizer_profile_version = None;
    assert!(matches!(
        repository.create(missing.clone()).await,
        Err(AiModelRepositoryError::InvalidConfig(_))
    ));

    missing.status = AiModelStatus::Disabled;
    let historical = repository.create(missing).await.unwrap();
    assert_eq!(historical.context_window, None);

    let mut unknown = text_model("Unknown Profile");
    unknown.tokenizer_profile_key = Some("unknown.profile".to_string());
    assert!(matches!(
        AiModelService::new(repository.clone(), definition_registry())
            .create(unknown, false)
            .await,
        Err(novex_api::application::ai_models::AiModelApplicationError::InvalidConfig(_))
    ));

    let mut image = image_model("Image With Context");
    image.context_window = Some(128000);
    image.tokenizer_profile_key = Some("openai.o200k".to_string());
    image.tokenizer_profile_version = Some("1.0.0".to_string());
    assert!(matches!(
        repository.create(image).await,
        Err(AiModelRepositoryError::InvalidConfig(_))
    ));

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn changing_model_type_reconciles_old_and_new_type_defaults() {
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let repository = PostgresAiModelRepository::new(pool.clone());
    let text_default = repository.create(text_model("Text A")).await.unwrap();
    let text_replacement = repository.create(text_model("Text B")).await.unwrap();
    let image_default = repository.create(image_model("Image A")).await.unwrap();

    let mut update = update_from(&text_default);
    update.model_type = ModelType::Image;
    update.api_protocol = ApiProtocol::OpenAiImages;
    update.reasoning_effort = None;
    update.max_output_tokens = None;
    update.context_window = None;
    update.tokenizer_profile_key = None;
    update.tokenizer_profile_version = None;
    update.settings = json!({
        "supported_sizes": ["1024x1024"],
        "default_size": "1024x1024",
        "max_images_per_request": 4
    });
    update.replacement_model_id = Some(text_replacement.id);
    let moved = repository.update(text_default.id, update).await.unwrap();

    assert_eq!(moved.model_type, ModelType::Image);
    assert!(
        !moved.is_default,
        "existing image default must remain stable"
    );
    assert!(
        repository
            .get(text_replacement.id)
            .await
            .unwrap()
            .is_default
    );
    assert!(repository.get(image_default.id).await.unwrap().is_default);

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn create_filter_and_replace_default_are_type_scoped_and_atomic() {
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let repository = PostgresAiModelRepository::new(pool.clone());
    let text_a = repository.create(text_model("Text A")).await.unwrap();
    let text_b = repository.create(text_model("Text B")).await.unwrap();
    let image = repository.create(image_model("Image A")).await.unwrap();

    assert!(text_a.is_default);
    assert!(!text_b.is_default);
    assert!(image.is_default);

    let replaced = repository
        .set_default(text_b.id, text_b.version)
        .await
        .unwrap();
    assert!(replaced.is_default);
    assert_eq!(replaced.version, text_b.version + 1);
    assert!(!repository.get(text_a.id).await.unwrap().is_default);

    let listed = repository
        .list(AiModelListFilter {
            model_type: Some(ModelType::Text),
            provider_name: Some("openai".to_string()),
            api_protocol: Some(ApiProtocol::OpenAiResponses),
            search: Some("Text B".to_string()),
            ..AiModelListFilter::default()
        })
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, text_b.id);

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn update_preserves_blank_credentials_and_rejects_stale_versions() {
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let repository = PostgresAiModelRepository::new(pool.clone());
    let model = repository.create(text_model("Text A")).await.unwrap();
    let mut update = update_from(&model);
    update.display_name = "Text A Updated".to_string();
    let updated = repository.update(model.id, update).await.unwrap();
    assert_eq!(updated.display_name, "Text A Updated");
    assert_eq!(updated.api_key, "test-key-original");
    assert_eq!(updated.version, model.version + 1);

    let error = repository
        .update(model.id, update_from(&model))
        .await
        .expect_err("stale model version must not overwrite newer data");
    assert!(matches!(error, AiModelRepositoryError::VersionConflict(id) if id == model.id));

    let runtime = repository
        .resolve_enabled(model.id, ModelType::Text)
        .await
        .unwrap();
    assert_eq!(runtime.api_key, "test-key-original");
    assert_eq!(runtime.snapshot.display_name, "Text A Updated");
    assert!(matches!(
        repository.resolve_enabled(model.id, ModelType::Image).await,
        Err(AiModelRepositoryError::TypeMismatch { .. })
    ));

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn disabling_default_requires_replacement_unless_it_is_the_only_enabled_model() {
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let repository = PostgresAiModelRepository::new(pool.clone());
    let text_a = repository.create(text_model("Text A")).await.unwrap();
    let text_b = repository.create(text_model("Text B")).await.unwrap();

    let error = repository
        .change_status(ChangeAiModelStatusInput {
            id: text_a.id,
            version: text_a.version,
            status: AiModelStatus::Disabled,
            replacement_model_id: None,
            allow_no_default: false,
        })
        .await
        .expect_err("default must be replaced while another enabled model exists");
    assert!(matches!(error, AiModelRepositoryError::ReplacementRequired(id) if id == text_a.id));

    let disabled_a = repository
        .change_status(ChangeAiModelStatusInput {
            id: text_a.id,
            version: text_a.version,
            status: AiModelStatus::Disabled,
            replacement_model_id: Some(text_b.id),
            allow_no_default: false,
        })
        .await
        .unwrap();
    assert_eq!(disabled_a.status, AiModelStatus::Disabled);
    let default_b = repository.get(text_b.id).await.unwrap();
    assert!(default_b.is_default);

    let disabled_b = repository
        .change_status(ChangeAiModelStatusInput {
            id: default_b.id,
            version: default_b.version,
            status: AiModelStatus::Disabled,
            replacement_model_id: None,
            allow_no_default: true,
        })
        .await
        .unwrap();
    assert!(!disabled_b.is_default);
    assert!(repository
        .list_enabled_options(ModelType::Text)
        .await
        .unwrap()
        .is_empty());

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn deletion_is_physical_without_references_and_logical_with_history() {
    let (admin_pool, pool, database_name) = migrated_pool().await;
    let repository = PostgresAiModelRepository::new(pool.clone());
    let disposable = repository.create(text_model("Disposable")).await.unwrap();
    let outcome = repository
        .delete(DeleteAiModelInput {
            id: disposable.id,
            version: disposable.version,
            replacement_model_id: None,
            allow_no_default: true,
        })
        .await
        .unwrap();
    assert_eq!(outcome, DeleteAiModelOutcome::Physical);
    assert!(matches!(
        repository.get(disposable.id).await,
        Err(AiModelRepositoryError::NotFound(id)) if id == disposable.id
    ));

    let referenced = repository.create(text_model("Referenced")).await.unwrap();
    sqlx::query(
        "INSERT INTO agent_runs (agent_type, status, model_id) VALUES ('script', 'succeeded', $1)",
    )
    .bind(referenced.id)
    .execute(&pool)
    .await
    .unwrap();
    let outcome = repository
        .delete(DeleteAiModelInput {
            id: referenced.id,
            version: referenced.version,
            replacement_model_id: None,
            allow_no_default: true,
        })
        .await
        .unwrap();
    let DeleteAiModelOutcome::Logical(deleted) = outcome else {
        panic!("referenced model must be logically deleted");
    };
    assert_eq!(deleted.status, AiModelStatus::Deleted);
    assert!(deleted.deleted_at.is_some());
    assert!(matches!(
        repository.resolve_enabled(referenced.id, ModelType::Text).await,
        Err(AiModelRepositoryError::Disabled(id)) if id == referenced.id
    ));

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
