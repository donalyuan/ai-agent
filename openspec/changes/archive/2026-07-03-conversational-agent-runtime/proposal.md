# conversational-agent-runtime Proposal

## 背景

当前脚本 Agent 只能一次性生成脚本、读取脚本、更新状态。操作者生成脚本后，无法继续通过自然语言要求 Agent 修改某个分镜、保留上下文或复用未来的 Skill/MCP 工具能力。

## 目标

1. 建立后端通用对话基座，使每类 Agent 都能以同一套会话、消息、运行记录和工具接口接入。
2. 第一阶段接入脚本 Agent，支持对话式修改指定分镜并落库。
3. 在 `apps/video-agent` 脚本详情中接入对话面板，使操作者能直接通过脚本 Agent 指定改稿方向。
4. 为后续 Memory、Skill、MCP Gateway、Tool Registry 留出稳定接口，不把能力写死在脚本页面。

## 非目标

1. 本 change 不实现完整 MCP Gateway、工具权限审批、模型路由后台或多租户 RBAC。
2. 本 change 不实现所有六类 Agent 的业务工具，只提供可复用接口并先接入脚本 Agent。
3. 前端聊天面板必须遵循既有 Pencil 原型确认门槛；没有原型确认前，不进入 `apps/video-agent` 页面编码。

## 影响范围

- 新增对话相关数据库表和后端仓储。
- 新增通用 Agent Runtime 类型和脚本对话处理服务。
- 新增 `POST /api/agent/conversations`、`GET /api/agent/conversations/:conversation_id/messages`、`POST /api/agent/conversations/:conversation_id/messages`。
- 脚本 Agent 对话消息可以读取脚本、调用 LLM 生成结构化分镜补丁，并更新对应分镜。
- `apps/video-agent` 需要新增脚本 Agent 对话 API client、脚本详情对话面板、发送中/错误/空状态、成功后刷新脚本详情。
