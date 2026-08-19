# 后端工程架构设计

## Context

阶段 0 已建立 `services/api`、三类 Worker、共享 JSON Schema、Provider/Storage Port 和 Docker Compose。当前 API 代码集中在 `app.py`、`db.py`、`domain/`、`ports/` 与 `skills/`，适合作为最小可执行基础，但不足以承载多集短剧、素材版本、时间线、Provider 管理和长任务编排。

本设计面向个人本地项目，优先控制认知和运维成本，同时保留未来拆分 Worker 或服务的边界。它定义目标结构，不要求在本次文档变更中重构阶段 0 代码。

## Goals / Non-Goals

**Goals:**

- 为 FastAPI 模块化单体定义可复制的业务模块模板和依赖规则。
- 明确 DDD 所有权、应用命令、事务、事件、Temporal 和 Worker 之间的关系。
- 让 API、Worker 和 Adapter 复用应用能力，而不是复制业务规则。
- 建立可由架构测试和分层测试验证的约束。
- 给出从阶段 0 平铺代码到目标结构的增量迁移方法。

**Non-Goals:**

- 不在本变更中移动后端代码或新增业务 API。
- 不拆分微服务，不新增 Redis、消息中间件或独立语义路由服务。
- 不定义真实 Provider、TOS 或媒体功能的具体实现细节。
- 不改变 `packages/contracts` 作为跨 Web/API/Worker 文档契约权威来源的决定。

## Decisions

### 1. 模块化单体承载业务能力

FastAPI 采用 `bootstrap`、`shared`、`modules`、`interfaces` 和 `infrastructure` 五个顶层区域。`modules` 按业务能力组织，每个模块内部再分 `domain`、`application`、`infrastructure` 与 `interfaces`。模块是所有权边界，目录层是依赖边界。

目标顶层结构：

```text
video_agent_api/
  bootstrap/
  shared/{domain,application,infrastructure}/
  modules/
    projects/ episodes/ assets/ workflows/ timelines/
    providers/ skills/ reviews/ usage/ exports/
  interfaces/{http,events}/
  infrastructure/{persistence,temporal,providers,storage,security,observability}/
```

每个复杂模块使用统一模板；简单模块可以省略空文件，但不能绕过依赖规则：

```text
<module>/
  domain/{entities,value_objects,events,policies}.py
  application/{commands,queries,handlers,dto,ports}.py
  infrastructure/{orm,repositories}.py
  interfaces/http.py
```

### 2. 依赖只能向内

允许的依赖是 `interfaces -> application -> domain`；`infrastructure` 实现 application/domain 声明的 Port，并由 composition root 注入。`domain` 不依赖 FastAPI、Pydantic HTTP DTO、SQLAlchemy、Temporal、AgentScope、FFmpeg 或 Provider SDK。

模块之间不得导入对方的 ORM、Repository 实现或私有实体。协作优先通过对方 application facade、稳定 ID、版本化共享契约或领域事件。`shared` 仅容纳真正跨模块且稳定的原语，不成为杂物目录。

### 3. 一个应用命令对应一个事务

Command Handler 是写入用例入口，通过 `UnitOfWork` 加载聚合、调用领域行为、保存聚合并提交。Repository 接口位于 application/domain 边界，实现位于 infrastructure。事务提交同时写入 Outbox；外部调用、对象存储、FFmpeg 和 Temporal 调用不得发生在数据库事务中。

Query Handler 可使用专用只读查询和 DTO，不要求通过聚合重建页面投影，但不能承载写入副作用。跨聚合强一致仅用于同一业务不变量；其他协作通过事件和可恢复流程实现。

### 4. FastAPI 是传输适配器

HTTP 路由只负责鉴权、请求解析、边界校验、调用 command/query handler、响应序列化和异常映射。路由不得直接访问 SQLAlchemy Session、Provider SDK、Storage SDK 或 FFmpeg。

统一异常映射：资源不存在为 `404`，`revision`/幂等冲突为 `409`，领域规则或结构化输入错误为 `422`，认证授权错误为 `401/403`，上游拒绝或协议错误为可诊断的 `502`，外部依赖暂不可用为 `503`。响应携带稳定 `error_code`、`message`、`trace_id` 和允许公开的 `details`。

### 5. Temporal 只做确定性编排

