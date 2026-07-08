## MODIFIED Requirements

### Requirement: 系统必须支持主题组选题评审快照

系统 SHALL 为内容策略主题组提供 AI 评审快照，使操作者可以在同一主题组内快速识别优先、备选、淘汰和疑似重复选题。评审快照 SHALL 只作为决策辅助，不得自动修改 `ContentTopic.status`。

#### Scenario: 主题组脚本优先级复用评审快照

- **GIVEN** 某主题组存在最新成功评审快照
- **WHEN** 系统计算主题组脚本产出优先级
- **THEN** 系统 SHALL 复用该评审快照中的推荐层级、风险标记和相似选题引用
- **AND** 系统 SHALL 使用确定性规则计算排名指标和推荐候选
- **AND** 系统 SHALL NOT 在历史列表加载时额外调用 AI 重新排名
- **AND** 系统 SHALL NOT 因排名计算修改任何 `content_topics` 记录
