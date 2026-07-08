## ADDED Requirements

### Requirement: 系统必须支持主题组选题评审快照

系统 SHALL 为内容策略主题组提供 AI 评审快照，使操作者可以在同一主题组内快速识别优先、备选、淘汰和疑似重复选题。评审快照 SHALL 只作为决策辅助，不得自动修改 `ContentTopic.status`。

#### Scenario: 创建主题组评审快照

- **GIVEN** 项目下存在一个原始生成批次
- **AND** 该原始批次或其补充批次下存在未软删除选题
- **WHEN** 操作者请求评审该主题组
- **THEN** 系统 SHALL 读取原始批次和关联补充批次下所有未软删除选题
- **AND** 系统 SHALL 调用 AI 生成结构化评审结果
- **AND** 系统 SHALL 保存一条主题组评审快照
- **AND** 快照 SHALL 关联 `project_id` 和原始批次 `root_batch_id`
- **AND** 系统 SHALL NOT 修改任何选题的业务状态

#### Scenario: 评审结果必须提供分层和风险

- **GIVEN** AI 返回主题组评审结果
- **WHEN** 系统校验评审输出
- **THEN** 每条选题评审 SHALL 包含选题 ID、推荐层级、理由、风险标记和相似选题引用
- **AND** 推荐层级 SHALL 只允许 `priority`、`backup` 或 `reject`
- **AND** 风险标记 SHALL 只允许后端定义的稳定枚举
- **AND** 相似选题引用 SHALL 只允许指向同一主题组内的选题

#### Scenario: 评审失败不污染选题池

- **GIVEN** 操作者请求评审主题组
- **WHEN** AI 输出为空、无法解析、缺少必填字段、包含非法枚举或引用组外选题
- **THEN** 系统 SHALL 标记本次评审失败或返回明确错误
- **AND** 系统 SHALL NOT 保存成功评审快照
- **AND** 系统 SHALL NOT 修改任何 `content_topics` 记录

#### Scenario: 读取最新主题组评审

- **GIVEN** 某主题组存在多条成功评审快照
- **WHEN** 操作者打开该主题组
- **THEN** 系统 SHALL 返回该主题组最新成功评审快照
- **AND** 系统 SHALL NOT 返回其他项目或其他主题组的评审快照

#### Scenario: 自动评审默认不启用

- **GIVEN** 操作者通过选题 Agent 生成或补充选题
- **WHEN** 第一版自动评审开关未启用
- **THEN** 系统 SHALL NOT 自动创建主题组评审快照
- **AND** 操作者 SHALL 仍可手动触发评审
