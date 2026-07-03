# 设计文档

## 生成模式选择

`AppState` 根据 `OPENAI_REASONING_EFFORT` 选择脚本生成模式：

- `xhigh`：使用 `StepwiseSingleScene`。
- 其他值或未配置：使用现有 `Complete` 完整脚本生成路径。

该选择只影响脚本生成服务内部编排，不改变 HTTP API、数据库或前端契约。

## 分步编排

`StepwiseSingleScene` 生成流程：

1. 校验 `GenerateScriptRequest`，确认项目存在，确认 `parent_id` 属于同一项目。
2. 请求 LLM 生成元信息，只输出：
   - `title`
   - `hook`
3. 按 `1..=scene_count` 串行请求 LLM，每次只生成一个 `scene`。
4. 对每个步骤使用现有解析和业务约束：标题长度、hook 非空、分镜序号、旁白长度、画面描述、情绪、时长。
5. 聚合为现有 `ScriptLLMOutput`，复用 `build_script` 和 repository 保存逻辑。

不使用并发拆分，因为实测并发 `xhigh` 请求会触发当前供应商 `502 upstream_error`。

## Prompt 与输出预算

新增两类小 prompt：

- `build_metadata`：只要求输出 `title/hook`，prompt 级 `max_output_tokens = 400`。
- `build_single_scene`：只要求输出一个 `scene`，prompt 级 `max_output_tokens = 1200`。

完整脚本 prompt 保持原行为，不设置 prompt 级输出预算，继续使用全局 `OPENAI_MAX_OUTPUT_TOKENS`。

## novex-model 边界

`LLMPrompt` 增加可选字段 `max_output_tokens`。Responses API 分支优先使用 prompt 级输出预算；未设置时继续使用 `OpenAIConfig.responses_max_output_tokens`。

Chat Completions 分支不使用该字段，保持现有兼容行为。

## 错误处理

每个分步步骤沿用现有最多 3 次解析/校验重试策略。某个步骤出现 provider error 或 timeout 时，错误仍通过现有 `ScriptAgentError` 和 HTTP 错误映射返回。

分步模式的重试计数聚合到脚本 `content.metadata.retry_count`，并记录 `content.metadata.generation_mode = "stepwise_single_scene"`，用于后续排查。

## 测试

新增或调整测试覆盖：

1. prompt builder 能生成元信息 prompt 和单分镜 prompt。
2. 单分镜输出解析器校验目标 `sequence`。
3. `ScriptAgentService` 分步模式按 `metadata -> scene 1 -> scene 2 ...` 顺序调用 LLM 并保存脚本。
4. `xhigh` 路由测试确认 API 入口切换到分步模式。
5. `OpenAIClient` 测试确认 prompt 级 `max_output_tokens` 覆盖全局配置。
6. 真实供应商端到端验证 `xhigh` 下 6 分镜生成返回 HTTP 200。
