# script-agent-workspace Specification Delta

## MODIFIED Requirements

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

### Requirement: 前端实现前必须更新 Pencil 原型确认单一对话入口

系统 SHALL 在修改 `apps/video-agent` 前先更新视频工作台 Pencil 原型，展示脚本生成和脚本修改共用单一脚本 Agent 对话入口。

#### Scenario: 原型覆盖对话式脚本生成

- **GIVEN** 开发者准备实现对话式脚本生成前端
- **WHEN** 更新 `docs/prototypes/video-agent/video-agent.pen`
- **THEN** 原型 SHALL 展示无脚本时的对话式脚本生成状态
- **AND** 原型 SHALL 展示已有脚本时的对话式修改状态
- **AND** 原型 SHALL 展示生成参数不足时的 Agent 追问状态
- **AND** 原型 SHALL 展示生成成功后打开时间轴详情的状态
- **AND** 用户确认原型后 SHALL 进入前端编码
