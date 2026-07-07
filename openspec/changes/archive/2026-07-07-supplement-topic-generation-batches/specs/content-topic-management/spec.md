## MODIFIED Requirements

### Requirement: 选题 Agent 必须自动入库并记录批次

系统 SHALL 允许操作者基于项目定位和补充要求生成选题候选。Agent 生成结果 SHALL 自动写入选题池，状态为 `idea`，并关联一次生成批次。

#### Scenario: Agent 批量生成选题

- **GIVEN** 数据库中存在一个项目
- **AND** 操作者提交非空补充要求和生成数量
- **WHEN** 系统调用 `topic` Agent 生成候选
- **THEN** 系统 SHALL 创建 `topic_generation_batches` 记录
- **AND** 系统 SHALL 将生成候选写入 `content_topics`
- **AND** 每条候选的 `source` SHALL 为 `agent`
- **AND** 每条候选的 `status` SHALL 为 `idea`
- **AND** Agent 回复 metadata SHALL 包含 `batch_id`、`created_topic_ids` 和 `topic_count`

#### Scenario: 对历史生成批次补充选题

- **GIVEN** 项目下存在一个 `succeeded` 且仍有可见选题的生成批次
- **AND** 操作者基于该批次提交非空补充要求和生成数量
- **WHEN** 系统调用 `topic` Agent 生成补充候选
- **THEN** 系统 SHALL 创建新的 `topic_generation_batches` 记录
- **AND** 新批次的 `supplement_of_batch_id` SHALL 指向原始批次
- **AND** 系统 SHALL 将原始生成要求和同主题组已有选题作为上下文提供给 LLM
- **AND** 系统 SHALL 要求 LLM 生成与原主题相关且不重复已有选题的新候选
- **AND** 系统 SHALL NOT 修改原始批次的 `prompt`、`requested_count`、`source_run_id` 或 `metadata`
- **AND** 新生成的 `content_topics.batch_id` SHALL 指向补充批次本身
- **AND** Agent 回复 metadata SHALL 包含新批次 `batch_id`、`supplement_of_batch_id`、`created_topic_ids` 和 `topic_count`

#### Scenario: 补充批次必须归属同一项目

- **GIVEN** 数据库中存在项目 A 和项目 B
- **AND** 项目 A 下存在一个生成批次
- **WHEN** 操作者在项目 B 下请求补充项目 A 的批次
- **THEN** 系统 SHALL 拒绝请求
- **AND** 系统 SHALL NOT 创建新的生成批次
- **AND** 系统 SHALL NOT 写入新的选题

#### Scenario: 不允许补充不可管理批次

- **GIVEN** 目标生成批次不存在、状态不是 `succeeded`，或当前没有未软删除选题
- **WHEN** 操作者请求补充该批次
- **THEN** 系统 SHALL 拒绝请求
- **AND** 系统 SHALL 返回明确错误
- **AND** 系统 SHALL NOT 创建新的生成批次
- **AND** 系统 SHALL NOT 写入新的选题

#### Scenario: 查询选题生成批次历史

- **GIVEN** 项目下存在多个 `topic_generation_batches`
- **AND** 每个批次关联不同数量的选题
- **WHEN** 操作者打开内容策略页或查看历史生成
- **THEN** 系统 SHALL 按生成时间倒序返回该项目的生成批次
- **AND** 每个批次 SHALL 包含 `batch_id`、`prompt`、`status`、`requested_count`、`topic_count`、`supplement_of_batch_id` 和生成时间
- **AND** 系统 SHALL 只返回 `succeeded` 且 `topic_count > 0` 的生成批次作为历史生成入口
- **AND** 系统 SHALL NOT 返回失败批次或未产出选题的空批次
- **AND** 系统 SHALL NOT 返回其他项目的生成批次

#### Scenario: LLM 输出非法时整批失败

- **GIVEN** 操作者发起选题生成
- **WHEN** LLM 输出为空、无法解析、字段缺失或 score 越界
- **THEN** 系统 SHALL 标记本次批次为 `failed`
- **AND** 系统 SHALL NOT 部分写入选题
- **AND** 系统 SHALL 记录失败 run 和错误信息
