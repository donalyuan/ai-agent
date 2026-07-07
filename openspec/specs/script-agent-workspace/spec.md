# script-agent-workspace Specification

## Purpose
定义 `apps/video-agent/` 中 `VEDIO-AGENT / 视频工作台` 的桌面端脚本智能体前端闭环，包括项目选择、脚本生成、脚本列表、时间轴对照详情、状态流转、异步状态反馈和设计确认约束。
## Requirements
### Requirement: 前端实现前必须完成设计上下文与 Pencil 原型

系统 SHALL 在实现或迁移脚本 Agent 工作台前完成设计上下文、真实设计系统参考和 `Pencil MCP` 原型确认，避免直接凭主观描述进入编码。正式视频生产工作台的实现边界 SHALL 为 `apps/video-agent/`，并且工作台一级导航 SHALL 使用视频生产业务流程菜单，而不是只按 Agent 预留入口组织。涉及脚本生成和脚本修改入口变更时，原型 SHALL 展示二者共用单一脚本 Agent 对话入口。

#### Scenario: 原型覆盖对话式脚本生成

- **GIVEN** 开发者准备实现对话式脚本生成前端
- **WHEN** 更新 `docs/prototypes/video-agent/video-agent.pen`
- **THEN** 原型 SHALL 展示无脚本时的对话式脚本生成状态
- **AND** 原型 SHALL 展示已有脚本时的对话式修改状态
- **AND** 原型 SHALL 展示生成参数不足时的 Agent 追问状态
- **AND** 原型 SHALL 展示生成成功后打开时间轴详情的状态
- **AND** 用户确认原型后 SHALL 进入前端编码

### Requirement: 工作台必须支持项目选择

系统 SHALL 在 `apps/video-agent/` 中提供项目 API 和前端项目选择能力，使脚本生成始终绑定真实存在的内容项目，而不得硬编码 `project_id`。脚本工作台 SHALL NOT 提供项目创建或项目管理入口。

#### Scenario: 操作者选择已有项目

- **GIVEN** 数据库中已有一个或多个项目
- **WHEN** 操作者打开工作台
- **THEN** 页面 SHALL 调用 `GET /api/projects` 获取项目列表
- **AND** 操作者 SHALL 能选择当前项目
- **AND** 页面 SHALL 基于当前项目加载脚本列表

### Requirement: 工作台必须生成并展示结构化脚本

系统 SHALL 在 `apps/video-agent/` 中提供脚本生成表单和脚本详情视图，打通“生成脚本 -> 查看分镜”的浏览器闭环。当前 `admin/` 中已实现的工作台 SHALL 作为迁移资产复用，而不得因为目录边界修正被废弃。

#### Scenario: 从选题生成脚本并打开详情

- **GIVEN** 操作者已经在 `apps/video-agent` 工作台选中一个项目
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

系统 SHALL 在 `apps/video-agent/` 中支持按状态筛选脚本列表，并允许操作者更新当前脚本状态。

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

系统 SHALL 对 `apps/video-agent/` 中脚本 Agent 工作台的关键异步流程提供明确状态反馈，避免用户误判操作是否生效。工作台 SHALL 只覆盖桌面端运营生产场景，不包含移动端原型、移动端适配或移动端验收。

#### Scenario: API 不可用

- **GIVEN** `novex-api` 不可用或 `/health` 返回失败
- **WHEN** 操作者打开 `apps/video-agent` 工作台
- **THEN** 页面 SHALL 显示服务不可用状态
- **AND** 页面 SHALL 禁用生成脚本和状态更新等写操作
- **AND** 页面 SHALL 保留基础布局，不显示崩溃堆栈

#### Scenario: 生成脚本失败

- **GIVEN** 操作者已经提交脚本生成请求
- **WHEN** 后端返回 4xx 或 5xx 错误
- **THEN** 页面 SHALL 在生成面板显示错误信息
- **AND** 页面 SHALL 保留操作者已输入的 topic、style 和 scene_count
- **AND** 页面 SHALL 允许操作者修改后重试

### Requirement: 脚本工作台必须使用单一脚本 Agent 对话入口

系统 SHALL 在 `apps/video-agent` 脚本创作工作台中使用单一脚本 Agent 对话入口承载脚本生成和脚本修改，不得在右侧同时保留独立“生成脚本”大表单和脚本 Agent 对话输入框。

