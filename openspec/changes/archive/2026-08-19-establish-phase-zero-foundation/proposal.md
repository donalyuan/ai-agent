## Why

视频 Agent 工作台已经确认产品和技术方向，但仓库尚无可运行的工程、领域契约或可验证的本地运行基线。阶段 0 先把这些边界固化为可启动、无真实凭据可测试的单仓库基础，避免后续阶段分别发明不兼容的数据、Provider 与 Worker 约定。

## What Changes

- 建立 React 19 + TypeScript + Vite 8 Web、FastAPI + Pydantic 2 API、PostgreSQL + Alembic、共享 JSON Schema 与 Worker 的单仓库骨架。
- 定义 Project、Episode、Scene、Shot、Asset、AssetVersion、WorkflowDraft、WorkflowVersion 与 TimelineDocument 的版本化 JSON Schema，以及其数据库基础领域模型。
- 建立六个业务 Port、数据驱动的 Provider/Profile/Model 配置、`LocalWorkspaceAdapter`、Mock Provider 与不含真实 API Key 的启动路径。
- 建立自有 `SkillRegistry`、确定性 `SkillRouter` 和可选 semantic adapter 的边界，而不实现语义模型或多 Agent 产品能力。
- 建立包含 Web、API、PostgreSQL、Temporal、Agent、Generation、Media Worker 的 Docker Compose 基线、健康检查、结构化日志、类型/格式/迁移/契约测试和示例配置。

## Capabilities

### New Capabilities

- `local-engineering-runtime`: 单仓库目录、Compose 服务、健康检查、示例配置和无真实密钥的本地启动行为。
- `versioned-domain-contracts`: 九个 JSON Schema、基础 DDD 领域持久化模型和跨层版本/引用约束。
- `provider-and-storage-boundaries`: 六个 Port、可配置的 Provider/Profile/Model、Mock Provider 与本地工作区存储适配器。
- `skill-routing-foundation`: 自有 SkillRegistry、确定性路由与可选语义排序适配边界。
- `foundation-quality-gates`: 阶段 0 的结构化日志、TDD 测试、类型/格式/迁移/Schema 验证与 BDD 验收基线。

### Modified Capabilities

- 无。

## Impact

新增后续实现所需的应用目录、共享契约包、Python 服务与 Worker、Docker Compose、数据库迁移、示例环境配置和基础测试。不会接入真实 Provider、真实 TOS 凭据或任何付费调用，也不会实现完整生成、专业剪辑、多人、手机端、发布平台、TikTok、多 Agent 产品能力或语义模型。
