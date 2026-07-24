## Why

当前统一 Agent Runtime 仍在 `backend` 中通过业务类型分支分派，并直接聚合脚本、选题、声音和作品 Repository；新增 Agent 需要修改核心入口，缺失依赖也可能延迟到运行时才暴露。Novex 要继续建设版本化角色、Context、Memory、Tool 和虚拟制作团队，必须先建立不认识具体业务、可注册、可回放且保持现有行为的稳定执行内核。

## What Changes

- 在 `novex-ai-core` 定义稳定的 Agent 标识、Run/Step 状态和跨层执行快照基础类型。
- 在 `novex-agent` 建立 `AgentAdapter`、`AgentRegistry`、`AgentExecutionContext`、`AgentRunCoordinator` 和 Run/Step 记录端口。
- 通过 Bootstrap 注册 `topic`、`script`、`sound`、`work` Adapter，替代 Runtime 核心中的业务类型分支。
- 将具体业务 Repository 和输入解析留在对应 Adapter；缺失依赖、重复注册或非法定义在启动组装阶段失败。
- 统一用户消息保存、Run 创建、Adapter 执行、Assistant 消息保存以及成功/失败收尾，保证一次运行只完成一次终态转换。
- 统一会话型 Agent 的应用执行门面，并让脚本生成、项目策略草稿、主题组评审等非会话任务复用同一 Run 生命周期协调器。
- 保持现有 HTTP API、数据库结构、模型选择、Prompt、消息 metadata、Run/Step 审计语义和业务结果不变。
- 为后续版本化 `AgentDefinition`、Prompt、Context、Memory、Tool 和局部 Planner 保留稳定扩展边界，但本 change 不实现这些能力。

## Capabilities

### New Capabilities

- `agent-runtime-kernel`: 定义可注册的 Agent 执行内核、Adapter 契约、执行上下文、运行生命周期和启动期依赖校验。

### Modified Capabilities

- `novex-foundation-architecture`: 将统一 Agent Runtime 的可复用执行内核明确归入 `novex-agent` / `novex-ai-core`，并约束 `backend` 只保留业务 Adapter、持久化实现和依赖组装。

## Impact

- 主要影响 `crates/novex-ai-core`、`crates/novex-agent`、`backend/src/application/agents`、`backend/src/application/conversations.rs`、脚本/项目/选题应用服务、`backend/src/bootstrap` 及相关测试。
- 不新增数据库 migration，不修改前端，不改变公开 HTTP 请求或响应。
- 现有脚本、选题、质量闸门、主题组评审、声音和作品 Agent 测试必须作为行为回归基线。
- 本 change 完成后，角色 Prompt、Context Compiler 和运行快照增强应作为后续独立 change 推进。
