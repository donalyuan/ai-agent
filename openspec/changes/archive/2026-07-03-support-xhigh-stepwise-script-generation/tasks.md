# 任务清单

- [x] 复现 `xhigh` 完整脚本生成约 30 秒返回供应商 `502 upstream_error`
- [x] 验证小 payload + `xhigh` 可用
- [x] 验证 1 分镜串行 `xhigh` 请求可用
- [x] 验证 2 分镜和并发拆分在当前供应商下不稳定
- [x] 设计 `xhigh` 下元信息 + 单分镜串行生成模式
- [x] 新增元信息 prompt 和单分镜 prompt
- [x] 新增元信息和单分镜输出解析/校验逻辑
- [x] 实现 `ScriptGenerationMode::StepwiseSingleScene`
- [x] 在 API 状态中根据 `OPENAI_REASONING_EFFORT=xhigh` 切换分步模式
- [x] 为 `LLMPrompt` 增加 prompt 级 `max_output_tokens`
- [x] 为元信息和单分镜请求设置更小输出预算
- [x] 增加 prompt、解析器、服务层、路由层和 OpenAI client 测试
- [x] 运行相关 Rust 测试
- [x] 在真实环境中验证 `xhigh` 下 6 分镜生成返回 HTTP 200
