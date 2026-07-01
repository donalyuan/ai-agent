# video-agent 项目记忆

> 本文件是 video-agent 项目级记忆统一入口，记录长期偏好、稳定规则、历史决策和跨会话背景。

## 记忆文件索引

### 仓库约定
- 详见 `memory/project-memory-structure.md` — 项目记忆结构规则
- 详见 `memory/frontend-design-skill-requirement.md` — 前端设计约束

### 项目背景
- 详见 `memory/project-tech-stack.md` — 技术栈与架构设计
- 详见 `memory/mvp-requirements.md` — MVP需求边界与验收标准
- 详见 `memory/database-schema.md` — 简化版数据库设计

## 核心决策（2026-07-01）

### 技术选型
- **后端**: Rust + Axum + SQLx + PostgreSQL
- **向量库**: Milvus Standalone（20万素材规模）
- **任务队列**: Redis + 简单Job表
- **Worker**: Python（FastAPI）处理视频生成和平台发布
- **前端**: Next.js 14 + TypeScript + shadcn/ui

### MVP边界
- **第一版不做权限系统**（无租户、无RBAC、无多用户）
- **只做核心功能**：选题→脚本→素材→生成→发布→数据回流
- **平台范围**: 抖音 + 小红书
- **视频生成**: Runway + 可灵
- **并发设计**: 20个任务/分钟
- **交付周期**: 4个月

### 架构原则
1. 数据库极简设计，去掉tenant_id、owner_id等权限字段
2. Python只做视频SDK和平台对接，核心逻辑在Rust
3. 复杂结构先用JSONB，稳定后再拆表
4. 使用AI辅助开发（Claude/Cursor生成样板代码）

### 开发环境
- 环境初始化必须从 `/server/docker-compose.yml` 进入，并 include `/server/video-agent/docker-compose.yml`
- 已复用现有 PostgreSQL 服务 `biga-postgres`，本项目使用独立数据库 `video_agent`
- 已复用现有 Redis 服务 `bs-redis`，本项目使用 Redis DB index `/2`
- 当前服务端口：API `18180->8080`，Worker `18181->8081`，Web `18182->3000`
- 本项目服务容器内工作目录统一为 `/app`

### 六大Agent
1. **选题Agent**: 热点分析 + 爆款选题生成
2. **脚本Agent**: 结构化脚本 + 分镜生成
3. **素材Agent**: 语义检索 + 智能匹配
4. **视频Agent**: 多平台视频生成编排
5. **发布Agent**: 多平台自动发布
6. **优化Agent**: 数据回流 + 策略优化（Month 4）

## 记忆文件约定

1. 本文件是统一入口，具体内容分散在项目根目录的 `memory/` 文件夹
2. 每次新会话开始前、上下文压缩后恢复时，必须先读取本文件
3. 只记录已确认且后续会复用的信息，禁止写入临时探索、一次性报错、敏感信息
4. 重大决策变更时，同步更新本文件和对应的详细记忆文件
5. `memory/` 文件夹跟随项目，可跨机器同步
