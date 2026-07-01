---
name: project-tech-stack
description: AI视频生成Agent系统的技术选型和架构设计
metadata:
  node_type: memory
  type: project
  originSessionId: 322147b7-81d9-49fa-b62d-971d5fa7a0f8
---

# AI视频生成Agent - 技术栈与架构

## 项目定位
AI视频内容生产系统，从选题到发布的全流程自动化。**第一版专注核心功能，不做权限系统**。

## 核心技术栈

### 后端
- **Rust + Axum**: API服务和Agent编排
- **SQLx**: 类型安全的数据库操作
- **PostgreSQL**: 主数据库（关系数据 + JSONB）
- **Milvus Standalone**: 素材向量检索（20万素材规模）
- **Redis**: 任务队列 + 缓存
- **MinIO**: 本地对象存储

### Python Worker
- **FastAPI**: Worker HTTP服务
- **视频生成SDK**: Runway、可灵、Pika、HeyGen
- **平台SDK**: 抖音、小红书

### 前端
- **Next.js 14 + TypeScript**
- **shadcn/ui**: UI组件库

### LLM
- 统一模型调用层，支持OpenAI/Claude/Qwen/DeepSeek

## 系统规模
- 素材量：初期 < 10万，最多20万
- 并发量：最多20个视频生成任务/分钟
- 平台：MVP只做抖音和小红书
- 爆款分析：智能方案（LLM自动分析）

## 六大Agent
1. **选题Agent**: 热点分析 + 爆款选题生成
2. **脚本Agent**: 结构化脚本 + 分镜生成
3. **素材Agent**: 语义检索 + 智能匹配
4. **视频Agent**: 多平台视频生成编排
5. **发布Agent**: 多平台自动发布
6. **优化Agent**: 数据回流 + 策略优化

## 数据库设计原则
- **极简设计**：去掉租户、权限、RBAC相关表
- **核心表**：projects, materials, scripts, scenes, videos, publish_tasks, metrics
- **Agent表**：agent_runs, agent_steps（用于调试和trace）
- **分析表**：viral_videos, content_strategies

## 项目结构
```
video-agent/
├── src/              # Rust主服务
│   ├── api/          # HTTP接口
│   ├── agents/       # 六大Agent实现
│   ├── services/     # LLM、Milvus、Worker调用
│   └── infra/        # DB、Redis、存储
├── python-worker/    # Python异步Worker
│   ├── generators/   # 视频生成
│   ├── publishers/   # 平台发布
│   └── analytics/    # 爆款分析
└── frontend/         # Next.js前端
```

## 开发模式
- **单人开发 + AI辅助**（Claude/Cursor）
- **本地开发**：Docker Compose一键启动
- **云部署**：容器化部署

## 当前开发环境约定（2026-07-01）

- 顶层入口：`/server/docker-compose.yml`
- 项目 Compose：`/server/video-agent/docker-compose.yml`
- PostgreSQL：复用 `biga-postgres`，数据库名 `video_agent`
- Redis：复用 `bs-redis`，DB index `/2`
- API：`video-agent-api`，宿主机端口 `18180`，容器端口 `8080`
- Worker：`video-agent-worker`，宿主机端口 `18181`，容器端口 `8081`
- Web：`video-agent-web`，宿主机端口 `18182`，容器端口 `3000`
- 容器内项目路径：`/app`

## 实施计划（4个月）
- Month 1: 基础框架 + LLM + Milvus
- Month 2: 选题/脚本/素材Agent
- Month 3: 视频生成 + 发布
- Month 4: 爆款分析 + 优化

## 为什么选Rust
1. 并发性能（20任务/分钟稳定运行）
2. 类型安全（状态机编译期保证）
3. 未来扩展（复用基座做其他AI应用）
4. 学习时间可接受（有AI辅助开发）

**Why**: 架构设计和技术选型决策，后续开发参考。

**How to apply**:
- 所有新功能优先看能否复用现有Agent
- 数据表设计保持简单，MVP不加权限字段
- Python只做视频生成和平台对接，核心逻辑在Rust
- 使用Claude生成样板代码（CRUD、Agent模板）
