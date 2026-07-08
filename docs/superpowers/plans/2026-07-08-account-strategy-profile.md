# Account Strategy Profile Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在内容策略下实现独立“账号策略”页面，沉淀 `projects.strategy_profile`，支持 AI 生成策略草稿并在人工确认后保存。

**Architecture:** 第一版继续以 `projects` 作为内容账号和内容生产边界；后端扩展 project repository/API，并由 `AgentRuntime` 统一格式化账号策略上下文注入选题生成、质量闸门和主题组评审。前端新增独立页面级组件承载账号策略编辑，当前选题池不展示账号策略区块。

**Tech Stack:** Rust + Axum + SQLx + PostgreSQL JSONB；Next.js 14 + TypeScript + Vitest + Playwright；OpenSpec spec-driven workflow。

---

### File Map

- Create: `backend/migrations/20260708020000_account_strategy_profile.sql`，新增 `projects.strategy_profile JSONB`。
- Modify: `backend/src/repositories/project_repository.rs`，新增 `AccountStrategyProfile`、创建/更新输入和 repository 方法。
- Modify: `backend/src/agents/models/request.rs`，扩展 project DTO、策略资料请求/响应和校验。
- Modify: `backend/src/lib.rs`，新增 `PUT /api/projects/:project_id/strategy-profile` 与 `POST /api/projects/:project_id/strategy-profile/draft`。
- Modify: `backend/src/agents/conversational_runtime.rs`，新增 `format_account_strategy_context` 并注入三个 topic prompt。
- Modify: `backend/tests/database_migrations.rs`、`backend/tests/project_repository_contract.rs`、`backend/tests/project_routes.rs`、`backend/tests/topic_agent_runtime.rs`，先补红灯测试再实现。
- Modify: `apps/video-agent/app/lib/api.ts`、`apps/video-agent/app/lib/api.test.ts`，扩展类型和 API wrapper。
- Create: `apps/video-agent/app/pages/content-strategy/AccountStrategyPage.tsx`，独立账号策略页面。
- Modify: `apps/video-agent/app/pages/content-strategy/topicModel.ts`，扩展 `ContentStrategyView` 与策略资料表单 helper。
- Modify: `apps/video-agent/app/pages/content-strategy/ContentStrategyPage.tsx`，保持当前选题池聚焦选题列表，不展示账号策略区块。
- Modify: `apps/video-agent/app/page.tsx`、`apps/video-agent/app/components/workspace/WorkspaceShell.tsx`，接入账号策略状态、二级菜单和“当前账号”文案。
- Modify: `apps/video-agent/app/page.test.tsx`、`apps/video-agent/e2e/workspace.spec.ts`，覆盖前端验收。
- Modify: `openspec/changes/account-strategy-profile/tasks.md`，完成一组同步勾选。

### Task 1: 后端策略资料存储与 DTO

**Files:**
- Create: `backend/migrations/20260708020000_account_strategy_profile.sql`
- Modify: `backend/tests/database_migrations.rs`
- Modify: `backend/src/repositories/project_repository.rs`
- Modify: `backend/tests/project_repository_contract.rs`
- Modify: `backend/src/agents/models/request.rs`
- Modify: `backend/tests/script_api_models.rs`

- [ ] **Step 1: Write failing migration/model tests**

Add assertions that migrated `projects` has `strategy_profile`, defaults to `{}`, and `ProjectResponse` serializes it.

Run: `docker exec ai-agent-api cargo test --test database_migrations --test project_repository_contract --test script_api_models`

Expected: fail because the column/type fields do not exist yet.

- [ ] **Step 2: Implement migration and model**

Add `AccountStrategyProfile` with normalized text/list fields, `Default`, `Serialize`, `Deserialize`, and project repository row mapping through `serde_json::Value`.

- [ ] **Step 3: Verify green**

Run: `docker exec ai-agent-api cargo test --test database_migrations --test project_repository_contract --test script_api_models`

Expected: pass.

### Task 2: 后端账号策略 API 与 AI 草稿

**Files:**
- Modify: `backend/tests/project_routes.rs`
- Modify: `backend/src/lib.rs`
- Modify: `backend/src/repositories/project_repository.rs`
- Modify: `backend/src/agents/models/request.rs`

- [ ] **Step 1: Write failing route tests**

Cover:
- `GET /api/projects` returns `strategy_profile`.
- `POST /api/projects` accepts optional `strategy_profile`.
- `PUT /api/projects/:project_id/strategy-profile` updates name, positioning, description, profile.
- invalid name/long fields/too many array items return `400` without partial update.
- missing project returns `404`.
- draft route returns `{draft, draft_summary}` and does not write `projects.strategy_profile`.
- invalid draft output returns error and does not write.

