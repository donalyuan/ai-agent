# 设计文档

## 模块边界

### 迁入 `crates/novex-model`

- `LLMClient` trait
- `LLMError`
- `OpenAIConfig`
- `OpenAIClient`
- Chat Completions 请求/响应 DTO
- Responses API 请求/响应 DTO
- provider HTTP 错误格式化

### 留在 `backend`

- `ScriptPrompt`
- `ScriptPromptBuilder`
- `ScriptLLMOutput`
- `ScriptLLMScene`
- `LLMOutputError`
- `ScriptAgentService`

理由：Prompt 和输出校验包含 video-agent 脚本业务规则，例如标题长度、分镜数量、旁白长度和分镜顺序；这些不是通用模型 provider 能力。

## 依赖方向

`backend` SHALL 依赖 `novex-model`。`novex-model` SHALL 不依赖 `backend`。

目标依赖关系：

```text
backend -> novex-model
backend -> repositories / axum / sqlx
novex-model -> reqwest / serde / async-trait
```

## 兼容策略

现有 `ScriptAgentService` 仍通过 `Arc<dyn LLMClient>` 注入 provider。迁移后只改变 trait 的来源路径，不改变 service 的调用方式。

## 测试策略

1. 将通用 provider mock 测试迁移到 `crates/novex-model` 或保留在后端集成测试中引用 crate API。
2. 保留 backend 脚本业务测试，确保 Prompt 与脚本解析约束不回退。
3. 运行后端相关测试和 workspace 测试。
4. 用当前 Responses API 配置执行一次真实脚本生成验证。

## 风险

- 移动 trait 可能影响测试中的 fake LLM 类型导入路径。
- `backend/src/lib.rs` 的 Lazy client 构造需要改为引用 `novex-model` config 字段。
- crate 依赖调整可能暴露当前 `crates/novex-model` 依赖不足，需要同步更新 `Cargo.toml`。
