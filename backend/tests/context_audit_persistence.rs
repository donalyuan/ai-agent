use novex_agent::{AuditedCallOwner, PersistContextCompileAttempt, PersistContextSnapshot};
use novex_ai_core::{canonical_json, sha256_hex, ContextDecisionCode, ContextPayload};
use novex_ai_core::{ContextCompileRequest, ContextCompiler, ContextSnapshot, LogicalModelInput};
use novex_api::domain::conversation::FinishAgentRunInput;
use novex_api::repositories::{
    ContextAuditListFilter, ContextAuditRecord, ContextAuditRepositoryError,
    ConversationRepository, ModelCallOwner, ModelCallRepositoryError,
    PostgresContextAuditRepository, PostgresConversationRepository, PostgresModelCallRepository,
    PrepareModelCall, PrepareModelCallWithContext,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

mod support;

use support::test_database::{insert_enabled_text_model, TestDatabase};

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
    let slash = base
        .rfind('/')
        .expect("DATABASE_URL must include a database");
    format!("{}{}{}", &base[..=slash], database_name, query)
}

async fn migrated_pool() -> (PgPool, PgPool, TestDatabase) {
    let base_url = database_url();
    let name = format!("video_agent_context_audit_{}", Uuid::new_v4().simple());
    let admin_url = with_database_name(&base_url, "postgres");
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&admin_url)
        .await
        .expect("admin database should be reachable");
    sqlx::query(&format!(r#"CREATE DATABASE "{name}""#))
        .execute(&admin)
        .await
        .expect("temporary context audit database should be created");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&with_database_name(&base_url, &name))
        .await
        .expect("temporary context audit database should be reachable");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations should run");
    (admin, pool, TestDatabase::new(&admin_url, &name))
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

async fn column_exists(pool: &PgPool, table: &str, column: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns WHERE table_schema='public' AND table_name=$1 AND column_name=$2)",
    )
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn trigger_exists(pool: &PgPool, table: &str, trigger: &str) -> bool {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM pg_trigger trigger_info
            JOIN pg_class table_info ON table_info.oid = trigger_info.tgrelid
            JOIN pg_namespace namespace ON namespace.oid = table_info.relnamespace
            WHERE namespace.nspname='public' AND table_info.relname=$1
              AND trigger_info.tgname=$2 AND NOT trigger_info.tgisinternal
        )
        "#,
    )
    .bind(table)
    .bind(trigger)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn seed_owners(pool: &PgPool) -> (Uuid, Uuid, Uuid, Uuid) {
    let conversation_id = sqlx::query_scalar(
        "INSERT INTO agent_conversations (agent_type, title) VALUES ('script', 'context audit') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let run_a = sqlx::query_scalar(
        "INSERT INTO agent_runs (agent_type, status) VALUES ('script', 'running') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let run_b = sqlx::query_scalar(
        "INSERT INTO agent_runs (agent_type, status) VALUES ('script', 'running') RETURNING id",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let eval_run_id = sqlx::query_scalar(
        r#"
        INSERT INTO eval_runs (
            candidate_key, candidate_version, candidate_digest, case_set_version,
            evaluator_version, approved_real_calls, max_cases, max_input_tokens,
            max_output_tokens, max_retries, max_cost_micros
        ) VALUES (
            'context.policy', '1.0.0', $1, 'context-contract@1', 'novex-eval@1',
            FALSE, 1, 0, 0, 0, 0
        ) RETURNING id
        "#,
    )
    .bind("e".repeat(64))
    .fetch_one(pool)
    .await
    .unwrap();
    (conversation_id, run_a, run_b, eval_run_id)
}

fn selected_decisions(candidate_id: &str) -> Value {
    json!([{
        "candidate_id": candidate_id,
        "source_kind": "user_instruction",
        "source_id": candidate_id,
        "source_version": "1",
        "content_hash": "a".repeat(64),
        "token_count": 4,
        "decision": "selected",
        "selected_payload": {"kind": "text_fragment", "text": "safe selected text"}
    }])
}

fn excluded_decisions(candidate_id: &str) -> Value {
    json!([{
        "candidate_id": candidate_id,
        "source_kind": "reference",
        "source_id": candidate_id,
        "source_version": "1",
        "content_hash": "b".repeat(64),
        "token_count": 4096,
        "decision": "budget_excluded"
    }])
}

#[derive(Deserialize)]
struct ContextContractFixture {
    request: ContextCompileRequest,
    final_logical_input: LogicalModelInput,
}

fn compiled_snapshot(owner_id: Uuid) -> (ContextSnapshot, ContextCompileRequest) {
    let fixture: ContextContractFixture = serde_json::from_str(include_str!(
        "../../agent-definitions/fixtures/context-compile-contract-v1.json"
    ))
    .unwrap();
    let mut request = fixture.request;
    request.owner_id = owner_id.to_string();
    let profile = request.tokenizer_profile.clone();
    let compiled = ContextCompiler::compile(request.clone()).unwrap();
    let snapshot =
        ContextCompiler::finalize(&compiled, &profile, fixture.final_logical_input).unwrap();
    (snapshot, request)
}

fn governed_model_call(
    snapshot_id: Uuid,
    snapshot: &ContextSnapshot,
    run_id: Uuid,
    model_id: Uuid,
) -> PrepareModelCall {
    let user = snapshot.logical_input.messages[0]
        .content
        .as_str()
        .unwrap()
        .to_string();
    PrepareModelCall {
        owner: ModelCallOwner::AgentRun(run_id),
        root_call_id: None,
        parent_call_id: None,
        node_key: snapshot.node_key.clone(),
        attempt: 1,
        agent_key: "fixture.agent".into(),
        agent_version: "1.0.0".into(),
        prompt_key: "fixture.prompt".into(),
        prompt_version: "1.0.0".into(),
        registry_digest: "a".repeat(64),
        prompt_snapshot: json!({
            "schema_version": "2",
            "registry_digest": "a".repeat(64),
            "agent_key": "fixture.agent",
            "agent_version": "1.0.0",
            "prompt_key": "fixture.prompt",
            "prompt_version": "1.0.0",
            "node_key": snapshot.node_key,
            "system": snapshot.logical_input.system,
            "user": user,
            "variables": {},
            "fragments": [],
            "tool_profile": "chat",
            "output_schema": snapshot.logical_input.output_schema,
            "tool_schema": snapshot.logical_input.tool_schema,
            "max_output_tokens": 32,
            "context_snapshot_id": snapshot_id,
            "context_digest": snapshot.digest,
            "logical_input": snapshot.logical_input
        }),
        context_sources: json!([]),
        memory_sources: json!([]),
        tool_schema: snapshot.logical_input.tool_schema.clone(),
        model_id,
        behavior_fingerprint: "b".repeat(64),
        model_snapshot: json!({"provider":"fixture","upstream_model":"fixture-model"}),
        parameters: json!({"max_output_tokens":32}),
        asset_references: json!([]),
        known_secrets: vec![],
    }
}

fn refresh_attempt_digest(attempt: &mut novex_ai_core::ContextCompileAttempt) {
    let mut value = serde_json::to_value(&*attempt).unwrap();
    value.as_object_mut().unwrap().remove("digest");
    attempt.digest = sha256_hex(canonical_json(&value).as_bytes());
}

async fn insert_snapshot(
    pool: &PgPool,
    conversation_id: Option<Uuid>,
    agent_run_id: Option<Uuid>,
    eval_run_id: Option<Uuid>,
    candidate_id: &str,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO context_snapshots (
            conversation_id, agent_run_id, eval_run_id, node_key, status, compiled_at,
            policy_key, policy_version, tokenizer_profile_key, tokenizer_profile_version,
            tokenizer_mode, model_context_window, budget_ledger, decisions, selected_order,
            logical_input, context_digest
        ) VALUES (
            $1, $2, $3, 'script.complete', 'succeeded', '2026-07-25T00:00:00Z',
            'video.script.context', '1.0.0', 'openai.cl100k', '1.0.0', 'exact', 128000,
            $4, $5, $6, $7, $8
        ) RETURNING id
        "#,
    )
    .bind(conversation_id)
    .bind(agent_run_id)
    .bind(eval_run_id)
    .bind(json!({
        "model_context_window": 128000,
        "dynamic_context_budget": 120000,
        "selected_context_tokens": 4,
        "final_input_tokens": 12
    }))
    .bind(selected_decisions(candidate_id))
    .bind(json!([candidate_id]))
    .bind(json!({
        "system": "system",
        "messages": [{"role": "user", "content": "safe selected text"}],
        "tool_schema": null,
        "output_schema": null
    }))
    .bind("c".repeat(64))
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn migrations_create_context_audit_binding_and_model_call_contract() {
    let (admin, pool, database) = migrated_pool().await;

    for table in ["context_snapshots", "context_compile_attempts"] {
        assert!(table_exists(&pool, table).await, "{table} should exist");
    }
    for (table, columns) in [
        (
            "context_snapshots",
            vec![
                "conversation_id",
                "agent_run_id",
                "eval_run_id",
                "node_key",
                "status",
                "policy_key",
                "policy_version",
                "tokenizer_profile_key",
                "tokenizer_profile_version",
                "budget_ledger",
                "decisions",
                "selected_order",
                "logical_input",
                "context_digest",
            ],
        ),
        (
            "context_compile_attempts",
            vec![
                "conversation_id",
                "agent_run_id",
                "eval_run_id",
                "node_key",
                "status",
                "stage",
                "code",
                "budget_ledger",
                "decisions",
                "attempt_digest",
            ],
        ),
    ] {
        for column in columns {
            assert!(
                column_exists(&pool, table, column).await,
                "{table}.{column} should exist"
            );
        }
    }
    for (table, column) in [
        ("agent_conversations", "last_context_compile_attempt_id"),
        ("agent_runs", "context_compile_attempt_id"),
        ("agent_steps", "context_compile_attempt_id"),
        ("agent_conversation_bindings", "context_policy_bindings"),
        ("agent_conversation_bindings", "tokenizer_profile_key"),
        ("agent_conversation_bindings", "tokenizer_profile_version"),
        ("agent_conversation_bindings", "tokenizer_profile_digest"),
        ("agent_run_bindings", "context_policy_bindings"),
        ("agent_run_bindings", "tokenizer_profile_key"),
        ("agent_run_bindings", "tokenizer_profile_version"),
        ("agent_run_bindings", "tokenizer_profile_digest"),
        ("model_calls", "context_snapshot_id"),
        ("model_calls", "context_digest"),
        ("model_calls", "context_policy_key"),
        ("model_calls", "context_policy_version"),
        ("model_calls", "tokenizer_profile_key"),
        ("model_calls", "tokenizer_profile_version"),
        ("model_calls", "context_budget_summary"),
    ] {
        assert!(
            column_exists(&pool, table, column).await,
            "{table}.{column} should exist"
        );
    }
    assert!(trigger_exists(&pool, "context_snapshots", "context_snapshots_immutable").await);
    assert!(
        trigger_exists(
            &pool,
            "context_compile_attempts",
            "context_compile_attempts_immutable"
        )
        .await
    );
    assert!(
        trigger_exists(
            &pool,
            "model_calls",
            "model_calls_context_evidence_immutable"
        )
        .await
    );

    pool.close().await;
    admin.close().await;
    drop(database);
}

#[tokio::test]
async fn context_records_enforce_owner_status_digest_payload_and_immutability() {
    let (admin, pool, database) = migrated_pool().await;
    let (conversation_id, run_a, _, eval_run_id) = seed_owners(&pool).await;

    let snapshot_id = insert_snapshot(&pool, None, Some(run_a), None, "selected-1")
        .await
        .expect("valid selected Snapshot should persist");
    assert!(
        sqlx::query("UPDATE context_snapshots SET node_key='other.node' WHERE id=$1")
            .bind(snapshot_id)
            .execute(&pool)
            .await
            .is_err(),
        "Snapshot must reject updates"
    );
    assert!(
        sqlx::query("DELETE FROM context_snapshots WHERE id=$1")
            .bind(snapshot_id)
            .execute(&pool)
            .await
            .is_err(),
        "Snapshot must reject deletes"
    );

    assert!(
        insert_snapshot(&pool, None, None, None, "ownerless")
            .await
            .is_err(),
        "Snapshot must have exactly one owner"
    );
    assert!(
        insert_snapshot(
            &pool,
            Some(conversation_id),
            Some(run_a),
            None,
            "two-owners"
        )
        .await
        .is_err(),
        "Snapshot must reject multiple owners"
    );

    let invalid_digest = sqlx::query(
        r#"
        INSERT INTO context_compile_attempts (
            eval_run_id, node_key, status, compiled_at, stage, code,
            budget_ledger, decisions, attempt_digest
        ) VALUES ($1, 'eval.context', 'failed', NOW(), 'budget',
                  'context_budget_exceeded', NULL, $2, 'short')
        "#,
    )
    .bind(eval_run_id)
    .bind(excluded_decisions("excluded-1"))
    .execute(&pool)
    .await;
    assert!(
        invalid_digest.is_err(),
        "Attempt digest must be canonical sha256"
    );

    let invalid_status = sqlx::query(
        r#"
        INSERT INTO context_compile_attempts (
            eval_run_id, node_key, status, compiled_at, stage, code,
            budget_ledger, decisions, attempt_digest
        ) VALUES ($1, 'eval.context', 'succeeded', NOW(), 'budget',
                  'context_budget_exceeded', NULL, $2, $3)
        "#,
    )
    .bind(eval_run_id)
    .bind(excluded_decisions("excluded-2"))
    .bind("d".repeat(64))
    .execute(&pool)
    .await;
    assert!(invalid_status.is_err(), "Attempt status must remain failed");

    let leaked_payload = json!([{
        "candidate_id": "excluded-secret",
        "source_kind": "reference",
        "source_id": "reference-1",
        "source_version": "1",
        "content_hash": "f".repeat(64),
        "token_count": 9000,
        "decision": "budget_excluded",
        "selected_payload": {"kind": "text_fragment", "text": "must not persist"}
    }]);
    let leaked_attempt = sqlx::query(
        r#"
        INSERT INTO context_compile_attempts (
            eval_run_id, node_key, status, compiled_at, stage, code,
            budget_ledger, decisions, attempt_digest
        ) VALUES ($1, 'eval.context', 'failed', NOW(), 'budget',
                  'context_budget_exceeded', NULL, $2, $3)
        "#,
    )
    .bind(eval_run_id)
    .bind(leaked_payload)
    .bind("d".repeat(64))
    .execute(&pool)
    .await;
    assert!(
        leaked_attempt.is_err(),
        "excluded decisions must never retain payload bodies"
    );

    let attempt_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO context_compile_attempts (
            eval_run_id, node_key, status, compiled_at, stage, code,
            budget_ledger, decisions, attempt_digest
        ) VALUES ($1, 'eval.context', 'failed', NOW(), 'budget',
                  'context_budget_exceeded', NULL, $2, $3)
        RETURNING id
        "#,
    )
    .bind(eval_run_id)
    .bind(excluded_decisions("excluded-safe"))
    .bind("d".repeat(64))
    .fetch_one(&pool)
    .await
    .expect("minimal failed Attempt should persist");
    assert!(
        sqlx::query("UPDATE context_compile_attempts SET code='other' WHERE id=$1")
            .bind(attempt_id)
            .execute(&pool)
            .await
            .is_err(),
        "CompileAttempt must reject updates"
    );
    assert!(
        sqlx::query("DELETE FROM context_compile_attempts WHERE id=$1")
            .bind(attempt_id)
            .execute(&pool)
            .await
            .is_err(),
        "CompileAttempt must reject deletes"
    );
    let fake_calls: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_calls")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        fake_calls, 0,
        "failed compilation must not create ModelCall"
    );

    pool.close().await;
    admin.close().await;
    drop(database);
}

