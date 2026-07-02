# 任务清单

- [ ] 确认 `novex-model` 当前依赖和导出边界
- [ ] 在 `crates/novex-model` 新增 LLM provider 模块与测试
- [ ] 将 `LLMClient`、`LLMError`、`OpenAIConfig`、`OpenAIClient` 迁入 `novex-model`
- [ ] 调整 `backend` 引用路径，保留脚本 Prompt 和输出校验在 backend
- [ ] 移动或改写 Chat Completions 与 Responses API 测试
- [ ] 更新 Cargo 依赖配置
- [ ] 运行相关 Rust 测试
- [ ] 执行真实脚本生成端到端验证
- [ ] 更新 memory 中的 LLM 模块归属说明
