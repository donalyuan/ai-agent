## Context

`backend` 当前是 Axum + SQLx 模块化单体。Repository 已按领域拆分，但 `lib.rs`、`agents/models/request.rs` 和 `agents/conversational_runtime.rs` 分别聚合了 HTTP、DTO 与多类 Agent 编排，且部分 Repository 依赖 `agents::models`。本次变更必须在不改变 HTTP、数据库和模型调用行为的前提下调整源代码边界，并与进行中的 `script-to-asset-generation` change 保持独立。

## Goals / Non-Goals

**Goals:**

- 固定 `bootstrap -> api -> application -> domain` 的主依赖方向。
- 让 API 按业务模块保存路由、handler 和 DTO。
- 让 Application Service 承担用例和跨 Repository 编排。
- 让 Domain 类型脱离 Axum、SQLx、API DTO 和 Agent Runtime。
- 保留统一 Agent Runtime 入口，同时拆分脚本、选题生成、质量闸门和主题组评审实现。
- 一次性切换仓库内公共模块路径并补充必要注释。

**Non-Goals:**

- 不改变任何外部 API、数据库结构、模型调用策略或业务状态流转。
- 不拆分全部 Repository trait 与 PostgreSQL adapter。
- 不将当前 video-agent 业务 Runtime 迁入 `crates/novex-agent`。
- 不修改其他 OpenSpec change 的规格和任务状态。

## Decisions

### 1. 采用业务模块组织与分层边界结合的结构

`backend/src` 新增 `bootstrap`、`api`、`application` 和 `domain`。`api` 内按 `projects`、`topics`、`materials`、`conversations`、`scripts`、`asset_generation`、`ai_models` 和 `workspace` 组织，每个模块拥有自己的 DTO、handler 和路由组合。

相比纯技术分层，该结构减少修改单一业务时的跨目录跳转；相比严格 Clean Architecture，它不要求本轮重写所有持久化端口，能控制范围并仍然建立清晰依赖方向。

### 2. HTTP handler 只处理传输职责

handler 只提取路径、查询和 JSON，执行 DTO 校验，调用 Application Service，并把结果或错误映射为 HTTP 响应。现有 handler 内的 Repository 协作、模型调用、重试、幂等和状态流转迁入对应 Application Service。

简单 CRUD 同样经过 Application Service，避免业务规则在后续迭代中重新回流到 API 层。

### 3. 领域模型使用新的唯一模块路径

脚本、选题和对话实体、枚举及状态解析错误迁入 `domain`。Repository 返回领域对象，不再依赖 `agents::models`。所有生产代码和测试同步更新 import，不通过 `pub use` 暴露旧路径。

选择一次性迁移是因为 `novex-api` 是 monorepo 内部服务 crate，当前没有公开 SDK 兼容要求；保留两套路由只会增加长期维护成本。

### 4. Agent Runtime 采用统一门面加能力模块

统一 `AgentRuntime` 继续负责加载会话、持久化本轮消息与运行记录、识别 `agent_type` 并分派。脚本、选题生成、质量闸门和主题组评审分别迁入独立模块，Prompt、共享类型和错误也独立保存。

拆分不得改变原有 run/step 写入顺序、失败收尾、模型快照、同模型重试、主题组归一和 assistant metadata。

### 5. 错误按层转换

Domain 错误不包含 HTTP 类型；Application 错误包装 Repository、模型和业务校验失败；API 层将其映射到既有状态码和 JSON。公共 JSON rejection 与内部错误结构放在 `api/error.rs`，业务特有映射留在对应模块，避免新的全局巨型错误文件。

### 6. 注释解释边界和业务原因

模块顶部说明职责与依赖限制；公共 Service 和 Agent 入口使用 Rust 文档注释；重试、幂等、质量闸门、主题组归一、失败收尾和事务顺序解释设计原因。普通赋值、显而易见分支和 CRUD 不添加复述型注释。

## Risks / Trade-offs

- [大规模模块迁移导致 import 或路由遗漏] -> 重构前记录全量测试基线，按业务模块迁移并全仓库搜索旧路径。
- [移动代码时无意改变错误响应] -> 保留现有路由集成测试，并逐项对比 URL、方法、状态码和 JSON 字段。
- [Agent Runtime 拆分破坏执行顺序] -> 以现有 Runtime 测试覆盖消息、run、step、模型快照和失败路径，先机械迁移再调整可见性。
- [Application Service 变成新的巨型层] -> 按业务用例拆文件，Agent 能力再按生成、质量和评审拆分；禁止全局万能 Service。
- [不实施完整端口/适配器分离仍保留部分技术债] -> 本轮先解决 API 与 Runtime 的已确认范围，Repository 端口重构另立 change，避免把结构迁移扩大成全后端重写。

## Migration Plan

1. 在 Compose API 容器运行 `cargo test --workspace` 建立基线。
2. 创建 `domain` 并迁移领域类型，更新 Repository 和测试 import。
3. 创建 `bootstrap` 并迁移配置、状态和运行时构建。
4. 创建 Application Service 并迁移 handler 内的业务编排。
5. 按功能创建 API 模块并迁移 DTO、handler、错误映射和路由。
6. 拆分 Agent Runtime，保持统一入口和执行顺序。
7. 删除旧聚合实现并搜索旧模块路径与兼容导出。
8. 运行格式检查、全量测试和 Clippy，再核对 OpenSpec tasks 状态。

本变更不包含数据迁移或灰度双写。若验证失败，回退对应源代码迁移批次即可，不需要数据库回滚。

## Open Questions

无。范围、模块路径迁移策略、错误边界、注释标准和验收方式均已确认。