Run: `docker exec ai-agent-api cargo test --test project_routes`

Expected: fail because routes and validation are missing.

- [ ] **Step 2: Implement routes and validation**

Use existing `ScriptApiError` style. Normalize strings by trimming, de-duplicate list fields, and cap draft `max_output_tokens` at `1200`.

- [ ] **Step 3: Verify green**

Run: `docker exec ai-agent-api cargo test --test project_routes`

Expected: pass.

### Task 3: Agent Prompt 注入账号策略上下文

**Files:**
- Modify: `backend/tests/topic_agent_runtime.rs`
- Modify: `backend/src/agents/conversational_runtime.rs`

- [ ] **Step 1: Write failing prompt tests**

Insert a project with `strategy_profile` and assert generated LLM prompts include target audience, pillars, tone, forbidden topics, reference accounts and topic preferences for:
- topic generation
- quality gate
- topic group review

Run: `docker exec ai-agent-api cargo test --test topic_agent_runtime account_strategy -- --nocapture`

Expected: fail because prompts only contain positioning/description.

- [ ] **Step 2: Implement shared context**

Add `format_account_strategy_context(project: &Project) -> String` and pass it to `build_topic_generation_prompt`, `build_topic_quality_gate_prompt`, and `build_topic_group_review_prompt`.

- [ ] **Step 3: Verify green**

Run: `docker exec ai-agent-api cargo test --test topic_agent_runtime`

Expected: pass.

### Task 4: Frontend API 与账号策略页面

**Files:**
- Modify: `apps/video-agent/app/lib/api.ts`
- Modify: `apps/video-agent/app/lib/api.test.ts`
- Create: `apps/video-agent/app/pages/content-strategy/AccountStrategyPage.tsx`
- Modify: `apps/video-agent/app/pages/content-strategy/topicModel.ts`
- Modify: `apps/video-agent/app/pages/content-strategy/ContentStrategyPage.tsx`
- Modify: `apps/video-agent/app/components/workspace/WorkspaceShell.tsx`
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/page.test.tsx`
- Modify: `apps/video-agent/app/styles.css`

- [ ] **Step 1: Write failing frontend tests**

Cover:
- topbar label is `当前账号`.
- content strategy submenu has `账号策略 / 历史生成 / 当前选题池`.
- account strategy page renders profile/missing state/edit form.
- AI draft prefills form but does not update selected project before save.
- save success syncs project list and current account name.
- save failure keeps old profile.
- topic pool only shows summary entry, not full strategy form.

Run: `docker exec ai-agent-video-agent npm run test -- app/page.test.tsx app/lib/api.test.ts`

Expected: fail because API wrappers/page are missing.

- [ ] **Step 2: Implement frontend**

Add `AccountStrategyPage` as the page-level file. Keep `page.tsx` responsible for loading/saving state and route switching only.

- [ ] **Step 3: Verify green**

Run: `docker exec ai-agent-video-agent npm run test -- app/page.test.tsx app/lib/api.test.ts`

Expected: pass.

### Task 5: E2E, OpenSpec Tasks, Final Verification

**Files:**
- Modify: `apps/video-agent/e2e/workspace.spec.ts`
- Modify: `openspec/changes/account-strategy-profile/tasks.md`

- [ ] **Step 1: Write/update E2E**

Cover navigating to `内容策略 > 账号策略`, saving a profile, and seeing the strategy summary before generating topics.

- [ ] **Step 2: Run verification**

Run:
- `docker exec ai-agent-api cargo fmt -- --check`
- `docker exec ai-agent-api cargo test`
- `docker exec ai-agent-api cargo clippy --all-targets --all-features -- -D warnings`
- `docker exec ai-agent-video-agent npm run lint`
- `docker exec ai-agent-video-agent npm run test`
- `docker exec ai-agent-video-agent npm run build`
- `docker exec ai-agent-video-agent npm run test:e2e`
- `openspec instructions apply --change "account-strategy-profile" --json`
- `openspec validate --all`
- `git diff --check`
- `git status --short`

Expected: all verification commands exit `0`; OpenSpec progress matches checked tasks. If a command cannot run because a container is unavailable, report the exact blocker.

### Self-Review

- Spec coverage: storage, API, AI draft, prompt injection, independent frontend page, pool summary, validation, cost control and failure behavior are mapped to tasks.
- Placeholder scan: no `TBD` or unresolved implementation placeholder remains.
- Type consistency: backend uses `AccountStrategyProfile`; frontend uses `AccountStrategyProfile` / `AccountStrategyFormState`; API names match OpenSpec endpoints.
