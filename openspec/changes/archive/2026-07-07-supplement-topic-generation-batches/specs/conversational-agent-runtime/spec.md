## MODIFIED Requirements

### Requirement: 通用 Agent Runtime 必须支持 topic Agent adapter

通用对话 Agent Runtime SHALL 支持 `topic` adapter，使内容策略页可以通过对话消息触发选题生成、持久化选题和记录生成批次。

#### Scenario: topic Agent 生成普通选题批次

- **GIVEN** 已存在一个 `topic` Agent 会话
- **AND** 操作者消息包含非空补充要求
- **WHEN** Runtime 处理该消息
- **THEN** Runtime SHALL 创建 `agent_runs` 和 `agent_steps`
- **AND** Runtime SHALL 创建一个没有 `supplement_of_batch_id` 的 `topic_generation_batches`
- **AND** Runtime SHALL 调用 LLM 生成结构化选题
- **AND** Runtime SHALL 将候选写入 `content_topics`
- **AND** Runtime SHALL 保存 assistant 消息
- **AND** assistant 消息 metadata SHALL 包含 `batch_id`、`created_topic_ids`、`topic_count` 和 `status`

#### Scenario: topic Agent 生成补充批次

- **GIVEN** 已存在一个 `topic` Agent 会话
- **AND** 操作者消息包含非空补充要求
- **AND** 请求 metadata 包含同项目下可补充的 `supplement_of_batch_id`
- **AND** 原始批次及其补充批次下存在未软删除选题
- **WHEN** Runtime 处理该消息
- **THEN** Runtime SHALL 创建新的 `topic_generation_batches`
- **AND** 新批次 SHALL 记录 `supplement_of_batch_id`
- **AND** Runtime SHALL 在调用 LLM 时注入原始批次 prompt、同主题组已有选题和当前会话历史消息摘要
- **AND** Runtime SHALL 要求 LLM 基于同一主题继续扩展并避免重复已有选题
- **AND** Runtime SHALL 将候选写入 `content_topics`
- **AND** 新候选的 `batch_id` SHALL 指向新补充批次
- **AND** assistant 消息 metadata SHALL 包含新批次 `batch_id`、`supplement_of_batch_id`、`created_topic_ids`、`topic_count` 和 `status`

#### Scenario: topic Agent 补充目标不可用

- **GIVEN** 请求 metadata 包含不存在、跨项目、失败或空批次的 `supplement_of_batch_id`
- **WHEN** Runtime 处理该消息
- **THEN** Runtime SHALL 拒绝生成
- **AND** Runtime SHALL NOT 创建新的 `topic_generation_batches`
- **AND** Runtime SHALL NOT 写入新的 `content_topics`
- **AND** Runtime SHALL 返回明确错误
