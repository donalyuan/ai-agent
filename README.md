# Novex AI Agent Foundation

> 可复用 AI Agent 基座。`apps/video-agent` 是当前第一个业务应用，用于视频内容生产链路。

## 项目定位

Novex 面向可复用 AI Agent 基座建设，沉淀模型调用、RAG、Agent Runtime、Tools、Memory、Eval、Worker 编排和业务应用模板等能力。`apps/video-agent` 是首个业务应用，覆盖视频内容生产链路。

## 目录结构

```text
backend/                 Rust 控制面 API
admin/                   Next.js 管理后台
apps/
  video-agent/           视频内容生产业务应用
crates/
  novex-ai-core/         Run Graph、Trace、Policy 等通用 AI 领域边界
  novex-model/           模型注册、路由、能力描述和用量边界
  novex-agent/           Agent runtime、planner、tool loop 边界
  novex-rag/             chunk、embedding、retrieval、rerank、citation 边界
  novex-tools/           tool registry、executor、permission、audit 边界
  novex-memory/          session/user/org/project memory 边界
  novex-eval/            eval runner、指标和报告边界
services/
  video-worker/          Python 视频生成和平台发布 sidecar
templates/               客户交付模板
infra/                   部署与环境配置
docs/                    架构、需求、项目记忆、实施计划和交付文档
```

## 快速开始

本项目开发环境必须从顶层 Compose 入口启动，并复用 `/server/docker-compose.yml` 中已经运行的 `biga-postgres` 与 `bs-redis`。

```bash
docker compose -f /server/docker-compose.yml up -d --build ai-agent-api ai-agent-video-worker ai-agent-admin
```

### 访问地址

- API health: `http://127.0.0.1:18180/health`
- API ready: `http://127.0.0.1:18180/ready`
- Video worker health: `http://127.0.0.1:18181/health`
- 智能体工作台: `http://127.0.0.1:18182`

`admin/` 当前首屏为 `AI-AGENT / 智能体工作台`，左侧预留六个智能体入口，当前实现脚本智能体的项目、脚本生成、时间轴对照详情和状态更新闭环。

### 常用验证

```bash
docker compose -f /server/docker-compose.yml config --services
docker exec ai-agent-api cargo test -p novex-api
docker exec ai-agent-admin npm run test
docker exec ai-agent-admin npm run lint
docker exec ai-agent-admin npm run build
openspec validate script-agent-workspace --json
```

## 架构原则

1. `backend/` 承担控制面 API 和业务编排入口。
2. 可复用 AI 能力沉淀到 `crates/*`，避免堆进 `backend/src`。
3. Python 只作为 `services/*` sidecar/runtime，不进入核心控制面。
4. 业务应用放入 `apps/*`；video-agent 是第一个业务应用。
5. 后续功能新增、行为修改、协议改造和测试规则变化必须走 OpenSpec。

## 文档导航

- [`ARCHITECTURE.md`](./ARCHITECTURE.md) - Novex 长期架构基准
- [`MEMORY.md`](./MEMORY.md) - 项目记忆统一入口
- [`CLAUDE.md`](./CLAUDE.md) - Claude Code 工作规范
- [`docs/README.md`](./docs/README.md) - 文档入口
- [`docs/memory/README.md`](./docs/memory/README.md) - 项目记忆主题索引
- [`docs/requirements/README.md`](./docs/requirements/README.md) - 需求文档索引
- [`apps/video-agent/README.md`](./apps/video-agent/README.md) - video-agent 应用说明
