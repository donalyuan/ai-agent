# Novex 项目记忆

> 本文件是 Novex AI Agent Foundation 的项目级总索引，只记录跨主题稳定约束和定位。具体业务决策统一保存在对应主题文件中。

## 记忆索引

### 仓库约定

- `docs/memory/project-memory-structure.md`：项目记忆结构规则
- `docs/memory/frontend-design-skill-requirement.md`：前端设计约束
- `docs/memory/project-tech-stack.md`：技术栈与架构设计

### 视频工作台

- `docs/memory/video-agent-workspace-flow.md`：视频工作台菜单、业务流、Agent 分层、当前有效产品决策和开发阶段规划
- `docs/requirements/video-agent-mvp.md`：MVP 需求边界与验收标准
- `docs/requirements/video-agent-database-schema.md`：简化版数据库设计
- `docs/requirements/video-agent-full-spec.md`：完整需求文档
- `openspec/specs/`：已归档 change 合并后的当前正式规格
- `openspec/changes/`：当前进行中的 change、设计和任务状态；具体进度以 `openspec list --json` 与对应 `tasks.md` 为准

## 全局稳定约束

### 项目定位与架构

- 当前仓库是 **Novex AI Agent Foundation monorepo**；`apps/video-agent/` 是首个业务应用，`admin/` 是 Novex 平台管理后台。
- `apps/video-agent/` 承载视频内容生产流程；`admin/` 承载用户、权限、模型、工具、MCP、任务、日志、运行状态、成本、限额和健康检查等控制面能力。
- 后端采用 Rust + Axum + SQLx + PostgreSQL；向量库采用 Milvus Standalone；任务基础设施采用 Redis；视频生成与平台发布 Worker 采用 Python；前端采用 Next.js + TypeScript + shadcn/ui。
- `backend/` 承担控制面 API 和业务编排入口，可复用 AI 能力放入 `crates/*`，Python sidecar/runtime 放入 `services/*`，业务应用放入 `apps/*`。
- 系统长期边界以 `ARCHITECTURE.md` 和仓库当前代码为准，不得用历史记忆覆盖当前仓库事实。

### 产品与开发治理

- 视频工作台对外品牌名为 `VEDIO-AGENT`，展示名为“视频工作台”；不得使用 `Novex Admin` 作为工作台展示品牌。
- 视频工作台一级导航按“内容策略 -> 脚本创作 -> 素材管理 -> 作品生产 -> 发布运营 -> 数据分析 -> 工作流任务”组织，并以数据库持久化菜单配置为唯一来源；详细规则见 `docs/memory/video-agent-workspace-flow.md`。
- 视频工作台 Pencil 原型源文件统一为 `docs/prototypes/video-agent/video-agent.pen`；原型修改必须通过 Pencil MCP 写入并验证，不得直接手改 JSON 后视为已更新。
- 新增功能、行为修改、协议改造或测试规则变化必须遵循 `CLAUDE.md` 的 OpenSpec 工作流。
- OpenSpec change 达到 `all_done` 后只报告可归档；未经用户明确命令，不得自动归档。
- Video Agent 当前只覆盖桌面端运营工作台；移动端原型、适配和验收必须另行提出 OpenSpec change。
- 本项目禁止调用 GitNexus，包括其 skill、MCP 工具和 CLI。

### 开发环境

- 环境从 `/server/docker-compose.yml` 统一编排，并 include `/server/ai-agent/docker-compose.yml`。
- 复用 PostgreSQL 服务 `biga-postgres` 的独立数据库 `video_agent`，复用 Redis 服务 `bs-redis` 的 DB index `/2`。
- 当前映射端口：API `18180->8080`、Video Worker `18181->8081`、Admin `18182->3000`、Video Agent `18183->3000`。
- Compose 服务名：`ai-agent-api`、`ai-agent-video-worker`、`ai-agent-admin`、`ai-agent-video-agent`；容器内项目路径统一为 `/app`。
- `apps/video-agent` 未显式配置 `NEXT_PUBLIC_API_BASE_URL` 时，根据当前页面 `hostname` 派生 `<protocol>//<hostname>:18180`。

## 记忆维护规则

1. 新会话开始前、上下文压缩恢复后，先读取本文件；涉及具体主题时再读取对应主题记忆、需求或 OpenSpec。
2. 新增稳定规则时优先更新对应主题文件；只有跨主题约束、全局规则或索引变化才同步更新本文件。
3. 只记录已确认且后续仍可能复用的信息；禁止记录临时探索、一次性报错、未确认猜测、短期进度快照和敏感信息。
4. 主题文件保存当前有效决策；被新决策覆盖的旧口径应删除或明确标为历史，不得与当前规则并列。
5. 实现状态和任务数字以仓库代码、`openspec list --json` 和对应 `tasks.md` 为准，不在本文件复制易过期快照。
