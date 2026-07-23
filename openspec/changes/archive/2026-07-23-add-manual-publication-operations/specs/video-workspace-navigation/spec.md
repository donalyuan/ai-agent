# video-workspace-navigation Delta Specification

## ADDED Requirements

### Requirement: 发布运营必须提供可用的发布工作台入口

系统 SHALL 通过数据库菜单配置启用一级菜单“发布运营”，并提供二级页面“发布工作台”路由到 `/publishing/workbench`；该入口 SHALL 替换尚未实现的“发布排程”规划节点。

#### Scenario: 查询启用后的发布运营菜单

- **GIVEN** 人工发布运营菜单迁移已经执行
- **WHEN** 前端请求视频工作台菜单树
- **THEN** “发布运营” SHALL 为可见、可进入且状态为 active
- **AND** 其子菜单 SHALL 包含“发布工作台”
- **AND** “发布工作台” SHALL 使用模块键 `publishing.workbench` 和路由 `/publishing/workbench`

#### Scenario: 从作品库进入发布工作台

- **GIVEN** 操作者已为选定完成版本创建发布交接和发布计划
- **WHEN** 前端导航到该计划
- **THEN** 共享工作台骨架 SHALL 保持完整七个一级业务菜单
- **AND** “发布运营 / 发布工作台” SHALL 处于选中状态
- **AND** 页面 SHALL 加载明确发布计划而非默认选择其他作品版本
