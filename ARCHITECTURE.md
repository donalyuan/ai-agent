# Novex Personal AI Workbench Architecture

## 1. 产品定位

Novex 是 local-first、单用户、可扩展多个领域应用的个人 AI 工作台基座。视频生产是首个领域应用，不是系统的唯一用途；后续编程、知识研究等能力分别进入 `apps/*`，共享通用 Agent Runtime、模型注册和本地会话能力。

当前不把多租户、客户交付模板、企业 RBAC、插件市场或远程托管作为默认目标。单用户不等于无安全边界：付费生成、正式发布、删除正式数据和外部写操作仍必须经过明确确认、幂等与成本限制。

## 2. 总体架构

```text
apps/* 个人领域工作台
  video-agent / future coding / research workbenches
                    |
agent-definitions/
  版本化 AgentDefinition / PromptDefinition / schema / release index
  Rust 与 Pi 只读加载，数据库不得反向覆盖
                    |
services/agent-runtime
  Node.js 24 + Pi Agent Harness 0.82.0
  Turn / Tool Loop / SSE / Session Tree
  PromptCompiler / SQLite ModelCall 审计
                    |
PostgreSQL ai_models
  文本、图片、视频模型配置唯一事实源
                    |
Rust Backend + crates/*
  领域状态 / Adapter / Run-Step / Gate / PromptCompiler
  PostgreSQL ModelCall / EvalRun / EvalReport
                    |
Python video-worker
  视频生成、语音、发布等外部任务
```

## 3. 代码边界

