# Novex 项目记忆

> 本文件是 Novex AI Agent 基座项目级记忆统一入口，记录长期偏好、稳定规则、历史决策和跨会话背景。`apps/video-agent` 是当前第一个业务应用。

## 记忆文件索引

### 仓库约定
- 详见 `docs/memory/project-memory-structure.md` — 项目记忆结构规则
- 详见 `docs/memory/frontend-design-skill-requirement.md` — 前端设计约束

### 项目背景
- 详见 `docs/memory/project-tech-stack.md` — 技术栈与架构设计
- 详见 `docs/requirements/video-agent-mvp.md` — MVP需求边界与验收标准
- 详见 `docs/requirements/video-agent-database-schema.md` — 简化版数据库设计
- 详见 `docs/requirements/video-agent-full-spec.md` — video-agent 完整需求文档

## 核心决策（2026-07-01）

### 技术选型
- **后端**: Rust + Axum + SQLx + PostgreSQL
- **向量库**: Milvus Standalone（20万素材规模）
- **任务队列**: Redis + 简单Job表
- **Worker**: Python（FastAPI）处理视频生成和平台发布
- **前端**: Next.js 14 + TypeScript + shadcn/ui

### 当前架构决策
- 当前仓库定位已从根级 video-agent MVP 调整为 **Novex AI Agent Foundation monorepo**
- `video-agent` 保留为 `apps/video-agent` 下的首个业务应用
- OpenSpec change `align-novex-foundation-architecture` 已于 2026-07-01 归档
- `script-agent-mvp` 可在 Novex 基座结构下恢复开发，继续从 T2.3 开始
- 已完成的 `script-agent-mvp` 数据库、模型和 Repository 初始能力保留并迁移，不丢弃

### 架构原则
1. `backend/` 承担控制面 API 和业务编排入口
2. 可复用 AI 能力沉淀到 `crates/*`
3. Python 只做 `services/*` sidecar/runtime
4. 业务应用放入 `apps/*`
5. video-agent 业务范围仍参考 `docs/requirements/video-agent-mvp.md`

### 开发环境
- 环境初始化必须从 `/server/docker-compose.yml` 进入，并 include `/server/video-agent/docker-compose.yml`
- 已复用现有 PostgreSQL 服务 `biga-postgres`，本项目使用独立数据库 `video_agent`
- 已复用现有 Redis 服务 `bs-redis`，本项目使用 Redis DB index `/2`
- 当前服务端口：API `18180->8080`，Video Worker `18181->8081`，Admin `18182->3000`
- Compose 服务名：`novex-api`、`novex-video-worker`、`novex-admin`
- 本项目服务容器内工作目录统一为 `/app`

### 六大Agent
1. **选题Agent**: 热点分析 + 爆款选题生成
2. **脚本Agent**: 结构化脚本 + 分镜生成
3. **素材Agent**: 语义检索 + 智能匹配
4. **视频Agent**: 多平台视频生成编排
5. **发布Agent**: 多平台自动发布
6. **优化Agent**: 数据回流 + 策略优化（Month 4）

## 记忆文件约定

1. 本文件是统一入口，具体主题记忆位于 `docs/memory/`，产品与需求文档位于 `docs/requirements/`
2. 每次新会话开始前、上下文压缩后恢复时，必须先读取本文件
3. 只记录已确认且后续会复用的信息，禁止写入临时探索、一次性报错、敏感信息
4. 重大决策变更时，同步更新本文件和对应的详细记忆文件
5. `docs/memory/` 与 `docs/requirements/` 跟随项目，可跨机器同步
