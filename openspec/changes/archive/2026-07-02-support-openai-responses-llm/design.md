# 设计文档

## 方案

在现有 `OpenAIClient` 内增加协议选择逻辑：

- `base_url` 以 `/responses` 结尾时，视为 Responses API endpoint，直接 POST `base_url`。
- 其他情况保持现有行为，POST `{base_url}/chat/completions`。

该选择逻辑留在 provider adapter 层，不向 `ScriptAgentService` 暴露协议差异。

## 请求结构

Responses API 请求体：

```json
{
  "model": "gpt-5.5",
  "temperature": 0.8,
  "input": [
    {
      "role": "system",
      "content": [{ "type": "input_text", "text": "..." }]
    },
    {
      "role": "user",
      "content": [{ "type": "input_text", "text": "..." }]
    }
  ],
  "text": {
    "format": { "type": "json_object" }
  }
}
```

该结构已通过当前网关 `/responses` 端点验证。

## 配置项

Responses API 额外支持以下环境变量：

- `OPENAI_REASONING_EFFORT`：推理强度，默认 `low`；设置为 `none` 时不发送 `reasoning` 字段。
- `OPENAI_MAX_OUTPUT_TOKENS`：最大输出 token，默认 `3000`。

这些配置只影响 Responses API 分支；Chat Completions 分支保持当前行为。

## 响应解析

优先从以下结构提取文本：

```json
{
  "output": [
    {
      "content": [
        { "type": "output_text", "text": "{...}" }
      ]
    }
  ]
}
```

若未找到非空文本，返回 `LLMError::Provider("missing response output text")`。

## 错误处理

HTTP 非 2xx 继续沿用现有 `format_provider_error`，保留状态码和响应体。网络超时仍映射为 `LLMError::Timeout`。

## 测试

1. 新增 Responses mock server 测试，断言请求路径、请求体和响应提取。
2. 新增配置化测试，覆盖自定义推理强度、输出 token 上限和关闭 `reasoning` 字段。
3. 保留现有 Chat Completions mock server 测试。
4. 运行 `script_llm`、`script_agent_service`、`script_routes` 测试。
5. 用真实运行中的 API 进行一次端到端生成验证。
