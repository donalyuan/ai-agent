# conversational-agent-runtime Tasks

## 1. OpenSpec 与设计边界

- [x] 创建 `openspec/changes/conversational-agent-runtime/proposal.md`。
- [x] 创建 `openspec/changes/conversational-agent-runtime/design.md`，覆盖 DDD、BDD、SDD、TDD。
- [x] 创建 `openspec/changes/conversational-agent-runtime/specs/conversational-agent-runtime/spec.md`。
- [x] 明确通用对话 Runtime、脚本 Agent adapter、Skill/MCP/Tool 预留边界。
- [x] 更新 Pencil 原型 `docs/prototypes/video-agent/video-agent.pen`，加入脚本 Agent 对话面板。
- [x] 获得用户对脚本 Agent 对话面板原型的明确确认后，进入前端编码。

## 2. 数据库迁移与数据约束

- [x] 新增 `backend/migrations/20260703030000_agent_conversations.sql`。
- [x] 创建 `agent_conversations` 表，字段包含 `project_id`、`agent_type`、`subject_type`、`subject_id`、`title`、`status`、`metadata`、时间戳。
- [x] 创建 `agent_messages` 表，字段包含 `conversation_id`、`role`、`content`、`metadata`、`created_at`。
- [x] 为 `agent_conversations.agent_type`、`agent_conversations.status`、`agent_messages.role` 增加 CHECK 约束。
- [x] 增加 `idx_agent_conversations_project`、`idx_agent_conversations_subject`、`idx_agent_messages_conversation_created` 等查询索引。
- [x] 扩展 `backend/tests/database_migrations.rs`，验证新增表、索引和约束。

## 3. 会话领域模型与仓储

- [x] 新增 `backend/src/agents/conversation.rs`。
- [x] 定义 `AgentConversation`、`AgentMessage`、`AgentRunRecord`。
- [x] 定义 `AgentConversationStatus` 和 `AgentMessageRole` 枚举及数据库值转换。
- [x] 定义 `CreateAgentConversationInput`、`CreateAgentMessageInput`、`CreateAgentRunInput`、`CreateAgentStepInput`、`FinishAgentRunInput`。
- [x] 新增 `backend/src/repositories/conversation_repository.rs`。
- [x] 实现 `ConversationRepository::create_conversation`。
- [x] 实现 `ConversationRepository::get_conversation`。
- [x] 实现 `ConversationRepository::save_message`。
- [x] 实现 `ConversationRepository::list_messages`。
- [x] 实现 `ConversationRepository::create_run`、`add_step`、`finish_run`。
- [x] 新增 `backend/tests/conversation_repository_contract.rs`，覆盖会话、消息、run、step 的持久化行为。

## 4. 脚本仓储扩展

- [x] 扩展 `ScriptRepository` trait，新增 `update_scene(script_id, scene)`。
- [x] 在 `PostgresScriptRepository` 中实现按 `script_id + sequence` 更新分镜。
- [x] 更新脚本 `updated_at`，保证修改后脚本详情有新时间戳。
- [x] 新增 `ScriptRepositoryError::SceneNotFound`。
- [x] 更新 `ScriptAgentError` 对新增仓储错误的映射。
- [x] 更新 `backend/tests/script_postgres_repository.rs`，覆盖分镜更新。
- [x] 更新 `backend/tests/script_repository_contract.rs` 和 `backend/tests/script_agent_service.rs` 中的测试替身。

## 5. 通用 Agent Runtime 与脚本 Adapter

- [x] 新增 `backend/src/agents/conversational_runtime.rs`。
- [x] 实现 `AgentRuntime::handle_turn`，统一处理一轮用户消息。
- [x] 在 Runtime 中保存用户消息。
- [x] 在 Runtime 中创建 `agent_runs` 运行记录。
- [x] 按 `agent_type` 路由到业务 adapter。
- [x] 对未接入 adapter 的 Agent 类型返回稳定错误，不伪造成功回复。
- [x] 在失败时把 run 标记为 `failed` 并记录错误。
- [x] 实现脚本 adapter：读取绑定脚本。
- [x] 实现脚本 adapter：构造脚本分镜补丁 prompt。
- [x] 实现脚本 adapter：调用 `novex_model::LLMClient`。
- [x] 实现脚本 adapter：解析 `scene_sequence`、`narration`、`visual_description`、`emotion`、`duration_sec`、`reply`。
- [x] 实现脚本 adapter：校验补丁合法性。
- [x] 实现脚本 adapter：调用 `update_scene` 更新分镜。
- [x] 实现脚本 adapter：保存 Agent 回复消息并写入 metadata。
- [x] 新增 `backend/tests/conversational_script_agent.rs`，覆盖一次对话式分镜修改完整链路。

