# Topic Group Script Priority Ranking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build theme-group script-production priority ranking so the History page surfaces the topic groups most worth turning into scripts immediately.

**Architecture:** Add a backend read model that aggregates original topic batches with supplement batches, evaluates latest review freshness, and computes deterministic script priority. The frontend consumes that read model for the History sidebar while preserving the existing topic review, supplement, and topic-pool synchronization flow.

**Tech Stack:** Rust + Axum + SQLx + PostgreSQL backend; Next.js + TypeScript + Vitest frontend; Pencil MCP for prototype updates; Docker Compose containers for validation.

---

## File Structure

- Modify `docs/prototypes/video-agent/video-agent.pen`: add History sidebar ranking prototype before frontend code.
- Modify `backend/src/agents/models/topic.rs`: add `TopicGroupReviewFreshness`, `TopicGroupScriptPriorityStatus`, metrics, priority, and group summary structs.
- Modify `backend/src/repositories/topic_repository.rs`: add repository methods for topic group summaries and priority computation.
- Modify `backend/src/agents/models/request.rs`: add response DTOs for `/api/projects/:project_id/topic-groups`.
- Modify `backend/src/lib.rs`: add route and handler for topic group summaries.
- Add `backend/tests/topic_group_priority_repository.rs`: repository and scoring behavior tests.
- Add `backend/tests/topic_group_priority_routes.rs`: API contract tests.
- Modify `apps/video-agent/app/lib/api.ts`: add TypeScript types and `listTopicGroups`.
- Modify `apps/video-agent/app/lib/api.test.ts`: API wrapper tests.
- Modify `apps/video-agent/app/page.tsx`: load topic-group summaries and pass them to History page.
- Modify `apps/video-agent/app/pages/content-strategy/TopicHistoryPage.tsx`: render sort toggle and ranked group cards.
- Modify `apps/video-agent/app/page.test.tsx`: integration behavior tests.
- Modify `openspec/changes/rank-topic-groups-for-script-production/tasks.md`: mark tasks as completed immediately after verification.

## Task 1: Pencil Prototype Gate

**Files:**
- Modify: `docs/prototypes/video-agent/video-agent.pen`
- Modify: `openspec/changes/rank-topic-groups-for-script-production/tasks.md`

- [ ] **Step 1: Read current Pencil document structure**

Run:

```bash
true
```

Use Pencil MCP:

```text
open_document("/server/ai-agent/docs/prototypes/video-agent/video-agent.pen")
batch_get(filePath="/server/ai-agent/docs/prototypes/video-agent/video-agent.pen", readDepth=2)
```

Expected: identify the existing History page frame and sidebar nodes.

- [ ] **Step 2: Update the History sidebar prototype**

Use Pencil MCP `batch_design` to add:

```text
排序切换：脚本优先 / 按时间
建议立刻出脚本：86 分 · 3 个候选 · 风险低
需重新评审：评审已过期 · 点击评审当前主题组
需补充：缺少无风险脚本候选
暂缓：重复/偏离定位较多
```

Expected: History page prototype shows script-priority ranking states without changing the existing three-column layout.

- [ ] **Step 3: Validate prototype through Pencil MCP**

Use:

```text
batch_get(filePath="/server/ai-agent/docs/prototypes/video-agent/video-agent.pen", patterns=[{name:"历史生成"}], readDepth=3)
snapshot_layout(filePath="/server/ai-agent/docs/prototypes/video-agent/video-agent.pen", problemsOnly=true)
```

Expected: updated nodes exist; no obvious overlap or clipped elements in the changed History page frame.

- [ ] **Step 4: Mark prototype task complete**

Update `tasks.md`:

```markdown
- [x] 更新 `docs/prototypes/video-agent/video-agent.pen`，覆盖历史生成页主题组脚本优先级排序。
```

Expected: OpenSpec progress advances, but frontend code remains untouched until user confirms prototype.

## Task 2: Backend Repository Red Tests

**Files:**
- Add: `backend/tests/topic_group_priority_repository.rs`

