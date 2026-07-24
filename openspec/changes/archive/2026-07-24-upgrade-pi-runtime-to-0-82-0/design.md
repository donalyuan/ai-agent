## Context

Novex `services/agent-runtime` 当前精确锁定三个 Pi `0.81.1` 包，并通过 `AgentHarness`、`NodeExecutionEnv`、自有 `AgentTool` 和 SQLite Session Repo 提供本地单用户 Runtime。联网预检确认三个 npm 包共同 latest 为 `0.82.0`，GitHub 存在 `v0.82.0` tag（`083e61621276bff9f6faefab87ce07fcd98734e2`）。

对 npm tarball 与 tag 源码的审查结论：

- 三个包的 Node engine 均保持 `>=22.19.0`，Novex 正式镜像 Node.js 24 满足要求。
- 三个包 `exports` 结构未变；`pi-storage-sqlite-node` 除版本、changelog 和内部 Pi 依赖外没有实现差异，因此没有 SQLite schema migration。
- `pi-agent-core` 存在明确 breaking change：`AgentHarness` 从 `ExecutionEnv + AgentTool` 改为应用定义的 `toolContext + AgentHarnessTool`。
- `createReadTool/createWriteTool/createEditTool/createBashTool` 已真实进入 `0.82.0` npm tarball并从根入口导出；上游 `edit` 使用 `edits[]/oldText/newText` 等不同语义，不能在依赖升级中直接替换 Novex 的 `old_text/new_text` 协议。
- `pi-ai` exports 不变，但 Provider、retry、OpenAI Responses/Completions 实现有更新；必须用现有模型映射、脱敏和 fake-provider 回归验证。

升级前 Runtime `/health`、`/ready` 均通过，SQLite volume 为 `server_ai-agent-session-data`，当前 `GET /sessions` 为空列表。

## Goals / Non-Goals

**Goals:**

- 将三个 Pi 包原子升级到精确版本 `0.82.0`。
- 迁移到 `AgentHarnessToolContext` 契约并保持 Runtime HTTP/SSE、工具和 Session 外部行为。
- 验证 npm clean install、类型、构建、fake-provider、安全审计、SQLite volume、Compose 和跨服务边界。
- 保留明确的数据备份与代码/镜像回滚路径。

**Non-Goals:**

- 不切换到上游 execution tool factory，不改变工具参数和结果协议。
- 不启用 constrained sampling、Kimi/OpenRouter OAuth 或新的模型目录能力。
- 不改变 SQLite schema、Runtime HTTP API、Compose 端口、PostgreSQL 模型来源或 Pi/Rust 责任边界。
- 不调用真实模型、视频生成、平台发布，不自动归档或提交 Git。

## Decisions

### 1. 三包同步精确升级

`package.json` 中三个直接依赖统一设为 `0.82.0`，在隔离临时目录基于当前 manifest/lockfile 用 `--package-lock-only --ignore-scripts --save-exact` 生成候选。候选必须通过项目检查脚本，且 `resolved`/`integrity` 来自 npm registry；原文件 SHA-256 在应用前必须仍与预检一致。

不采用单包升级、浮动 semver 或 GitHub commit，因为 storage 与 agent-core 的内部 peer graph 需要同版，且项目消费的是 npm 发布产物。

### 2. 使用新的 Harness tool context，保留 Novex 工具协议

定义 Runtime Harness 为 `AgentHarness<ExecutionToolContext>`，工具数组使用 `AgentHarnessTool<ExecutionToolContext>`。`AgentHarness` 构造参数从：

```text
env + AgentTool[]
```

迁移为：

```text
toolContext: { env } + AgentHarnessTool<ExecutionToolContext>[]
```

自有工具不再在创建时闭包捕获 env，而是在每次 `execute` 的第五个参数中读取 `{ env }`。这符合 `0.82.0` 的 turn snapshot 语义，同时保留现有 schema、标签、输出 details、最大 timeout 和 SSE 更新行为。

虽然上游 factory 已发布，但其 `edit` schema 和实现语义与 Novex 不同。直接采用会扩大外部行为面并使既有 Session/tool transcript 兼容性不足以定论，因此本次明确保留自有适配器；是否切换必须另提 change 并设计 transcript migration。

### 3. 用行为测试锁定破坏性适配

先应用依赖并运行类型检查，使旧 `env`/`AgentTool` 装配在新类型下失败，确认真实迁移点。实现后扩展 fake-provider workspace Tool Loop 测试，连续执行：

