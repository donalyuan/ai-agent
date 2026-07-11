use novex_api::repositories::{
    AssetCandidateSource, AssetCandidateStatus, AssetCandidateType, AssetGenerationProvider,
    AssetGenerationRepository, AssetGenerationRepositoryError, AssetGenerationTaskStatus,
    AssetGenerationTaskType, CreateAssetCandidateInput, CreateAssetGenerationTaskInput,
    CreateMaterialInput, MaterialRepository, MaterialRepositoryError, MaterialStatus, MaterialType,
    PostgresAssetGenerationRepository, PostgresMaterialRepository,
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
        .expect("temporary asset generation repository database should be created");
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
    let database_name = format!("video_agent_asset_repo_test_{}", suffix);
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
        .expect("temporary asset generation repository database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&test_pool)
        .await
        .expect("migrations should run for asset generation repository test database");

    (admin_pool, test_pool, database_name)
}

async fn insert_project(pool: &PgPool, id: Uuid) {
    sqlx::query(
        r#"
        INSERT INTO projects (id, name, positioning, description)
        VALUES ($1, '测试账号', '', '')
        "#,
    )
    .bind(id)
    .execute(pool)
    .await
    .expect("project fixture should be inserted");
}

async fn insert_script(pool: &PgPool, id: Uuid, project_id: Uuid) {
    sqlx::query(
        r#"
        INSERT INTO scripts (id, project_id, title, hook, content, status)
        VALUES ($1, $2, 'AI 图片脚本', '开场钩子', $3, 'draft')
        "#,
    )
    .bind(id)
    .bind(project_id)
    .bind(json!({ "topic": "AI 图片生成" }))
    .execute(pool)
    .await
    .expect("script fixture should be inserted");
}

async fn insert_scene(pool: &PgPool, id: Uuid, script_id: Uuid, sequence: i32) {
    sqlx::query(
        r#"
        INSERT INTO scenes (
            id, script_id, sequence, narration, visual_description, emotion, duration_sec
        )
        VALUES ($1, $2, $3, '旁白', '画面指令', '平静', 8)
        "#,
    )
    .bind(id)
    .bind(script_id)
    .bind(sequence)
    .execute(pool)
    .await
    .expect("scene fixture should be inserted");
}

fn asset_task_input(
    project_id: Uuid,
    script_id: Uuid,
    scene_id: Uuid,
    status: AssetGenerationTaskStatus,
) -> CreateAssetGenerationTaskInput {
    CreateAssetGenerationTaskInput {
        project_id,
        script_id: Some(script_id),
        scene_id: Some(scene_id),
        model_id: None,
        provider: AssetGenerationProvider::GptImage2,
        task_type: AssetGenerationTaskType::ImageCandidates,
        status,
        candidate_count: 1,
        reference_material_ids: Vec::new(),
        idempotency_key: None,
        params: json!({ "image_candidates_per_scene": 1 }),
    }
}

#[tokio::test]
async fn asset_repository_dismisses_only_failed_tasks_and_hides_failed_candidates() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let repository = PostgresAssetGenerationRepository::new(test_pool.clone());
    let project_id = Uuid::new_v4();
    let script_id = Uuid::new_v4();
    let scene_id = Uuid::new_v4();
    insert_project(&test_pool, project_id).await;
    insert_script(&test_pool, script_id, project_id).await;
    insert_scene(&test_pool, scene_id, script_id, 1).await;

    let failed_task = repository
        .create_task(asset_task_input(
            project_id,
            script_id,
            scene_id,
            AssetGenerationTaskStatus::Failed,
        ))
        .await
        .expect("failed task fixture should be created");
    let failed_candidate = repository
        .create_candidate(CreateAssetCandidateInput {
            project_id,
            script_id,
            scene_id,
            material_id: None,
            candidate_type: AssetCandidateType::Image,
            source: AssetCandidateSource::AiGenerated,
            rank: 1,
            generation_task_id: Some(failed_task.id),
            metadata: json!({ "error_message": "provider denied image generation" }),
        })
        .await
        .expect("failed candidate fixture should be created");
    sqlx::query("UPDATE scene_asset_candidates SET status = 'failed' WHERE id = $1")
        .bind(failed_candidate.id)
        .execute(&test_pool)
        .await
        .expect("candidate fixture should become failed");

    let visible_task = repository
        .create_task(asset_task_input(
            project_id,
            script_id,
            scene_id,
            AssetGenerationTaskStatus::Completed,
        ))
        .await
        .expect("completed task fixture should be created");
    let visible_candidate = repository
        .create_candidate(CreateAssetCandidateInput {
            project_id,
            script_id,
            scene_id,
            material_id: None,
            candidate_type: AssetCandidateType::Image,
            source: AssetCandidateSource::AiGenerated,
            rank: 2,
            generation_task_id: Some(visible_task.id),
            metadata: json!({}),
        })
        .await
        .expect("visible candidate fixture should be created");

    let dismissed = repository
        .dismiss_task(failed_task.id)
        .await
        .expect("failed task should be dismissible");
    let dismissed_at = dismissed
        .dismissed_at
        .expect("dismissed task should expose audit timestamp");
    let dismissed_again = repository
        .dismiss_task(failed_task.id)
        .await
        .expect("dismissing the same task should be idempotent");
    assert_eq!(dismissed_again.dismissed_at, Some(dismissed_at));

    let listed_tasks = repository
        .list_tasks(script_id)
        .await
        .expect("visible tasks should list");
    assert_eq!(listed_tasks.len(), 1);
    assert_eq!(listed_tasks[0].id, visible_task.id);
    let listed_candidates = repository
        .list_candidates(script_id)
        .await
        .expect("visible candidates should list");
    assert_eq!(listed_candidates.len(), 1);
    assert_eq!(listed_candidates[0].id, visible_candidate.id);

    let persisted = sqlx::query_as::<_, (String, Option<String>, Option<chrono::DateTime<chrono::Utc>>)>(
        "SELECT status, error_message, dismissed_at FROM asset_generation_tasks WHERE id = $1",
    )
    .bind(failed_task.id)
    .fetch_one(&test_pool)
    .await
    .expect("dismissed task should remain persisted");
    assert_eq!(persisted.0, "failed");
    assert_eq!(persisted.2, Some(dismissed_at));
    assert!(sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM scene_asset_candidates WHERE id = $1)"
    )
    .bind(failed_candidate.id)
    .fetch_one(&test_pool)
    .await
    .expect("dismissed failed candidate audit row should remain"));

    let error = repository
        .dismiss_task(visible_task.id)
        .await
        .expect_err("completed task should not be dismissible");
    assert!(matches!(
        error,
        AssetGenerationRepositoryError::TaskNotDismissible(id) if id == visible_task.id
    ));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn asset_repository_creates_tasks_candidates_and_selects_one_per_scene() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let asset_repository = PostgresAssetGenerationRepository::new(test_pool.clone());
    let material_repository = PostgresMaterialRepository::new(test_pool.clone());
    let project_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let script_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
    let scene_id = Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap();
    insert_project(&test_pool, project_id).await;
    insert_script(&test_pool, script_id, project_id).await;
    insert_scene(&test_pool, scene_id, script_id, 1).await;

    let first_material = material_repository
        .create_material(CreateMaterialInput {
            project_id,
            material_type: MaterialType::Image,
            file_url: "/assets/existing/first.png".to_string(),
            file_name: "first.png".to_string(),
            thumbnail_url: None,
            tags: vec!["人物".to_string()],
            metadata: json!({ "source": "existing" }),
        })
        .await
        .expect("first material should be created");
    let second_material = material_repository
        .create_material(CreateMaterialInput {
            project_id,
            material_type: MaterialType::Image,
            file_url: "/assets/existing/second.png".to_string(),
            file_name: "second.png".to_string(),
            thumbnail_url: None,
            tags: vec!["人物".to_string()],
            metadata: json!({ "source": "existing" }),
        })
        .await
        .expect("second material should be created");

    let task = asset_repository
        .create_task(CreateAssetGenerationTaskInput {
            project_id,
            script_id: Some(script_id),
            scene_id: Some(scene_id),
            model_id: None,
            provider: AssetGenerationProvider::GptImage2,
            task_type: AssetGenerationTaskType::ImageCandidates,
            status: AssetGenerationTaskStatus::Pending,
            candidate_count: 3,
            reference_material_ids: vec![first_material.id],
            idempotency_key: None,
            params: json!({ "prompt": "生成分镜候选图" }),
        })
        .await
        .expect("asset generation task should be created");
    assert_eq!(task.status, AssetGenerationTaskStatus::Pending);
    let completed_task = asset_repository
        .update_task_status(
            task.id,
            AssetGenerationTaskStatus::Completed,
            json!({ "created_material_count": 2 }),
            None,
        )
        .await
        .expect("asset generation task status should update");
    assert_eq!(completed_task.status, AssetGenerationTaskStatus::Completed);
    assert_eq!(completed_task.result["created_material_count"], 2);
    assert!(completed_task.error_message.is_none());

    let first_candidate = asset_repository
        .create_candidate(CreateAssetCandidateInput {
            project_id,
            script_id,
            scene_id,
            material_id: Some(first_material.id),
            candidate_type: AssetCandidateType::Image,
            source: AssetCandidateSource::ExistingMaterial,
            rank: 1,
            generation_task_id: Some(task.id),
            metadata: json!({ "reason": "人物一致" }),
        })
        .await
        .expect("first candidate should be created");
    let second_candidate = asset_repository
        .create_candidate(CreateAssetCandidateInput {
            project_id,
            script_id,
            scene_id,
            material_id: Some(second_material.id),
            candidate_type: AssetCandidateType::Image,
            source: AssetCandidateSource::ExistingMaterial,
            rank: 2,
            generation_task_id: Some(task.id),
            metadata: json!({ "reason": "构图更好" }),
        })
        .await
        .expect("second candidate should be created");

    let selected_first = asset_repository
        .select_candidate(scene_id, first_candidate.id)
        .await
        .expect("first candidate should be selectable");
    assert_eq!(selected_first.status, AssetCandidateStatus::Selected);

    let selected_second = asset_repository
        .select_candidate(scene_id, second_candidate.id)
        .await
        .expect("second candidate should replace first selection");
    assert_eq!(selected_second.status, AssetCandidateStatus::Selected);

    let candidates = asset_repository
        .list_candidates(script_id)
        .await
        .expect("candidates should list");
    let selected_candidates: Vec<_> = candidates
        .iter()
        .filter(|candidate| candidate.status == AssetCandidateStatus::Selected)
        .collect();
    assert_eq!(selected_candidates.len(), 1);
    assert_eq!(selected_candidates[0].id, second_candidate.id);
    assert_eq!(
        candidates
            .iter()
            .find(|candidate| candidate.id == first_candidate.id)
            .expect("first candidate should still exist")
            .status,
        AssetCandidateStatus::Candidate
    );

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn asset_repository_rejects_archived_material_selection() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let asset_repository = PostgresAssetGenerationRepository::new(test_pool.clone());
    let material_repository = PostgresMaterialRepository::new(test_pool.clone());
    let project_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let script_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
    let scene_id = Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap();
    insert_project(&test_pool, project_id).await;
    insert_script(&test_pool, script_id, project_id).await;
    insert_scene(&test_pool, scene_id, script_id, 1).await;

    let material = material_repository
        .create_material(CreateMaterialInput {
            project_id,
            material_type: MaterialType::Image,
            file_url: "/assets/existing/archived.png".to_string(),
            file_name: "archived.png".to_string(),
            thumbnail_url: None,
            tags: vec!["历史人物".to_string()],
            metadata: json!({ "source": "existing" }),
        })
        .await
        .expect("archived material should be created");
    let candidate = asset_repository
        .create_candidate(CreateAssetCandidateInput {
            project_id,
            script_id,
            scene_id,
            material_id: Some(material.id),
            candidate_type: AssetCandidateType::Image,
            source: AssetCandidateSource::ExistingMaterial,
            rank: 1,
            generation_task_id: None,
            metadata: json!({ "reason": "已归档素材不应可选" }),
        })
        .await
        .expect("candidate should be recorded before material archive");
    material_repository
        .update_material_status(material.id, MaterialStatus::Archived)
        .await
        .expect("unselected candidate material should be archived");

    let error = asset_repository
        .select_candidate(scene_id, candidate.id)
        .await
        .expect_err("archived material candidate should not be selectable");
    assert!(matches!(
        error,
        AssetGenerationRepositoryError::CandidateNotSelectable(id) if id == candidate.id
    ));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn asset_repository_rejects_cross_project_material_candidate() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let asset_repository = PostgresAssetGenerationRepository::new(test_pool.clone());
    let material_repository = PostgresMaterialRepository::new(test_pool.clone());
    let project_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let other_project_id = Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap();
    let script_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
    let scene_id = Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap();
    insert_project(&test_pool, project_id).await;
    insert_project(&test_pool, other_project_id).await;
    insert_script(&test_pool, script_id, project_id).await;
    insert_scene(&test_pool, scene_id, script_id, 1).await;

    let other_project_material = material_repository
        .create_material(CreateMaterialInput {
            project_id: other_project_id,
            material_type: MaterialType::Image,
            file_url: "/assets/existing/other-project.png".to_string(),
            file_name: "other-project.png".to_string(),
            thumbnail_url: None,
            tags: vec!["人物".to_string()],
            metadata: json!({ "source": "other_project" }),
        })
        .await
        .expect("other project material should be created");

    let error = asset_repository
        .create_candidate(CreateAssetCandidateInput {
            project_id,
            script_id,
            scene_id,
            material_id: Some(other_project_material.id),
            candidate_type: AssetCandidateType::Image,
            source: AssetCandidateSource::ExistingMaterial,
            rank: 1,
            generation_task_id: None,
            metadata: json!({ "reason": "跨账号素材不应成为候选" }),
        })
        .await
        .expect_err("cross-project material candidate should be rejected");
    assert!(matches!(
        error,
        AssetGenerationRepositoryError::InvalidCandidateRelation(_)
    ));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}

