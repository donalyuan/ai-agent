# script-agent-mvp Specification Delta

## ADDED Requirements

### Requirement: LLM 客户端支持 Responses API

脚本 Agent 的 OpenAI-compatible LLM 客户端 SHALL 支持通过 Responses API 生成结构化脚本，同时保留现有 Chat Completions 兼容能力。

#### Scenario: 使用 Responses endpoint 生成脚本文本

- **GIVEN** `OPENAI_BASE_URL` 指向以 `/responses` 结尾的 endpoint
- **WHEN** 脚本 Agent 请求 LLM 生成脚本
- **THEN** 客户端 SHALL 直接 POST `OPENAI_BASE_URL`
- **AND** 请求体 SHALL 使用 Responses API 的 `input` 消息结构
- **AND** 请求体 SHALL 约束输出为 JSON object
- **AND** 客户端 SHALL 从 `output[].content[].text` 提取非空文本返回给脚本解析器

#### Scenario: 保留 Chat Completions 兼容模式

- **GIVEN** `OPENAI_BASE_URL` 未以 `/responses` 结尾
- **WHEN** 脚本 Agent 请求 LLM 生成脚本
- **THEN** 客户端 SHALL 继续 POST `{OPENAI_BASE_URL}/chat/completions`
- **AND** 客户端 SHALL 从 `choices[].message.content` 提取非空文本

### Requirement: Responses API 推理参数可配置

脚本 Agent 的 Responses API 客户端 SHALL 允许通过环境变量调整推理强度和最大输出 token，避免需要修改 Rust 代码才能切换模型运行参数。

#### Scenario: 配置推理强度和输出上限

- **GIVEN** `OPENAI_REASONING_EFFORT` 设置为非空且不是 `none`
- **AND** `OPENAI_MAX_OUTPUT_TOKENS` 设置为正整数
- **WHEN** 客户端调用 Responses API
- **THEN** 请求体 SHALL 包含 `reasoning.effort` 且值等于 `OPENAI_REASONING_EFFORT`
- **AND** 请求体 SHALL 包含 `max_output_tokens` 且值等于 `OPENAI_MAX_OUTPUT_TOKENS`

#### Scenario: 关闭 reasoning 字段

- **GIVEN** `OPENAI_REASONING_EFFORT` 设置为 `none`
- **WHEN** 客户端调用 Responses API
- **THEN** 请求体 SHALL 不包含 `reasoning` 字段
