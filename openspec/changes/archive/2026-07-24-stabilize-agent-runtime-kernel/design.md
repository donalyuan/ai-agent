## Context

当前 `AgentRuntime` 位于 `backend/src/application/agents/runtime`，统一保存消息和 Run 后，通过 `match conversation.agent_type` 调用脚本、选题、声音或作品处理方法。Runtime 同时持有多个业务 Repository，部分依赖以 `Option` 注入；`AgentTurnRequest` 也包含选题专属字段，声音能力则通过额外方法参数传入。这使核心入口认识具体业务、新 Agent 必须修改核心分支、依赖错误延迟到请求期暴露，也阻碍 `novex-agent` 和 `novex-ai-core` 成为真正可复用的基座。

现有行为已经由对话、模型路由、脚本、选题、声音和作品测试覆盖。本 change 必须把这些测试作为重构边界，不修改 HTTP、数据库、Prompt、模型调用和消息 metadata 语义。

## Goals / Non-Goals

**Goals:**

- 建立不认识具体业务类型的 Agent 执行内核和注册机制。
- 让 Kernel 统一拥有单轮消息与 Run 的生命周期，Adapter 只处理业务能力。
- 让业务专属输入在 Adapter 边界完成结构化解析和校验。
- 让具体 Repository 归对应业务 Adapter 持有，并在 Bootstrap 阶段完成依赖校验。
- 把可复用的执行契约放入 `novex-ai-core` / `novex-agent`，保持单向依赖。
- 保持现有脚本、选题、声音和作品 Agent 外部行为及审计记录可回归。

**Non-Goals:**

- 不引入新的角色 Prompt、`PromptDefinition`、Context Compiler 或增强运行快照。
- 不实现 Memory、Tool Registry、Tool Loop、自主 Planner 或虚拟制作团队工作流。
- 不新增 Agent 管理后台或动态插件安装。
- 不修改公开 API、数据库 schema、模型选择方式、业务状态机和前端。
- 不把所有业务逻辑搬进基础 crates；业务规则继续留在 `backend` Adapter 和 Application Service。

## Decisions

### DDD：执行内核与业务 Adapter 分离

核心领域对象如下：

- `AgentKey`：稳定的 Agent 类型标识，不包含业务实现。
- `AgentInvocation`：本轮通用输入，包含用户内容和已持久化、可审计的业务 payload。
- `AgentExecutionContext`：包含 conversation、project、subject、run、模型执行引用和 Step Recorder，但不包含具体业务 Repository。
- `AgentOutcome`：Adapter 返回的 Assistant 草稿、metadata 和运行输出摘要。
- `AgentAdapter`：声明 `AgentKey`，校验并执行本 Agent 的业务 payload。
- `AgentRegistry`：在启动期注册和解析 Adapter。
- `AgentRunCoordinator`：负责单轮生命周期，不承担脚本、选题、声音或作品规则。

选择 Registry + trait object，而不是在 Kernel 中维护业务枚举。业务枚举仍会要求每次新增 Agent 修改 Kernel；动态字符串到未校验闭包又缺少稳定契约。Registry 在保持 Rust trait 契约的同时允许 Bootstrap 组合不同 Adapter。

### BDD：Coordinator 统一单轮生命周期

执行顺序固定为：

1. 校验通用消息并加载会话。
2. 按 `AgentKey` 解析已注册 Adapter。
3. 保存用户消息并创建 running Run。
4. 构造 `AgentExecutionContext`，调用 Adapter。
5. Adapter 通过 Step Recorder 按现有顺序记录业务步骤并返回 `AgentOutcome`。
6. 保存 Assistant 消息并把 Run 结束为 `succeeded`；若 Adapter 或完成前持久化失败，则把已创建 Run 结束为 `failed`。

Adapter 不再直接完成 Run。Coordinator 内部使用显式状态守卫，防止同一 Run 被成功和失败路径重复收尾。Assistant 消息只从成功的 `AgentOutcome` 创建，失败路径不得伪造回复。

选择由 Coordinator 统一生命周期，而不是由每个 Adapter 自行保存消息和收尾 Run，因为后者会继续复制失败语义并使回放数据不一致。

### BDD：会话与非会话任务共享唯一生命周期语义

`AgentExecutor` 是 backend 调用 `AgentRunCoordinator` 的唯一正式门面，负责 Kernel 错误下转和 Domain 结果映射；`ConversationService` 与集成测试不得各自重复组装和调用 Coordinator。

脚本生成、项目策略草稿、主题组评审不产生标准 Conversation 消息，但仍属于可审计 Agent Run。它们通过通用 `RunLifecycleCoordinator` 执行 `start -> operation -> succeeded/failed`，业务层提供 `StartRun`、实际操作、成功输出和失败审计文本映射。失败收尾应保留原始业务错误，成功收尾失败则返回存储错误。

选择独立的通用 Run 生命周期协调器，而不是伪造 Conversation 交给 `AgentRunCoordinator`，因为非会话任务不应保存虚假的 User/Assistant 消息；同时迁移仓库中全部同类直接 Run 编排，避免只消除主题组评审一处重复。

### SDD：crate 与依赖方向

依赖方向固定为：

```text
backend
  -> novex-agent
       -> novex-ai-core
       -> novex-model
  -> novex-model
```