#[tokio::test]
async fn context_snapshot_queries_are_isolated_by_owner() {
    let (admin, pool, database) = migrated_pool().await;
    let (_, run_a, run_b, _) = seed_owners(&pool).await;
    insert_snapshot(&pool, None, Some(run_a), None, "run-a")
        .await
        .unwrap();
    insert_snapshot(&pool, None, Some(run_b), None, "run-b")
        .await
        .unwrap();

    let run_a_candidates: Vec<String> = sqlx::query_scalar(
        "SELECT decisions->0->>'candidate_id' FROM context_snapshots WHERE agent_run_id=$1",
    )
    .bind(run_a)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(run_a_candidates, ["run-a"]);

    pool.close().await;
    admin.close().await;
    drop(database);
}

#[tokio::test]
async fn repository_persists_typed_snapshot_and_minimal_attempt_without_model_call() {
    let (admin, pool, database) = migrated_pool().await;
    let (_, run_a, run_b, _) = seed_owners(&pool).await;
    let repository = PostgresContextAuditRepository::new(pool.clone());
    let (snapshot, request) = compiled_snapshot(run_a);

    let stored = repository
        .persist_snapshot_record(PersistContextSnapshot {
            owner: AuditedCallOwner::AgentRun(run_a),
            snapshot: snapshot.clone(),
            known_secrets: vec![],
        })
        .await
        .unwrap();
    assert_eq!(stored.owner, AuditedCallOwner::AgentRun(run_a));
    assert_eq!(stored.snapshot, snapshot);
    assert_eq!(repository.get_snapshot(stored.id).await.unwrap(), stored);

    let mismatch = repository
        .persist_snapshot_record(PersistContextSnapshot {
            owner: AuditedCallOwner::AgentRun(run_b),
            snapshot,
            known_secrets: vec![],
        })
        .await
        .unwrap_err();
    assert!(matches!(
        mismatch,
        ContextAuditRepositoryError::OwnerMismatch { .. }
    ));

    let mut invalid_request = request;
    invalid_request.schema_version = "invalid".into();
    let error = ContextCompiler::compile(invalid_request.clone()).unwrap_err();
    let attempt = error.attempt(&invalid_request);
    let stored_attempt = repository
        .persist_attempt_record(PersistContextCompileAttempt {
            owner: AuditedCallOwner::AgentRun(run_a),
            attempt: attempt.clone(),
            known_secrets: vec![],
        })
        .await
        .unwrap();
    assert_eq!(stored_attempt.attempt, attempt);
    assert_eq!(
        repository.get_attempt(stored_attempt.id).await.unwrap(),
        stored_attempt
    );
    let (records, total) = repository
        .list(
            &ContextAuditListFilter {
                owner: Some(AuditedCallOwner::AgentRun(run_a)),
                record_type: None,
                node_key: Some("fixture.node".into()),
            },
            20,
            0,
        )
        .await
        .unwrap();
    assert_eq!(total, 2);
    assert!(records.iter().any(
        |record| matches!(record, ContextAuditRecord::Snapshot(value) if value.id == stored.id)
    ));
    assert!(records.iter().any(|record| matches!(record, ContextAuditRecord::CompileAttempt(value) if value.id == stored_attempt.id)));
    let model_calls: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_calls")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(model_calls, 0);

    pool.close().await;
    admin.close().await;
    drop(database);
}

