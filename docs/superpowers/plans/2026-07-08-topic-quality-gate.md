# Topic Quality Gate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `topic` Agent 候选选题入库前增加质量闸门，只保存通过项，并为批次提供质量报告。

**Architecture:** 质量闸门挂在 `AgentRuntime::handle_topic_turn` 的候选生成和 `content_topics` 写入之间。后端新增 `topic_quality_evaluations` 快照表、domain model、repository 方法、LLM 评估 prompt 和批次报告 API；前端任务必须等用户明确确认原型后再执行。

**Tech Stack:** Rust, Axum, SQLx, PostgreSQL, serde/serde_json, tokio tests, OpenSpec.

---

## Scope

- 本计划执行 OpenSpec change `topic-quality-gate` 的后端和数据库部分。
- 不新增 `ContentTopic.status`，不新增 `quality_rejected`。
- 不自动确认、归档、删除选题或生成脚本。
- 不执行 `git add`、`git commit`、`git push`，除非用户另行明确要求。
- 前端实现任务等待用户明确口令，例如“确认开发”“按这个原型开发”“这个版本通过”。

## File Map

- Create: `backend/migrations/20260708010000_topic_quality_evaluations.sql`
  - 新增质量评估快照表、状态约束、更新时间触发器和索引。
- Modify: `backend/tests/database_migrations.rs`
  - 红灯测试要求 migration 创建表、约束和索引。
- Modify: `backend/src/agents/models/topic.rs`
  - 新增 `TopicQualityEvaluationStatus`、`TopicQualityDecision`、`TopicQualityFlag`、`TopicQualityGateItem`、`TopicQualityGateResult`、`TopicQualityEvaluation`。
- Modify: `backend/src/agents/models/mod.rs`
  - 导出质量闸门模型。
- Modify: `backend/src/agents/models/request.rs`
  - 新增 `TopicQualityEvaluationResponse`，并在批次列表摘要里预留质量摘要字段。
- Modify: `backend/src/repositories/topic_repository.rs`
  - 新增 `CreateTopicQualityEvaluationInput`、trait 方法、Postgres 实现、row mapper。
- Modify: `backend/tests/topic_repository_contract.rs`
  - 内存仓储支持创建和读取最新质量评估。
- Modify: `backend/tests/topic_postgres_repository.rs`
  - Postgres 仓储验证创建、最新读取、项目隔离和失败快照。
- Modify: `backend/src/agents/conversational_runtime.rs`
  - 新增质量评估 prompt、schema、解析校验、重写一次逻辑、失败处理和 step 记录。
- Modify: `backend/tests/topic_agent_runtime.rs`
  - 验证通过项入库、淘汰项不入库、低通过率重写、评估失败、补充上下文。
- Modify: `backend/src/lib.rs`
  - 新增 `GET /api/topic-generation-batches/:batch_id/quality-evaluation`。
- Modify: `backend/tests/topic_routes.rs`
  - 验证质量报告 API 只返回同项目最新评估。
- Modify: `openspec/changes/topic-quality-gate/tasks.md`
  - 每完成一个任务立即勾选。

---

## Task 1: Migration Red-Green

**Files:**
- Modify: `backend/tests/database_migrations.rs`
- Create: `backend/migrations/20260708010000_topic_quality_evaluations.sql`

- [ ] **Step 1: Write the failing migration test**

Add `topic_quality_evaluations` to the table list, and add assertions for:

```rust
assert!(
    constraint_exists(
        &test_pool,
        "topic_quality_evaluations",
        "topic_quality_evaluations_status_check"
    )
    .await,
    "topic quality evaluation status should be constrained"
);
for index in [
    "idx_topic_quality_evaluations_project_batch_created",
    "idx_topic_quality_evaluations_source_run",
    "idx_topic_quality_evaluations_status",
] {
    assert!(index_exists(&test_pool, index).await, "{index} index should exist");
}
```

- [ ] **Step 2: Run red**

Run:

```bash
docker exec ai-agent-api sh -lc 'cd /app/backend && /usr/local/cargo/bin/cargo test --test database_migrations migrations_create_video_agent_core_schema -- --exact'
```

Expected: FAIL because `topic_quality_evaluations` does not exist.

- [ ] **Step 3: Add migration**

Create the table with:

```sql
CREATE TABLE topic_quality_evaluations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    batch_id UUID NOT NULL REFERENCES topic_generation_batches(id) ON DELETE CASCADE,
    source_run_id UUID REFERENCES agent_runs(id) ON DELETE SET NULL,
    status VARCHAR(20) NOT NULL,
    pass_count INT NOT NULL DEFAULT 0,
    reject_count INT NOT NULL DEFAULT 0,
    rewrite_triggered BOOLEAN NOT NULL DEFAULT FALSE,
    result JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT topic_quality_evaluations_status_check
        CHECK (status IN ('succeeded', 'failed'))
);
```

