## Why

Novex 已将 Pi 作为本地个人 AI 工作台的 Runtime 基座，但上游升级涉及三个耦合包、SQLite 持久化、工具适配器和跨服务回归，靠临时操作容易造成版本漂移或数据兼容回归。需要把可重复、可审计的检查与升级流程固化为项目级 Skill。

## What Changes

- 新增项目级 `upgrade-pi-runtime` Skill，统一编排 Pi 当前版本检查、上游 npm/GitHub 对照、API 差异审查和升级验证。
- 提供只读版本检查脚本，读取项目中三个 Pi 包的精确版本并查询 npm 最新发布版本。
- 将实际升级约束为独立 OpenSpec change，并要求三个 Pi 包同步、精确锁定到同一 npm release。
- 固化 lockfile 生成、Runtime 测试、Compose 重建、SQLite 恢复、凭据脱敏、SSE/Tool Loop 及 Rust/Video Worker 回归清单。
- 实际升级完成后更新项目记忆中的 Pi 版本；达到 `all_done` 后仅报告可归档，不自动归档。

## Capabilities

### New Capabilities

- `pi-runtime-upgrade`: 定义 Novex 检查和安全升级 Pi Runtime 依赖的项目级自动化工作流与验收边界。

### Modified Capabilities

无。

## Impact

- 新增 `.agents/skills/upgrade-pi-runtime/` 下的 Skill 元数据、工作流说明和只读检查脚本。
- 新增本 change 的 OpenSpec 工件；本次不修改 `services/agent-runtime` 依赖、不重建服务，也不调用真实模型或视频生成。
- 后续每次实际升级将影响 `services/agent-runtime/package.json`、`package-lock.json`、Runtime 适配代码/测试，以及记录 Pi 当前版本的项目文档。
