# Novex Personal AI Workbench

> 本地单用户、多领域的个人 AI 工作台基座。`apps/video-agent` 是首个领域应用。

## 项目定位

Novex 面向个人本地使用，统一沉淀模型调用、Agent Runtime、Tools、Memory、Eval 和 Worker 编排能力。视频生产是首个领域应用；后续编程、知识研究等工作台放入独立 `apps/*`，共享同一通用 Runtime，不复制视频业务状态机。

## 目录结构

```text
agent-definitions/        版本化 Agent/Prompt Registry、schema、模板和发布索引
backend/                 Rust 控制面 API
admin/                   Next.js 管理后台
apps/
  video-agent/           视频内容生产业务应用
crates/
  novex-ai-core/         Definition、Prompt 编译、审计与通用 AI 领域边界
  novex-model/           模型注册、路由、能力描述和用量边界
  novex-agent/           受审计模型执行与 Agent Run 生命周期边界
  novex-rag/             chunk、embedding、retrieval、rerank、citation 边界
  novex-tools/           tool registry、executor、permission、audit 边界
  novex-memory/          session/user/org/project memory 边界
  novex-eval/            版本门禁、eval runner、指标和报告边界
services/
  agent-runtime/         Node.js 24 + Pi 的通用 Turn/Tool/Session Runtime
  video-worker/          Python 视频生成和平台发布 sidecar
infra/                   部署与环境配置
docs/                    架构、需求、项目记忆和实施计划
```

## 快速开始

本项目开发环境必须从顶层 Compose 入口启动，并复用 `/server/docker-compose.yml` 中已经运行的 `biga-postgres` 与 `bs-redis`。

```bash
docker compose -f /server/docker-compose.yml up -d --build ai-agent-api ai-agent-agent-runtime ai-agent-video-worker ai-agent-admin ai-agent-video-agent
```

### 访问地址

- API health: `http://127.0.0.1:18180/health`
- API ready: `http://127.0.0.1:18180/ready`
- Video worker health: `http://127.0.0.1:18181/health`
- 管理后台: `http://127.0.0.1:18182`
- 视频生产工作台: `http://127.0.0.1:18183`
- Pi Agent Runtime health: `http://127.0.0.1:18184/health`
- Pi Agent Runtime ready: `http://127.0.0.1:18184/ready`

`admin/` 当前首屏为 Novex 平台管理后台入口，承载用户、权限、模型、工具、任务、日志、成本、限额和环境健康等控制面能力。`apps/video-agent/` 承载 `VEDIO-AGENT / 视频工作台`，左侧预留六个智能体入口，当前实现脚本智能体的项目选择、脚本生成、时间轴对照详情和状态更新闭环。

### 常用验证

```bash
docker compose -f /server/docker-compose.yml config --services
docker exec ai-agent-api cargo test -p novex-api
docker exec ai-agent-admin npm run test
docker exec ai-agent-admin npm run lint
docker exec ai-agent-admin npm run build
docker exec ai-agent-video-agent npm run test
docker exec ai-agent-video-agent npm run lint
docker exec ai-agent-video-agent npm run build
docker build --target test -t novex-agent-runtime-test services/agent-runtime
docker run --rm novex-agent-runtime-test npm run lint
docker run --rm novex-agent-runtime-test npm test
openspec validate realign-video-agent-workspace-boundary --json
```

Agent Runtime 使用 Pi `0.82.0`、代码级 `agent-definitions/` 与 PostgreSQL `ai_models`，并将 Session Tree、固定 Definition/模型 binding 和 namespaced `ModelCall` 审计持久化到 `ai-agent-session-data` 卷中的 `/data/agent-sessions.sqlite`。Runtime 已采用 `toolContext + AgentHarnessTool` 契约，同时保留 Novex 自有工具 schema。仅运行 `npm run test` 会使用 fake provider，不调用真实模型、不触发视频生成或发布费用；Runtime API 和请求示例见 [`services/agent-runtime/README.md`](./services/agent-runtime/README.md)。

## 架构原则

1. Pi Runtime 承担新工作台的通用 Turn、Tool Loop、SSE 和 Session Tree。
2. Rust backend 继续拥有视频领域 Adapter、Run/Step、领域状态和高风险 Gate。
3. `agent-definitions/` 是 Agent/Prompt 唯一事实源；数据库不得保存模板正文或覆盖代码定义。
4. PostgreSQL `ai_models` 是模型配置唯一来源；Rust/Pi 每次模型调用都经固定 binding 和独立 `ModelCall` 审计。
5. 业务应用放入 `apps/*`；video-agent 是第一个领域应用。
6. 后续功能新增、行为修改、协议改造和测试规则变化必须走 OpenSpec。

## 文档导航

- [`ARCHITECTURE.md`](./ARCHITECTURE.md) - Novex 长期架构基准
- [`MEMORY.md`](./MEMORY.md) - 项目记忆统一入口
- [`CLAUDE.md`](./CLAUDE.md) - Claude Code 工作规范
- [`docs/README.md`](./docs/README.md) - 文档入口
- [`docs/memory/README.md`](./docs/memory/README.md) - 项目记忆主题索引
- [`docs/requirements/README.md`](./docs/requirements/README.md) - 需求文档索引
- [`apps/video-agent/README.md`](./apps/video-agent/README.md) - video-agent 应用说明
