# 单镜头素材重生费用防重 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 保证单镜头重生在快速连点、网络重试和跨页面并发下只产生一条可计费图片任务，同时允许终态后明确发起新一轮生成。

**Architecture:** 前端同步锁负责即时交互防抖，每次用户操作生成 UUID `Idempotency-Key`，未收到成功响应时保留原 key 重试。后端仓储在分镜级事务锁内依次按幂等键和在途任务复用，否则创建；永久请求映射表与数据库部分唯一索引共同保证并发及迟到重试幂等，Worker 继续用 `FOR UPDATE SKIP LOCKED` 保证单任务单消费者。

**Tech Stack:** React 18、TypeScript、Axum、Rust、SQLx、PostgreSQL、Vitest、Tokio。

---

### Task 1: 后端幂等与并发行为测试

**Files:**
- Modify: `backend/tests/asset_generation_routes.rs`

- [x] 新增缺少或非法 `Idempotency-Key` 返回 `400` 的路由测试。
- [x] 新增相同 key 连续请求只创建一条任务并返回同一 `task_id` 的测试。
- [x] 新增不同 key 并发请求同一分镜只创建一条在途任务的测试。
- [x] 新增任务完成后新 key 创建新任务、旧 key 仍返回旧任务的测试。
- [x] 在 API 容器运行定向测试，确认测试因当前接口未实现该协议而失败。

### Task 2: 数据库与仓储原子防重

**Files:**
- Create: `backend/migrations/20260710020000_scene_asset_generation_idempotency.sql`
- Modify: `backend/src/repositories/asset_generation_repository.rs`
- Modify: `backend/tests/database_migrations.rs`

- [x] 新增部分唯一索引和 `asset_generation_task_requests` 永久 key 映射表。
- [x] 为仓储增加分镜级事务锁下的 `create_or_reuse_scene_image_task`，按“相同 key、同分镜在途、创建”顺序返回任务及是否新建。
- [x] 增加 migration 结构断言和仓储并发测试。
- [x] 运行仓储与 migration 测试并确认通过。

### Task 3: API 幂等协议

**Files:**
- Modify: `backend/src/lib.rs`

- [x] 从 `Idempotency-Key` 请求头读取 UUID，缺失或非法时返回明确的 `400`。
- [x] 将数据库键规范化为 `scene-image:<scene_id>:<uuid>`，调用专用仓储方法。
- [x] 新建返回 `201 Created`，复用返回 `200 OK`。
- [x] 运行 Task 1 的全部定向测试并确认通过。

### Task 4: 前端同步锁与请求头

**Files:**
- Modify: `apps/video-agent/app/lib/api.ts`
- Modify: `apps/video-agent/app/lib/api.test.ts`
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/page.test.tsx`

- [x] 先新增 API wrapper 会发送 `Idempotency-Key` 的失败测试。
- [x] 先新增同步触发两次 handler 仅调用一次 API，以及响应丢失后复用原 key 的失败测试。
- [x] 扩展通用请求选项支持额外 headers，单镜头 wrapper 显式接收幂等键。
- [x] 使用 `useRef` 同步锁覆盖状态更新前窗口，并用非安全上下文可用的 `crypto.getRandomValues()` 生成 UUID；成功响应后才清除 key。
- [x] 运行前端定向测试和 TypeScript lint 并确认通过。

### Task 5: 全链路验证

**Files:**
- Modify: `openspec/changes/script-to-asset-generation/tasks.md`

- [x] 运行后端素材生成路由、仓储和 migration 测试。
- [x] 运行前端 API、页面测试和 lint。
- [x] 运行 Worker 素材生成测试，确认既有单任务消费行为未回归。
- [x] 运行 `openspec validate script-to-asset-generation --strict` 与 apply instructions，确认全部任务完成。
- [x] 运行 `git diff --check`，核对 diff 不包含空白错误。

本计划不执行 `git add`、`git commit` 或 `git push`；提交操作需用户另行明确确认。