#[tokio::test]
async fn snapshot_and_prepared_model_call_are_atomic_and_consistent() {
    let (admin, pool, database) = migrated_pool().await;
    let (_, run_id, _, _) = seed_owners(&pool).await;
    let model_id = insert_enabled_text_model(&pool).await;
    let repository = PostgresModelCallRepository::new(pool.clone());

    let (snapshot, _) = compiled_snapshot(run_id);
    let snapshot_id = Uuid::new_v4();
    let record = repository
        .prepare_with_context(PrepareModelCallWithContext {
            model_call: governed_model_call(snapshot_id, &snapshot, run_id, model_id),
            context: PersistContextSnapshot {
                owner: AuditedCallOwner::AgentRun(run_id),
                snapshot: snapshot.clone(),
                known_secrets: vec![],
            },
        })
        .await
        .unwrap();
    assert_eq!(record.schema_version, "2");
    assert_eq!(record.context_snapshot_id, Some(snapshot_id));
    assert_eq!(
        record.context_digest.as_deref(),
        Some(snapshot.digest.as_str())
    );
    assert_eq!(
        record.context_policy_key.as_deref(),
        Some(snapshot.policy_key.as_str())
    );
    assert_eq!(
        record.tokenizer_profile_key.as_deref(),
        Some(snapshot.tokenizer_profile_key.as_str())
    );
    let stored_snapshot: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM context_snapshots WHERE id=$1 AND agent_run_id=$2)",
    )
    .bind(snapshot_id)
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(stored_snapshot);

    let (mismatch_snapshot, _) = compiled_snapshot(run_id);
    let mismatch_id = Uuid::new_v4();
    let mut mismatch_call = governed_model_call(mismatch_id, &mismatch_snapshot, run_id, model_id);
    mismatch_call.prompt_snapshot["context_digest"] = Value::String("f".repeat(64));
    let mismatch = repository
        .prepare_with_context(PrepareModelCallWithContext {
            model_call: mismatch_call,
            context: PersistContextSnapshot {
                owner: AuditedCallOwner::AgentRun(run_id),
                snapshot: mismatch_snapshot,
                known_secrets: vec![],
            },
        })
        .await
        .unwrap_err();
    assert!(matches!(
        mismatch,
        ModelCallRepositoryError::ContextMismatch(_)
    ));
    let mismatch_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM context_snapshots WHERE id=$1")
            .bind(mismatch_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(mismatch_rows, 0);

    let (rollback_snapshot, _) = compiled_snapshot(run_id);
    let rollback_id = Uuid::new_v4();
    let failure = repository
        .prepare_with_context(PrepareModelCallWithContext {
            model_call: governed_model_call(
                rollback_id,
                &rollback_snapshot,
                run_id,
                Uuid::new_v4(),
            ),
            context: PersistContextSnapshot {
                owner: AuditedCallOwner::AgentRun(run_id),
                snapshot: rollback_snapshot,
                known_secrets: vec![],
            },
        })
        .await;
    assert!(
        failure.is_err(),
        "invalid ModelCall FK should fail the transaction"
    );
    let rollback_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM context_snapshots WHERE id=$1")
            .bind(rollback_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        rollback_rows, 0,
        "failed prepared call must roll back Snapshot"
    );

    pool.close().await;
    admin.close().await;
    drop(database);
}

