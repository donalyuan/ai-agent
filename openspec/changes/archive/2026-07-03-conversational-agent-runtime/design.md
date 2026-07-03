# conversational-agent-runtime Design

## DDD

核心领域概念分为四层：

- `Conversation`：一次可连续对话的 Agent 会话，绑定 `agent_type`、`project_id` 和可选业务资源，例如 `script_id`。
- `Message`：用户、Agent、系统或工具消息，保存结构化 `metadata`，用于恢复上下文和回放。
- `AgentRun` / `AgentStep`：一次用户消息触发的运行记录，复用已有 `agent_runs`、`agent_steps` 的语义，记录 LLM、工具和数据修改步骤。
- `Tool` 接口：当前先以内置脚本工具实现 `read_script`、`update_scene`，后续可替换为 `novex-tools` / MCP Gateway 注册工具。

脚本 Agent 的业务规则属于脚本上下文：只能修改当前会话绑定脚本内存在的分镜；LLM 必须输出结构化分镜补丁；落库后返回最新脚本摘要和修改说明。

## BDD

第一阶段用户场景是：操作者在脚本详情中看到脚本 Agent 对话面板，发起或继续脚本 Agent 会话，输入“把第 3 镜改得更有冲突感”。系统读取当前脚本和历史消息，调用模型生成分镜修改结果，更新 `scenes` 表，并返回 Agent 消息。前端展示用户消息和 Agent 回复，并刷新脚本详情中的分镜内容。

错误场景：脚本不存在返回 404；会话不存在返回 404；LLM 输出不是合法补丁时返回 500 并记录失败 run；请求消息为空返回 400；脚本会话缺少 `script_id` 返回 400。

前端错误场景：创建会话失败、发送消息失败、API 不可用或当前未选中脚本时，页面必须在对话面板内显示错误状态，不得影响左侧脚本列表和时间轴详情浏览。

## SDD

数据库新增：

- `agent_conversations(id, project_id, agent_type, subject_type, subject_id, title, status, metadata, created_at, updated_at)`。
- `agent_messages(id, conversation_id, role, content, metadata, created_at)`。

后端新增接口：

- `POST /api/agent/conversations`：创建会话。
- `GET /api/agent/conversations/:conversation_id/messages`：读取消息列表。
- `POST /api/agent/conversations/:conversation_id/messages`：发送用户消息并触发 Agent 回复。

通用 Runtime 接口先在后端内落地，暴露可迁移到 `crates/novex-agent` 的类型：`AgentRuntime`、`AgentTurnRequest`、`AgentTurnResponse`、`AgentAdapter`。脚本 Agent 通过 adapter 接入。LLM 仍复用 `novex-model::LLMClient`。

前端新增接口和组件：

- `apps/video-agent/app/lib/api.ts` 增加 `createAgentConversation`、`listAgentMessages`、`sendAgentMessage` 及对应类型。
- `apps/video-agent/app/page.tsx` 在脚本详情区域或右侧操作区加入脚本 Agent 对话面板。
- 对话面板绑定当前 `selectedScript.script_id` 和 `selectedProjectId`，首次发送前创建会话，后续复用会话 ID。
- 发送消息成功后刷新当前脚本详情，确保时间轴同步展示最新分镜。
- 前端实现前必须先更新 `docs/prototypes/video-agent/video-agent.pen` 并获得确认；当前环境无 Pencil MCP 时，前端编码保持阻塞。

## TDD

测试入口：

- `backend/tests/conversation_repository_contract.rs` 验证会话和消息持久化。
- `backend/tests/conversational_script_agent.rs` 验证脚本 Agent 对话修改分镜。
- `backend/tests/conversation_routes.rs` 验证 HTTP 创建会话、发送消息、读取消息和错误状态。
- `backend/tests/database_migrations.rs` 增加新增表、索引、约束验证。
- `apps/video-agent/app/lib/api.test.ts` 验证对话 API client 请求路径、方法、payload 和错误处理。
- `apps/video-agent/app/page.test.tsx` 验证脚本详情出现对话面板、发送消息、错误展示和脚本详情刷新。
- `apps/video-agent/e2e/workspace.spec.ts` 验证桌面工作台能在脚本详情中完成一次对话式改稿交互。

每个行为先写失败测试，再实现仓储、服务和路由。完成后运行相关 Rust 测试和 OpenSpec 状态检查。
