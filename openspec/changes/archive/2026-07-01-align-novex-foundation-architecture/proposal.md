## Why

当前仓库按 video-agent MVP 简化结构初始化，`backend/frontend/python-worker` 直接承载单一视频业务。这个结构与 `ARCHITECTURE.md` 的 Novex 长期 AI 基座设想不一致，会让后续培训、知识库、客服、研发助手等能力难以复用统一的模型、Agent、工具、记忆、评测和交付边界。

现在已经进入脚本 Agent 业务开发早期，及时回到 Novex 基座路线，可以保留已验证的 video-agent 初始能力，同时避免继续把业务代码写进不可扩展的根级应用结构。

## What Changes

- **BREAKING**: 仓库目录从 video-agent MVP 结构调整为 Novex AI foundation monorepo 结构。
- 新增长期基座目录：`crates/`、`services/`、`admin/`、`apps/`、`templates/`、`infra/`、`docs/`。
- `backend/` 保留为控制面 API，不再作为所有 AI 业务逻辑的唯一承载点。
- 现有 `frontend/` 迁移为 `admin/` 的初始管理后台骨架。
- 现有 `python-worker/` 迁移到 `services/video-worker/`，后续 worker/runtime 服务统一放入 `services/`。
- 新建 `apps/video-agent/`，将 video-agent 作为 Novex 的第一个业务应用/能力包承载。
- 新建 Rust workspace crates 边界，至少包含 `novex-ai-core`、`novex-model`、`novex-agent`、`novex-rag`、`novex-tools`、`novex-memory`、`novex-eval`。
- 保留并迁移当前 `script-agent-mvp` 已完成的初始数据库、脚本领域模型、请求/响应模型、Repository trait 和 PostgreSQL Repository，不丢弃已验证工作。
- 更新 Compose、README、memory 和 OpenSpec 说明，使后续开发默认以 Novex 基座结构为准。

## Capabilities

### New Capabilities

- `novex-foundation-architecture`: 定义 Novex 基座 monorepo 目录边界、Rust workspace crate 边界、业务应用放置规则、服务运行时放置规则和 video-agent 迁移规则。

### Modified Capabilities

- `environment-bootstrap`: 开发环境入口仍从顶层 `/server/docker-compose.yml` 启动，但项目内部 compose、构建上下文和服务路径需要迁移到 Novex 基座目录结构。

## Impact

- 影响目录：`backend/`、`frontend/`、`python-worker/`、`docker-compose.yml`、`README.md`、`MEMORY.md`、`docs/memory/*.md`、`docs/requirements/*.md`、`openspec/changes/script-agent-mvp/`。
- 影响构建：Rust workspace、Next.js admin 构建、Python worker 测试、Docker Compose build context。
- 影响测试：后端 `cargo test`、worker `pytest`、admin `npm run lint/build`、OpenSpec validate、实际数据库迁移验证。
- 影响后续开发顺序：暂停继续扩展 `script-agent-mvp` 业务任务，先完成本 change 的架构迁移，再恢复 video-agent 业务开发。
