# AI 视频生成 Agent 系统

> AI 驱动的视频内容生产系统，从选题到发布的全流程自动化

## 项目状态

🚧 **当前阶段：开发环境初始化完成，业务功能待实现**

- ✅ 完成完整需求文档
- ✅ 完成数据库设计
- ✅ 确认技术栈选型
- ✅ Docker Compose 开发环境已接入顶层 `/server/docker-compose.yml`
- 🔄 进入基础 API 与 Agent 链路开发阶段

## 核心能力

### 六大 Agent

1. **选题 Agent** - 热点分析 + 爆款选题生成
2. **脚本 Agent** - 结构化脚本 + 分镜生成
3. **素材 Agent** - 语义检索 + 智能匹配
4. **视频 Agent** - 多平台视频生成编排
5. **发布 Agent** - 多平台自动发布
6. **优化 Agent** - 数据回流 + 策略优化

### 完整闭环

```
选题 → 脚本 → 素材匹配 → 视频生成 → 发布分发 → 数据回流 → 策略优化
```

## 技术栈

### 后端
- **Rust + Axum** - API 服务和 Agent 编排
- **SQLx + PostgreSQL** - 类型安全的数据库操作
- **Milvus Standalone** - 素材向量检索（20万规模）
- **Redis** - 任务队列 + 缓存

### Python Worker
- **FastAPI** - Worker HTTP 服务
- **视频生成 SDK** - Runway、可灵
- **平台 SDK** - 抖音、小红书

### 前端
- **Next.js 14 + TypeScript**
- **shadcn/ui** - UI 组件库

## MVP 范围

### ✅ 核心功能（必做）
- 素材库管理与语义检索
- 选题与脚本生成
- 视频生成（Runway + 可灵）
- 平台发布（抖音 + 小红书）
- 数据回流与分析

### ❌ 暂不实现
- 多租户与 RBAC 权限系统
- 可视化 Workflow 编排
- 多 Agent 协作
- 账号矩阵运营

## 文档导航

### 核心文档
- [`CLAUDE.md`](./CLAUDE.md) - Claude Code 工作规范
- [`MEMORY.md`](./MEMORY.md) - 项目记忆统一入口
- [`ARCHITECTURE.md`](./ARCHITECTURE.md) - 系统架构设计
- [`ai_video_agent_full_spec.md`](./ai_video_agent_full_spec.md) - 完整需求文档

### 项目记忆
- [`memory/project-tech-stack.md`](./memory/project-tech-stack.md) - 技术栈与架构
- [`memory/mvp-requirements.md`](./memory/mvp-requirements.md) - MVP 需求边界
- [`memory/database-schema.md`](./memory/database-schema.md) - 数据库设计

## 开发计划

### Month 1-2（P0）
- 数据库 + 基础 API
- LLM 调用层
- 选题 Agent + 脚本 Agent
- 素材上传 + 检索

### Month 3（P1）
- 视频生成 Agent
- Python Worker（Runway + 可灵）
- 发布 Agent（抖音 + 小红书）
- 任务队列

### Month 4（P2）
- 爆款分析
- 优化 Agent
- 数据 Dashboard
- 打磨体验

## 快速开始

本项目开发环境必须从顶层 Compose 入口启动，并复用 `/server/docker-compose.yml` 中已经运行的 `biga-postgres` 与 `bs-redis`。

### 启动环境

```bash
docker compose -f /server/docker-compose.yml up -d --build video-agent-api video-agent-worker video-agent-web
```

### 访问地址

- API health: `http://127.0.0.1:18180/health`
- API ready: `http://127.0.0.1:18180/ready`
- Worker health: `http://127.0.0.1:18181/health`
- Web: `http://127.0.0.1:18182`

### 环境依赖

- PostgreSQL：复用 `biga-postgres`，自动创建独立数据库 `video_agent`
- Redis：复用 `bs-redis`，使用 DB index `/2`

### 常用验证

```bash
docker compose -f /server/docker-compose.yml config --services
docker compose -f /server/docker-compose.yml exec -T video-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test'
docker compose -f /server/docker-compose.yml exec -T video-agent-worker sh -lc 'cd /app && pytest tests -q'
docker compose -f /server/docker-compose.yml exec -T video-agent-web sh -lc 'cd /app && npm run lint'
docker compose -f /server/docker-compose.yml exec -T video-agent-web sh -lc 'cd /app && npm run build'
```

## 设计原则

1. **极简优先** - MVP 不做权限系统，复杂结构先用 JSONB
2. **并发稳定** - 支持 20 个视频生成任务/分钟
3. **模块清晰** - Python 只做视频 SDK 和平台对接，核心逻辑在 Rust
4. **AI 辅助** - 使用 Claude/Cursor 生成样板代码

## 系统规模

- 素材量：初期 < 10 万，最多 20 万
- 并发量：最多 20 个视频生成任务/分钟
- 平台：MVP 只做抖音和小红书
- 部署：Docker Compose 本地开发，容器化云部署

## License

待定

## 贡献指南

开发前请先阅读：
1. [`CLAUDE.md`](./CLAUDE.md) - 工作规范与约束
2. [`MEMORY.md`](./MEMORY.md) - 项目长期决策
3. [`memory/mvp-requirements.md`](./memory/mvp-requirements.md) - MVP 边界

---

📅 最后更新：2026-07-01
