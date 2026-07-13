# Rust API 与 Agent Runtime 分层重构设计

## 1. 背景

当前 `backend` 已按领域拆分 Repository，但 HTTP API、请求响应 DTO 和 Agent 编排仍集中在少数巨型文件中：

- `backend/src/lib.rs` 同时承担应用配置、状态构建、路由注册、HTTP handler、业务编排和错误转换。
- `backend/src/agents/models/request.rs` 混合项目、选题、素材、脚本、对话和素材生成 DTO。
- `backend/src/agents/conversational_runtime.rs` 混合 Agent 分派、脚本生成与修改、选题生成、质量闸门、主题组评审、Prompt 和错误定义。
- Repository 返回的部分领域对象依赖 `agents::models`，导致数据层反向依赖 Agent 模块。

这些结构使功能边界难以辨认，也增加修改时的回归范围。本次重构通过模块重组和依赖约束解决根因，不采用仅移动代码或保留旧路径的过渡兼容方案。

## 2. 目标与非目标

### 2.1 目标

1. 将 HTTP 传输、应用用例、领域模型、Agent 编排和持久化职责分离。
2. API 按业务功能拆分，避免重新形成技术层巨型文件。
3. 将统一 `AgentRuntime` 保留为入口，并把不同 Agent 能力拆成独立模块。
4. 将 HTTP DTO 从 Agent 模型中移出，将脚本、选题和对话领域类型迁入 `domain`。
5. 所有仓库内调用方和测试切换到新模块路径，不保留旧路径 `pub use` 兼容层。
6. 为模块职责、业务不变量、重试、幂等和复杂编排添加必要注释。

### 2.2 非目标

1. 不新增或删除 HTTP API，不修改 URL、HTTP 方法、状态码、JSON 字段或错误协议。
2. 不修改数据库表、migration、SQL 语义和数据生命周期。
3. 不改变模型选择、调用协议、超时、重试、Prompt 语义或成本控制。
4. 不把业务型 Runtime 强行迁入 `crates/novex-agent`；当前实现尚未形成可复用通用边界。
5. 不实施全仓库严格 Clean Architecture，不拆分全部 Repository trait 与 PostgreSQL adapter。
6. 不改变进行中的 `script-to-asset-generation` OpenSpec 行为规格或任务状态。

## 3. 方案选择

本次采用“业务模块组织 + 明确分层”方案：

```text
bootstrap
   |
   v
api -> application -> domain
          |
          v
     repositories / novex-model
```

未采用纯技术分层，因为单一业务会被分散到多个顶层目录；未采用严格 Clean/Hexagonal Architecture，因为它要求同时重写全部 Repository 端口和 adapter，超出已确认的 API 与 Agent Runtime 范围。

## 4. 目标目录与职责

```text
backend/src/
  lib.rs
  main.rs
  bootstrap/
    mod.rs
    config.rs
    state.rs
    runtime.rs
  api/
    mod.rs
    router.rs
    error.rs
    health.rs
    ai_models/
    workspace/
    projects/
    topics/
    materials/
    conversations/
    scripts/
    asset_generation/
  application/
    mod.rs
    ai_models.rs
    projects.rs
    topics.rs
    materials.rs
    conversations.rs
    scripts.rs
    asset_generation.rs
    agents/
      mod.rs
      runtime/
        mod.rs
        types.rs
        error.rs
        script.rs
        topic_generation.rs
        topic_quality.rs
        topic_review.rs
        prompt.rs
  domain/
    mod.rs
    conversation.rs
    script.rs
    topic.rs
  repositories/
  model_config_import.rs
  model_routing.rs
```

职责约束如下：

- `lib.rs` 只声明并导出稳定的新模块，以及应用构建入口。
- `bootstrap` 读取配置并组装数据库、Redis、模型解析器、Application Service 和 Router。
- `api` 只处理 HTTP 提取、DTO 校验、调用 Application Service 和响应转换。
- `application` 负责用例顺序、事务边界、Repository 协作和模型调用。
- `application::agents::runtime` 负责统一 Agent 分派及各 Agent 能力实现。
- `domain` 保存不依赖 Axum 和 SQLx 的业务实体、枚举、状态规则及解析错误。
- `repositories` 负责持久化并返回领域对象，不得依赖 `api` 或 `application::agents`。

## 5. 依赖规则

