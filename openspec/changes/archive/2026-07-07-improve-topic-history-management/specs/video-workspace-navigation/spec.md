## MODIFIED Requirements

### Requirement: 视频工作台菜单必须由数据库持久化配置驱动

系统 SHALL 使用数据库中的菜单配置作为 `apps/video-agent` 工作台导航的单一来源，支持业务菜单分级、排序、启用、隐藏、禁用、模块归属和 Agent 关联。

#### Scenario: 初始化七个一级业务菜单

- **GIVEN** 数据库迁移已经执行
- **WHEN** 系统查询 `video_workspace_menus` 中 `parent_id IS NULL` 的菜单
- **THEN** 结果 SHALL 包含且仅按顺序包含以下一级业务菜单：内容策略、脚本创作、素材管理、作品生产、发布运营、数据分析、工作流任务
- **AND** `内容策略` SHALL 默认 `is_enabled = true` 且 `status = active`
- **AND** `脚本创作` SHALL 默认 `is_enabled = true` 且 `status = active`
- **AND** 其他尚未实现的一级菜单 SHALL 默认可见但不可进入，状态 SHALL 为 `planned`

#### Scenario: 内容策略承载当前选题池闭环

- **GIVEN** 菜单种子数据已经写入
- **WHEN** 系统查询 `内容策略` 的子菜单
- **THEN** 子菜单 SHALL 至少包含当前选题池入口
- **AND** 该入口 SHALL 关联选题池模块或 `topic-generation-agent`
- **AND** 当前“生成选题 -> 确认选题 -> 从选题生成脚本”的能力 SHALL 归属 `内容策略`

#### Scenario: 内容策略承载历史生成管理入口

- **GIVEN** 菜单种子数据已经写入
- **WHEN** 系统查询 `内容策略` 的子菜单
- **THEN** 结果 SHALL 包含历史生成管理入口
- **AND** 该入口 SHALL 指向内容策略模块下的历史生成列表页或独立二级视图
- **AND** 该入口 SHALL 排在当前选题池入口上方
- **AND** 该入口 SHALL 不改变内容策略作为一级业务菜单的归属