Also create:

```sql
CREATE INDEX idx_topic_quality_evaluations_project_batch_created
    ON topic_quality_evaluations (project_id, batch_id, created_at DESC, id DESC);
CREATE INDEX idx_topic_quality_evaluations_source_run
    ON topic_quality_evaluations (source_run_id)
    WHERE source_run_id IS NOT NULL;
CREATE INDEX idx_topic_quality_evaluations_status
    ON topic_quality_evaluations (status);
```

- [ ] **Step 4: Run green**

Run the same `cargo test` command. Expected: PASS.

---

## Task 2: Domain Models and Repository Contract

**Files:**
- Modify: `backend/src/agents/models/topic.rs`
- Modify: `backend/src/agents/models/mod.rs`
- Modify: `backend/src/repositories/topic_repository.rs`
- Modify: `backend/tests/topic_repository_contract.rs`

- [ ] **Step 1: Write failing contract test**

Add a test that creates two quality evaluations for the same batch and asserts the latest succeeded one is returned:

```rust
let result = TopicQualityGateResult {
    summary: "本批 2 条中 1 条通过，1 条淘汰。".to_string(),
    items: vec![TopicQualityGateItem {
        candidate_key: "candidate-1".to_string(),
        title: "AI 工具选题".to_string(),
        decision: TopicQualityDecision::Pass,
        quality_score: 86,
        flags: vec![],
        reason: "贴合账号定位。".to_string(),
    }],
};
```

- [ ] **Step 2: Run red**

Run:

```bash
docker exec ai-agent-api sh -lc 'cd /app/backend && /usr/local/cargo/bin/cargo test --test topic_repository_contract topic_repository_trait_supports_topic_quality_evaluations -- --exact'
```

Expected: compile FAIL until model and trait methods exist.

- [ ] **Step 3: Add domain model and trait signatures**

Add enums and structs mirroring the OpenSpec JSON. Add repository methods:

```rust
async fn create_topic_quality_evaluation(
    &self,
    input: CreateTopicQualityEvaluationInput,
) -> Result<TopicQualityEvaluation, TopicRepositoryError>;

async fn get_latest_topic_quality_evaluation(
    &self,
    project_id: Uuid,
    batch_id: Uuid,
) -> Result<Option<TopicQualityEvaluation>, TopicRepositoryError>;
```

- [ ] **Step 4: Implement memory repository for the contract test**

Store `TopicQualityEvaluation` in a `Mutex<HashMap<Uuid, TopicQualityEvaluation>>` and select latest by `created_at` then `id`.

- [ ] **Step 5: Run green**

Run the same contract test. Expected: PASS.

---

## Task 3: Postgres Repository

**Files:**
- Modify: `backend/src/repositories/topic_repository.rs`
- Modify: `backend/tests/topic_postgres_repository.rs`

- [ ] **Step 1: Write failing Postgres test**

Add a test that inserts a project, batch, run id, creates a succeeded quality evaluation, then asserts:

- `project_id` and `batch_id` match.
- `pass_count`, `reject_count`, `rewrite_triggered` are preserved.
- `result.items[0].flags` preserves `duplicate`.
- querying with another `project_id` returns `None`.

- [ ] **Step 2: Run red**

Run:

```bash
docker exec ai-agent-api sh -lc 'cd /app/backend && /usr/local/cargo/bin/cargo test --test topic_postgres_repository postgres_topic_repository_persists_topic_quality_evaluations -- --exact'
```

Expected: FAIL until Postgres implementation exists.

- [ ] **Step 3: Implement SQL insert and latest query**

Serialize `TopicQualityGateResult` to JSONB on insert, parse it back in the row mapper, and filter latest query by both `project_id` and `batch_id`.

- [ ] **Step 4: Run green**

Run the same Postgres test. Expected: PASS.

---

## Task 4: Runtime Quality Gate

**Files:**
- Modify: `backend/src/agents/conversational_runtime.rs`
- Modify: `backend/tests/topic_agent_runtime.rs`

- [ ] **Step 1: Write failing runtime pass/reject test**

Script the LLM responses in reverse call order because `ScriptedLLMClient` uses `Vec::pop()`:

```rust
responses: vec![
    Ok(quality_json.to_string()),
    Ok(generation_json.to_string()),
]
```

Assert only passed candidates are inserted, rejected candidates are absent, `metadata.quality_gate.quality_score` exists, agent message metadata includes `quality_pass_count`, `quality_reject_count`, and step types include `evaluate_topic_quality`.

- [ ] **Step 2: Run red**

Run:

