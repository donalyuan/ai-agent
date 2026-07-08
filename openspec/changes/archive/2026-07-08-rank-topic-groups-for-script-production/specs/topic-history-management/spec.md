## MODIFIED Requirements

### Requirement: 系统必须提供历史生成独立列表页

`apps/video-agent` SHALL 在内容策略模块中提供历史生成独立列表页或独立二级视图，使操作者可以集中查看和管理选题 Agent 的生成批次。

#### Scenario: 按脚本产出优先级查看历史主题组

- **GIVEN** 操作者已选择一个项目
- **AND** 该项目存在多个成功且仍有可见选题的原始生成批次
- **WHEN** 操作者进入历史生成列表页
- **THEN** 页面 SHALL 默认按主题组脚本产出优先级展示历史主题组
- **AND** 补充批次 SHALL 归入其所属原始主题组参与计算
- **AND** 页面 SHALL NOT 将补充批次展示为独立排名对象
- **AND** 每个主题组入口 SHALL 展示脚本优先级状态、可解释分数或待评审状态、推荐候选数量和主要风险摘要
- **AND** 页面 SHALL 提供切换回按时间排序的入口

#### Scenario: 未评审主题组不得排为高优先级

- **GIVEN** 某主题组不存在成功主题组评审快照
- **WHEN** 系统计算历史主题组脚本产出优先级
- **THEN** 该主题组 SHALL 标记为 `needs_review`
- **AND** 系统 SHALL NOT 将该主题组展示为 `ready_for_script`
- **AND** 页面 SHALL 引导操作者手动触发主题组评审

#### Scenario: 过期评审不得排为高优先级

- **GIVEN** 某主题组存在成功主题组评审快照
- **AND** 快照中的选题集合与当前未软删除选题集合不一致
- **WHEN** 系统计算历史主题组脚本产出优先级
- **THEN** 该主题组 SHALL 标记为评审过期或需重新评审
- **AND** 系统 SHALL NOT 将该主题组展示为 `ready_for_script`
- **AND** 页面 SHALL 引导操作者重新评审当前主题组

#### Scenario: 高优先主题组推荐脚本候选

- **GIVEN** 某主题组存在最新且覆盖当前选题集合的成功评审快照
- **AND** 该主题组内存在优先推荐、无明显风险且未软删除的选题
- **WHEN** 系统计算脚本产出优先级
- **THEN** 该主题组 SHALL 可以标记为 `ready_for_script`
- **AND** 系统 SHALL 返回最多 3 个推荐脚本候选选题
- **AND** 推荐候选 SHALL 属于当前主题组
- **AND** 推荐候选 SHALL NOT 包含 `duplicate`、`hard_to_script`、`off_positioning`、`too_generic` 或 `compliance_risk` 风险标记

#### Scenario: 排名不改变选题生命周期

- **GIVEN** 项目下存在多个主题组和选题
- **WHEN** 系统计算或展示脚本产出优先级排名
- **THEN** 系统 SHALL NOT 修改任何 `ContentTopic.status`
- **AND** 系统 SHALL NOT 自动确认、归档、删除选题或生成脚本
- **AND** 现有确认、归档、软删除和从选题生成脚本的人工操作 SHALL 保持可用
