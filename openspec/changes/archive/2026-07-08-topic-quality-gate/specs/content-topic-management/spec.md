## ADDED Requirements

### Requirement: 系统必须在选题入库前执行质量闸门

系统 SHALL 在 `topic` Agent 生成候选选题后、写入 `content_topics` 前执行质量评估。质量评估 SHALL 判断候选是否贴合项目定位、是否具体、是否重复、是否可脚本化、是否存在风险，以及评分是否可信。

#### Scenario: 生成候选必须先通过质量闸门

- **GIVEN** 操作者在内容策略页提交选题生成要求
- **WHEN** `topic` Agent 生成候选选题
- **THEN** 系统 SHALL 在写入 `content_topics` 前评估每条候选质量
- **AND** 系统 SHALL 只将质量闸门通过的候选写入 `content_topics`
- **AND** 写入的选题 SHALL 保持 `idea` 状态
- **AND** 系统 SHALL NOT 为淘汰候选创建 `content_topics` 记录

#### Scenario: 质量闸门必须记录评估快照

- **GIVEN** 系统完成一次候选质量评估
- **WHEN** 评估输出可解析且通过后端校验
- **THEN** 系统 SHALL 保存一条 `topic_quality_evaluations` 记录
- **AND** 评估记录 SHALL 关联 `project_id`、`batch_id` 和 `source_run_id`
- **AND** 评估结果 SHALL 包含批次摘要、每条候选的质量分、决策、风险标记和原因
- **AND** 通过项写入 `content_topics` 时 SHALL 在 `metadata` 中保存质量摘要

#### Scenario: 低通过率触发最多一次自动重写

- **GIVEN** 首轮候选质量评估完成
- **AND** 通过率低于系统阈值
- **WHEN** 本轮生成尚未触发过自动重写
- **THEN** 系统 SHALL 基于淘汰原因自动请求 `topic` Agent 重写候选
- **AND** 系统 SHALL 再次执行质量评估
- **AND** 系统 SHALL NOT 在同一轮生成中触发第二次自动重写

#### Scenario: 质量评估失败不污染选题池

- **GIVEN** `topic` Agent 已生成候选选题
- **WHEN** 质量评估调用失败、输出无法解析、缺少必填字段或包含非法枚举
- **THEN** 系统 SHALL 标记生成批次为 `failed`
- **AND** 系统 SHALL 保存失败原因
- **AND** 系统 SHALL NOT 写入任何候选到 `content_topics`

#### Scenario: 重写后仍低质时只入库通过项

- **GIVEN** 系统已触发一次自动重写
- **WHEN** 重写后的质量评估存在通过项和淘汰项
- **THEN** 系统 SHALL 只将通过项写入 `content_topics`
- **AND** 系统 SHALL 在质量评估快照中保留淘汰项和原因
- **AND** 淘汰项 SHALL NOT 出现在当前选题池

#### Scenario: 重写后没有通过项时批次失败

- **GIVEN** 系统已触发一次自动重写
- **WHEN** 重写后的质量评估没有任何通过项
- **THEN** 系统 SHALL 标记生成批次为 `failed`
- **AND** 系统 SHALL NOT 写入任何候选到 `content_topics`
- **AND** Agent 回复 SHALL 说明本批次未产生可用选题

#### Scenario: 补充批次必须经过同一质量闸门

- **GIVEN** 操作者对历史主题组请求补充选题
- **WHEN** `topic` Agent 生成补充候选
- **THEN** 系统 SHALL 将同主题组已有可见选题作为质量评估上下文
- **AND** 系统 SHALL 使用同一质量闸门评估补充候选
- **AND** 系统 SHALL 避免把重复已有主题组选题的候选写入 `content_topics`

#### Scenario: 查询批次质量报告

- **GIVEN** 某生成批次存在质量评估快照
- **WHEN** 操作者打开该生成批次的质量报告
- **THEN** 系统 SHALL 返回该批次最新质量评估
- **AND** 响应 SHALL 包含通过数量、淘汰数量、是否重写、质量摘要和候选明细
- **AND** 系统 SHALL NOT 返回其他项目或其他批次的质量评估

#### Scenario: 质量闸门不得改变选题生命周期

- **GIVEN** 质量闸门完成评估
- **WHEN** 系统写入通过候选
- **THEN** 通过候选 SHALL 仍以 `idea` 状态进入选题池
- **AND** 系统 SHALL NOT 自动确认、归档、删除选题或生成脚本
- **AND** 系统 SHALL NOT 新增 `quality_rejected` 作为 `ContentTopic.status`
