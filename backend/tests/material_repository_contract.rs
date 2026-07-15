use novex_api::repositories::{
    AudioUsage, CreateMaterialInput, MaterialListFilter, MaterialRepository,
    MaterialRepositoryError, MaterialSourceFilter, MaterialStatus, MaterialStatusFilter,
    MaterialType, PostgresMaterialRepository, UpdateMaterialInput,
};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

mod support;

use support::test_database::TestDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://postgres:postgres@biga-postgres:5432/video_agent".to_string()
    })
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

async fn create_database(
    admin_pool: &PgPool,
    admin_url: &str,
    database_name: &str,
) -> TestDatabase {
    let query = format!(r#"CREATE DATABASE "{}""#, database_name);
    sqlx::query(&query)
        .execute(admin_pool)
        .await
        .expect("temporary material repository database should be created");
    TestDatabase::new(admin_url, database_name)
}

async fn drop_database(admin_pool: &PgPool, database_name: &str) {
    let disconnect = format!(
        r#"
        SELECT pg_terminate_backend(pid)
        FROM pg_stat_activity
        WHERE datname = '{}'
        "#,
        database_name
    );
    let drop = format!(r#"DROP DATABASE IF EXISTS "{}""#, database_name);

    let _ = sqlx::query(&disconnect).execute(admin_pool).await;
    let _ = sqlx::query(&drop).execute(admin_pool).await;
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let suffix = Uuid::new_v4().simple().to_string();
    let database_name = format!("video_agent_material_repo_test_{}", suffix);
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);

    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    let database_name = create_database(&admin_pool, &admin_url, &database_name).await;

    let test_pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&test_url)
        .await
        .expect("temporary material repository database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for material repository test database");

    (admin_pool, test_pool, database_name)
}

async fn insert_project(pool: &PgPool, id: Uuid, name: &str) {
    sqlx::query(
        r#"
        INSERT INTO projects (id, name, positioning, description)
        VALUES ($1, $2, '', '')
        "#,
    )
    .bind(id)
    .bind(name)
    .execute(pool)
    .await
    .expect("project fixture should be inserted");
}

