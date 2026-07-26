# Novex Agent Runtime

本服务是本地单用户个人 AI 工作台的通用执行入口，基于 Pi `0.82.0` 提供持久化 Session Tree、Turn/Tool Loop、SSE、steer、follow-up、abort、compact 和 fork。

Runtime 使用 Pi `0.82.0` 的 `toolContext + AgentHarnessTool` 契约，但继续维护 Novex 自有 `read/write/edit/bash` schema，保持既有 `old_text/new_text` edit 参数、Session transcript 与 SSE 行为。上游 execution tool factory 的协议迁移不属于本次依赖升级。

Novex 组合式 wrapper 持有 `AgentHarness`，只通过 Pi 公开 hook、`Models`、Tool 和 Session API 注入版本化 Prompt、Context 编译、调用审计与 Tool Gate；不修改 Pi 源码、不访问私有字段，也不复制 Turn/Tool Loop。启动时会校验 Pi owner 的完整 Definition inventory，缺失或非法定义时服务拒绝启动。

普通 Turn 与 Tool follow-up 在公开 `context` hook 中把实际 `AgentMessage[]` 映射为原子 `ContextCandidate`，按 Session 固定的 Context Policy 与 Tokenizer Profile 编译后返回替换消息。Tool request/result 按 call ID 整体采用或失败；steer/follow-up 保留独立 P0 来源。Pi 内部 compaction 与 branch summary 不经过该 hook，因此由公开 `Models.completeSimple()` 组合层治理真实 `Context`，各自使用独立 node、ContextSnapshot 与 ModelCall；摘要仍只是有损 Session Context，不会升级为正式长期 Memory。

## 数据边界

- PostgreSQL `ai_models` 是模型部署和凭据的唯一来源；enabled 文本模型必须显式配置 `context_window`、`tokenizer_profile_key` 与 `tokenizer_profile_version`。
- 仓库级 `agent-definitions/` 是 Agent/Prompt/Context Policy/Tokenizer Profile 定义唯一来源，随镜像发布；数据库不保存可覆盖的定义正文。
- `/data/agent-sessions.sqlite` 保存会话 metadata、不可变 Definition/模型/Context binding、消息、工具结果、分支、compaction，以及 namespaced ContextSnapshot、ContextCompileAttempt 和 ModelCall 审计。
- 脚本、素材、作品、发布状态与正式长期 Memory 不写入 Pi SQLite。
- `chat` profile 无本地工具；`workspace` profile 才启用 `read/write/edit/bash`，目录固定为 `/workspace`。

## 启动

```bash
docker compose -f /server/docker-compose.yml up -d --build ai-agent-agent-runtime
curl -fsS http://127.0.0.1:18184/health
curl -fsS http://127.0.0.1:18184/ready
```

## API

创建会话时必须提交 active `agent_key`、后台中已启用的文本模型 UUID 和 Definition 允许的工具 profile。请求不接受 `system_prompt` 或其他未声明字段；System 内容只来自随代码发布的 Definition Registry。

```bash
curl -N -X POST http://127.0.0.1:18184/sessions \
  -H 'content-type: application/json' \
  -d '{"agent_key":"personal.general","model_id":"00000000-0000-4000-8000-000000000000","tool_profile":"chat"}'
```

创建时会固定 Agent/Prompt/Context Policy/Tokenizer Profile 精确版本及 digest、registry digest、`model_id` 与 `behavior_fingerprint`。后续每次模型调用前重新解析 `ai_models`：仅凭据轮换可继续，停用、删除、能力不兼容、Context 配置缺失、Profile 与协议/操作者确认模型家族不兼容，或行为 fingerprint 漂移会在外部请求前失败。Runtime 不根据 `upstream_model` 猜测窗口或 tokenizer。普通 fork 继承 binding；版本或模型升级必须使用带 `upgrade` object 的显式 fork。

历史 Session 未设置自定义 `system_prompt` 时会幂等绑定到 `personal.general@1`；存在自定义文本时保持只读，必须显式 fork 并选择丢弃，或把旧文本降级为可见的普通 User instruction。旧文本不会再次进入 System 层。

主要端点：

- `GET/POST /sessions`、`GET/DELETE /sessions/:id`
- `GET /sessions/:id/entries?after_sequence=0&limit=200`
- `POST /sessions/:id/prompt`，返回 SSE
- `POST /sessions/:id/steer`、`follow-up`、`abort`、`compact`、`tree`、`fork`
- `GET /model-calls`、`GET /sessions/:id/model-calls`：支持 owner、node、Agent/Prompt version、model、status、time、limit/offset 筛选，仅返回摘要
- `GET /model-calls/:id`、`GET /model-calls/:id/export`：返回版本化脱敏记录、`source_runtime` 与 `record_hash`
- `POST /model-calls/:id/replay`：只接受无副作用的 `dry_run`，从历史编译输入重编译并返回结构化 diff
- `GET /migration/plan`：只读返回历史 Session 的幂等迁移计划

所有错误统一返回 `{"error":{"code":"...","message":"..."}}`。固定 binding 需要显式迁移时使用 `model_rebind_required`、`definition_rebind_required` 或 `session_migration_required`；Context schema、原子组、冲突、预算、required 来源或 tokenizer 失败会返回对应稳定 code，并在 message 中关联已持久化的 `ContextCompileAttempt` ID，不创建虚假 ModelCall；并发执行使用 `session_busy`。任意 `system_prompt` 会作为未知字段返回 `bad_request`，不会降级为用户消息。

每个实际 provider step 与显式 retry 都对应一个独立 `ContextSnapshot + ModelCall` attempt；两者在本地事务写入失败时不发送 provider 请求。PromptSnapshot v2 保存实际逻辑消息与 `context_snapshot_id/context_digest`，不再保存单一 Context JSON blob 或动态 fragment 副本。日志、SSE、错误、详情和导出执行同一脱敏规则，拒绝凭据、认证头、Cookie、base64 大对象和临时签名 URL。

## 无费用验证

以下命令只运行本地单元及 fake provider 合同测试，不调用 PostgreSQL 中的真实供应商凭据：

```bash
docker build --target test -t novex-agent-runtime-test .
docker run --rm novex-agent-runtime-test npm test
```

不要用真实 `model_id` 调用 `prompt` 或 `compact` 来做常规回归；它们会调用已配置的模型供应商。视频生成与平台发布不属于本服务的通用工具。
