# conversational-script-generation Design

## DDD

脚本 Agent 的对话会话分为两种状态，但仍使用同一个 `Conversation` 模型：

- **未绑定脚本会话**：`agent_type=script`，`project_id` 必填，`subject_type` 和 `subject_id` 为空。用于从自然语言生成新脚本。
- **已绑定脚本会话**：`agent_type=script`，`project_id` 必填，`subject_type=script`，`subject_id=<script_id>`。用于继续修改当前脚本。

脚本 Agent adapter 负责识别本轮用户意图：

1. 当前会话没有绑定脚本时，只允许生成脚本或追问缺失信息。
2. 当前会话已绑定脚本时，默认把自然语言理解为修改当前脚本；如果用户明确要求“新建/重新生成一条脚本”，则创建新的未绑定会话或显式重置当前会话，避免误把新脚本需求写到旧脚本。
3. 生成脚本所需业务参数仍是 `topic`、`style`、`scene_count`；参数可以来自用户消息、对话历史、快捷控件 metadata，或 Agent 的追问。

生成成功后，系统将新脚本落库，并把会话绑定到新脚本：`subject_type=script`、`subject_id=<new_script_id>`。Agent 回复 metadata 必须包含 `script_id`，前端据此刷新脚本列表和打开脚本详情。

## BDD

无脚本时，操作者在脚本 Agent 对话框输入“帮我生成一个关于 ChatGPT 工作流的 6 镜知识科普脚本”。系统创建或复用一个未绑定脚本会话，保存用户消息，调用脚本 Agent adapter 解析生成参数，调用现有脚本生成能力生成结构化脚本，保存脚本和分镜，保存 Agent 回复，返回新脚本 ID。前端展示 Agent 回复，刷新脚本列表，并自动打开新脚本详情。

如果用户只说“帮我生成脚本”，缺少选题、风格或分镜数，Agent 不应伪造参数生成脚本。系统应保存一条追问式 Agent 回复，例如询问选题、风格或分镜数；本轮不创建脚本。

已有脚本时，操作者继续输入“把第 2 镜改得更有冲突感”，系统保持现有对话式分镜修改路径，刷新当前脚本时间轴。

错误场景：API 不可用、生成失败、LLM 输出非法、缺少项目、项目不存在、脚本创建成功但绑定会话失败，都必须返回稳定错误并记录失败 run。前端错误只显示在脚本 Agent 对话面板内，不影响脚本列表和已打开详情浏览。

## SDD

后端延续现有接口：

- `POST /api/agent/conversations`：允许创建 `agent_type=script`、`project_id` 必填、`subject_type` / `subject_id` 为空的会话。
- `POST /api/agent/conversations/:conversation_id/messages`：Runtime 根据会话是否绑定脚本选择“生成脚本”或“修改脚本”路径。

对话响应 DTO 扩展：

- `AgentTurnResponse` 保留 `user_message`、`assistant_message`、`run`。
- `assistant_message.metadata` 中增加稳定业务字段：`intent`、`script_id`、`script_created`、`needs_input`、`missing_fields`。
- 若本轮创建新脚本，响应可额外返回 `script` 或由前端根据 `script_id` 调 `GET /api/scripts/:script_id` 获取详情；实现计划应优先选择一种，并在测试中固定。

脚本生成参数提取：

- 第一版不做复杂 NLU 框架，不新增独立意图服务。
- Adapter 通过 LLM 输出结构化 JSON：`intent`、`topic`、`style`、`scene_count`、`reply`、`missing_fields`。
- `style` 只允许 `knowledge`、`story`、`tutorial`。
- `scene_count` 只允许 3 到 12。
- 当 `missing_fields` 非空时，Agent 只回复追问，不调用脚本生成。

前端改造：

- `apps/video-agent/app/page.tsx` 移除右侧常驻 `GeneratePanel`。
- 右侧保留一个 `ScriptAgentConversationPanel`，无脚本时显示“描述你想生成的脚本”，已选脚本时显示“修改当前脚本”。
- 风格和分镜数可以作为对话面板内的紧凑快捷控件存在，但不得恢复为独立生成大表单。
- 生成成功后刷新脚本列表、打开新脚本详情，并将当前会话绑定到新脚本。
- 切换脚本时仍要避免旧会话迟到响应污染当前面板。

## TDD

后端先补失败测试：

- 创建未绑定脚本会话成功。
- 对未绑定脚本会话发送完整生成请求，生成脚本、保存消息、记录 run，并返回 `script_id`。
- 缺少必要字段时返回追问回复，不创建脚本。
- 生成失败时记录 failed run，前端可收到稳定错误。

前端先补失败测试：

- 页面不再同时展示独立“生成脚本”表单和脚本 Agent 对话输入。
- 无脚本时脚本 Agent 对话面板可发送生成请求。
- 生成成功后刷新脚本列表并打开新脚本详情。
- 已有脚本时同一面板继续支持对话式修改分镜。
- E2E 覆盖一次从无脚本对话生成脚本到时间轴展示的流程。

实现完成后运行前端 Vitest、Playwright E2E、后端 `cargo test --workspace` 和 OpenSpec 验证。
