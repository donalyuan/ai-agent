# Novex Personal AI Workbench Architecture

## 1. 产品定位

Novex 是 local-first、单用户、可扩展多个领域应用的个人 AI 工作台基座。视频生产是首个领域应用，不是系统的唯一用途；后续编程、知识研究等能力分别进入 `apps/*`，共享通用 Agent Runtime、模型注册和本地会话能力。

当前不把多租户、客户交付模板、企业 RBAC、插件市场或远程托管作为默认目标。单用户不等于无安全边界：付费生成、正式发布、删除正式数据和外部写操作仍必须经过明确确认、幂等与成本限制。

## 2. 总体架构

```text
apps/* 个人领域工作台
  video-agent / future coding / research workbenches
                    |
services/agent-runtime
  Node.js 24 + Pi Agent Harness 0.82.0
  Turn / Tool Loop / SSE / steering / abort
  Pi SQLite Session Tree / fork / compaction
                    |
PostgreSQL ai_models
  文本、图片、视频模型配置唯一事实源
                    |
Rust Backend + crates/*
  领域状态 / Adapter / Run-Step / Gate / API
                    |
Python video-worker
  视频生成、语音、发布等外部任务
```

## 3. 代码边界

```text
backend/                  Rust HTTP API、领域编排、Repository、视频 Adapter
crates/
  novex-ai-core/          Run、Trace、Policy 通用类型
  novex-model/            Rust 模型路由与 provider 合同
  novex-agent/            现有业务 Adapter 与 Run 生命周期合同
  novex-rag/              检索与引用边界
  novex-tools/            类型化领域工具和安全策略边界
  novex-memory/           正式长期 Memory 策略边界
  novex-eval/             评测边界
services/
  agent-runtime/          Pi 通用 Turn、Tool Loop、Session Tree、SSE
  video-worker/           Python 视频任务 sidecar
admin/                    模型和系统控制面
apps/video-agent/         视频生产领域工作台
```

新增非视频工作台必须位于独立 `apps/*` 领域边界，不得复制视频的 `ProductionState`、角色或 Gate。跨语言调用使用 HTTP、SSE、queue job 或类型化 Tool API，不直接依赖其他服务的内部代码。

## 4. Pi Runtime 边界

`services/agent-runtime` 直接使用：

- `@earendil-works/pi-agent-core` `0.82.0`
- `@earendil-works/pi-ai` `0.82.0`
- `@earendil-works/pi-storage-sqlite-node` `0.82.0`
- Node.js 24 的 `node:sqlite`

Pi Harness 负责新工作台的模型流、Turn、Tool Call、Observation、steering、follow-up、abort 和事件流。Rust Agent Kernel 不再新增同职责的第二套通用 Tool Loop。

Runtime 按 Pi `0.82.0` 使用 `toolContext + AgentHarnessTool` 契约。虽然上游已导出 execution tool factory，Novex 仍保留自有 `read/write/edit/bash` schema 和适配器，避免改变既有 `old_text/new_text` edit 参数、Session transcript 与 SSE 行为；切换上游 factory 必须另行设计协议迁移。

现有视频 Conversation API、Rust `AgentRunCoordinator`、业务 Adapter、Run/Step 和失败收尾保持不变。后续迁移单个领域 Agent 时，必须用独立 OpenSpec change 先把业务能力暴露为类型化 Tool，再确定唯一执行入口和旧路径删除计划；禁止双模型调用、双 Assistant 消息和双写。

## 5. 模型配置

PostgreSQL `ai_models` 是所有模型部署的唯一配置来源。Pi Runtime 每轮按会话的 `model_id` 重新解析当前启用的 `text` 模型，只支持仓库已正式支持的：

- `openai_responses` -> Pi `openai-responses`
- `openai_chat_completions` -> Pi `openai-completions`

Runtime 不根据 URL 猜测协议，不回退环境变量、Pi 内建模型目录或默认模型。API Key 仅在请求进程内传给 provider，不得进入 HTTP/SSE、日志、SQLite metadata、entry 或错误信息。

每轮调用前追加模型非敏感快照，包括 model id、供应商、协议、请求根地址、上游模型、推理等级、输出上限和超时。模型配置后续修改不得覆盖历史快照。

## 6. Session、Context 与 Memory

Pi SQLite 只拥有 Agent Session：

- 会话 metadata 与活动 leaf
- 消息和工具结果
- 模型变更与非敏感快照
- 分支、fork、branch summary
- compaction 与近期上下文

PostgreSQL 继续拥有项目、脚本、素材、作品、发布记录、模型配置及其他领域事实。两类存储不做跨库双写事务。

`Context` 是本轮动态装配的数据；Pi compaction 是有损 Session Context。正式长期 `Memory` 只保存已确认、稳定、可复用的信息，必须有来源、作用域、更新时间、失效和删除语义。compaction summary 中的未确认推断不得自动升级为长期 Memory。

## 7. Tool 与领域安全

Runtime 只允许预定义 profile：

- `chat`：无本地文件或命令工具。
- `workspace`：启用 Pi `read/write/edit/bash`，执行目录固定为配置的 workspace root。

HTTP 请求不得注入任意 Tool 实现。视频生成、平台发布、删除正式领域数据等能力必须通过拥有规则的 Rust 领域 API 暴露为类型化 Tool，并继续执行确认、幂等、预算、资源上限、重试和失败恢复。通用 bash 不是绕过领域状态机的正式业务入口。

## 8. 视频领域

视频生产采用受控工作流：业务状态机、固定检查点、成本限制、权限和最终写入权由代码控制。长期方向是 `ProductionOrchestrator + RoleDefinition + ProductionState + PromptCompiler + Gate` 的虚拟制作团队；这是视频领域扩展，不是通用 Pi Runtime 的内建模型。

自主 Planner 只能在明确授权的调研、开放分析或失败诊断节点中使用，不得绕过预算、人工确认、质量 Gate 或业务状态规则。简单视频应保留 Fast Lane，不强制启动完整角色链。

## 9. 运行与部署

开发环境由 `/server/docker-compose.yml` 统一编排并 include 本项目 Compose：

| 服务 | 宿主机端口 | 容器端口 | 数据 |
|---|---:|---:|---|
| `ai-agent-api` | 18180 | 8080 | PostgreSQL / assets |
| `ai-agent-video-worker` | 18181 | 8081 | PostgreSQL / Redis / assets |
| `ai-agent-admin` | 18182 | 3000 | API |
| `ai-agent-video-agent` | 18183 | 3000 | API |
| `ai-agent-agent-runtime` | 18184 | 8082 | PostgreSQL read-only model config / SQLite volume |

Agent Runtime 的 `/health` 只报告进程存活；`/ready` 分别验证 PostgreSQL 和可写 SQLite。SQLite 固定保存在命名卷 `ai-agent-session-data`，容器重建不得退化为内存存储。

常规自动测试必须使用 fake provider，不调用真实模型、视频生成或平台发布。真实外部调用只允许在明确成本上限与用户确认后执行。

## 10. 架构变更规则

以下变化必须先建立或更新 OpenSpec change：

- 新增工作台、服务、核心表或跨语言协议
- 模型配置来源或 provider 映射变化
- Pi/Rust 执行职责迁移
- Session 与正式 Memory 所有权变化
- 新增本地工具或领域外部动作
- 付费、发布、删除等 Gate 变化

当前基准是“Pi 通用执行内核 + Rust 领域控制与视频链路”。允许能力逐步扩展，但不允许出现模型第二事实源、同请求双执行、凭据落盘或通用工具绕过领域 Gate。
