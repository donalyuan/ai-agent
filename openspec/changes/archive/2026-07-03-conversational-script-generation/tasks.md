# conversational-script-generation Tasks

## 1. OpenSpec 与设计边界

- [x] 创建 `openspec/changes/conversational-script-generation/proposal.md`。
- [x] 创建 `openspec/changes/conversational-script-generation/design.md`，覆盖 DDD、BDD、SDD、TDD。
- [x] 创建 `openspec/changes/conversational-script-generation/specs/conversational-agent-runtime/spec.md`。
- [x] 创建 `openspec/changes/conversational-script-generation/specs/script-agent-workspace/spec.md`。
- [x] 明确产品约定：脚本生成和脚本修改都走同一个脚本 Agent 对话入口。
- [x] 运行 `openspec instructions apply --change "conversational-script-generation" --json`，确认新 change 可被识别。

## 2. 记忆与产品约定

- [x] 更新 `MEMORY.md`，记录脚本生成也应走脚本 Agent 对话入口。
- [x] 更新 `docs/memory/video-agent-workspace-flow.md`，记录脚本创作菜单的右侧主操作区不再并列展示生成表单和对话框。

## 3. Pencil 原型门禁

- [x] 更新 `docs/prototypes/video-agent/video-agent.pen`，把脚本创作右侧操作区改为单一脚本 Agent 对话入口。
- [x] 原型覆盖无脚本时通过对话生成脚本。
- [x] 原型覆盖已有脚本时通过同一对话修改脚本。
- [x] 原型覆盖生成参数不足时 Agent 追问。
- [x] 原型覆盖生成成功后脚本列表刷新并打开时间轴详情。
- [x] 获得用户对单一对话入口原型的明确确认后，进入前端编码。

## 4. 后端会话创建与 DTO 扩展

- [x] 更新 `backend/tests/conversation_routes.rs`，先写失败测试：允许创建 `agent_type=script`、`project_id` 必填、`subject_type` / `subject_id` 为空的未绑定脚本会话。
- [x] 更新 `backend/src/agents/models/request.rs`，允许脚本会话创建请求不传 `subject_type` / `subject_id`。
- [x] 更新 `backend/src/lib.rs` 的创建会话校验：有 `subject_id` 时继续校验脚本存在和项目归属；无 `subject_id` 时只校验项目存在。
- [x] 更新 `AgentTurnResponse` 或 assistant metadata 约定，稳定返回 `intent`、`script_id`、`script_created`、`needs_input`、`missing_fields`。
- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test conversation_routes'`。

## 5. 脚本 Agent 对话式生成 Adapter

- [x] 更新 `backend/tests/conversational_script_agent.rs`，先写失败测试：未绑定脚本会话发送完整生成请求后创建脚本、保存消息、记录 run、返回 `script_id`。
- [x] 更新 `backend/tests/conversational_script_agent.rs`，先写失败测试：缺少选题、风格或分镜数时保存追问回复，不创建脚本。
- [x] 更新 `backend/tests/conversational_script_agent.rs`，先写失败测试：LLM 或保存失败时记录 failed run，不伪造成功回复。
- [x] 扩展 `backend/src/agents/conversational_runtime.rs`：脚本会话无 `subject_id` 时进入生成路径。
- [x] 复用现有 `ScriptAgentService` 或抽取共享脚本生成函数，避免复制脚本生成落库逻辑。
- [x] 增加脚本生成意图结构化输出解析：`intent`、`topic`、`style`、`scene_count`、`reply`、`missing_fields`。
- [x] 校验 `style` 只允许 `knowledge`、`story`、`tutorial`。
- [x] 校验 `scene_count` 只允许 3 到 12。
- [x] 缺少字段时保存追问式 assistant 消息，并写入 `needs_input=true` metadata。
- [x] 生成成功后把会话绑定到新脚本，或实现稳定的会话绑定更新仓储方法。
- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test conversational_script_agent'`。

## 6. 前端 API Client 与类型

- [x] 更新 `apps/video-agent/app/lib/api.test.ts`，先写失败测试覆盖对话生成返回 metadata 中的 `script_id`、`script_created`、`needs_input`、`missing_fields`。
- [x] 更新 `apps/video-agent/app/lib/api.ts` 的 `AgentMessage` metadata 使用类型辅助，或新增脚本 Agent turn metadata 类型。
- [x] 若后端选择在 turn response 顶层返回 `script` 或 `script_id`，同步更新 TypeScript 类型和测试。
- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-agent sh -lc 'cd /app && npm run test -- app/lib/api.test.ts'`。

## 7. 前端单一脚本 Agent 对话入口

- [x] 更新 `apps/video-agent/app/page.test.tsx`，先写失败测试：页面不再同时展示独立“生成脚本”表单和脚本 Agent 对话输入。
- [x] 更新 `apps/video-agent/app/page.test.tsx`，先写失败测试：无脚本时对话面板可发送脚本生成请求。
- [x] 更新 `apps/video-agent/app/page.test.tsx`，先写失败测试：生成成功后刷新脚本列表并打开新脚本详情。
- [x] 更新 `apps/video-agent/app/page.test.tsx`，先写失败测试：生成参数不足时显示 Agent 追问且不新增脚本。
- [x] 更新 `apps/video-agent/app/page.test.tsx`，确认已有脚本时同一面板仍支持分镜修改。
- [x] 修改 `apps/video-agent/app/page.tsx`，移除右侧常驻 `GeneratePanel`。
- [x] 修改 `ScriptAgentConversationPanel` 文案：无脚本时引导“描述你想生成的脚本”，已选脚本时引导“修改当前脚本”。
- [x] 保留项目选择；无项目时禁用对话输入并提示先选择项目。
- [x] 生成成功后根据返回 `script_id` 调 `getScript`，刷新脚本列表并打开详情。
- [x] 确保切换脚本或生成脚本时迟到响应不会污染当前面板。
- [x] 修改 `apps/video-agent/app/styles.css`，删除或收敛旧生成表单专用样式，保证右侧只有一个主输入区。
- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-agent sh -lc 'cd /app && npm run test -- app/page.test.tsx'`。

## 8. 前端 E2E 与视觉验收

- [x] 更新 `apps/video-agent/e2e/workspace.spec.ts`，拦截未绑定脚本会话创建、对话生成消息和刷新后的脚本详情。
- [x] E2E 覆盖：项目有空脚本列表时，通过脚本 Agent 对话生成脚本并看到时间轴详情。
- [x] E2E 覆盖：已有脚本时，通过同一对话入口修改分镜。
- [x] 验证右侧操作区不再并列出现两个 textarea 输入区。
- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-agent sh -lc 'cd /app && npm run test:e2e'`。

## 9. 全量验证

- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-agent sh -lc 'cd /app && npm run test'`。
- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-agent sh -lc 'cd /app && npm run lint'`。
- [x] 运行 `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test --workspace'`。
- [x] 运行 `openspec instructions apply --change "conversational-script-generation" --json` 并确认 `state=all_done`。
