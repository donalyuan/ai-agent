# environment-bootstrap Specification

## Purpose
定义 Novex 本地 Docker Compose 开发环境的稳定启动、依赖复用与健康检查口径，确保开发者通过统一入口获得可重复、可验证的基础服务环境。
## Requirements
### Requirement: 环境必须从顶层 Compose 入口启动

系统 MUST 通过 `/server/docker-compose.yml` 统一识别并启动 Novex 本地开发服务，而不得要求开发者绕过顶层入口直接进入子项目 Compose。项目内部 Compose SHALL 使用 Novex 基座目录结构中的 `backend`、`admin`、`apps`、`services` 路径作为构建上下文。

#### Scenario: 顶层 Compose 能列出 Novex 服务

- **WHEN** 开发者执行 `docker compose -f /server/docker-compose.yml config --services`
- **THEN** 输出 SHALL 包含 `novex-api`
- **AND** 输出 SHALL 包含 `novex-video-worker`
- **AND** 输出 SHALL 包含 `novex-admin`

#### Scenario: 顶层 Compose 能启动 Novex 服务

- **WHEN** 开发者执行 `docker compose -f /server/docker-compose.yml up -d novex-api novex-video-worker novex-admin`
- **THEN** Docker Compose SHALL 使用 `/server/video-agent/docker-compose.yml` 中的服务定义
- **AND** 系统 SHALL 使用 Novex 基座目录下的构建上下文
- **AND** 系统 SHALL NOT 启动新的 PostgreSQL 或 Redis 服务作为 Novex 专属中间件

### Requirement: 环境必须复用已有 PostgreSQL 与 Redis 容器

系统 MUST 复用已有 `biga-postgres` 与 `bs-redis` 服务承载本项目开发环境依赖。

#### Scenario: API 使用已有 PostgreSQL 服务

- **WHEN** `novex-api` 容器启动
- **THEN** `DATABASE_URL` SHALL 指向 `biga-postgres:5432/video_agent`
- **AND** Compose SHALL NOT 定义新的 PostgreSQL 服务

#### Scenario: API 使用已有 Redis 服务

- **WHEN** `novex-api` 容器启动
- **THEN** `REDIS_URL` SHALL 指向 `bs-redis:6379/2`
- **AND** Compose SHALL NOT 定义新的 Redis 服务

#### Scenario: 数据库初始化幂等执行

- **WHEN** 数据库初始化服务执行且 `video_agent` 数据库不存在
- **THEN** 系统 SHALL 在已有 `biga-postgres` 中创建 `video_agent` 数据库
- **AND** 当数据库已存在时，该初始化 SHALL 返回成功

### Requirement: 最小服务必须提供健康检查

系统 MUST 为 Rust API 与 Python video worker 提供可测试的健康检查端点，作为环境初始化完成的最小可观察证据。

#### Scenario: API 健康检查返回稳定结构

- **WHEN** 调用 `novex-api` 的 `/health`
- **THEN** 响应 SHALL 包含 `service=novex-api`
- **AND** 响应 SHALL 包含 `status=ok`

#### Scenario: Worker 健康检查返回稳定结构

- **WHEN** 调用 `novex-video-worker` 的 `/health`
- **THEN** 响应 SHALL 包含 `service=novex-video-worker`
- **AND** 响应 SHALL 包含 `status=ok`

#### Scenario: API 就绪检查暴露依赖状态

- **WHEN** 调用 `novex-api` 的 `/ready`
- **THEN** 响应 SHALL 分别报告 PostgreSQL 与 Redis 状态
- **AND** 任一依赖不可用时 SHALL 返回 `503`

### Requirement: 视频工作台必须支持本机内网 IP 访问 API

`apps/video-agent` SHALL 在本地 Docker 开发环境中支持操作者通过运行机器的内网 IP 访问 `18183` 前端，并让浏览器端 API 请求指向同一运行机器的 `18180` API 端口。

#### Scenario: 通过内网 IP 打开视频工作台

- **GIVEN** 视频工作台前端运行在 `http://10.1.31.7:18183`
- **AND** 未显式配置 `NEXT_PUBLIC_API_BASE_URL`
- **AND** `ai-agent-video-agent` Compose 默认环境未注入 `NEXT_PUBLIC_API_BASE_URL=http://localhost:18180`
- **WHEN** 前端创建 API client
- **THEN** API base URL SHALL 为 `http://10.1.31.7:18180`
- **AND** 后续 `/health`、`/api/projects` 和 `/api/video-workspace/menus` 请求 SHALL 使用该 base URL

#### Scenario: 通过本机回环地址打开视频工作台

- **GIVEN** 视频工作台前端运行在 `http://127.0.0.1:18183`
- **AND** 未显式配置 `NEXT_PUBLIC_API_BASE_URL`
- **WHEN** 前端创建 API client
- **THEN** API base URL SHALL 为 `http://127.0.0.1:18180`

#### Scenario: 显式 API base URL 仍优先生效

- **GIVEN** 已配置 `NEXT_PUBLIC_API_BASE_URL=http://api.example.test/`
- **WHEN** 前端创建 API client
- **THEN** API base URL SHALL 为 `http://api.example.test`
- **AND** 系统 SHALL NOT 使用当前页面 hostname 派生 API 地址
