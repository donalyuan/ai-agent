# script-agent-workspace Specification Delta

## ADDED Requirements

### Requirement: 前端实现前必须完成设计上下文与 Pencil 原型

系统 SHALL 在实现脚本 Agent 工作台前完成设计上下文、真实设计系统参考和 `Pencil MCP` 原型确认，避免直接凭主观描述进入编码。

#### Scenario: 生成工作台实现计划前完成设计上下文

- **GIVEN** 仓库尚无项目根 `DESIGN.md`
- **WHEN** 开发者准备实现脚本 Agent 工作台
- **THEN** 系统 SHALL 先创建项目根 `DESIGN.md`
- **AND** `DESIGN.md` SHALL 定义 `AI-AGENT` 工作台的颜色、字体、间距、按钮、表单、列表、状态标签和响应式规则
- **AND** `DESIGN.md` SHALL 明确工作台展示名为“智能体工作台”，并预留选题、脚本、素材、视频、发布、优化六个智能体菜单入口
- **AND** 设计记录 SHALL 明确参考 `Ant Design`、`IBM Carbon` 和 `GitHub Primer` 的哪些模式

#### Scenario: Pencil 原型确认后才能编码

- **GIVEN** 脚本 Agent 工作台涉及新增或修改前端页面
- **WHEN** 开发者准备修改 `admin/` 代码
- **THEN** 系统 SHALL 先通过 `Pencil MCP` 输出桌面工作台原型
- **AND** 原型 SHALL 展示 `AI-AGENT` / “智能体工作台”标题和六个智能体菜单入口
- **AND** 原型 SHALL 覆盖无项目、无脚本、生成中、生成失败和状态更新失败状态
- **AND** 用户确认原型后 SHALL 进入编码
- **AND** 若当前环境没有 `Pencil MCP`，系统 SHALL 暂停实现并等待用户明确批准替代方案

### Requirement: 工作台必须支持项目选择

系统 SHALL 提供项目 API 和前端项目选择能力，使脚本生成始终绑定真实存在的内容项目，而不得硬编码 `project_id`。脚本工作台 SHALL NOT 提供项目创建或项目管理入口。

#### Scenario: 操作者选择已有项目

- **GIVEN** 数据库中已有一个或多个项目
- **WHEN** 操作者打开工作台
- **THEN** 页面 SHALL 调用 `GET /api/projects` 获取项目列表
- **AND** 操作者 SHALL 能选择当前项目
- **AND** 页面 SHALL 基于当前项目加载脚本列表

### Requirement: 工作台必须生成并展示结构化脚本

系统 SHALL 在 `admin/` 中提供脚本生成表单和脚本详情视图，打通“生成脚本 -> 查看分镜”的浏览器闭环。

#### Scenario: 从选题生成脚本并打开详情

- **GIVEN** 操作者已经选中一个项目
- **WHEN** 操作者提交 `topic`、`style` 和 `scene_count`
- **THEN** 页面 SHALL 调用 `POST /api/scripts/generate`
- **AND** `scene_count` SHALL 支持 3 到 12 的整数
- **AND** 成功后 SHALL 将新脚本加入当前项目脚本列表
- **AND** 页面 SHALL 自动打开新脚本详情
- **AND** 详情 SHALL 展示标题、状态、总时长和有序分镜
- **AND** 详情 SHALL 使用时间轴对照视图展示分镜顺序、旁白和画面指令

#### Scenario: 查看已有脚本分镜

- **GIVEN** 当前项目存在脚本
- **WHEN** 操作者点击脚本列表项
- **THEN** 页面 SHALL 调用 `GET /api/scripts/:script_id`
- **AND** 页面 SHALL 展示每个分镜的 sequence、narration、visual_description、emotion 和 duration_sec
- **AND** 分镜 SHALL 按 sequence 升序展示
- **AND** 分镜 SHALL 在时间轴中表达顺序，并在对照区展示旁白与画面描述

### Requirement: 工作台必须支持脚本列表筛选与状态更新

系统 SHALL 支持按状态筛选脚本列表，并允许操作者更新当前脚本状态。

#### Scenario: 按状态筛选脚本

- **GIVEN** 当前项目存在多个不同状态的脚本
- **WHEN** 操作者选择 `draft`、`approved` 或 `archived` 状态筛选
- **THEN** 页面 SHALL 调用 `GET /api/projects/:project_id/scripts?status=<status>`
- **AND** 列表 SHALL 只展示匹配状态的脚本
- **AND** 页面 SHALL 保持当前项目不变

#### Scenario: 更新脚本状态

- **GIVEN** 操作者已经打开一个脚本详情
- **WHEN** 操作者将状态更新为 `approved` 或 `archived`
- **THEN** 页面 SHALL 调用 `PUT /api/scripts/:script_id/status`
- **AND** 成功后 SHALL 同步详情状态
- **AND** 成功后 SHALL 同步脚本列表中的状态标签

### Requirement: 工作台必须提供完整的加载、空和错误状态

系统 SHALL 对脚本 Agent 工作台的关键异步流程提供明确状态反馈，避免用户误判操作是否生效。

#### Scenario: API 不可用

- **GIVEN** `novex-api` 不可用或 `/health` 返回失败
- **WHEN** 操作者打开工作台
- **THEN** 页面 SHALL 显示服务不可用状态
- **AND** 页面 SHALL 禁用生成脚本和状态更新等写操作
- **AND** 页面 SHALL 保留基础布局，不显示崩溃堆栈

#### Scenario: 生成脚本失败

- **GIVEN** 操作者已经提交脚本生成请求
- **WHEN** 后端返回 4xx 或 5xx 错误
- **THEN** 页面 SHALL 在生成面板显示错误信息
- **AND** 页面 SHALL 保留操作者已输入的 topic、style 和 scene_count
- **AND** 页面 SHALL 允许操作者修改后重试
