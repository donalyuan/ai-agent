# Improve Topic History Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the approved content strategy history generation list page and topic soft-delete rules.

**Architecture:** Add `deleted_at` to `content_topics`, keep business status unchanged, and enforce delete eligibility in the backend using both topic status and `scripts.topic_id`. Split the frontend into a page-level history component while keeping `app/page.tsx` as state orchestration.

**Tech Stack:** Rust + Axum + SQLx + PostgreSQL migrations, Next.js 14 + React + TypeScript + Vitest + Playwright.

---

### Task 1: Backend Topic Soft Delete Contract

**Files:**
- Modify: `backend/src/agents/models/topic.rs`
- Modify: `backend/src/repositories/topic_repository.rs`
- Test: `backend/tests/topic_repository_contract.rs`

- [ ] **Step 1: Write failing repository contract tests**

Add tests proving deleted topics disappear from `list_topics`, `count_topics_by_status`, and `list_generation_batches`, and that deleting a `scripted` topic returns an error.

- [ ] **Step 2: Run RED**

Run: `docker exec ai-agent-api cargo test --test topic_repository_contract`
Expected: compile failure or test failure because `deleted_at` / `soft_delete_topic` do not exist.

- [ ] **Step 3: Implement repository contract**

Add `deleted_at: Option<DateTime<Utc>>` to `ContentTopic`, add `soft_delete_topic(topic_id, has_script_reference)` to the trait, add `TopicCannotBeDeleted` error, and update in-memory filtering/counting.

- [ ] **Step 4: Run GREEN**

Run: `docker exec ai-agent-api cargo test --test topic_repository_contract`
Expected: all tests in the file pass.

### Task 2: PostgreSQL Migration And Repository

**Files:**
- Create: `backend/migrations/20260707010000_content_topic_soft_delete.sql`
- Modify: `backend/src/repositories/topic_repository.rs`
- Test: `backend/tests/topic_postgres_repository.rs`

- [ ] **Step 1: Write failing Postgres repository tests**

Assert that soft-deleted topics are excluded from list/stats/batch counts, and that a scripted topic or topic referenced by `scripts.topic_id` cannot be deleted.

- [ ] **Step 2: Run RED**

Run: `docker exec ai-agent-api cargo test --test topic_postgres_repository`
Expected: failure before migration/repository implementation.

- [ ] **Step 3: Add migration and SQL implementation**

Migration adds `content_topics.deleted_at TIMESTAMPTZ NULL`, comments, and indexes for default visible-topic queries. Repository SQL adds `deleted_at IS NULL` filters and implements soft delete with script-reference checks.

- [ ] **Step 4: Run GREEN**

Run: `docker exec ai-agent-api cargo test --test topic_postgres_repository`
Expected: all tests in the file pass.

### Task 3: Backend API

**Files:**
- Modify: `backend/src/lib.rs`
- Modify: `backend/src/agents/models/request.rs`
- Test: `backend/tests/topic_routes.rs`

- [ ] **Step 1: Write failing route tests**

Add tests for `DELETE /api/topics/:topic_id`: idea/approved topics delete successfully, scripted or referenced topics fail, and deleted topics cannot be prepared for script generation.

- [ ] **Step 2: Run RED**

Run: `docker exec ai-agent-api cargo test --test topic_routes`
Expected: failure because route and response behavior are missing.

- [ ] **Step 3: Implement route**

Allow `DELETE` in CORS, add `.route("/api/topics/:topic_id", put(update_topic).delete(delete_topic))`, return a stable response body for deleted topic ID and `deleted_at`, and reject non-deletable topics with existing API error style.

- [ ] **Step 4: Run GREEN**

Run: `docker exec ai-agent-api cargo test --test topic_routes`
Expected: all tests in the file pass.

### Task 4: Frontend API Client

**Files:**
- Modify: `apps/video-agent/app/lib/api.ts`
- Test: `apps/video-agent/app/lib/api.test.ts`

- [ ] **Step 1: Write failing API client test**

Add `deleteContentTopic(client, topicId)` test expecting `DELETE http://api.test/api/topics/<topicId>`.

- [ ] **Step 2: Run RED**

Run: `docker exec ai-agent-video-agent npm run test -- app/lib/api.test.ts`
Expected: failure because `deleteContentTopic` is missing.

- [ ] **Step 3: Implement API client**

Add `DeletedContentTopicResponse` type and `deleteContentTopic`.

- [ ] **Step 4: Run GREEN**

Run: `docker exec ai-agent-video-agent npm run test -- app/lib/api.test.ts`
Expected: API tests pass.

### Task 5: Frontend History Page

**Files:**
- Create: `apps/video-agent/app/pages/content-strategy/TopicHistoryPage.tsx`
- Modify: `apps/video-agent/app/pages/content-strategy/ContentStrategyPage.tsx`
- Modify: `apps/video-agent/app/pages/content-strategy/topicModel.ts`
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/styles.css`
- Test: `apps/video-agent/app/page.test.tsx`

- [ ] **Step 1: Write failing page tests**

Test that “历史生成” appears above “当前选题池”, clicking it shows a list-page view, ungenerated topics have “移除”, scripted topics show “已生成脚本，不可删除”, and delete success refreshes topics and batches.

- [ ] **Step 2: Run RED**

Run: `docker exec ai-agent-video-agent npm run test -- app/page.test.tsx`
Expected: failure because the history page and delete behavior are missing.

- [ ] **Step 3: Implement page-level component**

Create `TopicHistoryPage.tsx` for batch list/detail and row actions. Add content-strategy view state in `app/page.tsx` with `"history"` and `"pool"`. Keep `ContentStrategyPage.tsx` focused on the current pool.

- [ ] **Step 4: Implement styles**

Add compact list/table-like styling using existing tokens, fixed row heights where needed, danger action style for removable topics, and neutral lock state for scripted topics.

- [ ] **Step 5: Run GREEN**

Run: `docker exec ai-agent-video-agent npm run test -- app/page.test.tsx`
Expected: page tests pass.

### Task 6: E2E And OpenSpec Progress

**Files:**
- Modify: `apps/video-agent/e2e/workspace.spec.ts`
- Modify: `openspec/changes/improve-topic-history-management/tasks.md`

- [ ] **Step 1: Add E2E coverage**

Cover the content strategy history entry, list page visibility, and scripted-topic delete restriction.

- [ ] **Step 2: Run relevant E2E**

Run: `docker exec ai-agent-video-agent npm run test:e2e`
Expected: workspace E2E passes.

- [ ] **Step 3: Mark OpenSpec tasks as complete as each implementation group finishes**

Only mark tasks that were actually completed and verified.

### Task 7: Final Verification

**Files:**
- No new files.

- [ ] **Step 1: Run backend verification**

Run: `docker exec ai-agent-api cargo fmt -- --check`
Run: `docker exec ai-agent-api cargo test`
Run: `docker exec ai-agent-api cargo clippy --all-targets --all-features -- -D warnings`

- [ ] **Step 2: Run frontend verification**

Run: `docker exec ai-agent-video-agent npm run lint`
Run: `docker exec ai-agent-video-agent npm run test`
Run: `docker exec ai-agent-video-agent npm run build`
Run: `docker exec ai-agent-video-agent npm run test:e2e`

- [ ] **Step 3: Run OpenSpec and whitespace checks**

Run: `openspec instructions apply --change "improve-topic-history-management" --json`
Run: `openspec validate --all`
Run: `git diff --check`

- [ ] **Step 4: Do not commit**

Project rules require explicit user confirmation before `git add`, `git commit`, or `git push`.
