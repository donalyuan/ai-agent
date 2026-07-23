use novex_api::repositories::{
    PostgresPublicationRepository, PublicationRepositoryError, SavePublicationTarget,
};
use serde_json::json;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

mod support;
use support::test_database::TestDatabase;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@biga-postgres:5432/video_agent".into())
}

fn with_database_name(url: &str, name: &str) -> String {
    let query_start = url.find('?');
    let (base, query) = query_start
        .map(|i| (&url[..i], &url[i..]))
        .unwrap_or((url, ""));
    let slash = base.rfind('/').unwrap();
    format!("{}{}{}", &base[..=slash], name, query)
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase) {
    let base = database_url();
    let name = format!("publication_database_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base, "postgres");
    let test_url = with_database_name(&base, &name);
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{name}""#))
        .execute(&admin)
        .await
        .unwrap();
    let database = TestDatabase::new(&admin_url, &name);
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin, pool, database)
}

async fn seed_handoff(pool: &PgPool) -> (Uuid, Uuid) {
    let project: Uuid =
        sqlx::query_scalar("INSERT INTO projects (name) VALUES ('发布测试') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap();
    let script: Uuid = sqlx::query_scalar("INSERT INTO scripts (project_id,title,hook,content) VALUES ($1,'脚本','hook','{}') RETURNING id").bind(project).fetch_one(pool).await.unwrap();
    let work: Uuid = sqlx::query_scalar("INSERT INTO works (project_id,script_id,title,status) VALUES ($1,$2,'作品','succeeded') RETURNING id").bind(project).bind(script).fetch_one(pool).await.unwrap();
    let version: Uuid = sqlx::query_scalar("INSERT INTO work_versions (work_id,version_no,status,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot,prompt_snapshot,timeline_snapshot) VALUES ($1,1,'completed','v1','{}','{}','{}','{}','{}') RETURNING id").bind(work).fetch_one(pool).await.unwrap();
    let artifact: Uuid = sqlx::query_scalar("INSERT INTO work_artifacts (work_version_id,role,file_name,storage_path,mime_type,size_bytes,sha256) VALUES ($1,'final_video','final.mp4','works/final.mp4','video/mp4',1,$2) RETURNING id").bind(version).bind("a".repeat(64)).fetch_one(pool).await.unwrap();
    let handoff: Uuid = sqlx::query_scalar("INSERT INTO publication_handoffs (work_id,work_version_id,final_video_artifact_id,idempotency_key) VALUES ($1,$2,$3,'handoff') RETURNING id").bind(work).bind(version).bind(artifact).fetch_one(pool).await.unwrap();
    (handoff, artifact)
}

#[tokio::test]
async fn publication_repository_is_idempotent_revision_safe_and_platform_isolated() {
    let (admin, pool, _database) = migrated_pool().await;
    let (handoff, _) = seed_handoff(&pool).await;
    let repository = PostgresPublicationRepository::new(pool.clone());
    let first = repository.get_or_create_plan(handoff).await.unwrap();
    let repeated = repository.get_or_create_plan(handoff).await.unwrap();
    assert_eq!(first.id, repeated.id);
    assert!(first.created && !repeated.created);

    let input = |title: &str| SavePublicationTarget {
        title: title.into(),
        body: "正文".into(),
        tags: json!(["标签"]),
        cover_artifact_id: None,
        planned_at: None,
    };
    let douyin = repository
        .save_target(first.id, "douyin", None, "create-douyin", input("抖音"))
        .await
        .unwrap();
    let red = repository
        .save_target(first.id, "xiaohongshu", None, "create-red", input("小红书"))
        .await
        .unwrap();
    assert_ne!(douyin.id, red.id);
    let left = repository.clone();
    let right = repository.clone();
    let left_input = SavePublicationTarget {
        title: "小红书左".into(),
        body: "正文".into(),
        tags: json!([]),
        cover_artifact_id: None,
        planned_at: None,
    };
    let right_input = SavePublicationTarget {
        title: "小红书右".into(),
        body: "正文".into(),
        tags: json!([]),
        cover_artifact_id: None,
        planned_at: None,
    };
    let (left_result, right_result) = tokio::join!(
        left.save_target(first.id, "xiaohongshu", Some(1), "red-left", left_input),
        right.save_target(first.id, "xiaohongshu", Some(1), "red-right", right_input)
    );
    assert!(
        left_result.is_ok() ^ right_result.is_ok(),
        "并发写入必须只有一个 revision 成功"
    );
    let updated = repository
        .save_target(
            first.id,
            "douyin",
            Some(1),
            "update-douyin",
            input("抖音新版"),
        )
        .await
        .unwrap();
    assert_eq!(updated.draft_revision, 2);
    assert!(matches!(
        repository
            .save_target(first.id, "douyin", Some(1), "stale-douyin", input("过期"))
            .await,
        Err(PublicationRepositoryError::Conflict(_))
    ));
    assert_eq!(
        repository.details(first.id).await.unwrap()["targets"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    pool.close().await;
    admin.close().await;
}

#[tokio::test]
async fn publication_database_rejects_credentials_paths_invalid_results_and_event_mutation() {
    let (admin, pool, _database) = migrated_pool().await;
    let (handoff, _) = seed_handoff(&pool).await;
    let plan: Uuid =
        sqlx::query_scalar("INSERT INTO publication_plans (handoff_id) VALUES ($1) RETURNING id")
            .bind(handoff)
            .fetch_one(&pool)
            .await
            .unwrap();
    let target: Uuid = sqlx::query_scalar("INSERT INTO publication_targets (publication_plan_id,platform) VALUES ($1,'douyin') RETURNING id").bind(plan).fetch_one(&pool).await.unwrap();
    assert!(
        sqlx::query("UPDATE publication_targets SET result_snapshot=$2 WHERE id=$1")
            .bind(target)
            .bind(json!({"token":"secret"}))
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("INSERT INTO publication_events (publication_target_id,event_type,payload) VALUES ($1,'created',$2)").bind(target).bind(json!({"url":"https://example.test/file?signature=x"})).execute(&pool).await.is_err());
    let event: Uuid = sqlx::query_scalar("INSERT INTO publication_events (publication_target_id,event_type) VALUES ($1,'created') RETURNING id").bind(target).fetch_one(&pool).await.unwrap();
    assert!(
        sqlx::query("UPDATE publication_events SET event_type='cancelled' WHERE id=$1")
            .bind(event)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE publication_targets SET status='published' WHERE id=$1")
            .bind(target)
            .execute(&pool)
            .await
            .is_err()
    );
    pool.close().await;
    admin.close().await;
}
