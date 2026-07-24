## Why

Novex 的长期产品定位已经确认调整为本地单用户的个人 AI 工作台，视频生产只是首个领域应用；现有 Rust Agent Kernel 只覆盖业务 Adapter 分派和 Run 收尾，尚未提供通用 Tool Loop、流式事件、会话树、上下文压缩与本地扩展能力。现在引入 Pi Agent Harness，可以在不重写视频领域状态机的前提下补齐可复用执行内核，并停止为未计划的多租户和对外交付场景继续增加复杂度。

## What Changes

- **BREAKING**：将 Novex 的长期定位从面向客户交付的多租户 Agent Foundation 调整为 local-first、单用户、可承载多个领域工作台的个人 AI 工作台基座。
- 新增基于 `@earendil-works/pi-agent-core` 的本地 Agent Runtime 服务，使用 Pi Harness 承担流式 Turn/Tool Loop、steering、follow-up、取消和运行事件。
- 使用 Pi SQLite Session Storage 持久化会话树、分支、消息、工具结果和上下文压缩记录；Pi 会话不替代 PostgreSQL 中的领域状态与正式长期 Memory。
- 继续以 PostgreSQL `ai_models` 作为模型部署唯一配置来源，由本地 Runtime 按 `model_id` 解析启用文本模型，不新增环境变量或第二套模型目录兜底。
- 将本地 `read`、`write`、`edit`、`bash` 能力作为显式可启用工具集，并对付费生成、正式发布、删除正式领域数据等动作保留领域 Gate。
- 保留 Rust `novex-agent` 的现有业务 Adapter 与 Run 生命周期以维持视频工作台行为；新增 Pi Runtime 后不在 Rust Kernel 中重复建设第二套通用 Tool Loop。
- 将视频虚拟制作团队方向从通用 Agent 基座定位中拆分为视频领域扩展，后续编程、知识研究和其他工作台复用同一 Pi Runtime。
- 在顶层 Docker Compose 中加入持久化的本地 Agent Runtime 服务及健康检查。

## Capabilities

### New Capabilities
- `personal-agent-runtime`: 本地单用户个人 Agent Runtime 的会话、流式执行、工具循环、控制命令和领域边界。
- `local-agent-session-persistence`: 基于 Pi SQLite Session Storage 的会话树、分支、压缩与重启恢复。

### Modified Capabilities
- `novex-foundation-architecture`: 将长期定位改为本地单用户个人 AI 工作台，并允许 TypeScript Pi Runtime 作为通用 Agent 执行服务。
- `agent-runtime-kernel`: 明确 Rust Kernel 保留业务 Adapter/Run 生命周期，Pi Harness 承担新的通用 Turn/Tool Loop，避免双内核职责重叠。
- `model-routed-ai-execution`: 本地 Pi Runtime 必须继续按 `model_id` 使用 PostgreSQL 模型注册表和非敏感快照，不得形成第二事实源。
- `environment-bootstrap`: 顶层 Compose 必须识别、启动并健康检查本地 Agent Runtime，并持久化 SQLite 会话数据。

## Impact

- 新增 `services/agent-runtime/` TypeScript/Node.js 服务、Pi npm 依赖、SQLite 数据卷、HTTP/SSE Runtime API 和直接相关测试。
- 调整 `docker-compose.yml`、环境示例、架构文档、项目 memory 与服务健康检查说明。
- 现有 `backend`、`apps/video-agent`、PostgreSQL 业务表、模型管理 API 和视频 Worker 外部行为保持不变。
- Pi 采用 MIT License；分发依赖及任何复制源码必须保留许可证声明。