```text
agent-definitions/         跨 Rust/Pi 的版本化 Agent/Prompt Registry 唯一事实源
backend/                  Rust HTTP API、领域编排、Repository、视频 Adapter
crates/
  novex-ai-core/          Definition、Prompt 编译、审计与通用类型
  novex-model/            Rust 模型路由与 provider 合同
  novex-agent/            受审计模型执行与 Agent Run 生命周期合同
  novex-rag/              检索与引用边界
  novex-tools/            类型化领域工具和安全策略边界
  novex-memory/           正式长期 Memory 策略边界
  novex-eval/             版本激活门禁与评测合同
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

Pi Harness 负责新工作台的模型流、Turn、Tool Call、Observation、steering、follow-up、abort 和事件流。Novex 组合式 wrapper 只通过 Pi 公开 hook/API 接入固定 Definition、Prompt 编译、调用审计和 Tool Gate，不继承私有实现、不修改 Pi 源码，也不复制第二套 Tool Loop。Rust Agent Kernel 不再新增同职责的通用 Tool Loop。

Runtime 按 Pi `0.82.0` 使用 `toolContext + AgentHarnessTool` 契约。虽然上游已导出 execution tool factory，Novex 仍保留自有 `read/write/edit/bash` schema 和适配器，避免改变既有 `old_text/new_text` edit 参数、Session transcript 与 SSE 行为；切换上游 factory 必须另行设计协议迁移。

现有视频 Conversation API、Rust `AgentRunCoordinator`、业务 Adapter、Run/Step 和失败收尾保持不变。Rust 生产文本节点统一通过 `PromptCompiler + AuditedModelExecutor` 执行，Adapter 不持有裸 `LLMClient`；Pi 与 Rust 各自只执行自己拥有的 Definition node。后续迁移单个领域 Agent 时，必须用独立 OpenSpec change 先把业务能力暴露为类型化 Tool，再确定唯一执行入口和旧路径删除计划；禁止双模型调用、双 Assistant 消息和双写。

## 5. Definition、模型绑定与调用审计

`agent-definitions/` 是版本化 `AgentDefinition`、`PromptDefinition`、模板、schema 与发布索引的唯一事实源。Rust 与 Pi 使用强类型 loader 只读加载同一 Registry，并在构建、启动和发布时 fail-closed 校验 digest、引用、owner、状态和激活证据。PostgreSQL 只保存不可变 Definition 内容证据与 registry lifecycle manifest，不保存模板正文，也不能在线覆盖代码定义。

Definition 生命周期为 `candidate -> active -> supported -> revoked`。新 Session/Conversation 只绑定唯一 active 版本；既有绑定可继续 supported 版本，revoked 在模型请求前阻断。回滚通过代码发布重新激活既有 supported 版本，保留向前兼容表、历史 binding、`ModelCall`、`EvalRun` 和 `EvalReport`，不执行破坏性逆迁移。

PostgreSQL `ai_models` 是所有模型部署的唯一配置来源。Pi Runtime 每轮按会话的 `model_id` 重新解析当前启用的 `text` 模型，只支持仓库已正式支持的：

- `openai_responses` -> Pi `openai-responses`
- `openai_chat_completions` -> Pi `openai-completions`

Runtime 不根据 URL 猜测协议，不回退环境变量、Pi 内建模型目录或默认模型。API Key 仅在请求进程内传给 provider，不得进入 HTTP/SSE、日志、SQLite metadata、entry 或错误信息。

Session、Conversation 或非会话 Run 固定 `model_id + behavior_fingerprint`。每次调用前重新解析 `ai_models`：凭据轮换且 fingerprint 不变时可继续；协议、地址、上游模型、reasoning、输出上限、context window、行为 settings、启用状态或能力发生不兼容变化时，在外部请求前阻断并要求显式 rebind/fork。

每次实际模型步骤和显式重试都先建立一个独立 `ModelCall`，在脱敏输入持久化成功后才能调用 provider，并只允许一个 `succeeded`、`failed` 或 `aborted` 终态。Rust 写 PostgreSQL，Pi 写 namespaced SQLite；两者提供同 schema 的摘要、脱敏详情、版本化导出和无副作用 `dry_run` replay。真实模型对比只能进入有明确 case/token/retry/cost 预算并经确认的独立 `EvalRun`；常规验证使用 fake provider，真实模型调用数为零。

## 6. Session、Context 与 Memory

Pi SQLite 只拥有 Agent Session：

- 会话 metadata 与活动 leaf
- 消息和工具结果
- 不可变 Definition/Prompt 与模型行为 binding
- namespaced `ModelCall`、迁移事件和可恢复删除意图
- 分支、fork、branch summary
- compaction 与近期上下文

PostgreSQL 继续拥有项目、脚本、素材、作品、发布记录、模型配置及其他领域事实。两类存储不做跨库双写事务。

`Context` 是本轮动态装配的数据；Pi compaction 是有损 Session Context。正式长期 `Memory` 只保存已确认、稳定、可复用的信息，必须有来源、作用域、更新时间、失效和删除语义。compaction summary 中的未确认推断不得自动升级为长期 Memory。

当前只把既有 Context 装配结果转换为带来源和信任等级的 Prompt User 层结构化输入；统一 Context Compiler 的优先级、token 预算与裁剪策略，以及正式长期 Memory，均尚未在本基线实现。

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

Agent Runtime 启动时先校验 Registry 与生产 Definition inventory；`/health` 只报告进程存活，`/ready` 分别验证 PostgreSQL 和可写 SQLite。SQLite 固定保存在命名卷 `ai-agent-session-data`，容器重建不得退化为内存存储。Rust 与 Pi 发布镜像都包含同一 Registry digest，生产运行不依赖开发目录挂载。

常规自动测试必须使用 fake provider，不调用真实模型、视频生成或平台发布。真实外部调用只允许在明确成本上限与用户确认后执行。

## 10. 架构变更规则

以下变化必须先建立或更新 OpenSpec change：

- 新增工作台、服务、核心表或跨语言协议
- 模型配置来源或 provider 映射变化
- Pi/Rust 执行职责迁移
- Session 与正式 Memory 所有权变化
- 新增本地工具或领域外部动作
- 付费、发布、删除等 Gate 变化

当前基准是“代码级 Definition Registry + Pi 通用执行内核 + Rust 领域控制与视频链路 + 调用前持久化审计”。允许能力逐步扩展，但不允许出现模型或 Prompt 第二事实源、同请求双执行、凭据落盘、未审计模型调用或通用工具绕过领域 Gate。当前没有 Prompt 在线编辑、审计 Admin UI、统一 Context Compiler 或正式长期 Memory；这些能力必须通过后续独立 OpenSpec 推进。