- [ ] **Step 1: Write failing repository tests**

Create tests covering:

```rust
#[tokio::test]
async fn topic_group_summaries_rank_fresh_reviewed_groups_for_script_production() {
    // ready group has fresh review, ready candidates, and should rank first.
}

#[tokio::test]
async fn topic_group_summaries_mark_missing_and_stale_reviews_as_needs_review() {
    // missing review and stale review must return score None and status needs_review.
}

#[tokio::test]
async fn topic_group_summaries_fold_supplements_into_root_batch() {
    // supplement batch must not appear as an independent topic group.
}
```

- [ ] **Step 2: Run red tests**

Run:

```bash
docker exec ai-agent-api cargo test --test topic_group_priority_repository -- --nocapture
```

Expected: tests fail because topic group summary methods and models do not exist.

## Task 3: Backend Repository Implementation

**Files:**
- Modify: `backend/src/agents/models/topic.rs`
- Modify: `backend/src/repositories/topic_repository.rs`

- [ ] **Step 1: Add domain structs**

Add stable enums and structs:

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicGroupReviewFreshness {
    Fresh,
    Missing,
    Stale,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TopicGroupScriptPriorityStatus {
    ReadyForScript,
    NeedsReview,
    NeedsSupplement,
    Defer,
}
```

- [ ] **Step 2: Add repository methods**

Extend `TopicRepository` with:

```rust
async fn list_topic_group_summaries(
    &self,
    project_id: Uuid,
    sort: TopicGroupSort,
    limit: i64,
) -> Result<Vec<TopicGroupSummary>, TopicRepositoryError>;
```

- [ ] **Step 3: Implement scoring**

Implement deterministic scoring from OpenSpec:

```text
ready_candidate_count * 22
+ priority_count * 8
+ high_score_topic_count * 5
+ backup_count * 2
- reject_count * 6
- duplicate_count * 5
- hard_to_script_count * 10
- off_positioning_count * 10
- compliance_risk_count * 15
```

Expected: score is clamped to `0..100`; non-fresh review returns `score = None` and `status = NeedsReview`.

- [ ] **Step 4: Run green repository tests**

Run:

```bash
docker exec ai-agent-api cargo test --test topic_group_priority_repository -- --nocapture
```

Expected: repository tests pass.

## Task 4: Backend Route Red-Green

**Files:**
- Add: `backend/tests/topic_group_priority_routes.rs`
- Modify: `backend/src/agents/models/request.rs`
- Modify: `backend/src/lib.rs`

- [ ] **Step 1: Write failing route tests**

Add tests for:

```rust
#[tokio::test]
async fn topic_group_priority_route_returns_ranked_groups_for_project() {
}

#[tokio::test]
async fn topic_group_priority_route_supports_created_at_sort_and_project_isolation() {
}
```

- [ ] **Step 2: Run red route tests**

Run:

```bash
docker exec ai-agent-api cargo test --test topic_group_priority_routes -- --nocapture
```

Expected: tests fail with `404` or missing DTO/type errors.

- [ ] **Step 3: Add route and DTOs**

Add route:

```rust
.route("/api/projects/:project_id/topic-groups", get(list_topic_groups))
```

Add handler:

```rust
async fn list_topic_groups(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<TopicGroupListQuery>,
) -> Result<Json<TopicGroupListResponse>, ScriptApiError>
```

- [ ] **Step 4: Run green route tests**

Run:

```bash
docker exec ai-agent-api cargo test --test topic_group_priority_routes -- --nocapture
```

Expected: route tests pass.

## Task 5: Frontend API Red-Green

**Files:**
- Modify: `apps/video-agent/app/lib/api.ts`
- Modify: `apps/video-agent/app/lib/api.test.ts`

- [ ] **Step 1: Write failing API wrapper test**

Add test:

```typescript
it("lists topic groups with script priority sort", async () => {
  fetchMock.mockResolvedValueOnce(jsonResponse({ topic_groups: [] }));
  await listTopicGroups(client, project.project_id, { sort: "script_priority" });
  expect(fetchMock).toHaveBeenCalledWith(
    `http://api.test/api/projects/${project.project_id}/topic-groups?sort=script_priority`,
    expect.anything(),
  );
});
```

- [ ] **Step 2: Run red API test**

Run:

```bash
docker exec ai-agent-video-agent npm run test -- app/lib/api.test.ts
```

Expected: test fails because `listTopicGroups` is not exported.

- [ ] **Step 3: Add frontend types and wrapper**

Add:

```typescript
export type TopicGroupSort = "script_priority" | "created_at";
export type TopicGroupReviewFreshness = "fresh" | "missing" | "stale";
export type TopicGroupScriptPriorityStatus =
  | "ready_for_script"
  | "needs_review"
  | "needs_supplement"
  | "defer";
```

- [ ] **Step 4: Run green API test**

Run:

```bash
docker exec ai-agent-video-agent npm run test -- app/lib/api.test.ts
```

Expected: API test passes.

## Task 6: Frontend Page Red-Green

**Files:**
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/pages/content-strategy/TopicHistoryPage.tsx`
- Modify: `apps/video-agent/app/page.test.tsx`

- [ ] **Step 1: Write failing page tests**

Add tests:

```typescript
it("defaults history sidebar to script priority ranking", async () => {
  // render content strategy history page, expect 脚本优先 selected and ready group first.
});

it("marks stale or missing review groups as needing review", async () => {
  // expect 需重新评审 / 待评审 copy and existing review action available.
});
```

- [ ] **Step 2: Run red page test**

Run:

```bash
docker exec ai-agent-video-agent npm run test -- app/page.test.tsx
```

Expected: tests fail because topic group summaries are not loaded or rendered.

- [ ] **Step 3: Implement minimal page state and rendering**

Add state:

```typescript
const [topicGroups, setTopicGroups] = useState<TopicGroupSummary[]>([]);
const [topicGroupSort, setTopicGroupSort] = useState<TopicGroupSort>("script_priority");
```

Pass `topicGroups`, `topicGroupSort`, and `onTopicGroupSortChange` into `TopicHistoryPage`.

- [ ] **Step 4: Run green page test**

Run:

```bash
docker exec ai-agent-video-agent npm run test -- app/page.test.tsx
```

Expected: page tests pass.

## Task 7: Verification And OpenSpec Progress

**Files:**
- Modify: `openspec/changes/rank-topic-groups-for-script-production/tasks.md`

- [ ] **Step 1: Run backend verification**

Run:

```bash
docker exec ai-agent-api cargo fmt -- --check
docker exec ai-agent-api cargo test
docker exec ai-agent-api cargo clippy --all-targets --all-features -- -D warnings
```

Expected: all commands exit 0.

- [ ] **Step 2: Run frontend verification**

Run:

```bash
docker exec ai-agent-video-agent npm run lint
docker exec ai-agent-video-agent npm run test
docker exec ai-agent-video-agent npm run build
docker exec ai-agent-video-agent npm run test:e2e
```

Expected: all commands exit 0.

- [ ] **Step 3: Run OpenSpec verification**

Run:

```bash
openspec instructions apply --change "rank-topic-groups-for-script-production" --json
openspec validate --all
```

Expected: OpenSpec validates; tasks reflect actual implementation progress.

- [ ] **Step 4: Do not commit without explicit user confirmation**

Run:

```bash
git status --short
```

Expected: working tree shows implementation changes only. Do not run `git add`, `git commit`, or `git push` unless the user explicitly confirms.

## Self-Review

- Spec coverage: prototype gate, backend read model, route, frontend API, page rendering, and verification are covered.
- No frontend code starts before the Pencil prototype is updated and user confirms it.
- TDD order is preserved for backend repository, backend route, frontend API, and frontend page behavior.
- The plan does not include git commit steps because project rules require explicit user confirmation before `git add` / `git commit` / `git push`.
