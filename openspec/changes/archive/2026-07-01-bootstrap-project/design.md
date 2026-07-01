## DDD

本轮只建立环境边界，不落地业务领域模型。领域上仅确认三个运行单元：`video-agent-api` 负责 Rust API 与后续 Agent 编排入口，`video-agent-worker` 负责 Python 视频生成和平台对接 sidecar，`video-agent-web` 负责 Next.js 前端。数据库使用独立 `video_agent` 库隔离本项目数据，但不引入租户、RBAC 或业务表。

## BDD

开发者从 `/server/docker-compose.yml` 执行 Compose 命令后，应能启动本项目三个服务，并通过健康检查确认服务存活。API 的 `/health` 返回服务名、环境和状态；`/ready` 会检查 PostgreSQL 与 Redis 连接。Worker 的 `/health` 返回服务名和状态。前端首页展示项目名和三个基础服务端点，作为环境启动成功的可观察结果。

## SDD

Compose 入口必须从 `/server/docker-compose.yml` 进入，通过 `include` 加载 `/server/video-agent/docker-compose.yml`。本项目不得定义新的 PostgreSQL 或 Redis 服务；API 与 Worker 通过 `DATABASE_URL=postgres://postgres:postgres@biga-postgres:5432/video_agent` 和 `REDIS_URL=redis://bs-redis:6379/2` 访问已有容器。新增 `video-agent-db-init` 使用 `postgres:16-alpine` 客户端容器幂等创建 `video_agent` 数据库。

服务端口约定：

- API：容器内 `8080`，宿主机 `18180`
- Worker：容器内 `8081`，宿主机 `18181`
- Web：容器内 `3000`，宿主机 `18182`

## TDD

Rust API 先提供健康检查测试，锁定 `/health` 响应结构；Python Worker 先提供健康检查测试，锁定 `/health` 响应结构。Compose 配置通过 `docker compose -f /server/docker-compose.yml config --services` 验证服务可被顶层入口识别；环境启动通过 `docker compose -f /server/docker-compose.yml up -d video-agent-api video-agent-worker video-agent-web` 验证；最终通过容器内测试命令验证 API 与 Worker。

## Error Handling

`video-agent-db-init` 必须幂等：数据库已存在时直接成功。API `/ready` 必须分别报告 PostgreSQL 与 Redis 的连通性；任一依赖不可用时返回 `503`，不得伪装为健康。`/health` 只表示进程存活，不承担依赖就绪判断。
