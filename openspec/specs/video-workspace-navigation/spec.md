# video-workspace-navigation Specification

## Purpose
TBD - created by archiving change persist-video-workspace-menu-hierarchy. Update Purpose after archive.
## Requirements
### Requirement: 视频工作台菜单必须由数据库持久化配置驱动

系统 SHALL 使用数据库中的菜单配置作为 `apps/video-agent` 工作台导航的单一来源，支持业务菜单分级、排序、启用、隐藏、禁用、模块归属和 Agent 关联。

#### Scenario: 菜单表保存业务菜单层级

- **GIVEN** 数据库迁移已经执行
- **WHEN** 系统初始化视频工作台菜单
- **THEN** 数据库 SHALL 存在 `video_workspace_menus` 表
- **AND** 表 SHALL 支持 `parent_id` 表达父子层级
- **AND** 表 SHALL 支持 `menu_key`、`label`、`description`、`route_path`、`icon`、`menu_type`、`module_key`、`agent_key`、`sort_order`、`is_enabled`、`is_visible`、`status`、`metadata`、`created_at` 和 `updated_at`
- **AND** `menu_key` SHALL 在表内唯一
- **AND** 菜单 SHALL 能通过 `sort_order` 在同级内稳定排序

#### Scenario: 初始化七个一级业务菜单

- **GIVEN** 数据库迁移已经执行
- **WHEN** 系统查询 `video_workspace_menus` 中 `parent_id IS NULL` 的菜单
- **THEN** 结果 SHALL 包含且仅按顺序包含以下一级业务菜单：内容策略、脚本创作、素材管理、作品生产、发布运营、数据分析、工作流任务
- **AND** `脚本创作` SHALL 默认 `is_enabled = true` 且 `status = active`
- **AND** 其他尚未实现的一级菜单 SHALL 默认可见但不可进入，状态 SHALL 为 `planned`

#### Scenario: 脚本创作承载当前脚本闭环

- **GIVEN** 菜单种子数据已经写入
- **WHEN** 系统查询 `脚本创作` 的子菜单
- **THEN** 子菜单 SHALL 至少包含当前脚本生成闭环入口
- **AND** 该入口 SHALL 关联脚本生成模块或 `script-generation-agent`
- **AND** 当前“生成脚本 -> 查看分镜 -> 更新时间轴详情/状态”的能力 SHALL 继续归属 `脚本创作`

### Requirement: 后端必须提供树形菜单读取接口

系统 SHALL 提供 `GET /api/video-workspace/menus`，返回视频工作台可见菜单树，供 `apps/video-agent` 渲染导航。

#### Scenario: 获取可见菜单树

- **GIVEN** 数据库中存在视频工作台菜单数据
- **WHEN** 前端请求 `GET /api/video-workspace/menus`
- **THEN** API SHALL 返回 HTTP 200
- **AND** 响应体 SHALL 包含 `menus` 数组
- **AND** 每个菜单节点 SHALL 包含 `menu_id`、`menu_key`、`label`、`description`、`route_path`、`icon`、`menu_type`、`module_key`、`agent_key`、`sort_order`、`is_enabled`、`status`、`metadata` 和 `children`
- **AND** API SHALL 按父子层级返回树形结构
- **AND** 同级菜单 SHALL 按 `sort_order` 升序返回

#### Scenario: 隐藏菜单不返回

- **GIVEN** 数据库中存在 `is_visible = false` 的菜单
- **WHEN** 前端请求 `GET /api/video-workspace/menus`
- **THEN** API 响应 SHALL NOT 包含该隐藏菜单
- **AND** API 响应 SHALL NOT 包含该隐藏菜单的不可见子树

#### Scenario: 禁用菜单仍返回状态

- **GIVEN** 数据库中存在 `is_enabled = false` 且 `is_visible = true` 的菜单
- **WHEN** 前端请求 `GET /api/video-workspace/menus`
- **THEN** API 响应 SHALL 包含该菜单
- **AND** 该菜单节点 SHALL 返回 `is_enabled = false`
- **AND** 该菜单节点 SHALL 返回 `status` 以便前端展示计划态或禁用态

### Requirement: 前端必须按业务菜单树渲染视频工作台导航

系统 SHALL 在 `apps/video-agent` 中使用菜单 API 渲染视频工作台导航，不得继续将 6 个 Agent 作为一级菜单硬编码。

#### Scenario: 首屏展示业务一级菜单

- **GIVEN** `GET /api/video-workspace/menus` 返回 7 个一级业务菜单
- **WHEN** 操作者打开 `apps/video-agent` 工作台
- **THEN** 左侧导航 SHALL 展示内容策略、脚本创作、素材管理、作品生产、发布运营、数据分析、工作流任务
- **AND** 页面 SHALL 默认选中 `脚本创作`
- **AND** 页面 SHALL NOT 将选题智能体、脚本智能体、素材智能体、视频智能体、发布智能体、优化智能体作为一级导航展示

#### Scenario: 禁用菜单不可进入

- **GIVEN** API 返回某个菜单 `is_enabled = false`
- **WHEN** 操作者点击该菜单
- **THEN** 页面 SHALL 保持当前可用视图不变
- **AND** 该菜单 SHALL 以禁用态展示
- **AND** 页面 SHALL 不发起该菜单对应业务数据加载请求

#### Scenario: 菜单加载失败不回退硬编码菜单

- **GIVEN** 菜单 API 返回错误或网络失败
- **WHEN** 操作者打开视频工作台
- **THEN** 页面 SHALL 展示菜单加载失败状态
- **AND** 页面 SHALL 保留 `VEDIO-AGENT / 视频工作台` 壳层
- **AND** 页面 SHALL NOT 使用前端硬编码 Agent 数组恢复导航

#### Scenario: 脚本创作视图保持现有生产闭环

- **GIVEN** 操作者位于 `脚本创作` 菜单
- **WHEN** 操作者生成脚本、打开已有脚本或更新脚本状态
- **THEN** 页面 SHALL 继续调用现有脚本 API
- **AND** 页面 SHALL 继续展示时间轴对照详情
- **AND** 页面 SHALL 继续支持 3 到 12 个分镜
