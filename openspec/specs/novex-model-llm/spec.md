# novex-model-llm Specification

## Purpose
Define the reusable LLM provider client boundary owned by `crates/novex-model`, including OpenAI-compatible Chat Completions and Responses API behavior consumed by `backend` and future agents.
## Requirements
### Requirement: novex-model 提供可复用 LLM provider 客户端

`novex-model` SHALL 提供由显式 API 调用协议和模型部署配置驱动的 OpenAI-compatible LLM provider 客户端能力，供 backend 和未来 Agent 复用。

#### Scenario: backend 通过 novex-model 调用 LLM

- **GIVEN** backend 已根据 `model_id` 解析一个启用文本模型
- **WHEN** `ScriptAgentService` 或 Agent Runtime 调用 `LLMClient::generate_script`
- **THEN** `LLMClient` trait SHALL 来自 `novex-model`
- **AND** OpenAI-compatible provider 请求 SHALL 由 `novex-model` 实现
- **AND** backend SHALL 只负责业务 prompt、模型选择和输出校验

#### Scenario: 显式使用 Responses API 协议

- **GIVEN** 模型配置的 `api_protocol=openai_responses`
- **WHEN** `novex-model` OpenAI-compatible client 发起模型请求
- **THEN** 客户端 SHALL POST 由 `request_base_url` 和 Responses 稳定路径组成的 endpoint
- **AND** 客户端 SHALL 使用模型记录中的推理等级与最大输出 Token
- **AND** 客户端 SHALL NOT 根据 URL 后缀推断协议

#### Scenario: 显式使用 Chat Completions 协议

- **GIVEN** 模型配置的 `api_protocol=openai_chat_completions`
- **WHEN** `novex-model` OpenAI-compatible client 发起模型请求
- **THEN** 客户端 SHALL POST 由 `request_base_url` 和 Chat Completions 稳定路径组成的 endpoint
- **AND** 客户端 SHALL NOT 因 URL 形态切换到 Responses API

#### Scenario: 不支持的文本协议被拒绝

- **GIVEN** 调用方把非文本协议传给 LLM 客户端
- **WHEN** 客户端构造请求
- **THEN** 客户端 SHALL 返回配置错误
- **AND** 客户端 SHALL NOT 发起 HTTP 请求

### Requirement: Responses API 支持 prompt 级输出 token 上限

`novex-model` 的 OpenAI-compatible Responses API 客户端 SHALL 支持单次 prompt 覆盖最大输出 token，以便调用方根据任务粒度控制输出预算。

#### Scenario: prompt 覆盖全局输出上限

- **GIVEN** `OpenAIConfig.responses_max_output_tokens` 已设置为正整数
- **AND** `LLMPrompt.max_output_tokens` 设置为正整数
- **WHEN** 客户端调用 Responses API
- **THEN** 请求体中的 `max_output_tokens` SHALL 等于 `LLMPrompt.max_output_tokens`
- **AND** 不 SHALL 使用全局 `OpenAIConfig.responses_max_output_tokens`

#### Scenario: prompt 未设置输出上限时使用全局配置

- **GIVEN** `OpenAIConfig.responses_max_output_tokens` 已设置为正整数
- **AND** `LLMPrompt.max_output_tokens` 为空
- **WHEN** 客户端调用 Responses API
- **THEN** 请求体中的 `max_output_tokens` SHALL 等于 `OpenAIConfig.responses_max_output_tokens`
