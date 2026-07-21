## Why

声音 Agent 当前只收到右侧输入框文本，无法读取操作者所指的当前旁白和声音配置；同时 Prompt 将音色目录截断为前 80 项，导致 Agent 无法在全部可用音色中推荐。这使已归档规格中的“根据旁白内容推荐”无法成立。

## What Changes

- 声音 Agent 消息请求携带当前旁白、已选音色、语言、声音参数和字幕断句的结构化上下文。
- 后端只接受声音会话的声音上下文，校验上下文与会话绑定的 TTS 模型一致，并将其纳入本轮 Prompt。
- Prompt 覆盖当前模型的全部可用音色；目录条目仅保留推荐所需字段和规范化语言代码，不携带试听文案等冗余数据。
- 声音 Agent 的 Responses 请求使用供应商已验证支持的 `json_object` 输出模式，并在 Prompt 中声明完整 JSON 契约；Rust Runtime 继续执行严格结构和业务校验。
- Agent 继续只输出建议，不自动调用 TTS/ASR；应用建议和生成仍由操作者主动确认。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `sound-subtitle-generation`: 补充声音 Agent 必须读取当前编辑上下文，并基于完整可用音色目录生成建议的行为约束。

## Impact

- 前端声音工作台 Agent 消息请求类型与发送逻辑。
- 后端会话消息 DTO、声音 Agent Runtime、Prompt 组装、输出格式和输入输出校验。
- 声音 Agent 路由测试、Runtime 单元测试及前端交互测试。
- 不修改声音任务、TTS/ASR 协议、数据库 schema 或外部生成确认流程。
