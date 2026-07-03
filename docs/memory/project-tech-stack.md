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
- **LLM 接入**：通用 OpenAI-compatible provider 客户端归属 `crates/novex-model`，暴露 `LLMClient`、`LLMPrompt`、`LLMError`、`OpenAIConfig` 和 `OpenAIClient`；`backend/` 只保留脚本 Agent 的业务 Prompt 构造、LLM 输出解析和脚本业务校验。客户端同时支持 Chat Completions 与 Responses API；当 `OPENAI_BASE_URL` 以 `/responses` 结尾时直接走 Responses endpoint，并使用 JSON object 输出约束。Responses 分支使用 SSE 流式响应以避开上游同步请求 30 秒窗口，并发送 Codex-compatible `User-Agent`。Responses 分支支持 `OPENAI_REASONING_EFFORT` 和 `OPENAI_MAX_OUTPUT_TOKENS` 配置，`OPENAI_REASONING_EFFORT=none` 时不发送 `reasoning` 字段。

### Rust 基座 crates
- `crates/novex-ai-core`：Run Graph、Trace、Policy、通用 AI 领域模型
- `crates/novex-model`：模型注册、能力描述、路由、用量、健康检查和 OpenAI-compatible LLM provider 客户端
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

## 当前开发环境约定（2026-07-02）

- 顶层入口：`/server/docker-compose.yml`
- 项目 Compose：`/server/ai-agent/docker-compose.yml`
- PostgreSQL：复用 `biga-postgres`，数据库名 `video_agent`
- Redis：复用 `bs-redis`，DB index `/2`
- API：`ai-agent-api`，宿主机端口 `18180`，容器端口 `8080`
- Video Worker：`ai-agent-video-worker`，宿主机端口 `18181`，容器端口 `8081`
- Admin：`ai-agent-admin`，宿主机端口 `18182`，容器端口 `3000`
- 容器内项目路径：`/app`
- 当前已验证脚本生成链路可通过 Responses API 使用 `gpt-5.4-mini` 和 `gpt-5.5` 完成端到端生成；`gpt-5.5` 需要 Responses SSE 流式路径和 Codex-compatible `User-Agent`，同步 Responses 请求在完整脚本生成场景下会触发上游约 30 秒 502。

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
- `script-agent-mvp`：已完成并归档，脚本生成、读取、列表、状态更新 API 已实现
- 当前优先级：建设脚本 Agent 前端工作台，打通“生成脚本 -> 查看分镜 -> 更新状态”的可用闭环；素材匹配和视频生成编排作为后续 OpenSpec change 推进
- 前端工作台的对外可见产品品牌名为 `AI-AGENT`，展示名为“智能体工作台”；原型、UI 和当前工作台设计文档不得使用 `Novex Admin` 作为展示品牌
- 智能体工作台不是单一脚本 Agent 页面，桌面端壳层必须预留六个智能体菜单入口：选题智能体、脚本智能体、素材智能体、视频智能体、发布智能体、优化智能体；当前 `script-agent-workspace` 只实现脚本智能体模块闭环
- 脚本智能体详情展示已选定“时间轴对照视图”：左侧表达分镜顺序和节奏节点，右侧并排展示旁白与画面指令；后续实现不要回退成纯卡片流或纯表格
- 脚本 Agent 前端工作台当前仅覆盖桌面端运营后台，不涉及移动端原型、移动端适配或移动端验收；后续如需要移动端，应单独提出 OpenSpec change

**Why**: 架构设计和技术选型决策，后续开发参考。

**How to apply**:
- 新能力先判断归属：`backend`、`apps/*`、`crates/*`、`services/*`
- 不再把可复用 AI 基建能力直接堆进 `backend/src`
- Python 只做 sidecar/runtime，核心控制面仍在 Rust
