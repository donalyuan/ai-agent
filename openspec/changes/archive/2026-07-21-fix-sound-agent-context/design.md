## Context

声音工作台目前通过通用 Agent 消息接口只提交 `content + model_id`。声音 Runtime 从会话读取 TTS 模型 ID，再加载音色目录，但 Prompt 只包含用户消息并使用 `.take(80)` 截断目录。因而“旁边的文本”没有指向真实旁白，目录后 335 项也不可能被推荐。

本变更跨越 Next.js 页面、通用会话 API 和 Rust 声音 Runtime，但不改变数据库和 TTS/ASR 任务协议。用户消息及 Agent run 已有 JSONB metadata/input，可保存脱敏上下文用于审计。

## Goals / Non-Goals

**Goals:**

- 每轮声音建议显式携带当前旁白、音色、语言、参数和字幕断句。
- 后端验证上下文绑定的 TTS 模型与声音会话一致，禁止客户端借会话读取其他模型目录。
- Prompt 覆盖全部可用音色，同时剔除试听文本、头像等与推荐无关的大字段。
- 保持目录外音色和不支持语言的服务端拒绝规则。
- 保存本轮声音上下文快照，使失败请求也可追踪实际输入。

**Non-Goals:**

- 不新增结构化情绪字段，不伪造供应商未支持的能力。
- 不让 Agent 自动试听、生成 TTS/ASR 或绕过人工确认。
- 不在本变更中增加跨供应商自动切换或上游错误重试。
- 不修改音色目录同步、声音任务或素材入库数据模型。

## Decisions

1. 通用消息请求增加可选 `sound_context`，而不是把旁白拼进用户自由文本。结构化字段可独立校验、审计并避免自然语言引用歧义；其他 Agent 不传该字段，现有协议保持兼容。
2. `sound_context` 包含 `speech_model_id`、`tts_text`、`voice_type`、`language`、`parameters` 和 `subtitle_segments`。前端每次发送时从当前编辑状态创建快照，不依赖会话创建时状态。
3. Runtime 将上下文写入用户消息 metadata 和 run input，并只允许 `sound` 会话使用。声音会话缺少上下文时返回稳定校验错误，防止继续产生无法理解当前编辑区的建议。
4. Prompt 使用全部 `is_available=true` 音色，但每项只保留 `voice_type/name/description/language_codes`。`language_codes` 从目录 `languages` 的字符串项或对象 `Language/language/Value/value` 规范化提取，明确排除试听 `Text`。相比直接发送 415 条原始目录，这能完整覆盖选择空间并显著降低输入体积。
5. Prompt 同时提供模型声明的参数能力和当前参数；推荐结果继续经过目录音色与语言校验，前端应用后仍走既有表单能力校验和生成预检。
6. 不把完整旁白复制到 Agent run 的顶层日志字段之外；其审计副本位于已有数据库 JSONB，不输出到服务日志，沿用当前对话数据访问边界。
7. 声音 Agent 不向 OpenAI-compatible 客户端传递严格 `json_schema`，而是显式使用 `json_object`。真实供应商诊断已证明同一 `gpt-5.6-sol + Responses + SSE + temperature + reasoning` 请求在 `json_object` 下返回 `200`，改为严格 `json_schema` 后返回 `502`。完整输出契约序列化进 Prompt，响应仍由 `SoundRecommendation` 的 `deny_unknown_fields`、必填字段、字幕对齐、目录音色、语言和参数能力校验共同约束；这是一条确定协议，不做失败后的自动降级。

## Risks / Trade-offs

- [完整目录仍会增加 Prompt token] -> 仅发送推荐必需字段，并用测试保证没有试听文案；目录规模后续若显著增长，应独立设计检索式候选选择，不能恢复静默截断。
- [旧客户端不发送上下文] -> API 字段保持可选，但声音 Runtime 明确返回“声音消息缺少当前编辑上下文”；其他 Agent 不受影响。
- [会话创建后切换 TTS 模型] -> 前端现有逻辑重置声音会话；后端再次比较 `speech_model_id`，不一致时拒绝请求。
- [用户要求结构化情绪但协议不支持] -> Prompt 明确只能使用目录音色、语言和已声明参数，不生成虚构字段或自动执行。
- [供应商 `json_object` 不提供服务端 schema 强约束] -> Prompt 携带完整契约，Rust 必须在保存建议前严格解析并执行业务校验，任何无效输出直接失败且不得自动改用其他模式重试。

## Migration Plan

1. 先发布兼容读取可选 `sound_context` 的后端。
2. 发布前端，使所有声音 Agent 新消息携带当前快照。
3. 无数据库迁移；回滚时恢复旧前后端即可，历史消息 metadata 中新增字段可被旧代码忽略。

## Open Questions

无。
