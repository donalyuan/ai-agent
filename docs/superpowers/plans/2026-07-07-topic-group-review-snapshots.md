# Topic Group Review Snapshots Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build topic-group AI review snapshots so content strategy users can quickly screen too many topics by priority, backup, rejection, duplicate relations, and risks.

**Architecture:** Add a theme-group review snapshot model keyed by `project_id + root_batch_id`, store structured AI review output as JSONB, and expose create/latest APIs. The frontend keeps the existing content strategy and history pages but shares a review-layered topic list component between them. AI review never changes `ContentTopic.status`.

**Tech Stack:** Rust + Axum + SQLx + PostgreSQL, existing Agent Runtime/LLM client, Next.js 14 + TypeScript + Vitest + Playwright, Pencil prototype.

---

## File Structure

- Modify: `docs/prototypes/video-agent/video-agent.pen` — Pencil source for the confirmed desktop prototype.
- Create: `backend/migrations/20260707040000_topic_review_snapshots.sql` — review snapshot table and indexes.
- Modify: `backend/src/agents/models/topic.rs` — topic review domain structs and stable enums.
- Modify: `backend/src/agents/models/request.rs` — API response/request DTOs.
- Modify: `backend/src/repositories/topic_repository.rs` — review snapshot repository methods.
- Modify: `backend/src/agents/conversational_runtime.rs` — topic-group review prompt, LLM parse, run/step recording.
- Modify: `backend/src/lib.rs` — review routes and error mapping.
- Test: `backend/tests/topic_review_repository.rs` — repository and migration behavior.
- Test: `backend/tests/topic_review_routes.rs` — route and validation behavior.
- Test: `backend/tests/topic_agent_runtime.rs` — LLM review parsing and failure semantics.
- Modify: `apps/video-agent/app/lib/api.ts` and `apps/video-agent/app/lib/api.test.ts` — API client types and tests.
- Create: `apps/video-agent/app/pages/content-strategy/TopicReviewList.tsx` — shared layered review list.
- Modify: `apps/video-agent/app/pages/content-strategy/ContentStrategyPage.tsx` — current pool review entry and display.
- Modify: `apps/video-agent/app/pages/content-strategy/TopicHistoryPage.tsx` — history page review entry and display.
- Modify: `apps/video-agent/app/page.tsx` and `apps/video-agent/app/page.test.tsx` — shared state orchestration and tests.
- Modify: `apps/video-agent/e2e/workspace.spec.ts` — browser workflow coverage.
- Modify: `openspec/changes/topic-group-review-snapshots/tasks.md` — tick items as they complete.

No `git add`, `git commit`, or `git push` is allowed without explicit user confirmation.

## Task 1: Pencil Prototype

**Files:**
- Modify: `docs/prototypes/video-agent/video-agent.pen`
- Modify: `openspec/changes/topic-group-review-snapshots/tasks.md`

- [ ] **Step 1: Add history review prototype frame**

Add a desktop frame named `桌面 - 主题组评审历史生成 v3 待确认`. It must show:

- Left menu with `内容策略` active and `历史生成` active.
- Three-column history layout.
- Manual button `评审当前主题组`.
- Middle column layered sections: `优先推荐`、`可备选`、`建议淘汰`、`疑似重复`.
- Risk tags and duplicate relation labels.
- Existing actions still visible: `确认选题`、`生成脚本`、`归档`、`移除`.

- [ ] **Step 2: Add current pool review sync frame**

Add a desktop frame named `桌面 - 当前选题池评审同步 v3 待确认`. It must show:

- Left menu with `内容策略` active and `当前选题池` active.
- A topic-batch notice showing the current root batch context.
- The same latest review snapshot content as the history frame.
- Fallback note: `查看全部选题时不展示主题组评审`.

- [ ] **Step 3: Verify prototype is readable**

Run:

```bash
python -m json.tool docs/prototypes/video-agent/video-agent.pen >/tmp/video-agent.pen.json
```

Expected: command exits `0`.

- [ ] **Step 4: Re-read new frames through Pencil MCP**

Use `mcp__pencil.batch_get` with patterns matching `主题组评审` and `评审同步`.

Expected: both new frames are returned with direct children.

## Task 2: Database And Repository Tests

**Files:**
- Create: `backend/tests/topic_review_repository.rs`
- Create: `backend/migrations/20260707040000_topic_review_snapshots.sql`
- Modify: `backend/src/agents/models/topic.rs`
- Modify: `backend/src/repositories/topic_repository.rs`

- [ ] **Step 1: Write failing repository tests**

Add tests proving:

- A review snapshot can be created for `project_id + root_batch_id`.
- `get_latest_topic_review_snapshot(project_id, root_batch_id)` returns newest succeeded snapshot.
- Other projects cannot read the snapshot.
- Failed snapshots do not replace latest succeeded snapshot.

- [ ] **Step 2: Run failing tests**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T novex-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test topic_review_repository'
```

Expected: compile failure because model and repository methods do not exist.

- [ ] **Step 3: Add migration, model, and repository implementation**

Add `topic_review_snapshots` with `project_id`, `root_batch_id`, `source_run_id`, `status`, `review_summary`, `result`, `error_message`, `metadata`, timestamps, and indexes.

- [ ] **Step 4: Run repository tests**

Run the same command.

Expected: tests pass.

## Task 3: Review Runtime And Route Tests

**Files:**
- Modify: `backend/tests/topic_agent_runtime.rs`
- Create: `backend/tests/topic_review_routes.rs`
- Modify: `backend/src/agents/conversational_runtime.rs`
- Modify: `backend/src/agents/models/request.rs`
- Modify: `backend/src/lib.rs`

- [ ] **Step 1: Write failing runtime tests**

Cover:

- Valid LLM output creates a succeeded review snapshot.
- Review prompt includes project positioning, original batch prompt, original topics, supplement topics, and source labels.
- Output referencing a topic outside the group fails.
- Invalid `priority` and invalid `risk_flags` fail.
- Neither success nor failure changes topic status.

- [ ] **Step 2: Write failing route tests**

Cover:

- `POST /api/topic-groups/:root_batch_id/reviews` creates review for current project context.
- `GET /api/topic-groups/:root_batch_id/reviews/latest` returns latest succeeded snapshot.
- Missing batch, cross-project batch, and empty visible topic group return stable errors.

- [ ] **Step 3: Run failing tests**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T novex-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test topic_agent_runtime --test topic_review_routes'
```