#[tokio::test]
async fn compile_attempt_redacts_canaries_and_rejects_unminimized_or_unsafe_payloads() {
    let (admin, pool, database) = migrated_pool().await;
    let (_, run_id, _, _) = seed_owners(&pool).await;
    let repository = PostgresContextAuditRepository::new(pool.clone());
    let (snapshot, mut request) = compiled_snapshot(run_id);
    request.schema_version = "invalid".into();
    let error = ContextCompiler::compile(request.clone()).unwrap_err();
    let mut base_attempt = error.attempt(&request);
    let mut excluded = snapshot.decisions[0].clone();
    excluded.decision = ContextDecisionCode::BudgetExcluded;
    excluded.selected_payload = None;
    excluded.source_id = "NOVEX_CANARY_SECRET_DO_NOT_PERSIST_context_audit".into();
    base_attempt.decisions = vec![excluded];
    refresh_attempt_digest(&mut base_attempt);

    let stored = repository
        .persist_attempt_record(PersistContextCompileAttempt {
            owner: AuditedCallOwner::AgentRun(run_id),
            attempt: base_attempt.clone(),
            known_secrets: vec![],
        })
        .await
        .unwrap();
    assert_eq!(stored.attempt.decisions[0].source_id, "[REDACTED]");

    let mut leaked = base_attempt.clone();
    leaked.decisions[0].selected_payload = Some(ContextPayload::Text {
        text: "excluded body must not persist".into(),
    });
    refresh_attempt_digest(&mut leaked);
    assert!(matches!(
        repository
            .persist_attempt_record(PersistContextCompileAttempt {
                owner: AuditedCallOwner::AgentRun(run_id),
                attempt: leaked,
                known_secrets: vec![],
            })
            .await
            .unwrap_err(),
        ContextAuditRepositoryError::InvalidRecord(_)
    ));

    let mut base64 = base_attempt.clone();
    base64.decisions[0].source_id = "A".repeat(5000);
    refresh_attempt_digest(&mut base64);
    assert!(matches!(
        repository
            .persist_attempt_record(PersistContextCompileAttempt {
                owner: AuditedCallOwner::AgentRun(run_id),
                attempt: base64,
                known_secrets: vec![],
            })
            .await
            .unwrap_err(),
        ContextAuditRepositoryError::UnsafeAudit(_)
    ));

    let mut signed_url = base_attempt;
    signed_url.decisions[0].source_id =
        "https://assets.invalid/file?X-Amz-Signature=secret&X-Amz-Expires=60".into();
    refresh_attempt_digest(&mut signed_url);
    assert!(matches!(
        repository
            .persist_attempt_record(PersistContextCompileAttempt {
                owner: AuditedCallOwner::AgentRun(run_id),
                attempt: signed_url,
                known_secrets: vec![],
            })
            .await
            .unwrap_err(),
        ContextAuditRepositoryError::UnsafeAudit(_)
    ));

    let model_calls: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM model_calls")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(model_calls, 0);

    pool.close().await;
    admin.close().await;
    drop(database);
}