```text
write -> edit(old_text/new_text) -> final assistant response
```

测试同时断言 SSE tool start/end 顺序、唯一终态、文件最终内容、消息持久化和凭据不泄漏。现有 profile、concurrency、steer/follow-up/abort、模型映射、SQLite tree/compaction 测试全部保留。

### 4. SQLite 无 schema migration但仍执行 volume 备份恢复验证

上游 storage tarball没有实现差异，不新增 migration 代码。重建前仍停止 Runtime，并将实际 `/data` source volume 完整复制到带时间戳的 backup volume；即使当前 Session 为空，也验证 SQLite 文件存在、非空和可读。

重建后比较升级前后 Session ID 集合，检查 `/health`、`/ready`，再重启一次确认持久化。如果启动或数据读取失败，先回滚 manifest/lockfile 和旧镜像；恢复 backup volume 需要用户最终确认，因为它会覆盖数据。

### 5. 完整门禁后才更新版本事实

Runtime 镜像 build 承担 Node 24 下的 `npm ci` 与 build，再独立运行 lint、test 和 `npm audit --audit-level=high`。随后运行 Rust workspace 与 Video Worker 本地测试，不访问真实外部供应商。全部通过后才把 memory/README/ARCHITECTURE 中 `0.81.1` 更新为 `0.82.0`。

## Risks / Trade-offs

- [Harness breaking type 影响工具执行] -> 显式泛型和 toolContext 迁移，加连续 write/edit fake-provider 测试。
- [Provider 内部变化改变 SSE/错误语义] -> 保留现有 Provider 映射，运行 Responses/Completions、abort、唯一终态和脱敏测试。
- [上游 factory 与自有适配器长期重复] -> 本次优先协议稳定；另行评估切换，不在依赖升级中隐式改变行为。
- [SQLite 重建后不可读] -> 停服一致性备份 named volume、两次启动检查和可确认的数据恢复路径。
- [lockfile 带入额外依赖漂移] -> 隔离生成、审查 diff、三包/registry/integrity 校验。
- [工作区已有未提交内容] -> 只修改本 change 列明文件，应用前比较 SHA-256，不清理或覆盖无关修改。

## Migration Plan

1. 完成上游 release/tarball 审查和目标版本预检。
2. 在隔离目录生成候选 manifest/lockfile并执行离线一致性检查。
3. 确认原文件 SHA-256 未变后应用三包版本与 lockfile。
4. 迁移 Harness/tool types，扩展行为测试并完成 Runtime Docker 门禁。
5. 记录 Session IDs，停止 Runtime，创建并验证 backup volume。
6. Compose 重建 Runtime，执行 health/ready、Session 与重启恢复验证。
7. 运行 Rust/Video Worker 回归并同步版本文档。

回滚时恢复 `0.81.1` manifest/lockfile和对应 Runtime 镜像。只有数据已被失败启动修改且用户确认后，才从记录的 backup volume 恢复 source volume。

## Open Questions

无。本次已明确保留 Novex 自有工具协议；上游 factory 切换不属于本 change。

## Verification Evidence

- 升级前 `GET /sessions`：`{"sessions":[]}`。
- SQLite source volume：`server_ai-agent-session-data`。
- SQLite backup volume：`server_ai-agent-session-data-backup-pi-0-82-0-20260724155223`。
- source 与 backup 的 `agent-sessions.sqlite` 均为 `81920` 字节，SHA-256 均为 `a91ec9f596cd56620233d815b9041da2463eebd9e877274d77541308b32d0365`。
- backup 在临时容器可写目录执行 `PRAGMA integrity_check` 返回 `ok`；backup volume 保留且未覆盖 source volume。
- Compose 强制重建后容器内三个 Pi 包均为 `0.82.0`；首次启动和再次重启后的 `/health`、`/ready` 均通过，Session 集合均为 `{"sessions":[]}`，Docker health 为 `healthy`。
- Runtime Node.js 24 Docker clean `npm ci`、build、lint 与 `npm audit --audit-level=high` 均通过，audit 为 `0 vulnerabilities`；Vitest 为 `3 files / 11 tests passed`。
- Rust API 容器内 `cargo test --workspace` 退出码为 `0`；Video Worker 测试确认使用 Fake Provider/注入网络入口后，容器内 `pytest tests -q` 为 `189 passed, 24 warnings`，警告均为既有 FastAPI `on_event` deprecation。
