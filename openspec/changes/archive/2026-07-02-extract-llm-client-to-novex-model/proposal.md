# 抽取 LLM 客户端到 novex-model

## 背景

当前 `backend/src/agents/llm.rs` 同时承载脚本 Agent 的业务 Prompt、LLM provider 协议适配、OpenAI-compatible 配置和请求响应 DTO。脚本生成闭环已打通，但 LLM provider 能力属于 Novex AI Agent Foundation 的可复用基座能力，不应长期留在 `backend` 控制面模块中。

项目架构约定要求：`backend/` 负责控制面 API 和业务编排，通用 AI 能力沉淀到 `crates/*`。`crates/novex-model` 的定位是模型注册、路由、provider capability 和用量边界，适合作为 LLM provider 客户端的归属。

## Why

如果继续把通用 LLM provider 能力留在 `backend`，后续多个 Agent 会重复实现模型调用、配置解析、Responses API 兼容和错误处理，导致基座能力无法复用。迁移到 `crates/novex-model` 可以让 `backend` 回到控制面和业务编排职责，并为后续模型注册、路由、用量统计和 provider 能力描述留出稳定边界。

## 目标

1. 将通用 OpenAI-compatible LLM 客户端能力迁移到 `crates/novex-model`。
2. 保留脚本 Agent 当前生成行为、API 响应结构、数据库结构和端到端能力。
3. 让后续选题 Agent、素材 Agent、优化 Agent 等复用同一个 provider adapter。
4. 保持 Chat Completions 与 Responses API 兼容路径。

## What Changes

- 新增 `crates/novex-model` LLM provider 模块。
- 将 OpenAI-compatible client、config、error、DTO 从 backend 迁入 `novex-model`。
- backend 改为引用 `novex-model` 的 `LLMClient` 和 `OpenAIClient`。
- backend 保留脚本 Prompt 构造和脚本输出业务校验。

## 非目标

- 不新增多 provider 路由策略。
- 不实现模型用量计费统计。
- 不改脚本 Agent HTTP API。
- 不改 Prompt 业务内容。
- 不改数据库 schema。

## 成功标准

- `crates/novex-model` 暴露可复用的 `LLMClient`、OpenAI-compatible config、client、错误类型和协议 DTO。
- `backend` 不再维护通用 OpenAI 请求/响应 DTO。
- `backend` 只保留脚本业务 Prompt 构造、脚本 LLM 输出解析和业务校验。
- 现有 `script_llm`、`script_agent_service`、`script_routes` 测试继续通过。
- 真实脚本生成仍可通过 Responses API 完成端到端验证。