1. `domain` 不得依赖 `api`、`application`、`repositories`、Axum 或 SQLx。
2. `api` 不得直接执行 SQL、构造 Prompt 或持有业务状态机。
3. `repositories` 不得依赖 HTTP DTO 或 Agent Runtime 类型。
4. 业务模块不得调用其他业务模块的 handler。
5. 跨模块协作通过 Application Service、Repository 接口或领域类型完成。
6. `AgentRuntime` 只识别 Agent 类型、加载对话上下文并分派，具体能力由子模块实现。
7. 简单 CRUD 也进入 Application Service，避免规则重新回流到 handler。

## 6. 数据流

普通业务请求：

```text
HTTP Request
  -> feature handler
  -> request DTO validation
  -> Application Service
  -> Repository
  -> Domain Result
  -> response DTO
  -> HTTP Response
```

对话 Agent 请求：

```text
HTTP Request
  -> ConversationApplicationService
  -> AgentRuntime
  -> Script / TopicGeneration / TopicQuality / TopicReview
  -> Repository + ModelClientResolver
  -> Domain Result
  -> Agent response DTO
  -> HTTP Response
```

原有对话、run、step 的写入顺序、失败收尾、模型快照和主题组归一逻辑保持不变。

## 7. 错误处理

1. `domain` 错误只描述业务失败，不包含 HTTP 类型。
2. `application` 用例错误包装 Repository、模型调用、校验和状态冲突。
3. `api` 将 Application 错误映射为现有状态码和 JSON 错误结构。
4. 公共 JSON rejection 和内部错误响应结构放在 `api/error.rs`。
5. 业务特有映射保留在对应 API 模块，避免形成新的全局巨型错误文件。
6. 数据库和上游模型内部细节不得直接返回客户端。

## 8. 注释标准

必须添加注释的位置：

- 模块顶部说明职责、允许依赖和禁止承担的职责。
- 公共 Application Service、Agent 入口和关键输入输出类型使用 Rust 文档注释。
- Agent 分派、失败收尾、重试、质量闸门、主题组归一和幂等键解释设计原因与不变量。
- Repository 中依赖特定事务顺序或状态一致性的代码说明原因。

禁止添加的注释：

- 复述变量赋值、分支判断和普通 CRUD 表面行为。
- 与实现不一致的未来规划。
- 为满足注释数量而添加的无业务信息说明。

## 9. 迁移策略

迁移按可独立验证的顺序进行：

1. 运行全量测试建立重构前基线。
2. 创建 `domain`，迁移脚本、选题和对话领域类型，并更新 Repository 与测试 import。
3. 创建 `bootstrap`，迁移配置、状态与运行时构建。
4. 创建 `application`，把 handler 中的业务编排迁入对应 Service。
5. 创建按功能拆分的 `api`，迁移 DTO、handler、错误映射和路由。
6. 拆分 Agent Runtime，并让统一入口委派到各能力模块。
7. 删除旧 `agents/models/request.rs`、`agents/conversational_runtime.rs` 和 `lib.rs` 中的旧实现。
8. 全仓库搜索旧模块路径，确保没有兼容导出或残留调用。

迁移期间不允许保留两套可工作的入口作为长期状态。每个阶段完成时必须同时切换调用方和测试。

## 10. 测试与验收

### 10.1 验证顺序

1. 重构前在 Compose API 容器执行 `cargo test --workspace` 并记录基线。
2. 每迁移一个业务模块，运行其路由、Repository 和 Application Service 相关测试。
3. Agent Runtime 拆分后运行脚本 Agent、选题生成、质量闸门、主题评审和连续对话测试。
4. 最后在容器内执行：

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### 10.2 验收条件

1. 所有现有测试通过，且没有因重构删除或弱化断言。
2. 所有路由、HTTP 方法、状态码和响应字段保持不变。
3. `lib.rs` 仅保留模块声明和应用入口，不再包含业务 handler。
4. 旧巨型 `request.rs` 和 `conversational_runtime.rs` 被删除，职责迁入明确模块。
5. 不产生新的千行聚合文件；文件拆分首先以单一职责为准。
6. 全仓库不存在旧公共模块路径兼容层。
7. `domain` 不依赖 Axum、SQLx、API DTO 或 Agent Runtime。
8. 必要注释覆盖复杂业务规则，同时不存在大面积复述型注释。

## 11. DDD、BDD、SDD、TDD 审视

- `DDD`：领域实体和状态规则迁入 `domain`；HTTP DTO 与 Agent 编排不再充当领域模型容器。
- `BDD`：用户可观察的 API、对话、脚本、选题和素材生成行为保持不变。
- `SDD`：新模块路径一次性切换，不保留兼容层；不改协议、数据库和 OpenSpec 行为规格。
- `TDD`：以重构前全量测试为基线，按模块验证，最终执行格式、测试和 Clippy 全量检查。
