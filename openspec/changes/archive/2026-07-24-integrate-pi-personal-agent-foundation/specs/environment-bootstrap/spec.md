## ADDED Requirements

### Requirement: 顶层 Compose 必须提供本地 Pi Agent Runtime
系统 SHALL 通过 `/server/docker-compose.yml` 统一识别和启动 `ai-agent-agent-runtime`，并 SHALL 使用持久化数据卷保存 Pi SQLite Session。

#### Scenario: 顶层 Compose 列出 Agent Runtime
- **WHEN** 开发者执行 `docker compose -f /server/docker-compose.yml config --services`
- **THEN** 输出 SHALL 包含 `ai-agent-agent-runtime`

#### Scenario: Agent Runtime 启动并通过健康检查
- **WHEN** 开发者通过顶层 Compose 启动 Agent Runtime
- **THEN** 服务 SHALL 在容器内监听配置端口
- **AND** `/health` SHALL 返回 `service=novex-agent-runtime` 与 `status=ok`
- **AND** `/ready` SHALL 分别验证 PostgreSQL 与 SQLite Session Store

#### Scenario: 会话数据在容器重建后保留
- **GIVEN** Runtime 已创建持久化会话
- **WHEN** Runtime 容器被重新创建但数据卷未删除
- **THEN** 原 `session_id` SHALL 仍可打开
- **AND** Runtime SHALL NOT 退化到内存存储
