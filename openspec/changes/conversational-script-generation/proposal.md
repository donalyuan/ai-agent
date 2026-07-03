# conversational-script-generation Proposal

## 背景

`conversational-agent-runtime` 已让脚本 Agent 支持对已有脚本做对话式分镜修改，但脚本创建仍保留独立表单：操作者需要在“生成脚本”面板填写 `topic`、`style`、`scene_count`，而修改脚本则在“脚本 Agent 对话”面板输入自然语言。

这会把同一个脚本 Agent 的能力拆成两个入口，也让右侧出现两个相似输入区。用户已确认新的产品约定：脚本生成也应走对话 Agent，生成脚本和修改脚本都应由同一个脚本 Agent 对话入口承载。

## 目标

1. 让脚本 Agent 对话支持“无脚本会话”的新脚本生成。
2. 统一脚本创作入口：无脚本时通过对话生成脚本，有脚本时通过同一对话修改脚本。
3. 前端移除右侧并列的大型生成表单，不再让“生成输入框”和“对话输入框”同时抢占主操作区。
4. 保留 `topic`、`style`、`scene_count` 的业务约束，但改由 Agent 对话解析、追问或快捷控件收集。
5. 继续复用通用 Agent Runtime、`agent_conversations`、`agent_messages`、`agent_runs`、`agent_steps`，不新增孤立聊天或表单专用路径。

## 非目标

1. 不实现完整选题池、`topic_id` 或内容策略模块；当前生成仍使用用户提供的选题文本或对话中解析出的选题文本。
2. 不移除后端 `POST /api/scripts/generate` 兼容接口；本 change 只要求视频工作台前端不再以独立大表单作为主入口。
3. 不实现所有 Agent 的对话生成能力；本 change 只扩展脚本 Agent。
4. 不引入完整 MCP Gateway、工具权限审批或模型路由后台。
5. 不覆盖移动端原型、移动端适配或移动端验收。

## 影响范围

- 后端脚本 Agent Runtime adapter 支持 `subject_type` / `subject_id` 为空的脚本会话。
- 脚本 Agent adapter 增加“生成脚本”意图，复用现有脚本生成服务和结构化脚本解析能力。
- 对话 turn 响应需要让前端知道本轮是否创建了新脚本，以及新脚本 ID。
- `apps/video-agent` 右侧操作区改为单一脚本 Agent 对话面板；无脚本状态下对话面板用于生成脚本。
- Pencil 原型 `docs/prototypes/video-agent/video-agent.pen` 必须先更新并获得确认，再进入前端编码。
