## Why

阶段 0 已验证工程边界，但 API 仍只有健康检查，数据库模型也没有一个可调用的业务垂直切片。现在先实现项目与剧集，使模块化单体的领域、应用、持久化和 HTTP 边界获得一个可回归的最小事实源，后续场次、镜头和资产可以在其上增量迁移。

## What Changes

- 新增 `Project` 与 `Episode` 的领域规则、状态和乐观并发语义。
- 新增 `projects/episodes` 的 application command/query、Repository 和 Unit of Work 边界，并由 SQLAlchemy 适配器实现。
- 暴露创建、读取和列表 HTTP API；更新使用 `If-Match`/revision，冲突返回可诊断的 `409`。
- 为项目/剧集的父级归属、显示编号和状态约束补充数据库迁移与回归测试。
- 增加领域、应用、HTTP、架构依赖和 BDD/TDD 测试，并更新 OpenSpec 追溯与项目记忆。

明确不改变 `Scene`、`Shot`、`Asset`、`Workflow`、`Timeline`、Provider、Skill、Worker 或前端产品能力；不接入真实 Provider、TOS、AgentScope、SSE、Outbox 或媒体处理。

## Capabilities

### New Capabilities

- `projects-episodes-slice`: 项目与剧集的领域模型、并发规则、应用用例和持久化边界。
- `projects-episodes-http-api`: 项目与剧集的 HTTP 请求/响应、错误映射和健康运行时集成。

### Modified Capabilities

无。

## Impact

- `services/api/src/video_agent_api/domain/`、新增 `application/`、`adapters/` 与 `interfaces/` 模块。
- `services/api/src/video_agent_api/app.py`、数据库迁移和 `services/api/tests/`。
- `openspec/changes/implement-projects-episodes-slice/`、`docs/phase-zero-traceability.md`、`docs/agent/PROJECT.md`、`docs/agent/HANDOFF.md`。
- 保持现有 Compose、共享 Schema、Mock Provider/LocalWorkspaceAdapter 和阶段 0 非目标不变。
