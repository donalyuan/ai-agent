use novex_api::application::production_media_validation::{
    build_production_media_validation_plan, MediaAnalysisCapability,
    ProductionMediaValidationError, ProductionMediaValidationLimits,
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

mod support;

use support::test_database::TestDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@biga-postgres:5432/video_agent".into())
}

fn with_database_name(database_url: &str, database_name: &str) -> String {
    let slash = database_url.rfind('/').unwrap();
    format!("{}{}", &database_url[..=slash], database_name)
}

async fn database() -> (PgPool, TestDatabase) {
    let base_url = database_url();
    let database_name = format!("production_media_auth_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{}""#, database_name))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&with_database_name(&base_url, &database_name))
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (pool, TestDatabase::new(&admin_url, &database_name))
}

async fn insert_provider_models(pool: &PgPool) {
    sqlx::query(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, protocol_version,
            auth_scheme, request_base_url, upstream_model, api_key, settings,
            status, is_default
        ) VALUES (
            'Media auth video', 'video', 'fixture', 'volcengine_ark_video', 'v1',
            'bearer', 'https://example.invalid/video', 'fixture-video', 'secret',
            '{"min_duration_seconds":4,"max_duration_seconds":15,"max_reference_images":1,"max_prompt_chars":500,"aspect_ratios":["9:16"],"resolutions":["1080p"],"generate_audio":true}',
            'enabled', TRUE
        )
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO ai_models (
            display_name, model_type, provider_name, api_protocol, protocol_version,
            auth_scheme, request_base_url, upstream_model, api_key,
            catalog_access_key, catalog_secret_key, settings,
            status, is_default
        ) VALUES (
            'Media auth TTS', 'speech', 'fixture', 'volcengine_tts_v3', 'v3',
            'api_key', 'https://example.invalid/tts', 'fixture-tts', 'secret',
            'fixture-catalog-ak', 'fixture-catalog-sk',
            '{"resource_id":"seed-tts-2.0","supported_audio_formats":["mp3"]}',
            'enabled', TRUE
        ), (
            'Media auth ASR', 'speech', 'fixture', 'volcengine_asr_v3', 'v3',
            'api_key', 'https://example.invalid/asr', 'fixture-asr', 'secret',
            NULL, NULL,
            '{"resource_id":"volc.seedasr.auc","supported_audio_formats":["mp3"]}',
            'enabled', TRUE
        )
        "#,
    )
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn real_media_validation_requires_exact_approval_and_complete_capabilities() {
    let (pool, _database) = database().await;
    insert_provider_models(&pool).await;
    let limits = ProductionMediaValidationLimits::conservative_v3();

    let blocked = build_production_media_validation_plan(&pool, None, limits.clone())
        .await
        .unwrap();
    assert!(!blocked.authorization_ready);
    assert_eq!(blocked.items.len(), 4);
    assert!(blocked
        .blockers
        .iter()
        .any(|blocker| blocker.contains("media_analysis")
            && blocker.contains("MediaEvidenceProvider")));
    assert!(matches!(
        blocked.approved_items(true, &blocked.authorization_digest),
        Err(ProductionMediaValidationError::CapabilityBlocked(_))
    ));

    let ready = build_production_media_validation_plan(
        &pool,
        Some(MediaAnalysisCapability {
            provider_key: "fixture-media-evidence".into(),
            configuration_fingerprint: "c".repeat(64),
            vision_capability_version: "fixture-vision@1".into(),
            audio_capability_version: "fixture-audio-asr@1".into(),
        }),
        limits,
    )
    .await
    .unwrap();
    assert!(ready.authorization_ready);
    assert_eq!(
        ready.authorization_state,
        "awaiting_explicit_user_confirmation"
    );
    assert_eq!(ready.authorization_digest.len(), 64);
    assert_eq!(ready.totals.max_real_calls, 4);
    assert_eq!(ready.totals.max_retries, 0);
    assert_eq!(ready.totals.max_cost_micros, 900_000);
    assert!(ready.items.iter().all(|item| {
        !item.approved_real_calls
            && item.blockers.is_empty()
            && item
                .model_binding
                .as_ref()
                .is_none_or(|binding| binding.configuration_fingerprint.len() == 64)
    }));
    assert!(matches!(
        ready.approved_items(false, &ready.authorization_digest),
        Err(ProductionMediaValidationError::ApprovalRequired)
    ));
    assert!(matches!(
        ready.approved_items(true, &"0".repeat(64)),
        Err(ProductionMediaValidationError::ApprovalRequired)
    ));
    assert!(ready
        .approved_items(true, &ready.authorization_digest)
        .unwrap()
        .iter()
        .all(|item| item.approved_real_calls));

    for table in [
        "work_generation_runs",
        "work_generation_steps",
        "sound_subtitle_tasks",
        "asset_generation_tasks",
        "media_evidence_snapshots",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "authorization planning must not create {table}");
    }
}
