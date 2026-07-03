# 支持 xhigh 推理等级的分步脚本生成

## 背景

脚本 Agent 在 Responses API 下使用 `OPENAI_REASONING_EFFORT=xhigh` 时，当前供应商对完整脚本一次性生成请求表现不稳定：

- 小 payload + `xhigh` 可正常返回。
- 1 个分镜 + `xhigh` 可正常返回。
- 2 个分镜 + `xhigh` 偶发可返回，但不稳定。
- 3 个及以上分镜 + `xhigh` 会在约 30 秒返回供应商 `502 upstream_error`。
- 多个 `xhigh` 请求并发也会触发供应商 `502 upstream_error`。

用户明确要求系统必须使用 `xhigh`，不能通过降级为 `low` 规避问题。因此需要调整脚本生成编排方式，让后端在保持 HTTP API 和业务输出契约不变的前提下，将大请求拆成供应商可稳定处理的小请求。

## 目标

1. 在 `OPENAI_REASONING_EFFORT=xhigh` 时，脚本 Agent 使用分步串行生成：先生成 `title/hook`，再逐个生成单分镜。
2. 保持 `POST /api/scripts/generate` 请求和响应结构不变。
3. 保持非 `xhigh` 配置的完整脚本一次性生成路径，避免扩大行为变更面。
4. 支持 prompt 级 `max_output_tokens`，让小步骤请求使用与任务规模匹配的输出预算。
5. 用自动化测试和真实供应商端到端请求验证 6 分镜生成可用。

## 非目标

- 不引入异步任务队列或前端轮询。
- 不改变 `scripts`、`scenes` 数据库结构。
- 不改变视频工作台前端表单和详情展示。
- 不实现多供应商路由或 provider registry。
- 不把 `xhigh` 静默降级为其他推理等级。

## 成功标准

- 当 `OPENAI_REASONING_EFFORT=xhigh` 时，后端 SHALL 自动使用分步串行生成模式。
- 分步生成 SHALL 仍返回一个完整结构化脚本，并保存到 `scripts` 和 `scenes`。
- 单个 prompt 可覆盖 Responses API 的 `max_output_tokens`，元信息和单分镜请求不再使用完整脚本的大输出预算。
- `script_llm`、`script_agent_service`、`script_routes`、`openai_client` 相关测试通过。
- 真实运行环境中，`OPENAI_REASONING_EFFORT=xhigh` 下 6 分镜生成返回 HTTP 200。

## 风险

- 分步串行请求会增加同步接口耗时；当前 6 分镜真实验证耗时约 109 秒，要求 `OPENAI_TIMEOUT_SECONDS` 覆盖该耗时。
- 单分镜独立生成可能降低跨分镜叙事连续性；本变更优先解决 `xhigh` 可用性，后续可通过生成大纲或上下文摘要增强连续性。
- 当前实现仍是同步 HTTP 请求；如果未来分镜数提升或供应商变慢，应迁移为任务化生成。