Workflow 只保存确定性状态、分支、等待、重试策略和 Activity/Child Workflow 调用，不访问网络、数据库、当前时间、随机数、本地文件或 SDK。Activity 执行 Provider、数据库、TOS、AgentScope 和 FFmpeg 等副作用，并携带业务幂等键。

API 通过 application handler 创建业务 Run 和 Outbox 命令；Temporal starter 在提交后启动稳定 Workflow ID。Worker 入口只装配 Workflow、Activity、application service 和 adapter，不复制领域规则。Agent、Generation、Media 使用独立 task queue 和凭据边界。

### 6. 持久事件是实时状态事实源

领域事件与业务修改在同一事务写入 Outbox。Publisher 将事件转换为运行事件、SSE 通知或后续集成消息。SSE 按持久化单调序号和 `Last-Event-ID` 补发，进程内队列仅用于唤醒连接，不能成为事实源。

### 7. 配置和装配集中管理

环境变量和 Docker Secret 只由 `bootstrap/settings` 读取；数据库中的 Provider、Model、Storage 配置由对应 Repository 加载。composition root 创建 Session/UoW、handlers、Temporal client 和 adapters。测试可以以 fake/in-memory Port 替换外部实现，业务模块不得读取全局环境或自行创建客户端。

### 8. 测试按边界分层

- `tests/unit/<module>/domain`：实体、值对象、策略和领域事件，无数据库和网络。
- `tests/unit/<module>/application`：handler、Port fake、事务和错误分支。
- `tests/adapters`：SQLAlchemy Repository、Provider、Storage、加密和 FFmpeg adapter 契约。
- `tests/integration`：PostgreSQL、Alembic、Outbox、Temporal test environment 和 HTTP 组合。
- `tests/architecture`：禁止依赖、模块私有边界和业务代码固定 Provider 配置扫描。
- `tests/contract`：OpenAPI、JSON Schema、事件 payload 和跨进程契约。
- `tests/bdd`：以用户可观察场景串联 API、Workflow、Activity、事件和持久化结果。

测试顺序遵循 TDD：先写失败测试，再实现最小行为。外部适配器使用录制样例或明确的沙箱测试，默认测试不得产生付费调用。

### 9. 以功能切片渐进迁移

当前 `app.py`、`db.py`、`domain/`、`ports/` 和 `skills/` 保持可运行。后续每个功能 change 选择一个垂直切片，先建立目标模块与测试，再迁移路由、handler、Repository 和 adapter；旧入口只做兼容委派，最后删除已无调用的平铺实现。

迁移期间禁止同一业务规则在旧服务和新模块各维护一份。数据库表名和跨层 JSON Schema 不因移动目录而改变；需要修改契约或迁移数据时必须另建 OpenSpec change。

## Risks / Trade-offs

- [模块内四层产生文件数量] → 简单模块允许合并同层文件，但不允许反向依赖或绕过应用入口。
- [共享内核不断膨胀] → 新增共享代码必须至少被两个模块稳定复用，并通过架构评审；业务概念仍归属单一模块。
- [Outbox 与 Temporal 启动存在最终一致窗口] → 稳定 Workflow ID、Outbox 重试和幂等 starter 保证可恢复。
- [迁移期新旧结构并存] → 每个 change 记录模块迁移状态，架构测试禁止新增对旧平铺模块的依赖。
- [Repository 过度抽象] → 接口围绕聚合和用例定义，不暴露通用 CRUD 或 SQLAlchemy Query。

## Migration Plan

1. 建立 `bootstrap`、共享错误/UoW、架构测试和 composition root，不改变现有 API 行为。
2. 选择 Projects/Episodes 作为首个垂直切片，迁移领域、handler、Repository 和 HTTP 路由。
3. 迁移 Assets/Workflows/Timelines，并引入 Outbox 与持久化事件读取。
4. 迁移 Providers/Skills/Usage，再把真实 Adapter 装配到 Generation/Agent Worker。
5. 接入 Temporal Workflow/Activity 和 Media Worker；通过 BDD 场景验证运行、重试、取消与 SSE 补发。
6. 删除无调用的阶段 0 兼容层，更新 ADR、模块图和交接记录。

回滚以功能切片为单位：保留原路由委派和数据库兼容，撤销新 composition wiring；不得用目录级回滚掩盖已经发布的 Schema 或 migration 变化。