#[tokio::test]
async fn context_failure_links_preserve_existing_run_step_and_conversation_finalization() {
    let (admin, pool, database) = migrated_pool().await;
    let (conversation_id, run_id, other_run_id, _) = seed_owners(&pool).await;
    sqlx::query(
        "UPDATE agent_runs SET input=jsonb_build_object('conversation_id', $2::text) WHERE id=$1",
    )
    .bind(run_id)
    .bind(conversation_id)
    .execute(&pool)
    .await
    .unwrap();
    let step_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO agent_steps (
            agent_run_id, step_order, step_type, status, error_message
        ) VALUES ($1, 1, 'context_compile', 'failed', 'context compile failed')
        RETURNING id
        "#,
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let repository = PostgresContextAuditRepository::new(pool.clone());
    let (_, mut request) = compiled_snapshot(run_id);
    request.schema_version = "invalid".into();
    let attempt = ContextCompiler::compile(request.clone())
        .unwrap_err()
        .attempt(&request);
    let attempt_id = repository
        .persist_attempt_record(PersistContextCompileAttempt {
            owner: AuditedCallOwner::AgentRun(run_id),
            attempt,
            known_secrets: vec![],
        })
        .await
        .unwrap()
        .id;

    repository
        .link_failure(
            AuditedCallOwner::AgentRun(run_id),
            attempt_id,
            Some(step_id),
        )
        .await
        .unwrap();
    let linked_run: (String, Option<String>, Option<Uuid>) = sqlx::query_as(
        "SELECT status, error_message, context_compile_attempt_id FROM agent_runs WHERE id=$1",
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked_run, ("running".into(), None, Some(attempt_id)));
    let linked_step: Option<Uuid> =
        sqlx::query_scalar("SELECT context_compile_attempt_id FROM agent_steps WHERE id=$1")
            .bind(step_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(linked_step, Some(attempt_id));
    let linked_conversation: Option<Uuid> = sqlx::query_scalar(
        "SELECT last_context_compile_attempt_id FROM agent_conversations WHERE id=$1",
    )
    .bind(conversation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked_conversation, Some(attempt_id));

    let finished = PostgresConversationRepository::new(pool.clone())
        .finish_run(FinishAgentRunInput {
            agent_run_id: run_id,
            status: "failed".into(),
            output: None,
            error_message: Some("context compile failed".into()),
            context_compile_attempt_id: Some(attempt_id),
        })
        .await
        .unwrap();
    assert_eq!(finished.status, "failed");
    assert_eq!(
        finished.error_message.as_deref(),
        Some("context compile failed")
    );
    assert_eq!(finished.context_compile_attempt_id, Some(attempt_id));

    let wrong_owner =
        sqlx::query("UPDATE agent_runs SET context_compile_attempt_id=$2 WHERE id=$1")
            .bind(other_run_id)
            .bind(attempt_id)
            .execute(&pool)
            .await;
    assert!(wrong_owner.is_err());

    pool.close().await;
    admin.close().await;
    drop(database);
}

#[tokio::test]
async fn explicit_owner_deletion_cascades_context_without_touching_adjacent_forks() {
    let (admin, pool, database) = migrated_pool().await;
    let (conversation_id, run_id, fork_run_id, _) = seed_owners(&pool).await;
    let model_id = insert_enabled_text_model(&pool).await;
    let context_repository = PostgresContextAuditRepository::new(pool.clone());
    let model_repository = PostgresModelCallRepository::new(pool.clone());

    let (conversation_snapshot, mut conversation_request) = compiled_snapshot(conversation_id);
    context_repository
        .persist_snapshot_record(PersistContextSnapshot {
            owner: AuditedCallOwner::Conversation(conversation_id),
            snapshot: conversation_snapshot,
            known_secrets: vec![],
        })
        .await
        .unwrap();
    conversation_request.schema_version = "invalid".into();
    let conversation_attempt = ContextCompiler::compile(conversation_request.clone())
        .unwrap_err()
        .attempt(&conversation_request);
    context_repository
        .persist_attempt_record(PersistContextCompileAttempt {
            owner: AuditedCallOwner::Conversation(conversation_id),
            attempt: conversation_attempt,
            known_secrets: vec![],
        })
        .await
        .unwrap();

    for owner_run_id in [run_id, fork_run_id] {
        let (snapshot, mut request) = compiled_snapshot(owner_run_id);
        let snapshot_id = Uuid::new_v4();
        model_repository
            .prepare_with_context(PrepareModelCallWithContext {
                model_call: governed_model_call(snapshot_id, &snapshot, owner_run_id, model_id),
                context: PersistContextSnapshot {
                    owner: AuditedCallOwner::AgentRun(owner_run_id),
                    snapshot,
                    known_secrets: vec![],
                },
            })
            .await
            .unwrap();
        request.schema_version = "invalid".into();
        let attempt = ContextCompiler::compile(request.clone())
            .unwrap_err()
            .attempt(&request);
        context_repository
            .persist_attempt_record(PersistContextCompileAttempt {
                owner: AuditedCallOwner::AgentRun(owner_run_id),
                attempt,
                known_secrets: vec![],
            })
            .await
            .unwrap();
    }

    model_repository
        .delete_owner(ModelCallOwner::Conversation(conversation_id))
        .await
        .unwrap();
    let conversation_evidence: i64 = sqlx::query_scalar(
        r#"
        SELECT
            (SELECT COUNT(*) FROM context_snapshots WHERE conversation_id=$1)
            + (SELECT COUNT(*) FROM context_compile_attempts WHERE conversation_id=$1)
        "#,
    )
    .bind(conversation_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(conversation_evidence, 0);

    model_repository
        .delete_owner(ModelCallOwner::AgentRun(run_id))
        .await
        .unwrap();
    let deleted_evidence: i64 = sqlx::query_scalar(
        r#"
        SELECT
            (SELECT COUNT(*) FROM context_snapshots WHERE agent_run_id=$1)
            + (SELECT COUNT(*) FROM context_compile_attempts WHERE agent_run_id=$1)
            + (SELECT COUNT(*) FROM model_calls WHERE agent_run_id=$1)
        "#,
    )
    .bind(run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(deleted_evidence, 0);
    let fork_evidence: i64 = sqlx::query_scalar(
        r#"
        SELECT
            (SELECT COUNT(*) FROM context_snapshots WHERE agent_run_id=$1)
            + (SELECT COUNT(*) FROM context_compile_attempts WHERE agent_run_id=$1)
            + (SELECT COUNT(*) FROM model_calls WHERE agent_run_id=$1)
        "#,
    )
    .bind(fork_run_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        fork_evidence, 3,
        "adjacent fork evidence must remain isolated"
    );

    let definition_delete_links: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pg_constraint constraint_info
        JOIN pg_class source_table ON source_table.oid=constraint_info.conrelid
        JOIN pg_class target_table ON target_table.oid=constraint_info.confrelid
        WHERE constraint_info.contype='f'
          AND source_table.relname IN ('context_snapshots', 'context_compile_attempts')
          AND target_table.relname IN (
              'definition_releases', 'definition_release_manifests',
              'definition_release_manifest_entries'
          )
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        definition_delete_links, 0,
        "Registry revoke/rollback must not own historical Context evidence"
    );

    pool.close().await;
    admin.close().await;
    drop(database);
}
