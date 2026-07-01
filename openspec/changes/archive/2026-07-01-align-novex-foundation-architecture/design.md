## Context

仓库当前以 video-agent MVP 为根级应用：Rust API 位于 `backend/`，Next.js 页面位于 `frontend/`，Python worker 位于 `python-worker/`。这套结构适合快速跑通单一视频业务，但与 `ARCHITECTURE.md` 中 Novex 长期 AI Agent 基座不一致。

用户已明确要求先完全回到 Novex 基座路线：暂停继续扩展 video-agent 业务，先补 `backend/admin/apps/crates/services` 等长期基础框架。同时，当前 `script-agent-mvp` 已完成并验证了数据库迁移、脚本领域模型、请求/响应模型、Repository trait 和 PostgreSQL Repository，不能丢弃，应作为 `apps/video-agent` 的初始业务能力保留并迁移。

## Goals / Non-Goals

**Goals:**

- 将仓库根结构调整为 Novex foundation monorepo。
- 让 video-agent 成为 `apps/video-agent` 下的第一个业务能力，而不是根级仓库身份。
- 建立 `crates/` 中长期可复用 Rust 基座边界，避免后续 Agent/RAG/Model/Tools/Memory/Eval 能力继续堆进 `backend/src`。
- 将 Python worker 归入 `services/`，为 parser/model/video/sandbox runtime 留出统一边界。
- 将当前 `frontend/` 迁移为 `admin/` 初始管理后台。
- 保留 `script-agent-mvp` 当前已验证代码，并迁移到新结构中可继续开发的位置。
- 更新 Compose、README、memory、OpenSpec，使后续开发默认遵循新结构。

**Non-Goals:**

- 不在本 change 内实现完整 RBAC、租户、权限系统。
- 不在本 change 内完成全部 Novex crates 的业务实现；允许先建立可编译空 crate 或最小模块边界。
- 不在本 change 内继续开发脚本 Agent 的 LLM、Service、HTTP 路由。
- 不引入 Kubernetes、Dify/FastGPT 全家桶或新的生产部署体系。

## Decisions

### Decision 1: 采用 Novex monorepo 结构，不继续扩展 video-agent 根级结构

目标目录：

```text
backend/                 Rust 控制面 API
admin/                   Next.js 管理后台
apps/
  video-agent/           视频内容生产应用/能力包
crates/
  novex-ai-core/
  novex-model/
  novex-agent/
  novex-rag/
  novex-tools/
  novex-memory/
  novex-eval/
services/
  video-worker/          原 python-worker 迁移目标
templates/
infra/
docs/
```

替代方案是只创建目录不迁移代码，但这会继续让现有业务散落在旧路径。选择直接迁移，是因为当前业务仍早期，迁移成本低，且能立即让后续开发以新边界为准。

### Decision 2: 保留 `backend/` 为控制面 API，先不把它改名

`ARCHITECTURE.md` 明确 `backend/` 承担 HTTP API、控制面和业务编排。当前 Rust API 已在 `backend/`，因此不移动该顶层目录。后续会从 `backend/src` 中逐步抽出可复用 AI 能力到 `crates/*`。

替代方案是把当前 `backend/` 迁入 `apps/video-agent/backend`，再新建空 `backend/`。这会在短期造成 Compose、测试、迁移路径和 OpenSpec 改动过大。当前选择是保留控制面入口，同时让 video-agent 业务代码逐步迁到 `apps/video-agent` 或 `crates`。

### Decision 3: video-agent 作为应用包保留，已有业务能力迁入新边界

`apps/video-agent/` 承载视频业务的应用上下文、前台页面、业务说明、OpenSpec 业务追踪。已实现的脚本 Agent 数据库和仓储能力先保留在 Rust 侧，并在本 change 中更新路径引用；后续恢复 `script-agent-mvp` 时，再决定哪些代码进一步抽到 `crates/novex-agent` 或 `crates/novex-ai-core`。

### Decision 4: `services/video-worker` 承接现有 Python worker

Python 只作为 sidecar/runtime，不进入核心控制面。现有 worker 是视频生成和平台发布的雏形，因此迁移到 `services/video-worker`，后续 parser/model/sandbox 等服务也放入 `services/`。

### Decision 5: 先建立 Rust workspace crate 边界，再逐步填充

本 change 至少建立 `crates/*` 的 Cargo workspace 成员和基础包元数据。它们可以先是最小可编译 crate，避免一次性把未成熟业务逻辑强行拆散。后续新能力按 `ARCHITECTURE.md` 模块归属规则进入对应 crate。

## Risks / Trade-offs

- [Risk] 当前 `script-agent-mvp` 与本架构 change 同时 active，路径迁移可能让旧 tasks 描述失效。  
  → Mitigation: 本 change 更新 `script-agent-mvp` 的相关路径说明或明确其恢复开发前需要 rebasing 到新结构。

- [Risk] 大量移动文件可能破坏 Docker Compose build context。  
  → Mitigation: 先修改 compose 并执行 `docker compose -f /server/docker-compose.yml config --services`、后端测试、worker 测试和 admin 构建/检查。

- [Risk] 过早拆分 crates 可能增加样板复杂度。  
  → Mitigation: 本 change 只建立边界和最小可编译 crate，不强行实现全部抽象。

- [Risk] 现有 README/memory 与新架构冲突。  
  → Mitigation: 同步更新项目记忆和文档，将 Novex 基座路线设为新的稳定约束。

## Migration Plan

1. 暂停 `script-agent-mvp` 业务任务，不继续实现 T2.3 以后内容。
2. 新建 Novex 基座目录骨架。
3. 迁移 `frontend/` 到 `admin/`，更新 compose 和文档中的路径。
4. 迁移 `python-worker/` 到 `services/video-worker/`，更新 compose 和验证命令。
5. 新建 `apps/video-agent/`，放置 video-agent 应用说明、业务 OpenSpec 衔接文档和后续前台承载位置。
6. 新建 `crates/*` workspace 成员，保证 Rust workspace 可构建。
7. 更新 `backend/Cargo.toml` 为 workspace 成员，并确认现有脚本 Agent 初始能力仍可测试。
8. 更新 memory、README、OpenSpec active change 说明。
9. 执行验证：OpenSpec validate、后端 `cargo test`、worker `pytest`、admin lint/build、Compose 服务列表和必要的健康检查。

Rollback 策略：本 change 不执行数据库破坏性迁移；如目录迁移失败，可通过 Git diff 回滚文件移动和 compose 路径修改，运行库 `video_agent` 保持原状态。
