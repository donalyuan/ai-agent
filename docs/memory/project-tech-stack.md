---
name: project-tech-stack
description: Novex AI Agent 基座技术选型与架构设计
metadata:
  node_type: memory
  type: project
  originSessionId: 322147b7-81d9-49fa-b62d-971d5fa7a0f8
---

# Novex 技术栈与架构

## 项目定位

Novex 是可复用 AI Agent Foundation。`apps/video-agent` 是第一个业务应用，用于视频内容生产闭环。后续培训、知识库、客服、研发助手等应用必须复用同一套基座能力，而不是复制一套根级项目。

## 核心技术栈

### 控制面
- **Rust + Axum**：`backend/` 控制面 API 和业务编排入口
- **SQLx + PostgreSQL**：类型安全数据库访问
- **Redis**：任务队列与缓存

### Rust 基座 crates
- `crates/novex-ai-core`：Run Graph、Trace、Policy、通用 AI 领域模型
- `crates/novex-model`：模型注册、能力描述、路由、用量和健康检查
- `crates/novex-agent`：Agent runtime、planner、tool loop
- `crates/novex-rag`：chunk、embedding、retrieval、rerank、citation
- `crates/novex-tools`：tool registry、executor、permission、audit
- `crates/novex-memory`：session/user/org/project memory
- `crates/novex-eval`：eval runner、指标和报告

### 服务运行时
- `services/video-worker`：Python/FastAPI 视频生成和平台发布 sidecar
- 后续 parser/model/sandbox 等 runtime 统一放入 `services/*`

### 前端与应用
- `admin/`：Next.js + TypeScript 管理后台
- `apps/video-agent/`：视频内容生产业务应用

## 当前开发环境约定（2026-07-01）

- 顶层入口：`/server/docker-compose.yml`
- 项目 Compose：`/server/video-agent/docker-compose.yml`
- PostgreSQL：复用 `biga-postgres`，数据库名 `video_agent`
- Redis：复用 `bs-redis`，DB index `/2`
- API：`novex-api`，宿主机端口 `18180`，容器端口 `8080`
- Video Worker：`novex-video-worker`，宿主机端口 `18181`，容器端口 `8081`
- Admin：`novex-admin`，宿主机端口 `18182`，容器端口 `3000`
- 容器内项目路径：`/app`

## video-agent 业务边界

video-agent 业务仍保留以下 MVP 范围，详见 [`docs/requirements/video-agent-mvp.md`](../requirements/video-agent-mvp.md)：

- 选题 Agent
- 脚本 Agent
- 素材 Agent
- 视频 Agent
- 发布 Agent
- 优化 Agent

## 当前 OpenSpec 状态

- `align-novex-foundation-architecture`：已于 2026-07-01 归档，主 specs 已同步
- `script-agent-mvp`：保留已验证成果，可在 Novex 基座结构下从 T2.3 恢复

**Why**: 架构设计和技术选型决策，后续开发参考。

**How to apply**:
- 新能力先判断归属：`backend`、`apps/*`、`crates/*`、`services/*`
- 不再把可复用 AI 基建能力直接堆进 `backend/src`
- Python 只做 sidecar/runtime，核心控制面仍在 Rust