#[tokio::test]
async fn material_repository_creates_filters_archives_and_restores_materials() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let repository = PostgresMaterialRepository::new(test_pool.clone());
    let project_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    insert_project(&test_pool, project_id, "测试账号").await;

    let video = repository
        .create_material(CreateMaterialInput {
            project_id,
            material_type: MaterialType::Video,
            file_url: "https://cdn.example.com/videos/demo.mp4".to_string(),
            file_name: "AI 工具演示.mp4".to_string(),
            thumbnail_url: Some("https://cdn.example.com/covers/demo.jpg".to_string()),
            tags: vec!["教程".to_string(), "素材".to_string()],
            metadata: json!({ "source_note": "人工整理" }),
        })
        .await
        .expect("video material should be created");
    let subtitle = repository
        .create_material(CreateMaterialInput {
            project_id,
            material_type: MaterialType::Subtitle,
            file_url: "https://cdn.example.com/subtitles/demo.vtt".to_string(),
            file_name: "demo.vtt".to_string(),
            thumbnail_url: None,
            tags: vec!["字幕".to_string(), "中英双语".to_string()],
            metadata: json!({ "language": "zh-CN", "subtitle_format": "vtt" }),
        })
        .await
        .expect("subtitle material should be created");

    assert_eq!(video.status, MaterialStatus::Active);
    assert_eq!(
        video.metadata["thumbnail_url"],
        "https://cdn.example.com/covers/demo.jpg"
    );

    let active = repository
        .list_materials(project_id, MaterialListFilter::default())
        .await
        .expect("active materials should list");
    assert_eq!(active.len(), 2);
    assert!(active.iter().any(|material| material.id == subtitle.id));

    let archived = repository
        .update_material_status(video.id, MaterialStatus::Archived)
        .await
        .expect("material should archive");
    assert_eq!(archived.status, MaterialStatus::Archived);
    let active_after_archive = repository
        .list_materials(project_id, MaterialListFilter::default())
        .await
        .expect("active materials after archive should list");
    assert_eq!(active_after_archive.len(), 1);
    assert_eq!(active_after_archive[0].id, subtitle.id);

    let archived_only = repository
        .list_materials(
            project_id,
            MaterialListFilter {
                status: MaterialStatusFilter::Archived,
                ..MaterialListFilter::default()
            },
        )
        .await
        .expect("archived materials should list");
    assert_eq!(archived_only.len(), 1);
    assert_eq!(archived_only[0].id, video.id);

    repository
        .update_material_status(video.id, MaterialStatus::Active)
        .await
        .expect("material should restore");
    let subtitle_by_filters = repository
        .list_materials(
            project_id,
            MaterialListFilter {
                material_type: Some(MaterialType::Subtitle),
                q: Some("demo".to_string()),
                tag: Some("字幕".to_string()),
                ..MaterialListFilter::default()
            },
        )
        .await
        .expect("subtitle material should list by filters");
    assert_eq!(subtitle_by_filters.len(), 1);
    assert_eq!(subtitle_by_filters[0].id, subtitle.id);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_repository_rejects_cross_project_update() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let repository = PostgresMaterialRepository::new(test_pool.clone());
    let project_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let other_project_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
    insert_project(&test_pool, project_id, "账号 A").await;
    insert_project(&test_pool, other_project_id, "账号 B").await;

    let material = repository
        .create_material(CreateMaterialInput {
            project_id,
            material_type: MaterialType::Image,
            file_url: "https://cdn.example.com/images/demo.png".to_string(),
            file_name: "demo.png".to_string(),
            thumbnail_url: None,
            tags: vec!["图片".to_string()],
            metadata: json!({}),
        })
        .await
        .expect("image material should be created");

    let error = repository
        .update_material(
            material.id,
            UpdateMaterialInput {
                project_id: other_project_id,
                material_type: MaterialType::Image,
                file_url: "https://cdn.example.com/images/changed.png".to_string(),
                file_name: "changed.png".to_string(),
                thumbnail_url: None,
                tags: vec!["图片".to_string()],
                metadata: json!({}),
            },
        )
        .await
        .expect_err("cross-project update should be rejected");
    assert!(matches!(
        error,
        MaterialRepositoryError::MaterialNotFound(id) if id == material.id
    ));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_repository_filters_work_generation_snapshots_and_keeps_legacy_audio_visible() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let repository = PostgresMaterialRepository::new(test_pool.clone());
    let project_id = Uuid::new_v4();
    let work_id = Uuid::new_v4();
    let work_version_id = Uuid::new_v4();
    insert_project(&test_pool, project_id, "作品素材账号").await;

    let generated = repository
        .create_material(CreateMaterialInput {
            project_id,
            material_type: MaterialType::Audio,
            file_url: "/assets/generated/artifacts/tts.wav".to_string(),
            file_name: "作品配音.wav".to_string(),
            thumbnail_url: None,
            tags: vec!["TTS".to_string()],
            metadata: json!({
                "source": "work_generation",
                "storage_provider": "local",
                "audio_usage": "tts",
                "work_id": work_id,
                "work_version_id": work_version_id,
                "generation_run_id": Uuid::new_v4(),
                "generation_step_id": Uuid::new_v4(),
                "artifact_role": "tts_audio",
                "model_snapshot": {"model_id": Uuid::new_v4(), "upstream_model": "doubao-tts"},
                "voice_snapshot": {"voice_id": "zh_female_1", "language": "zh-CN"},
                "prompt_snapshot": {"text_summary": "测试配音"},
                "timeline_snapshot": {"version": "timeline-v1"},
                "resource_usage": {"duration_sec": 1.0, "character_count": 4}
            }),
        })
        .await
        .expect("work-generated audio should be created");
    let legacy = repository
        .create_material(CreateMaterialInput {
            project_id,
            material_type: MaterialType::Audio,
            file_url: "https://cdn.example.com/legacy.mp3".to_string(),
            file_name: "历史音频.mp3".to_string(),
            thumbnail_url: None,
            tags: Vec::new(),
            metadata: json!({}),
        })
        .await
        .expect("legacy audio without usage should remain valid");

    assert_eq!(generated.audio_usage, Some(AudioUsage::Tts));
    assert_eq!(generated.source.as_deref(), Some("work_generation"));
    assert_eq!(generated.work_id, Some(work_id));
    assert_eq!(generated.work_version_id, Some(work_version_id));
    assert_eq!(legacy.audio_usage, None);

    let filtered = repository
        .list_materials(
            project_id,
            MaterialListFilter {
                material_type: Some(MaterialType::Audio),
                audio_usage: Some(AudioUsage::Tts),
                source: Some(MaterialSourceFilter::WorkGeneration),
                work_id: Some(work_id),
                work_version_id: Some(work_version_id),
                ..MaterialListFilter::default()
            },
        )
        .await
        .expect("work production filters should compose");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, generated.id);

    let all_audio = repository
        .list_materials(
            project_id,
            MaterialListFilter {
                material_type: Some(MaterialType::Audio),
                ..MaterialListFilter::default()
            },
        )
        .await
        .expect("legacy audio should stay visible without an audio usage filter");
    assert_eq!(all_audio.len(), 2);
    assert!(all_audio.iter().any(|material| material.id == legacy.id));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn materials_database_constraint_rejects_nested_credentials() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let project_id = Uuid::new_v4();
    insert_project(&test_pool, project_id, "敏感信息约束账号").await;

    let result = sqlx::query(
        r#"
        INSERT INTO materials (project_id, material_type, file_url, file_name, metadata)
        VALUES ($1, 'audio', '/assets/generated/secret.wav', 'secret.wav', $2)
        "#,
    )
    .bind(project_id)
    .bind(json!({"model_snapshot": {"id_token": "must-not-persist"}}))
    .execute(&test_pool)
    .await;

    assert!(
        result.is_err(),
        "nested credential keys must be rejected by PostgreSQL"
    );
    let count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM materials WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(&test_pool)
            .await
            .unwrap();
    assert_eq!(count, 0);

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
