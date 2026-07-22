use std::borrow::Cow;

use novex_api::repositories::{
    PostgresWorkGenerationRepository, WorkGenerationRepository, WorkPlanRecord,
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
    let (base, query) = query_start
        .map(|index| (&database_url[..index], &database_url[index..]))
        .unwrap_or((database_url, ""));
    let slash_index = base.rfind('/').unwrap();
    format!("{}{}{}", &base[..=slash_index], database_name, query)
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let database_name = format!("work_library_database_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
    let database = TestDatabase::new(&admin_url, &database_name);
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&test_url)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    (admin_pool, pool, database)
}

async fn pool_before_version_governance() -> (PgPool, PgPool, TestDatabase) {
    const GOVERNANCE_VERSION: i64 = 20260722030000;
    let base_url = database_url();
    let database_name = format!("work_library_pre_governance_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let test_url = with_database_name(&base_url, &database_name);
    let admin_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .unwrap();
    sqlx::query(&format!(r#"CREATE DATABASE "{database_name}""#))
        .execute(&admin_pool)
        .await
        .unwrap();
    let database = TestDatabase::new(&admin_url, &database_name);
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&test_url)
        .await
        .unwrap();
    let all = sqlx::migrate!("./migrations");
    let before = sqlx::migrate::Migrator {
        migrations: Cow::Owned(
            all.iter()
                .filter(|migration| migration.version < GOVERNANCE_VERSION)
                .cloned()
                .collect(),
        ),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };
    before.run(&pool).await.unwrap();
    (admin_pool, pool, database)
}

async fn apply_version_governance(pool: &PgPool) {
    sqlx::raw_sql(include_str!(
        "../migrations/20260722030000_work_version_governance.sql"
    ))
    .execute(pool)
    .await
    .unwrap();
}

async fn table_exists(pool: &PgPool, table: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema='public' AND table_name=$1)",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_work(pool: &PgPool, suffix: &str) -> Uuid {
    let project_id: Uuid =
        sqlx::query_scalar("INSERT INTO projects (name) VALUES ($1) RETURNING id")
            .bind(format!("作品库项目-{suffix}"))
            .fetch_one(pool)
            .await
            .unwrap();
    let script_id: Uuid = sqlx::query_scalar(
        "INSERT INTO scripts (project_id,title,hook,content) VALUES ($1,$2,'hook','{}') RETURNING id",
    )
    .bind(project_id)
    .bind(format!("作品库脚本-{suffix}"))
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query_scalar(
        "INSERT INTO works (project_id,script_id,title) VALUES ($1,$2,$3) RETURNING id",
    )
    .bind(project_id)
    .bind(script_id)
    .bind(format!("作品库作品-{suffix}"))
    .fetch_one(pool)
    .await
    .unwrap()
}

fn work_plan(work_id: Uuid, marker: &str) -> WorkPlanRecord {
    WorkPlanRecord {
        id: Uuid::new_v4(),
        work_id,
        work_version_id: Uuid::nil(),
        plan_version: 1,
        status: "ready".into(),
        input_fingerprint: format!("{marker:0<64}"),
        llm_model_id: None,
        video_model_id: None,
        tts_model_id: None,
        capability_snapshot: json!({"marker": marker}),
        output_snapshot: json!({"aspect_ratio": "9:16", "marker": marker}),
        prompt_snapshot: json!({"full_prompt": marker}),
        timeline_snapshot: json!({"duration_seconds": 30}),
        resource_usage: json!({"video_task_count": 2}),
        warnings: json!([]),
    }
}

#[tokio::test]
async fn repeated_plan_saves_reuse_one_draft_version_and_create_plan_revisions() {
    let (admin_pool, pool, _database) = migrated_pool().await;
    let work_id = seed_work(&pool, "repeat-plan").await;
    let repository = PostgresWorkGenerationRepository::new(pool.clone());

    let first = repository
        .save_plan(
            work_id,
            "manifest-v1",
            json!({"revision": 1}),
            &work_plan(work_id, "first"),
        )
        .await
        .unwrap();
    let second = repository
        .save_plan(
            work_id,
            "manifest-v2",
            json!({"revision": 2}),
            &work_plan(work_id, "second"),
        )
        .await
        .unwrap();

    assert_eq!(first.work_version_id, second.work_version_id);
    assert_eq!((first.plan_version, second.plan_version), (1, 2));
    let versions: Vec<(Uuid, i32, serde_json::Value, serde_json::Value)> = sqlx::query_as(
        "SELECT id,version_no,input_snapshot,prompt_snapshot FROM work_versions WHERE work_id=$1",
    )
    .bind(work_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].1, 1);
    assert_eq!(versions[0].2, json!({"revision": 2}));
    assert_eq!(versions[0].3, json!({"full_prompt": "second"}));
    let plans: Vec<(i32, String)> = sqlx::query_as(
        "SELECT plan_version,status FROM work_plans WHERE work_id=$1 ORDER BY plan_version",
    )
    .bind(work_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(plans, vec![(1, "invalidated".into()), (2, "ready".into())]);
    assert_eq!(
        sqlx::query_scalar::<_, Option<Uuid>>("SELECT current_version_id FROM works WHERE id=$1")
            .bind(work_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        Some(second.work_version_id)
    );

    pool.close().await;
    admin_pool.close().await;
}

#[tokio::test]
async fn concurrent_plan_saves_serialize_into_one_draft_version() {
    let (admin_pool, pool, _database) = migrated_pool().await;
    let work_id = seed_work(&pool, "concurrent-plan").await;
    let repository = PostgresWorkGenerationRepository::new(pool.clone());
    let left_repository = repository.clone();
    let right_repository = repository.clone();
    let left_plan = work_plan(work_id, "left");
    let right_plan = work_plan(work_id, "right");

    let (left, right) = tokio::join!(
        left_repository.save_plan(
            work_id,
            "manifest-left",
            json!({"side": "left"}),
            &left_plan
        ),
        right_repository.save_plan(
            work_id,
            "manifest-right",
            json!({"side": "right"}),
            &right_plan
        ),
    );
    let left = left.unwrap();
    let right = right.unwrap();

    assert_eq!(left.work_version_id, right.work_version_id);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_versions WHERE work_id=$1")
            .bind(work_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    let plan_versions: Vec<i32> = sqlx::query_scalar(
        "SELECT plan_version FROM work_plans WHERE work_id=$1 ORDER BY plan_version",
    )
    .bind(work_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(plan_versions, vec![1, 2]);

    pool.close().await;
    admin_pool.close().await;
}

#[tokio::test]
async fn migration_creates_work_library_facts_and_enforces_version_immutability() {
    let (admin_pool, pool, _database) = migrated_pool().await;
    for table in [
        "work_artifacts",
        "work_timelines",
        "work_version_diff_plans",
        "work_diff_confirmations",
        "publication_handoffs",
    ] {
        assert!(table_exists(&pool, table).await, "缺少 {table}");
    }
    let work_library_menu: (String, bool, String) = sqlx::query_as(
        "SELECT route_path,is_enabled,status FROM video_workspace_menus WHERE menu_key='work-library'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        work_library_menu,
        ("/production/library".into(), true, "active".into())
    );

    let forbidden_columns: Vec<String> = sqlx::query_scalar(
        "SELECT column_name FROM information_schema.columns
         WHERE table_schema='public'
           AND table_name IN ('works','work_versions','work_artifacts','work_timelines','work_version_diff_plans','work_diff_confirmations','publication_handoffs')
           AND (column_name ILIKE '%price%' OR column_name ILIKE '%currency%' OR column_name ILIKE '%cost%' OR column_name ILIKE '%amount%' OR column_name ILIKE '%budget%')",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        forbidden_columns.is_empty(),
        "作品库数据库不得包含金额字段: {forbidden_columns:?}"
    );

    let work_id = seed_work(&pool, "immutable").await;
    let version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_versions (work_id,version_no,status,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot) VALUES ($1,1,'completed','manifest','{}','{}','{}') RETURNING id",
    )
    .bind(work_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let mutation = sqlx::query("UPDATE work_versions SET input_snapshot=$2 WHERE id=$1")
        .bind(version_id)
        .bind(json!({"changed": true}))
        .execute(&pool)
        .await;
    assert!(mutation.is_err(), "非 draft 快照必须由数据库拒绝修改");

    let status_only = sqlx::query("UPDATE work_versions SET status='failed' WHERE id=$1")
        .bind(version_id)
        .execute(&pool)
        .await;
    assert!(
        status_only.is_ok(),
        "生命周期更新不应被快照不可变触发器误伤"
    );

    let running_version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_versions (work_id,version_no,status,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot) VALUES ($1,2,'draft','manifest','{}','{}','{}') RETURNING id",
    ).bind(work_id).fetch_one(&pool).await.unwrap();
    let plan_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_plans (work_id,work_version_id,plan_version,status,input_fingerprint,capability_snapshot,output_snapshot,prompt_snapshot,timeline_snapshot) VALUES ($1,$2,2,'confirmed',$3,'{}','{}','{}','{}') RETURNING id",
    ).bind(work_id).bind(running_version_id).bind("0".repeat(64)).fetch_one(&pool).await.unwrap();
    let run_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_runs (work_id,work_version_id,work_plan_id,idempotency_key,status,model_snapshot,capability_snapshot,prompt_snapshot,timeline_snapshot,parameter_snapshot) VALUES ($1,$2,$3,'lock-version','queued','{}','{}','{}','{}','{}') RETURNING id",
    ).bind(work_id).bind(running_version_id).bind(plan_id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM work_versions WHERE id=$1")
            .bind(running_version_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "running"
    );
    let step_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_generation_steps (run_id,step_no,step_type,status) VALUES ($1,1,'compose','queued') RETURNING id",
    ).bind(run_id).fetch_one(&pool).await.unwrap();
    sqlx::query("UPDATE work_generation_steps SET status='succeeded' WHERE id=$1")
        .bind(step_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM work_versions WHERE id=$1")
            .bind(running_version_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        "completed"
    );

    pool.close().await;
    admin_pool.close().await;
}

#[tokio::test]
async fn source_version_and_artifact_constraints_reject_cross_work_or_invalid_data() {
    let (admin_pool, pool, _database) = migrated_pool().await;
    let first_work_id = seed_work(&pool, "source").await;
    let second_work_id = seed_work(&pool, "draft").await;
    let source_version_id: Uuid = sqlx::query_scalar(
        "INSERT INTO work_versions (work_id,version_no,status,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot) VALUES ($1,1,'completed','manifest','{}','{}','{}') RETURNING id",
    )
    .bind(first_work_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let cross_work = sqlx::query(
        "INSERT INTO work_versions (work_id,version_no,status,source_version_id,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot) VALUES ($1,1,'draft',$2,'manifest','{}','{}','{}')",
    )
    .bind(second_work_id)
    .bind(source_version_id)
    .execute(&pool)
    .await;
    assert!(cross_work.is_err(), "来源版本必须属于同一作品");

    let invalid_artifact = sqlx::query(
        "INSERT INTO work_artifacts (work_version_id,role,file_name,storage_path,mime_type,size_bytes,sha256) VALUES ($1,'final_video','bad.mp4','bad.mp4','video/mp4',-1,'short')",
    )
    .bind(source_version_id)
    .execute(&pool)
    .await;
    assert!(
        invalid_artifact.is_err(),
        "artifact 大小与 SHA-256 必须受约束"
    );

    pool.close().await;
    admin_pool.close().await;
}

#[tokio::test]
async fn version_governance_migration_keeps_referenced_facts_and_removes_only_safe_drafts() {
    let (admin_pool, pool, _database) = pool_before_version_governance().await;

    let normalized_work_id = seed_work(&pool, "normalize-current").await;
    let normalized_old: Uuid = sqlx::query_scalar(
        "INSERT INTO work_versions (work_id,version_no,status,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot) VALUES ($1,1,'draft','old','{}','{}','{}') RETURNING id",
    )
    .bind(normalized_work_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let normalized_current: Uuid = sqlx::query_scalar(
        "INSERT INTO work_versions (work_id,version_no,status,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot) VALUES ($1,2,'draft','current','{}','{}','{}') RETURNING id",
    )
    .bind(normalized_work_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    for (version_id, plan_version, status) in [
        (normalized_old, 1, "invalidated"),
        (normalized_current, 2, "ready"),
    ] {
        sqlx::query("INSERT INTO work_plans (work_id,work_version_id,plan_version,status,input_fingerprint,capability_snapshot,output_snapshot,prompt_snapshot,timeline_snapshot) VALUES ($1,$2,$3,$4,$5,'{}','{}','{}','{}')")
            .bind(normalized_work_id).bind(version_id).bind(plan_version).bind(status).bind("0".repeat(64)).execute(&pool).await.unwrap();
    }

    let protected_work_id = seed_work(&pool, "protected-history").await;
    let mut versions = Vec::new();
    for version_no in 1..=10 {
        let version_id: Uuid = sqlx::query_scalar(
            "INSERT INTO work_versions (work_id,version_no,status,source_manifest_version,input_snapshot,model_snapshot,parameter_snapshot) VALUES ($1,$2,'draft',$3,'{}','{}','{}') RETURNING id",
        )
        .bind(protected_work_id)
        .bind(version_no)
        .bind(format!("manifest-{version_no}"))
        .fetch_one(&pool)
        .await
        .unwrap();
        versions.push(version_id);
    }
    sqlx::query("UPDATE work_versions SET source_version_id=$2,derivation_kind='edit' WHERE id=$1")
        .bind(versions[7])
        .bind(versions[6])
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE works SET current_version_id=$2 WHERE id=$1")
        .bind(protected_work_id)
        .bind(versions[9])
        .execute(&pool)
        .await
        .unwrap();
    let mut plan_ids = Vec::new();
    for (index, version_id) in versions.iter().enumerate() {
        let status = if index == 8 {
            "confirmed"
        } else if index == 9 {
            "ready"
        } else {
            "invalidated"
        };
        let plan_id: Uuid = sqlx::query_scalar("INSERT INTO work_plans (work_id,work_version_id,plan_version,status,input_fingerprint,capability_snapshot,output_snapshot,prompt_snapshot,timeline_snapshot) VALUES ($1,$2,$3,$4,$5,'{}','{}','{}','{}') RETURNING id")
            .bind(protected_work_id).bind(version_id).bind((index + 1) as i32).bind(status).bind("1".repeat(64)).fetch_one(&pool).await.unwrap();
        plan_ids.push(plan_id);
    }

    let artifact_v2: Uuid = sqlx::query_scalar("INSERT INTO work_artifacts (work_version_id,role,file_name,storage_path,mime_type,size_bytes,sha256) VALUES ($1,'reusable_intermediate','v2.bin','works/v2.bin','application/octet-stream',1,$2) RETURNING id")
        .bind(versions[1]).bind("2".repeat(64)).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO work_timelines (work_version_id) VALUES ($1)")
        .bind(versions[2])
        .execute(&pool)
        .await
        .unwrap();
    let diff_id: Uuid = sqlx::query_scalar("INSERT INTO work_version_diff_plans (work_id,source_version_id,draft_version_id,plan_version,source_fingerprint,draft_fingerprint,changes,affected_nodes,reused_artifact_ids,resource_usage) VALUES ($1,$2,$3,1,$4,$5,'[]','[]','[]','{}') RETURNING id")
        .bind(protected_work_id).bind(versions[3]).bind(versions[4]).bind("3".repeat(64)).bind("4".repeat(64)).fetch_one(&pool).await.unwrap();
    let artifact_v6: Uuid = sqlx::query_scalar("INSERT INTO work_artifacts (work_version_id,role,file_name,storage_path,mime_type,size_bytes,sha256) VALUES ($1,'final_video','v6.mp4','works/v6.mp4','video/mp4',1,$2) RETURNING id")
        .bind(versions[5]).bind("5".repeat(64)).fetch_one(&pool).await.unwrap();
    sqlx::query("INSERT INTO publication_handoffs (work_id,work_version_id,final_video_artifact_id,idempotency_key) VALUES ($1,$2,$3,'migration-handoff')")
        .bind(protected_work_id).bind(versions[5]).bind(artifact_v6).execute(&pool).await.unwrap();
    let run_id: Uuid = sqlx::query_scalar("INSERT INTO work_generation_runs (work_id,work_version_id,work_plan_id,idempotency_key,status,model_snapshot,capability_snapshot,prompt_snapshot,timeline_snapshot,parameter_snapshot) VALUES ($1,$2,$3,'migration-failed-run','failed','{}','{}','{}','{}','{}') RETURNING id")
        .bind(protected_work_id).bind(versions[8]).bind(plan_ids[8]).fetch_one(&pool).await.unwrap();
    let step_id: Uuid = sqlx::query_scalar("INSERT INTO work_generation_steps (run_id,step_no,step_type,status) VALUES ($1,1,'video_segment','failed') RETURNING id")
        .bind(run_id).fetch_one(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO work_generation_attempts (step_id,attempt_no,status) VALUES ($1,1,'failed')",
    )
    .bind(step_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO work_diff_confirmations (diff_plan_id,idempotency_key,generation_run_id) VALUES ($1,'migration-diff-confirmation',$2)")
        .bind(diff_id).bind(run_id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE works SET current_version_id=$2 WHERE id=$1")
        .bind(protected_work_id)
        .bind(versions[9])
        .execute(&pool)
        .await
        .unwrap();

    apply_version_governance(&pool).await;

    assert_eq!(
        sqlx::query_scalar::<_, Option<Uuid>>("SELECT current_version_id FROM works WHERE id=$1")
            .bind(normalized_work_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        Some(normalized_current)
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_versions WHERE id=$1")
            .bind(normalized_old)
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_versions WHERE id=$1")
            .bind(versions[0])
            .fetch_one(&pool)
            .await
            .unwrap(),
        0,
        "只有无运行、无引用且计划全部失效的 V1 应被清理"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_versions WHERE id=$1")
            .bind(versions[7])
            .fetch_one(&pool)
            .await
            .unwrap(),
        0,
        "只负责引用来源版本但自身无下游事实的 V8 仍是安全清理候选"
    );
    for (index, version_id) in versions.iter().enumerate().skip(1) {
        if index == 7 {
            continue;
        }
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_versions WHERE id=$1")
                .bind(version_id)
                .fetch_one(&pool)
                .await
                .unwrap(),
            1,
            "存在下游事实或当前引用的 V{} 必须保留: {version_id}",
            index + 1
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM work_generation_attempts WHERE step_id=$1"
        )
        .bind(step_id)
        .fetch_one(&pool)
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_artifacts WHERE id=$1")
            .bind(artifact_v2)
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );

    pool.close().await;
    admin_pool.close().await;
}
