# content-topic-management Specification

## Purpose
TBD - created by archiving change content-topic-agent. Update Purpose after archive.
## Requirements
### Requirement: 系统必须提供独立选题池

系统 SHALL 为视频工作台提供独立选题实体，使具体选题不再只作为脚本生成输入文本存在。每个选题 SHALL 归属一个真实项目，并具有可追踪状态、来源、内容类型、评分和标签。

#### Scenario: 人工新增选题

- **GIVEN** 数据库中存在一个项目
- **WHEN** 操作者提交选题标题、角度、受众、看点、内容类型和标签
- **THEN** 系统 SHALL 创建一条 `content_topics` 记录
- **AND** 选题状态 SHALL 为 `idea`
- **AND** 选题来源 SHALL 为 `manual`

#### Scenario: 查询项目选题池

- **GIVEN** 项目下存在多条不同状态和来源的选题
- **WHEN** 操作者按状态、来源或批次查询选题
- **THEN** 系统 SHALL 只返回该项目下匹配条件的选题
- **AND** 系统 SHALL NOT 返回其他项目的选题

#### Scenario: 选题状态流转

- **GIVEN** 已存在一条 `idea` 选题
- **WHEN** 操作者确认该选题
- **THEN** 系统 SHALL 将状态更新为 `approved`
- **AND** 该选题 SHALL 可以进入脚本生成确认流程

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

### Requirement: 内容策略页必须提供选题池闭环

`apps/video-agent` SHALL 在“内容策略”页面提供项目策略摘要、选题生成、选题池筛选、选题详情和进入脚本创作的入口。

#### Scenario: 展示策略摘要和选题池

- **GIVEN** 操作者已选择一个项目
- **WHEN** 操作者打开“内容策略”页面
- **THEN** 页面 SHALL 展示项目名称、定位、描述、选题总数、已确认数和已成稿数
- **AND** 页面 SHALL 展示该项目的选题池

#### Scenario: 按状态和生成批次查看选题池

- **GIVEN** 项目下存在多个生成批次，且选题状态包含 `idea`、`approved`、`scripted` 和 `archived`
- **WHEN** 操作者打开“内容策略”页面
- **THEN** 页面 SHALL 提供 `全部`、`待评估`、`已确认`、`已成稿` 和 `已归档` 状态筛选
- **AND** 页面 SHALL 提供历史生成批次列表
- **AND** 历史生成批次列表 SHALL 只展示成功且有实际选题的批次
- **AND** 页面默认 SHALL 按最新生成批次展示选题
- **AND** 操作者 SHALL 可以切换到任一历史批次或查看全部选题

#### Scenario: 从已确认选题进入脚本确认

- **GIVEN** 已存在一条 `approved` 选题
- **WHEN** 操作者点击“生成脚本”
- **THEN** 页面 SHALL 打开脚本生成确认面板
- **AND** 面板 SHALL 展示选题快照
- **AND** 面板 SHALL 要求操作者确认 `style` 和 `scene_count`

### Requirement: 系统必须支持选题软删除

系统 SHALL 支持将未生成脚本的选题从管理视图软删除。软删除 SHALL 不复用 `archived` 状态，并且 SHALL 保留原始选题记录用于数据一致性和后续审计。

#### Scenario: 软删除未生成脚本选题

- **GIVEN** 数据库中存在一条未生成脚本、未软删除的选题
- **AND** 不存在任何脚本引用该选题
- **WHEN** 操作者删除该选题
- **THEN** 系统 SHALL 为该选题记录软删除时间
- **AND** 系统 SHALL NOT 改写该选题的业务状态
- **AND** 系统 SHALL 从默认选题管理视图中移除该选题

#### Scenario: 已生成脚本选题不可删除

- **GIVEN** 数据库中存在一条状态为 `scripted` 或已被脚本引用的选题
- **WHEN** 操作者删除该选题
- **THEN** 系统 SHALL 拒绝删除请求
- **AND** 系统 SHALL 返回明确错误
- **AND** 系统 SHALL NOT 修改该选题记录

#### Scenario: 默认查询排除软删除选题

- **GIVEN** 项目下同时存在正常选题和已软删除选题
- **WHEN** 操作者查询选题池、选题统计或生成批次历史
- **THEN** 系统 SHALL 只统计未软删除选题
- **AND** 系统 SHALL NOT 在默认选题池中返回已软删除选题
- **AND** 生成批次的 `topic_count` SHALL 只计算未软删除选题

#### Scenario: 软删除选题不能进入脚本生成

- **GIVEN** 数据库中存在一条已软删除选题
- **WHEN** 操作者请求该选题进入脚本生成确认流程
- **THEN** 系统 SHALL 拒绝请求
- **AND** 系统 SHALL NOT 创建脚本
