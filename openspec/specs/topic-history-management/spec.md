# topic-history-management Specification

## Purpose
TBD - created by archiving change improve-topic-history-management. Update Purpose after archive.
## Requirements
### Requirement: 系统必须提供历史生成独立列表页

`apps/video-agent` SHALL 在内容策略模块中提供历史生成独立列表页或独立二级视图，使操作者可以集中查看和管理选题 Agent 的生成批次。

#### Scenario: 查看历史生成列表

- **GIVEN** 操作者已选择一个项目
- **AND** 该项目存在多个成功且仍有可见选题的生成批次
- **WHEN** 操作者进入历史生成列表页
- **THEN** 页面 SHALL 按原始批次展示主题组入口
- **AND** 左侧历史列表 SHALL NOT 将补充批次展示为独立主题入口
- **AND** 每个主题组入口 SHALL 展示原始生成要求、原始生成时间、主题组选题数量和生成状态
- **AND** 补充批次 SHALL 仅在其所属原始主题组的关联补充批次区域展示
- **AND** 页面 SHALL 使用三列结构展示历史生成批次、当前主题选题和补充操作
- **AND** 页面 SHALL NOT 展示其他项目的生成批次

#### Scenario: 查看批次详情

- **GIVEN** 操作者位于历史生成列表页
- **WHEN** 操作者选择一个生成批次
- **THEN** 页面 SHALL 展示该批次所属主题组下未软删除的选题列表
- **AND** 主题组 SHALL 包含原始批次和其关联补充批次的选题
- **AND** 选题列表 SHALL 展示标题、来源、内容类型、评分、状态和关键标签
- **AND** 选题列表 SHALL 标识每条选题来自原始生成或补充生成
- **AND** 操作者 SHALL 可以从批次详情选择选题查看完整详情
- **AND** 当前选题池 SHALL 同步该主题组作为当前过滤条件

#### Scenario: 查看原始批次的补充批次

- **GIVEN** 操作者位于历史生成列表页
- **AND** 某原始批次存在一个或多个补充批次
- **WHEN** 操作者选择该原始批次
- **THEN** 页面 SHALL 展示该原始批次关联的补充批次列表
- **AND** 每个补充批次 SHALL 展示补充要求、生成时间、请求数量和当前可见选题数量
- **AND** 操作者 SHALL 可以切换到任一补充批次查看其选题

#### Scenario: 从历史批次补充选题

- **GIVEN** 操作者位于历史生成批次详情
- **AND** 当前批次可作为补充目标
- **WHEN** 操作者提交补充要求和生成数量
- **THEN** 页面 SHALL 调用选题 Agent 创建补充批次
- **AND** 补充成功后页面 SHALL 刷新历史批次列表和统计
- **AND** 页面 SHALL 保持选中该补充批次所属原始主题组
- **AND** 页面 SHALL 在同一主题组中同时展示原始批次和该补充批次生成的选题
- **AND** 页面 SHALL 在关联补充批次区域展示新创建的补充批次

#### Scenario: 返回当前选题池

- **GIVEN** 操作者位于历史生成列表页
- **WHEN** 操作者点击返回当前选题池入口
- **THEN** 页面 SHALL 回到内容策略主视图
- **AND** 当前项目选择 SHALL 保持不变
- **AND** 若操作者已在历史生成列表页选择批次，当前选题池 SHALL 展示该批次所属主题组的选题内容

### Requirement: 历史生成页必须支持批次内选题管理

历史生成页 SHALL 允许操作者对批次内未生成脚本的选题执行管理移除，并对不可删除选题给出明确状态。

#### Scenario: 移除未生成脚本选题

- **GIVEN** 批次详情中存在一条未生成脚本且未软删除的选题
- **WHEN** 操作者执行删除或移除操作并确认
- **THEN** 系统 SHALL 软删除该选题
- **AND** 页面 SHALL 从批次详情中移除该选题
- **AND** 批次的当前可见选题数量 SHALL 随之更新

#### Scenario: 已生成脚本选题不可删除

- **GIVEN** 批次详情中存在一条已生成脚本的选题
- **WHEN** 页面展示该选题的行操作
- **THEN** 页面 SHALL NOT 提供删除操作
- **AND** 页面 SHALL 展示该选题已生成脚本或不可删除的状态说明

#### Scenario: 批次可见选题清空后退出历史列表

- **GIVEN** 某个历史生成批次下所有选题都已被软删除
- **WHEN** 操作者刷新历史生成列表
- **THEN** 页面 SHALL NOT 再展示该批次作为可管理历史入口