- `novex-ai-core` 放置 `AgentKey`、Run/Step 状态和值对象，不依赖 Axum、SQLx 或 `backend`。
- `novex-agent` 放置 Adapter、Registry、ExecutionContext、Coordinator、Session/Run Recorder ports 和通用错误。
- `backend` 实现 PostgreSQL ports、具体 Adapter、业务 payload 类型、错误到 HTTP 的映射以及 Bootstrap 注册。
- 现有 Prompt 构建、LLM 输出解析、质量闸门、业务 Repository 和状态流转继续留在 `backend`。

不得通过 `pub use` 保留旧 Runtime 公共路径兼容层。生产代码和测试在同一 change 中迁移到唯一新路径，避免形成两个可用入口。

### SDD：业务 payload 使用 Schema 边界

统一 envelope 只包含所有 Agent 共用字段；选题补充批次、声音编辑快照等业务参数进入 `payload`。Kernel 将 payload 作为可持久化结构传给 Adapter，Adapter 必须立即反序列化为自己的强类型输入并执行完整校验。

选择“持久化 JSON envelope + Adapter 强类型解码”，而不是把业务字段继续加入公共请求，也不使用不可序列化的 `Any`。第一版不引入通用 JSON Schema 引擎；Adapter 的 Rust 类型和校验函数是权威契约，现有 HTTP DTO 保持不变。

### SDD：启动期注册和依赖失败

Bootstrap 构造每个 Adapter 时注入其必需 Repository 和服务，然后注册到 `AgentRegistry`。以下情况必须阻止应用完成启动组装：

- 重复 `AgentKey`；
- 空或非法 `AgentKey`；
- 已启用业务 Agent 缺少必需依赖；
- Adapter 声明与注册键不一致。

Kernel 不再通过 `Option<Repository>` 把缺失依赖延迟到运行时。未知会话 `agent_type` 仍在请求期返回稳定 unsupported error，因为它可能来自历史或非法数据。

### TDD：先锁定契约再迁移 Adapter

实现按测试驱动推进：

- 在 `novex-agent` 先建立 fake Adapter、fake Session Store 和 fake Run Recorder 的 Kernel contract tests。
- 覆盖注册成功、重复注册、未知 Agent、成功执行、Adapter 失败、Assistant 保存失败和单次终态收尾。
- 在 `backend` 为 `topic/script/sound/work` 建立同一 Adapter contract suite，验证 key、payload 校验和错误分类。
- 逐个迁移 Adapter，每迁移一个即运行对应 Runtime、路由和 Repository 测试。
- 最后运行 `cargo test --workspace` 和模块边界检查，并确认不存在旧 Runtime 入口及核心业务分派。

## Risks / Trade-offs

- [Risk] trait 和 ports 抽取过度，形成只为当前四个 Agent 服务的伪通用层。 -> 只提取已被至少两个 Agent 共同使用的生命周期与执行概念，业务 payload 和 Repository 留在 Adapter。
- [Risk] 迁移时改变消息 metadata、step 顺序或错误映射。 -> 以现有集成测试为 golden behavior，并增加前后等价断言，不在本 change 清理 Prompt 或业务流程。
- [Risk] Assistant 保存或 Run 收尾发生数据库故障时仍可能留下不完整运行。 -> Coordinator 保证代码路径只尝试一次终态转换，并保留原始错误；本 change 不承诺跨外部模型调用的全局事务，数据库原子完成接口只在不改变语义时采用。
- [Risk] 非会话任务失败后，失败收尾本身也可能失败。 -> 生命周期协调器始终保留原始业务错误，并只尝试一次失败收尾；残留 running Run 供后续运维检查，不用次生错误覆盖根因。
- [Risk] JSON payload 退化为无约束扩展袋。 -> Adapter 入口必须立即解码为 `deny_unknown_fields` 的强类型结构并校验；业务代码不得直接散读任意 JSON 字段。
- [Risk] 一次迁移四个 Adapter 造成较大回归面。 -> 先建立 Kernel contract，再按 script、topic、sound、work 顺序迁移，每一步保持工作区可测试。
- [Trade-off] 本 change 暂不产生用户可见能力。 -> 它消除后续 Prompt、Context、Memory 和虚拟制作团队继续堆入 `backend` 的结构风险，是后续能力的必要前置。

## Migration Plan

1. 在 `novex-ai-core` 建立通用值对象，在 `novex-agent` 建立 Kernel contract 和 fake 测试。
2. 在 `backend` 实现 Session/Run Recorder ports 与统一错误映射。
3. 把现有业务处理方法拆为 `script`、`topic`、`sound`、`work` Adapter，保持 Prompt 和业务 helper 原位。
4. 在 Bootstrap 注册四个 Adapter，ConversationService 改为调用 Coordinator。
5. 删除旧 `AgentRuntime` 聚合、业务 `match`、可选业务 Repository 和旧公共路径。
6. 运行相关测试、全量 workspace 测试、格式化和静态搜索验证。

该变更不涉及数据库或公开协议，部署按普通 API 镜像替换。若实现期回归无法在当前设计内解决，应回退本 change 的代码改动并修正规格，不保留双 Runtime 兼容路径。

## Open Questions

当前无阻塞问题。`AgentDefinition` 版本持久化、Prompt Snapshot 和 Context Compiler 的具体结构留给后续 change 决定。
