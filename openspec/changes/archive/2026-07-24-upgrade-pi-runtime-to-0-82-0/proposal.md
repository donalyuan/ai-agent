## Why

Pi 三个 Runtime 包已共同发布 `0.82.0`，提供可中止的 Provider retry、compaction 缓存修正和正式发布的 Harness execution tools；Novex 当前仍锁定 `0.81.1`。该版本同时把 `AgentHarness` 从 `env + AgentTool` 改为 `toolContext + AgentHarnessTool`，必须通过受测的内部适配升级，不能只改版本号。

## What Changes

- 将 `@earendil-works/pi-agent-core`、`@earendil-works/pi-ai`、`@earendil-works/pi-storage-sqlite-node` 同步精确升级到 npm/GitHub 已发布的 `0.82.0`。
- **BREAKING（内部）**：将 Runtime Harness 装配从旧 `ExecutionEnv`/`AgentTool` 契约迁移到 `ExecutionToolContext`/`AgentHarnessTool` 契约。
- 保留 Novex 现有 `read/write/edit/bash` 工具名称、参数 schema、结果和 SSE 行为；本次不切换到上游 factory，避免引入 `edit` 输入协议等用户可见变化。
- 使用隔离目录生成并审核 lockfile，在备份 SQLite volume 后重建 Runtime，验证 Session、SSE、Tool Loop、脱敏和跨服务回归。
- 更新项目记忆与运行文档中的 Pi 版本和适配器决策。

## Capabilities

### New Capabilities

- `pi-runtime-harness-compatibility`: 定义 Pi Harness 依赖发生破坏性升级时，Novex 通过内部适配保持工具、事件、Session 和安全契约的要求。

### Modified Capabilities

无。

## Impact

- 依赖与 lockfile：`services/agent-runtime/package.json`、`package-lock.json`。
- Runtime 适配：`services/agent-runtime/src/coordinator.ts`、`src/tools.ts` 及相关测试。
- 运行环境：重建 `ai-agent-agent-runtime`，但不改变端口、Compose 服务、PostgreSQL 模型来源或 SQLite 数据模型。
- 文档：`MEMORY.md`、Agent/技术栈主题 memory、`ARCHITECTURE.md`、根 README 与 Runtime README。
- 不调用真实模型、视频生成或发布，不改变 Rust Kernel 和业务工作台执行边界。