Expected: compile or assertion failures.

- [ ] **Step 4: Implement runtime and routes**

Implement structured review output parsing, stable enums, run/step recording, route handlers, and JSON responses. Reuse existing topic group lookup logic.

- [ ] **Step 5: Run backend tests**

Run the same command.

Expected: tests pass.

## Task 4: Frontend API And Shared Review List

**Files:**
- Modify: `apps/video-agent/app/lib/api.ts`
- Modify: `apps/video-agent/app/lib/api.test.ts`
- Create: `apps/video-agent/app/pages/content-strategy/TopicReviewList.tsx`

- [ ] **Step 1: Write failing API tests**

Add tests for:

- `createTopicGroupReview(client, rootBatchId)`.
- `getLatestTopicGroupReview(client, rootBatchId)`.
- Request URLs exactly match `/api/topic-groups/<rootBatchId>/reviews` and `/api/topic-groups/<rootBatchId>/reviews/latest`.

- [ ] **Step 2: Implement API types and client methods**

Define `TopicReviewPriority`, `TopicReviewRiskFlag`, `TopicReviewSnapshot`, `TopicReviewItem`, and the two API methods.

- [ ] **Step 3: Create shared `TopicReviewList` component**

Component props must include `topics`, `reviewSnapshot`, `writesDisabled`, and callbacks for existing topic actions. It groups reviewed topics by `priority`, shows risk flags, shows duplicate references, and falls back to ordinary rows when no snapshot is provided.

- [ ] **Step 4: Run frontend API tests**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T novex-video-agent sh -lc 'cd /app && npm test -- --run app/lib/api.test.ts'
```

Expected: tests pass.

## Task 5: Frontend Page Integration

**Files:**
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/page.test.tsx`
- Modify: `apps/video-agent/app/pages/content-strategy/ContentStrategyPage.tsx`
- Modify: `apps/video-agent/app/pages/content-strategy/TopicHistoryPage.tsx`
- Modify: `apps/video-agent/app/styles.css`
- Modify: `apps/video-agent/e2e/workspace.spec.ts`

- [ ] **Step 1: Write failing page tests**

Cover:

- History page shows `评审当前主题组`.
- Clicking review calls the new API and reloads latest snapshot.
- Current pool displays the same review snapshot when filtered by the selected theme group.
- Current pool in all-topics mode shows ordinary list and no review layering.
- Existing confirm, archive, remove, and generate-script actions still work.

- [ ] **Step 2: Integrate shared state**

Add orchestration for `reviewSnapshot`, `reviewLoading`, `reviewError`, and current `rootBatchId`. Load latest snapshot when theme group changes.

- [ ] **Step 3: Integrate history and pool pages**

Pass review props into both pages. Render `TopicReviewList` in theme-group mode and ordinary list in all-topics fallback.

- [ ] **Step 4: Add restrained CSS**

Use existing tokens for layered sections, risk tags, duplicate chips, and stable action row sizes. Do not add hero blocks, gradients, nested cards, or decorative backgrounds.

- [ ] **Step 5: Run frontend tests**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T novex-video-agent sh -lc 'cd /app && npm test -- --run app/page.test.tsx app/lib/api.test.ts'
```

Expected: tests pass.

- [ ] **Step 6: Run E2E**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T novex-video-agent sh -lc 'cd /app && npx playwright test e2e/workspace.spec.ts'
```

Expected: tests pass.

## Task 6: OpenSpec And Final Verification

**Files:**
- Modify: `openspec/changes/topic-group-review-snapshots/tasks.md`
- Modify if needed: `MEMORY.md`
- Modify if needed: `docs/memory/video-agent-workspace-flow.md`

- [ ] **Step 1: Mark completed OpenSpec tasks**

Tick only tasks actually completed.

- [ ] **Step 2: Run OpenSpec verification**

Run:

```bash
openspec instructions apply --change "topic-group-review-snapshots" --json
```

Expected: task progress matches actual completed work.

- [ ] **Step 3: Run final related backend tests**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T novex-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test topic_review_repository --test topic_review_routes --test topic_agent_runtime'
```

Expected: tests pass.

- [ ] **Step 4: Run final related frontend tests**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T novex-video-agent sh -lc 'cd /app && npm test -- --run app/page.test.tsx app/lib/api.test.ts'
```

Expected: tests pass.

- [ ] **Step 5: Report changed files and verification**

Run:

```bash
git status --short
```

Expected: only planned files are modified or created.

## Self-Review

- Spec coverage: plan covers prototype, data model, runtime, API, frontend sync display, all-topics fallback, tests, and OpenSpec progress.
- Placeholder scan: no `TBD`, `TODO`, or undefined future tasks are used.
- Type consistency: plan uses `TopicReviewSnapshot`, `root_batch_id`, `priority|backup|reject`, and stable `risk_flags` consistently with the approved design.
