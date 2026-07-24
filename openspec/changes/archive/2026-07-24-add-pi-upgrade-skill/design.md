## Context

`services/agent-runtime` 当前精确依赖 Pi `0.81.1` 的 `@earendil-works/pi-agent-core`、`@earendil-works/pi-ai` 和 `@earendil-works/pi-storage-sqlite-node`。三者共同影响 Agent Harness、Provider 映射、工具执行环境和 SQLite Session；其中项目还因 npm 包导出能力与上游源码存在差异而维护了 `AgentTool + NodeExecutionEnv` 适配器。升级不能简化为单包 `npm update`。

项目采用 Docker Compose 运行，Runtime 镜像为 Node.js 24，SQLite 位于持久卷。升级还可能影响 Rust 控制面和 Video Worker 的既有边界，因此需要项目级、可重复执行的维护流程。

## Goals / Non-Goals

**Goals:**

- 创建可由 Codex 发现和执行的项目级 `upgrade-pi-runtime` Skill。
- 可靠地区分只读版本检查与会修改仓库/运行环境的实际升级。
- 统一三包版本门槛、上游差异审查、lockfile 生成、数据保护和回归验证。
- 让升级记录通过独立 OpenSpec change 与项目记忆保持可审计。

**Non-Goals:**

- 本 change 不把 Pi 从 `0.81.1` 升级到当前最新版本。
- 不自动跟踪 GitHub 默认分支，也不默认使用 git commit 依赖。
- 不调用真实模型、视频生成或发布接口。
- 不自动提交 Git、更改用户未提交内容或归档 OpenSpec change。

## Decisions

### 1. 使用项目级 Skill，而非一次性文档

Skill 位于 `.agents/skills/upgrade-pi-runtime/`，以 `SKILL.md` 保存决策流程，以 `agents/openai.yaml` 提供发现元数据，以脚本完成容易出错的版本一致性检查。相比 README 清单，Skill 能在每次相关请求触发时携带项目约束，并减少人工遗漏。

### 2. 检查与升级采用两个明确模式

只读检查可直接运行，读取 `package.json`、`package-lock.json`，并在联网模式查询三个 npm 包和 GitHub tag。脚本不得安装依赖或改写仓库，版本落后本身不作为脚本失败；本地声明/lock 不一致、上游三包发布不齐或查询失败必须明确报错。

实际升级必须先创建独立 OpenSpec change，目标版本由用户请求或已验证的共同 npm release 决定。这样版本发现不会隐式触发运行环境变更。

### 3. npm release 是正常升级入口

三个 Pi 包必须都存在目标版本，并在 `package.json` 中精确锁定同一版本。GitHub `v<version>` tag 用于源码和 changelog 对照，但不能替代 npm 发布；默认禁止使用 `main`、浮动 semver 或 GitHub commit 依赖。

原因是项目消费的是 npm 包的实际 exports 和构建产物，GitHub 源码存在并不证明 npm tarball 已包含相同能力。例外情况必须作为新的架构决策单独提出，不能由升级 Skill 自动采用。

### 4. 发布产物和源码差异都要审查

升级前需对照当前 tag、目标 tag、release/changelog，并检查两版 npm tarball 的 `package.json` exports、类型声明和相关实现。尤其要确认上游 tool factory 是否真正进入 npm 产物，再决定继续保留、替换或修改当前 `AgentTool + NodeExecutionEnv` 适配器。

### 5. lockfile 在隔离目录生成并审核

复制 Runtime 的 manifest/lockfile 到 `mktemp -d`，使用目标版本和 `--package-lock-only --ignore-scripts --save-exact` 生成候选 lockfile。确认三包版本、resolved 来源、integrity 和依赖树后，再将明确的 manifest 变更与生成的 lockfile应用到工作区；若原文件在此期间变化则停止，避免覆盖并发编辑。

### 6. 验证覆盖 Runtime、持久化和上下游边界

先运行 Runtime 的 `npm ci`、lint、build、fake-provider 测试与 audit，再通过 Compose 重建 Runtime 并检查 `/health`、`/ready`。重建前备份 SQLite 持久卷并记录已有 Session 标识，重建后确认会话仍可读取；迁移失败时停止并从备份恢复到升级前镜像/依赖。

最后运行 Rust workspace 与 Video Worker 回归。所有自动测试禁止真实模型调用、视频生成和平台发布，避免外部费用与副作用。

### 7. 完成状态与文档同步

实际升级成功后更新 `MEMORY.md` 及直接相关架构/运行文档中的 Pi 版本，重新运行 OpenSpec apply instructions 并达到 `all_done`。只报告 change 可归档，未经用户命令不归档，也不执行 `git add`、`git commit` 或 `git push`。

## Risks / Trade-offs

- [npm 与 GitHub 发布不同步] -> 三包共同 npm 版本和对应 tag 都满足才允许升级，缺失时停止。
- [上游源码能力未进入 npm tarball] -> 同时审查 tag 源码与 `npm pack` 产物，以实际消费产物为准。
- [SQLite schema 或原生模块不兼容] -> 升级前备份持久卷，执行重建恢复检查，保留旧依赖和镜像回滚路径。
- [lockfile 带入无关依赖变化] -> 在隔离目录生成并审查 diff，只接受目标升级导致的变化。
- [回归测试误触发费用] -> 仅使用 fake provider 与本地测试，不发送真实生成或发布请求。
- [工作区已有未提交修改] -> 先记录并避开无关变更；若与升级文件重叠且无法无损合并则停止确认。

## Migration Plan

本 change 只新增 Skill，无运行时迁移。未来实际升级按“检查 -> 独立 change -> 备份 -> 隔离生成 lockfile -> 测试 -> Compose 重建 -> 持久化核验 -> 文档同步”的顺序执行。失败时恢复升级前 manifest/lockfile、重建旧 Runtime，并在必要时从备份恢复 SQLite 数据。

## Open Questions

无。目标版本和具体上游 API 变更由每次实际升级 change 基于当时发布事实确定。
