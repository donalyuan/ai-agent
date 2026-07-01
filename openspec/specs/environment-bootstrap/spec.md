# environment-bootstrap Specification

## Purpose
定义 video-agent 本地 Docker Compose 开发环境的稳定启动、依赖复用与健康检查口径。
## Requirements
### Requirement: 环境必须从顶层 Compose 入口启动

系统 MUST 通过 `/server/docker-compose.yml` 统一识别并启动 video-agent 的开发服务，而不得要求开发者绕过顶层入口直接进入子项目 Compose。

#### Scenario: 顶层 Compose 能列出 video-agent 服务

- **WHEN** 开发者执行 `docker compose -f /server/docker-compose.yml config --services`
- **THEN** 输出 SHALL 包含 `video-agent-api`
- **AND** 输出 SHALL 包含 `video-agent-worker`
- **AND** 输出 SHALL 包含 `video-agent-web`

#### Scenario: 顶层 Compose 能启动 video-agent 服务

- **WHEN** 开发者执行 `docker compose -f /server/docker-compose.yml up -d video-agent-api video-agent-worker video-agent-web`
- **THEN** Docker Compose SHALL 使用 `/server/video-agent/docker-compose.yml` 中的服务定义
- **AND** 系统 SHALL NOT 启动新的 PostgreSQL 或 Redis 服务作为 video-agent 专属中间件

### Requirement: 环境必须复用已有 PostgreSQL 与 Redis 容器

系统 MUST 复用已有 `biga-postgres` 与 `bs-redis` 服务承载本项目开发环境依赖。

#### Scenario: API 使用已有 PostgreSQL 服务

- **WHEN** `video-agent-api` 容器启动
- **THEN** `DATABASE_URL` SHALL 指向 `biga-postgres:5432/video_agent`
- **AND** Compose SHALL NOT 定义新的 PostgreSQL 服务

#### Scenario: API 使用已有 Redis 服务

- **WHEN** `video-agent-api` 容器启动
- **THEN** `REDIS_URL` SHALL 指向 `bs-redis:6379/2`
- **AND** Compose SHALL NOT 定义新的 Redis 服务

#### Scenario: 数据库初始化幂等执行

- **WHEN** `video-agent-db-init` 执行且 `video_agent` 数据库不存在
- **THEN** 系统 SHALL 在已有 `biga-postgres` 中创建 `video_agent` 数据库
- **AND** 当数据库已存在时，该初始化 SHALL 返回成功

### Requirement: 最小服务必须提供健康检查

系统 MUST 为 Rust API 与 Python Worker 提供可测试的健康检查端点，作为环境初始化完成的最小可观察证据。

#### Scenario: API 健康检查返回稳定结构

- **WHEN** 调用 `video-agent-api` 的 `/health`
- **THEN** 响应 SHALL 包含 `service=video-agent-api`
- **AND** 响应 SHALL 包含 `status=ok`

#### Scenario: Worker 健康检查返回稳定结构

- **WHEN** 调用 `video-agent-worker` 的 `/health`
- **THEN** 响应 SHALL 包含 `service=video-agent-worker`
- **AND** 响应 SHALL 包含 `status=ok`

#### Scenario: API 就绪检查暴露依赖状态

- **WHEN** 调用 `video-agent-api` 的 `/ready`
- **THEN** 响应 SHALL 分别报告 PostgreSQL 与 Redis 状态
- **AND** 任一依赖不可用时 SHALL 返回 `503`
