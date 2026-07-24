# pi-runtime-upgrade Specification

## Purpose
TBD - created by archiving change add-pi-upgrade-skill. Update Purpose after archive.
## Requirements
### Requirement: Pi 版本检查必须只读且完整
系统 MUST 提供项目级只读检查能力，同时报告 Runtime manifest 与 lockfile 中三个 Pi 包的版本；联网检查时还必须报告三个包的 npm 最新版本和上游 GitHub 最新稳定 tag。检查不得安装依赖、修改文件或重启服务。

#### Scenario: 检查发现可用新版本
- **WHEN** 用户要求检查 Pi 是否有更新，且三个 npm 包存在共同的新版本
- **THEN** 系统报告当前版本、lockfile 版本、每个包的最新版本和对应 GitHub tag，并明确这是候选升级而不修改项目

#### Scenario: 本地三包版本不一致
- **WHEN** manifest 中三个 Pi 包未精确锁定为同一版本，或 lockfile 与 manifest 不一致
- **THEN** 系统以非零状态结束检查并逐项报告不一致，不继续给出可直接升级结论

#### Scenario: 上游查询不完整
- **WHEN** npm 或 GitHub 查询失败，或三个 npm 包没有共同目标版本
- **THEN** 系统明确报告不足以判定，不以部分结果宣称版本可升级

### Requirement: 升级目标必须来自完整 npm release
系统 MUST 仅将三个 Pi 包均已发布且具有对应 GitHub tag 的同一精确版本视为正常升级目标。系统 MUST NOT 默认使用浮动 semver、GitHub 默认分支或 commit 依赖替代 npm release。

#### Scenario: 三包和 tag 均已发布
- **WHEN** 目标版本可从三个 npm 包精确解析且存在 `v<version>` GitHub tag
- **THEN** 系统允许该版本进入升级差异审查

#### Scenario: 源码领先于 npm 发布
- **WHEN** GitHub 已包含所需功能，但任一 Pi npm 包尚未发布相同目标版本
- **THEN** 系统停止正常升级流程，并将等待正式 npm release 作为可执行方案

### Requirement: 升级前必须审查消费产物差异
系统 MUST 对照当前与目标 GitHub tag 的 release/changelog 和相关源码，并检查当前与目标 npm tarball 的 exports、类型声明及相关构建产物。系统必须以 npm tarball 的实际能力决定项目适配器是否变化。

#### Scenario: tool factory 只存在于源码
- **WHEN** 目标 tag 源码包含 tool factory，但目标 npm tarball 未导出该能力
- **THEN** 系统保留项目现有 `AgentTool + NodeExecutionEnv` 适配器，并记录该判断依据

#### Scenario: npm 产物包含兼容 factory
- **WHEN** 目标 npm tarball 已导出兼容的 tool factory
- **THEN** 系统评估替换自有适配器的影响和测试范围，并在升级 change 中明确选择，不静默切换

### Requirement: 每次实际升级必须受独立 change 管理
系统 MUST 在修改 Pi 依赖前创建或选定一个只负责该次升级的 OpenSpec change，并将目标版本、差异结论、实施步骤和验证范围写入工件。系统不得将只读检查隐式升级为修改操作。

#### Scenario: 用户只要求检查版本
- **WHEN** 用户只要求确认是否存在 Pi 更新
- **THEN** 系统只执行检查并报告结果，不创建依赖变更、不重建服务

#### Scenario: 用户确认执行升级
- **WHEN** 用户明确要求升级到已验证的目标版本
- **THEN** 系统通过独立 OpenSpec change 执行并逐项更新任务状态

### Requirement: 三包和 lockfile 必须原子一致
系统 MUST 将三个 Pi 直接依赖精确锁定到同一目标版本，并在隔离临时目录生成候选 lockfile。应用前必须验证 manifest、lockfile、resolved 来源、integrity 和依赖树一致，且不得覆盖实施期间出现的并发文件修改。

#### Scenario: 候选 lockfile 验证通过
- **WHEN** 隔离生成的 manifest 和 lockfile 均将三包锁定为目标版本，来源与完整性字段有效且原工作文件未变化
- **THEN** 系统只应用已审查的 manifest 与生成 lockfile 变更

#### Scenario: 生成过程出现漂移
- **WHEN** 三包解析版本不一致、出现非 npm 来源、lockfile 带入无法解释的变化或原工作文件已被并发修改
- **THEN** 系统停止应用并报告具体差异，不覆盖现有工作区内容

### Requirement: 升级验证必须覆盖运行时与持久化边界
系统 MUST 运行 Runtime clean install、lint、build、fake-provider 测试和安全审计，重建 Compose Runtime 并验证 `/health`、`/ready`。系统还必须在备份 SQLite 持久卷后验证已有 Session 在重建后仍可读取，并覆盖凭据脱敏、SSE、Tool Loop 与 Session migration 行为。

#### Scenario: Runtime 全部验证通过
- **WHEN** 新版本通过静态检查、构建、测试、安全门槛、Compose 探针和 SQLite 重建恢复检查
- **THEN** 系统允许进入跨服务回归和文档同步阶段

#### Scenario: 数据或运行时验证失败
- **WHEN** 任一 Runtime 验证失败或重建后已有 Session 不可读取
- **THEN** 系统停止升级、保留诊断证据，并使用升级前依赖和数据备份执行回滚

### Requirement: 升级完成必须验证上下游并同步项目事实
系统 MUST 运行 Rust workspace 和 Video Worker 的直接相关回归，且所有测试不得调用真实模型、视频生成或平台发布。成功后必须更新项目记忆及相关文档中的 Pi 版本，并确认升级 change 达到 `all_done`；系统不得自动归档或执行 Git 提交操作。

#### Scenario: 所有验收完成
- **WHEN** Runtime、SQLite、Rust 和 Video Worker 验证均通过，版本文档已同步
- **THEN** 系统确认 OpenSpec 状态为 `all_done`，报告 change 可归档并等待用户命令

#### Scenario: 跨服务回归失败
- **WHEN** Rust 或 Video Worker 的相关回归失败
- **THEN** 系统不得将升级标记完成，并在升级 change 中保留未完成任务和失败证据
