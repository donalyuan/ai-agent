# script-agent-workspace Specification Delta

## MODIFIED Requirements

### Requirement: 前端实现前必须完成设计上下文与 Pencil 原型

系统 SHALL 在实现或迁移脚本 Agent 工作台前完成设计上下文、真实设计系统参考和 `Pencil MCP` 原型确认，避免直接凭主观描述进入编码。正式视频生产工作台的实现边界 SHALL 为 `apps/video-agent/`，并且工作台一级导航 SHALL 使用视频生产业务流程菜单，而不是只按 Agent 预留入口组织。

#### Scenario: 生成工作台实现计划前完成设计上下文

- **GIVEN** 仓库尚无项目根 `DESIGN.md`，或视频工作台导航结构发生变化
- **WHEN** 开发者准备实现或迁移脚本 Agent 工作台
- **THEN** 系统 SHALL 先创建或更新项目根 `DESIGN.md`
- **AND** `DESIGN.md` SHALL 定义 `VEDIO-AGENT` 工作台的颜色、字体、间距、按钮、表单、列表、状态标签和响应式规则
- **AND** `DESIGN.md` SHALL 明确工作台展示名为“视频工作台”
- **AND** `DESIGN.md` SHALL 明确一级导航为内容策略、脚本创作、素材管理、作品生产、发布运营、数据分析、工作流任务
- **AND** `DESIGN.md` SHALL 明确底层 Agent 状态只能作为二级菜单、模块状态或执行状态展示，不得替代一级业务菜单
- **AND** `DESIGN.md` SHALL 明确正式实现归属 `apps/video-agent/`
- **AND** 设计记录 SHALL 明确参考 `Ant Design`、`IBM Carbon` 和 `GitHub Primer` 的哪些模式

#### Scenario: Pencil 原型确认后才能编码

- **GIVEN** 脚本 Agent 工作台涉及新增、迁移或修改前端页面
- **WHEN** 开发者准备修改 `apps/video-agent/` 中的工作台代码
- **THEN** 系统 SHALL 先通过 `Pencil MCP` 输出桌面工作台原型，或复用已由用户确认且未改变交互范围的桌面原型
- **AND** 视频工作台 Pencil 原型源文件 SHALL 保存为 `docs/prototypes/video-agent/video-agent.pen`
- **AND** 后续视频工作台原型修改 SHALL 更新 `docs/prototypes/video-agent/video-agent.pen`，而不是使用 `docs/prototypes/script-agent-workspace/` 截图目录
- **AND** 原型 SHALL 展示 `VEDIO-AGENT` / “视频工作台”标题
- **AND** 原型 SHALL 展示内容策略、脚本创作、素材管理、作品生产、发布运营、数据分析、工作流任务七个一级业务菜单
- **AND** 原型 SHALL 覆盖无项目、无脚本、生成中、生成失败和状态更新失败状态
- **AND** 用户确认原型后 SHALL 进入编码
- **AND** 若当前环境没有 `Pencil MCP`，系统 SHALL 暂停实现并等待用户明确批准替代方案
