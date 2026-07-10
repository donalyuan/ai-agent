# 失败素材任务清理与镜头内容展示实施计划

> **For agentic workers:** Inline execution with strict TDD. Do not start the image Worker or call any image provider.

**Goal:** 按 Pencil 原型实现失败任务可审计清理，并把当前镜头的旁白和画面展示在候选素材上方。

**Architecture:** PostgreSQL 的 `asset_generation_tasks.dismissed_at` 作为页面隐藏事实来源，Rust 仓储负责原子状态校验和默认过滤，Axum 暴露幂等清理 API。React 页面只在用户确认后调用清理接口，成功后重新读取任务与候选；镜头上下文直接读取当前 `ScriptDetail.scenes`，不扩展后端协议。

**Tech Stack:** PostgreSQL、SQLx、Rust、Axum、React 18、Next.js 14、TypeScript、Vitest、Testing Library、Playwright。

---

### Task 1: 数据库与仓储清理语义

**Files:**
- Create: `backend/migrations/20260710030000_asset_generation_task_dismissal.sql`
- Modify: `backend/src/repositories/asset_generation_repository.rs`
- Modify: `backend/tests/database_migrations.rs`
- Modify: `backend/tests/asset_generation_repository_contract.rs`

- [ ] 新增失败测试，断言 migration 增加可空 `dismissed_at` 及默认查询索引。
- [ ] 新增仓储失败测试：`failed` 首次清理成功、重复清理保留原时间、非失败状态拒绝。
- [ ] 新增查询失败测试：任务列表排除已清理任务，候选列表只排除其关联 `failed` 候选并保留非失败候选。
- [ ] 运行定向测试，确认因 schema 和仓储方法缺失而失败。
- [ ] 实现 migration、模型字段、行映射、原子清理和默认过滤。
- [ ] 运行定向测试并确认通过。

### Task 2: 清理 API

**Files:**
- Modify: `backend/src/agents/models/request.rs`
- Modify: `backend/src/lib.rs`
- Modify: `backend/tests/asset_generation_routes.rs`

- [ ] 新增路由失败测试：失败任务返回 `200` 和稳定 `dismissed_at`，重复调用幂等。
- [ ] 新增非失败任务 `409 Conflict` 与不存在任务 `404 Not Found` 测试。
- [ ] 运行路由测试，确认因路由缺失而失败。
- [ ] 新增 `POST /api/asset-generation-tasks/:task_id/dismiss`，映射仓储错误且不触碰 Worker。
- [ ] 在任务响应中暴露 `dismissed_at`，运行路由测试并确认通过。

### Task 3: 前端 API 与清理交互

**Files:**
- Modify: `apps/video-agent/app/lib/api.ts`
- Modify: `apps/video-agent/app/lib/api.test.ts`
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/page.test.tsx`
- Modify: `apps/video-agent/app/pages/script-creation/AssetCandidatePanel.tsx`

- [ ] 新增 API wrapper 失败测试，断言清理路径、`POST` 方法和响应字段。
- [ ] 新增页面失败测试：仅失败任务显示“清理失败任务”，点击后出现原型确认文案。
- [ ] 新增确认提交失败测试：同一弹窗重复点击只提交一次；成功后刷新任务和候选，失败时卡片保留并显示错误。
- [ ] 运行前端定向测试，确认因接口和交互缺失而失败。
- [ ] 实现 API wrapper、清理目标状态、确认弹窗、同步请求锁和成功刷新。
- [ ] 运行前端定向测试并确认通过。

### Task 4: 镜头上下文与原型布局

**Files:**
- Modify: `apps/video-agent/app/pages/script-creation/AssetCandidatePanel.tsx`
- Modify: `apps/video-agent/app/page.test.tsx`
- Modify: `apps/video-agent/app/styles.css`
- Modify: `apps/video-agent/e2e/workspace.spec.ts`

- [ ] 新增页面失败测试，断言候选区顶部完整展示“旁白”和“画面”，空字段显示明确空值。
- [ ] 新增结构断言，确保旁白和画面不出现在左侧分镜列表。
- [ ] 运行定向测试，确认当前页面缺少镜头上下文而失败。
- [ ] 实现稳定双栏镜头内容区，并按 Pencil 调整三栏宽度、任务卡片和弹窗样式。
- [ ] 补充 E2E 关键可见性断言并运行定向测试。

### Task 5: 全量验证与运行态检查

**Files:**
- Modify: `openspec/changes/script-to-asset-generation/tasks.md`

- [ ] 运行后端 migration、仓储、路由和后端全量测试。
- [ ] 运行前端 API、页面、全量测试、lint 和 build。
- [ ] 运行 Playwright 截图，比较桌面页面与 Pencil 原型并检查无重叠。
- [ ] 运行 `openspec validate script-to-asset-generation --strict` 和 apply instructions。
- [ ] 运行 `git diff --check`，确认 Worker 保持关闭且无供应商请求。
- [ ] 回写 `13.4` 至 `13.7` 完成状态，启动前端开发服务器供用户验收。

本计划不执行 `git add`、`git commit`、`git push`，不启动可计费 Worker，不调用图片供应商。