#### Scenario: 无脚本时通过对话生成脚本

- **GIVEN** 操作者已选择一个项目
- **AND** 当前项目没有选中的脚本
- **WHEN** 操作者在脚本 Agent 对话面板输入“生成一个关于 ChatGPT 工作流的 6 镜知识科普脚本”并发送
- **THEN** 页面 SHALL 创建或复用未绑定脚本的 `script` Agent 会话
- **AND** 页面 SHALL 调用对话消息接口发送用户消息
- **AND** 成功后页面 SHALL 刷新脚本列表
- **AND** 页面 SHALL 自动打开新生成脚本详情
- **AND** 页面 SHALL 展示新脚本的时间轴对照视图

#### Scenario: 有脚本时通过同一对话修改脚本

- **GIVEN** 操作者已打开一个脚本详情
- **WHEN** 操作者在同一个脚本 Agent 对话面板输入“把第 2 镜改得更有冲突感”并发送
- **THEN** 页面 SHALL 复用当前脚本绑定的 `script` Agent 会话
- **AND** 页面 SHALL 调用对话消息接口发送用户消息
- **AND** 成功后页面 SHALL 刷新当前脚本详情
- **AND** 页面 SHALL NOT 创建独立生成表单流程

#### Scenario: 页面不得并列展示两个生成/对话输入区

- **GIVEN** 操作者打开 `apps/video-agent` 脚本创作工作台
- **WHEN** 页面完成加载
- **THEN** 右侧操作区 SHALL 只展示一个主要自然语言输入入口
- **AND** 页面 SHALL NOT 同时展示“生成脚本”的 `topic` textarea 和“脚本 Agent 对话”的修改 textarea
- **AND** 若需要 `style` 或 `scene_count`，页面 MAY 在对话面板内提供紧凑快捷控件或让 Agent 追问
- **AND** 页面 SHALL NOT 恢复为独立大表单生成入口

#### Scenario: 生成参数不足时在对话面板内追问

- **GIVEN** 操作者已选择一个项目
- **WHEN** 操作者只输入“帮我生成一个脚本”
- **THEN** 页面 SHALL 展示 Agent 的追问回复
- **AND** 页面 SHALL NOT 新增脚本列表项
- **AND** 页面 SHALL 保持对话输入可继续补充信息

#### Scenario: 对话生成失败只影响对话面板

- **GIVEN** 操作者已选择一个项目
- **WHEN** 对话式生成脚本失败
- **THEN** 页面 SHALL 在脚本 Agent 对话面板内展示错误
- **AND** 页面 SHALL 保留脚本列表和已打开脚本详情的浏览能力

### Requirement: 脚本创作必须展示来源选题

当脚本由已确认选题生成时，`apps/video-agent` 脚本创作页面 SHALL 展示脚本来源选题，并使用脚本保存的选题快照解释历史上下文。

#### Scenario: 查看由选题生成的脚本

- **GIVEN** 已存在一个由 `topic_id` 生成的脚本
- **WHEN** 操作者打开脚本详情
- **THEN** 页面 SHALL 展示来源选题标题和内容类型
- **AND** 页面 SHALL 基于 `topic_snapshot` 展示生成时的选题摘要
- **AND** 页面 SHALL NOT 因选题后续编辑而改变该脚本的历史快照展示

### Requirement: 内容策略页进入脚本创作前必须确认生成参数

系统 SHALL 在内容策略页从选题进入脚本创作前展示确认面板，使操作者确认 `style` 和 `scene_count` 后再生成脚本。

#### Scenario: approved 选题打开脚本确认面板

- **GIVEN** 操作者在内容策略页选中一条 `approved` 选题
- **WHEN** 操作者点击“生成脚本”
- **THEN** 页面 SHALL 请求 `POST /api/topics/:topic_id/prepare-script`
- **AND** 页面 SHALL 展示选题快照
- **AND** 页面 SHALL 允许操作者确认 `style` 和 `scene_count`
- **AND** 操作者确认后 SHALL 使用 `topic_id` 创建脚本

#### Scenario: archived 选题不能进入脚本创作

- **GIVEN** 操作者在内容策略页查看一条 `archived` 选题
- **WHEN** 页面展示选题操作
- **THEN** 页面 SHALL NOT 提供“生成脚本”主操作

