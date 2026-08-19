## Context

当前 API 只有健康检查，虽然阶段 0 已有 `Project`/`Episode` SQLAlchemy 表，但业务代码没有领域对象、应用用例、Repository/UoW 或 HTTP 适配边界。目标架构要求从阶段 0 平铺入口按垂直功能切片迁移；本 change 只选择 `projects/episodes`，并保留现有 Mock/Local、Worker 和 Compose 边界。

## Goals / Non-Goals

**Goals:**

- 用纯 Python 领域对象表达项目和剧集的名称、父级归属、编号、状态与 revision 规则。
- 用 application command/query 服务协调领域对象和抽象 Repository/UoW。
- 用 SQLAlchemy adapter 实现 PostgreSQL/SQLite 可测试持久化，并提供可替换的内存 adapter。
- 提供最小项目/剧集 HTTP API，使用 camelCase JSON、稳定错误码和 `If-Match` 乐观锁。
- 以迁移、架构依赖检查、契约测试和 BDD/TDD 测试证明该切片可运行。

**Non-Goals:**

- 不实现 Scene、Shot、Asset、Workflow、Timeline、Provider、Skill、SSE、Outbox、Temporal 业务 workflow 或前端产品页面。
- 不引入真实 Provider、TOS、AgentScope、FFmpeg、外部网络或付费调用。
- 不一次性移动现有所有阶段 0 模块；旧健康/runtime/ports 入口继续作为兼容边界。

## Decisions

- **纯领域对象 + ORM 映射**：领域层使用 dataclass 和显式行为，SQLAlchemy 只出现在 adapter。这样可测试状态与并发规则，也避免把数据库列定义当成领域 API。替代方案是直接把 ORM 当领域对象，代价是违反目标依赖方向并让单元测试依赖数据库。
- **Repository/UoW 以 Protocol 定义**：application 只依赖最小异步接口；生产使用 SQLAlchemy adapter，测试使用共享状态的内存 adapter。替代方案是让 service 直接接收 `AsyncSession`，会把事务和查询泄漏到应用层。
- **HTTP 以独立 router 和 Pydantic DTO 适配**：请求/响应模型保留 camelCase，`If-Match` 解析为期望 revision；领域异常集中映射为 404/409/422。替代方案是让 FastAPI 路由直接操作 ORM，无法复用命令并难以验证边界。
- **明确契约转换边界**：`packages/contracts` 的持久化/领域 JSON Schema 保留 `schema_version` snake_case；HTTP DTO 按 API 约定输出 `schemaVersion` camelCase。两者不是同一个 JSON 实例，转换由 Pydantic alias 显式完成，并由 HTTP contract tests 验证，避免把传输字段误写回领域 Schema。
- **显示编号由数据库唯一约束保护**：`Episode.number` 在 API/领域层从 1 开始，ORM 暂存为兼容现有的 `display_number` 列，并增加 `title`；同一项目重复编号由唯一约束和应用诊断共同拒绝。替代方案是仅在 Python 检查，会在并发请求下产生重复事实。
- **默认无数据库时显式不可用**：未配置 `DATABASE_URL` 的进程仍可启动和健康检查；业务端点返回明确的 `503 database_unavailable`，测试通过注入内存 UoW。不得静默降级到内存数据。

## Risks / Trade-offs

- [迁移与既有阶段 0 表不一致] -> 增量 migration 只增加 `episodes.title`、正数检查和父级唯一约束；保留旧列名映射并执行 upgrade/downgrade 回归。
- [数据库唯一约束错误码依赖后端] -> SQLAlchemy adapter 将 `IntegrityError` 转为稳定的 domain conflict，未知数据库错误继续抛出。
- [应用层暂时只覆盖一个垂直切片] -> 明确旧入口仍为迁移起点，并用架构测试防止新 application/domain 依赖 FastAPI/SQLAlchemy。
- [无数据库开发体验] -> 健康接口继续可用，业务端点显式报告不可用；不会把进程内 adapter 作为生产 fallback。

## Migration Plan

1. 应用 `0003_projects_episodes_slice`，为既有 `episodes` 增加可回填的 `title`、正数检查和 `(project_id, display_number)` 唯一约束。
2. 部署 API 的新 domain/application/adapter/interfaces 代码；已有健康端点和 runtime 配置不变。
3. 运行项目/剧集单元、契约、HTTP、架构和 migration 回归；失败时回滚代码与 `0003`，不触碰 `0001`/`0002`。

## Open Questions

无。本切片不决定后续 Scene/Shot 的 API 形状，也不引入前端业务界面。