## 6. 后端 HTTP API

- [x] 在 `backend/src/agents/models/request.rs` 中新增 `CreateAgentConversationRequest`。
- [x] 在 `backend/src/agents/models/request.rs` 中新增 `SendAgentMessageRequest`。
- [x] 在 `backend/src/agents/models/request.rs` 中新增 conversation/message/run/turn 响应 DTO。
- [x] 在 `backend/src/lib.rs` 注册 `POST /api/agent/conversations`。
- [x] 在 `backend/src/lib.rs` 注册 `GET /api/agent/conversations/:conversation_id/messages`。
- [x] 在 `backend/src/lib.rs` 注册 `POST /api/agent/conversations/:conversation_id/messages`。
- [x] 创建脚本会话时校验脚本存在。
- [x] 创建脚本会话时校验 `project_id` 与脚本归属一致。
- [x] 发送空消息返回 HTTP 400。
- [x] 会话不存在返回 HTTP 404，并包含 `conversation_id`。
- [x] 脚本不存在返回 HTTP 404，并包含 `script_id`。
- [x] 新增 `backend/tests/conversation_routes.rs`，覆盖创建会话、发送消息、读取消息和错误语义。

## 7. 前端 API Client

- [x] 在 `apps/video-agent/app/lib/api.ts` 中新增 `AgentConversation`、`AgentMessage`、`AgentRun`、`AgentTurnResponse` 类型。
- [x] 在 `apps/video-agent/app/lib/api.ts` 中新增 `createAgentConversation(client, payload)`。
- [x] 在 `apps/video-agent/app/lib/api.ts` 中新增 `listAgentMessages(client, conversationId)`。
- [x] 在 `apps/video-agent/app/lib/api.ts` 中新增 `sendAgentMessage(client, conversationId, payload)`。
- [x] 更新 `apps/video-agent/app/lib/api.test.ts`，验证三组对话 API 的 URL、method、payload 和错误处理。

## 8. 前端脚本 Agent 对话面板

- [x] 在 Pencil 原型确认后，更新 `apps/video-agent/app/page.tsx`，在脚本详情区域加入脚本 Agent 对话面板。
- [x] 对话面板应绑定当前 `selectedProjectId` 和 `selectedScript.script_id`。
- [x] 首次发送消息前自动创建 `script` 会话，后续复用当前脚本的 `conversation_id`。
- [x] 发送消息时调用 `sendAgentMessage`，展示发送中状态并禁用重复提交。
- [x] 发送成功后追加用户消息和 Agent 回复。
- [x] 发送成功后调用 `getScript` 刷新当前脚本详情，使时间轴同步更新。
- [x] 切换脚本时重置或加载对应脚本会话状态，避免把 A 脚本对话发到 B 脚本。
- [x] API 不可用或未选中脚本时禁用对话输入。
- [x] 创建会话失败、发送失败、LLM 输出失败时在对话面板内展示错误，不影响脚本列表和时间轴浏览。
- [x] 对话面板不得引入项目创建、选题管理或六 Agent 一级导航回退。

## 9. 前端测试与验收

- [x] 更新 `apps/video-agent/app/page.test.tsx`，验证脚本详情显示脚本 Agent 对话面板。
- [x] 更新 `apps/video-agent/app/page.test.tsx`，验证发送消息会创建会话并调用发送接口。
- [x] 更新 `apps/video-agent/app/page.test.tsx`，验证发送成功后刷新脚本详情。
- [x] 更新 `apps/video-agent/app/page.test.tsx`，验证发送失败错误展示在对话面板内。
- [x] 更新 `apps/video-agent/e2e/workspace.spec.ts`，拦截对话 API 并验证一次对话式改稿流程。
- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-agent sh -lc 'cd /app && npm run test'`。
- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-agent sh -lc 'cd /app && npm run lint'`。
- [x] 如涉及布局变化，运行 Playwright 截图/视觉检查，确认桌面视口无重叠。

## 10. 文档、记忆与验证

- [x] 更新 `MEMORY.md`，记录通用对话 Runtime 后端基座和脚本 Agent 接入。
- [x] 更新 `docs/memory/video-agent-workspace-flow.md`，记录所有业务 Agent 应复用统一 Runtime。
- [x] 创建 `docs/superpowers/plans/2026-07-03-conversational-agent-runtime.md`。
- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api'`。
- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test --workspace'`。
- [x] 前端完成后再次运行后端、前端和 OpenSpec 全量验证。
- [x] 前端完成后运行 `openspec instructions apply --change "conversational-agent-runtime" --json` 并确认 `state=all_done`。
