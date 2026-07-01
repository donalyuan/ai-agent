## Why

当前仓库仍处于规划阶段，缺少可通过统一 Docker Compose 入口启动的真实开发环境。项目约束要求本项目运行与验证优先在容器内完成，并且环境初始化必须从 `/server/docker-compose.yml` 进入；用户已明确要求复用已运行的 PostgreSQL 与 Redis 容器，避免重复启动基础中间件。

## What Changes

- 将 `video-agent` 纳入 `/server/docker-compose.yml` 的 `include` 入口。
- 新增本项目最小可运行服务骨架：Rust/Axum API、Python/FastAPI Worker、Next.js 前端。
- 复用现有 `biga-postgres` 与 `bs-redis` 服务，新增一次性数据库初始化服务创建独立 `video_agent` 数据库。
- 为 API 与 Worker 提供健康检查测试，锁定最小运行契约。
- 补充本项目 README 与 `CLAUDE.md` 中经验证的容器化开发命令。

## Capabilities

### New Capabilities

- `environment-bootstrap`: 系统可以从 `/server/docker-compose.yml` 统一启动 video-agent 的 API、Worker 与前端开发环境，并复用现有数据库和 Redis。

### Modified Capabilities

无。

## Impact

- Compose：修改 `/server/docker-compose.yml`，新增 `/server/video-agent/docker-compose.yml`。
- 后端：新增 `backend/` Rust/Axum 最小 API 与健康检查测试。
- Worker：新增 `python-worker/` FastAPI 最小 Worker 与健康检查测试。
- 前端：新增 `frontend/` Next.js 最小入口页，用于确认前端容器可启动。
- 数据库：不新增 PostgreSQL 容器；仅在现有 `biga-postgres` 中创建独立 `video_agent` 数据库。
- Redis：不新增 Redis 容器；使用现有 `bs-redis`，默认 DB index 为 `/2`，避开 bigA 的 `/1`。
- 非目标：不实现业务表迁移、不接入 Milvus/MinIO、不实现六大 Agent 业务能力、不引入权限系统。