#[tokio::test]
async fn material_repository_rejects_archiving_selected_asset_candidate_material() {
    let (admin_pool, test_pool, database_name) = migrated_pool().await;
    let asset_repository = PostgresAssetGenerationRepository::new(test_pool.clone());
    let material_repository = PostgresMaterialRepository::new(test_pool.clone());
    let project_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
    let script_id = Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap();
    let scene_id = Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap();
    insert_project(&test_pool, project_id).await;
    insert_script(&test_pool, script_id, project_id).await;
    insert_scene(&test_pool, scene_id, script_id, 1).await;

    let material = material_repository
        .create_material(CreateMaterialInput {
            project_id,
            material_type: MaterialType::Image,
            file_url: "/assets/existing/selected.png".to_string(),
            file_name: "selected.png".to_string(),
            thumbnail_url: None,
            tags: vec!["人物".to_string()],
            metadata: json!({ "source": "existing" }),
        })
        .await
        .expect("material should be created");
    let candidate = asset_repository
        .create_candidate(CreateAssetCandidateInput {
            project_id,
            script_id,
            scene_id,
            material_id: Some(material.id),
            candidate_type: AssetCandidateType::Image,
            source: AssetCandidateSource::ExistingMaterial,
            rank: 1,
            generation_task_id: None,
            metadata: json!({}),
        })
        .await
        .expect("candidate should be created");
    asset_repository
        .select_candidate(scene_id, candidate.id)
        .await
        .expect("candidate should be selected");

    let error = material_repository
        .update_material_status(material.id, MaterialStatus::Archived)
        .await
        .expect_err("selected candidate material should not be archived");
    assert!(matches!(
        error,
        MaterialRepositoryError::MaterialInUseAsSelectedCandidate(id) if id == material.id
    ));

    test_pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
