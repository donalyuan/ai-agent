# Novex Agent Runtime

本服务是本地单用户个人 AI 工作台的通用执行入口，基于 Pi `0.82.0` 提供持久化 Session Tree、Turn/Tool Loop、SSE、steer、follow-up、abort、compact 和 fork。

Runtime 使用 Pi `0.82.0` 的 `toolContext + AgentHarnessTool` 契约，但继续维护 Novex 自有 `read/write/edit/bash` schema，保持既有 `old_text/new_text` edit 参数、Session transcript 与 SSE 行为。上游 execution tool factory 的协议迁移不属于本次依赖升级。

## 数据边界

- PostgreSQL `ai_models` 是模型部署和凭据的唯一来源。
- `/data/agent-sessions.sqlite` 只保存会话 metadata、消息、工具结果、分支、模型非敏感快照和 compaction。
- 脚本、素材、作品、发布状态与正式长期 Memory 不写入 Pi SQLite。
- `chat` profile 无本地工具；`workspace` profile 才启用 `read/write/edit/bash`，目录固定为 `/workspace`。

## 启动

```bash
docker compose -f /server/docker-compose.yml up -d --build ai-agent-agent-runtime
curl -fsS http://127.0.0.1:18184/health
curl -fsS http://127.0.0.1:18184/ready
```

## API

创建会话时必须提交后台中已启用的文本模型 UUID：

```bash
curl -N -X POST http://127.0.0.1:18184/sessions \
  -H 'content-type: application/json' \
  -d '{"model_id":"00000000-0000-4000-8000-000000000000","tool_profile":"chat"}'
```

主要端点：

- `GET/POST /sessions`、`GET/DELETE /sessions/:id`
- `GET /sessions/:id/entries?after_sequence=0&limit=200`
- `POST /sessions/:id/prompt`，返回 SSE
- `POST /sessions/:id/steer`、`follow-up`、`abort`、`compact`、`tree`、`fork`

## 无费用验证

以下命令只运行本地单元及 fake provider 合同测试，不调用 PostgreSQL 中的真实供应商凭据：

```bash
docker build --target test -t novex-agent-runtime-test .
docker run --rm novex-agent-runtime-test npm test
```

不要用真实 `model_id` 调用 `prompt` 或 `compact` 来做常规回归；它们会调用已配置的模型供应商。视频生成与平台发布不属于本服务的通用工具。