```bash
docker exec ai-agent-api sh -lc 'cd /app/backend && /usr/local/cargo/bin/cargo test --test topic_agent_runtime topic_agent_filters_candidates_through_quality_gate_before_persisting -- --exact'
```

Expected: FAIL because current runtime directly persists all candidates.

- [ ] **Step 3: Add quality gate prompt, schema and parser**

Implement `TopicQualityLLMOutput::parse_and_validate(&raw, &candidates)` with:

- required non-empty `summary`;
- one item per candidate key;
- decisions only `pass` or `reject`;
- score range `0..=100`;
- allowed flags `too_generic`, `duplicate`, `off_positioning`, `hard_to_script`, `compliance_risk`, `score_untrusted`;
- pass only when score `>= 70` and no hard reject flags.

- [ ] **Step 4: Insert runtime flow**

Change flow to:

1. generate candidates;
2. evaluate quality;
3. if first pass rate `< 60%`, request rewrite once and re-evaluate;
4. create topics only for final pass items;
5. save `topic_quality_evaluations`;
6. mark batch `succeeded` only when at least one topic was created.

- [ ] **Step 5: Run green**

Run the same runtime test. Expected: PASS.

---

## Task 5: Rewrite and Failure Coverage

**Files:**
- Modify: `backend/tests/topic_agent_runtime.rs`
- Modify: `backend/src/agents/conversational_runtime.rs`

- [ ] **Step 1: Add failing tests**

Add tests for:

- first pass rate `< 60%` triggers exactly one rewrite;
- quality evaluation invalid JSON marks batch failed and creates no topics;
- rewrite returns no pass items marks batch failed and creates no topics;
- supplement generation quality prompt includes same topic group existing topics.

- [ ] **Step 2: Run red**

Run:

```bash
docker exec ai-agent-api sh -lc 'cd /app/backend && /usr/local/cargo/bin/cargo test --test topic_agent_runtime topic_agent_ -- --nocapture'
```

Expected: new tests FAIL before runtime handles all branches.

- [ ] **Step 3: Complete runtime failure handling**

Ensure failed quality evaluation writes a failed `topic_quality_evaluations` snapshot, failed batch error message, and failed `agent_steps` row. Ensure rewrite failure does not insert first-round candidates.

- [ ] **Step 4: Run green**

Run the targeted runtime tests. Expected: PASS.

---

## Task 6: Quality Report API

**Files:**
- Modify: `backend/src/agents/models/request.rs`
- Modify: `backend/src/agents/models/mod.rs`
- Modify: `backend/src/lib.rs`
- Modify: `backend/tests/topic_routes.rs`

- [ ] **Step 1: Write failing route test**

Insert two projects, one batch quality evaluation, then call:

```text
GET /api/topic-generation-batches/:batch_id/quality-evaluation?project_id=:project_id
```

Assert 200 returns latest report and wrong `project_id` returns 404.

- [ ] **Step 2: Run red**

Run:

```bash
docker exec ai-agent-api sh -lc 'cd /app/backend && /usr/local/cargo/bin/cargo test --test topic_routes topic_routes_get_latest_topic_quality_evaluation -- --exact'
```

Expected: FAIL because route does not exist.

- [ ] **Step 3: Add DTO and route**

Add `TopicQualityEvaluationResponse` and route:

```rust
.route(
    "/api/topic-generation-batches/:batch_id/quality-evaluation",
    get(get_latest_topic_quality_evaluation),
)
```

Resolve project ownership like topic group review: batch must belong to requested project when `project_id` is provided.

- [ ] **Step 4: Run green**

Run the same route test. Expected: PASS.

---

## Task 7: OpenSpec Task Updates and Backend Verification

**Files:**
- Modify: `openspec/changes/topic-quality-gate/tasks.md`

- [ ] **Step 1: Mark completed backend tasks**

Check off completed items in sections 3, 4 and backend verification. Do not mark front-end tasks until implemented after explicit prototype confirmation.

- [ ] **Step 2: Run backend verification**

Run:

```bash
docker exec ai-agent-api sh -lc 'cd /app/backend && /usr/local/cargo/bin/cargo test --test database_migrations --test topic_repository_contract --test topic_postgres_repository --test topic_agent_runtime --test topic_routes'
```

Expected: PASS.

- [ ] **Step 3: Run OpenSpec verification**

Run:

```bash
openspec validate --all
openspec instructions apply --change "topic-quality-gate" --json
```

Expected: validation PASS and progress matches checked tasks. `state` may remain `ready` until front-end tasks are completed.

---

## Self-Review

- Spec coverage: backend data model, repository, runtime quality gate, rewrite, failure handling and API are covered.
- Intentional gap: front-end tasks are not included because project rules require explicit prototype confirmation before front-end coding.
- No commit steps included because repository rules forbid `git add` / `git commit` / `git push` without explicit user confirmation.
