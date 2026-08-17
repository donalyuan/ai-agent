## Why

当前仓库缺少可由新会话稳定读取的项目记忆入口，现有规则仍指向已删除的 `MEMORY.md` 和 `docs/memory/`。这会使项目事实、长期决策和交接信息难以按证据维护。

## What Changes

- 建立以 Git、`AGENTS.md` 和 Markdown 为基础的第一阶段项目持久记忆体系。
- 将项目入口与 Claude 规则迁移到 `docs/agent/`，并移除已删除路径和未经当前仓库验证的架构断言。
- 建立 ADR 入口和第一条关于记忆方案的已接受决策记录。
- 明确事实源优先级、任务后维护、链接精简和秘密禁止规则。

## Capabilities

### New Capabilities

- `project-memory-governance`: 为项目代理提供可验证的记忆读取、维护、交接和决策记录规则。

### Modified Capabilities

- 无。

## Impact

影响项目规则文件、`docs/` 下的持久记忆与 ADR 文档，以及本 change 的 OpenSpec artifacts。不新增运行时依赖、服务、数据库或外部接口。
