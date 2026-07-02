# 支持 OpenAI Responses API 的 LLM 客户端

## 背景

当前脚本 Agent 的 `OpenAIClient` 只支持 Chat Completions 协议，会把 `OPENAI_BASE_URL` 固定拼接为 `.../chat/completions`。当前接入的网关主要通过 `/responses` 端点提供模型调用；当把 `OPENAI_BASE_URL` 配为 `/responses` 时，实际请求会变成 `/responses/chat/completions` 并返回上游错误。

## 目标

1. 让脚本 Agent 支持 OpenAI-compatible Responses API。
2. 保留现有 Chat Completions 兼容能力，避免破坏已有测试和旧配置。
3. 继续向 `ScriptAgentService` 返回原始脚本 JSON 文本，不改变脚本领域模型、HTTP API、数据库结构。

## 非目标

- 不引入多 provider 注册表。
- 不改脚本生成 Prompt 的业务内容。
- 不改 Admin 前端。
- 不处理流式输出；本次只支持同步 Responses JSON。

## 成功标准

- 当 `OPENAI_BASE_URL` 以 `/responses` 结尾时，客户端直接 POST 该端点。
- 请求体使用 Responses API 的 `input` 消息结构和 JSON 输出格式约束。
- 客户端能从 `output[].content[].type = "output_text"` 中提取脚本 JSON 文本。
- 现有 Chat Completions 测试继续通过。
- 真实 `/api/scripts/generate` 能通过当前网关完成一次脚本生成并保存脚本。

## 风险

- 第三方网关的 Responses API 兼容性可能与官方 OpenAI API 有差异；实现应以已验证的运行时响应结构为准，并保持错误信息可诊断。
- 当前 `docker-compose.yml` 中已出现敏感 key 默认值；本变更不处理该文件，但后续应把 key 移回环境变量或 `.env` 并轮换。
