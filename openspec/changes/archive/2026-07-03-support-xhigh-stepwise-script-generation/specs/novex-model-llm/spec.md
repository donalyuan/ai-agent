# novex-model-llm Specification Delta

## ADDED Requirements

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
