# novex-model-llm Specification

## Purpose
Define the reusable LLM provider client boundary owned by `crates/novex-model`, including OpenAI-compatible Chat Completions and Responses API behavior consumed by `backend` and future agents.
## Requirements
### Requirement: novex-model 提供可复用 LLM provider 客户端

`novex-model` SHALL 提供 OpenAI-compatible LLM provider 客户端能力，供 backend 和未来 Agent 复用。

#### Scenario: backend 通过 novex-model 调用 LLM

- **GIVEN** backend 需要生成脚本内容
- **WHEN** `ScriptAgentService` 调用 `LLMClient::generate_script`
- **THEN** `LLMClient` trait SHALL 来自 `novex-model`
- **AND** OpenAI-compatible provider 请求 SHALL 由 `novex-model` 实现
- **AND** backend SHALL 只负责脚本业务 prompt 和输出校验

#### Scenario: 保留 Responses API 兼容能力

- **GIVEN** `OPENAI_BASE_URL` 以 `/responses` 结尾
- **WHEN** `novex-model` OpenAI-compatible client 发起模型请求
- **THEN** 客户端 SHALL 直接 POST `OPENAI_BASE_URL`
- **AND** 客户端 SHALL 支持 `OPENAI_REASONING_EFFORT` 与 `OPENAI_MAX_OUTPUT_TOKENS` 配置

#### Scenario: 保留 Chat Completions 兼容能力

- **GIVEN** `OPENAI_BASE_URL` 未以 `/responses` 结尾
- **WHEN** `novex-model` OpenAI-compatible client 发起模型请求
- **THEN** 客户端 SHALL POST `{OPENAI_BASE_URL}/chat/completions`

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
