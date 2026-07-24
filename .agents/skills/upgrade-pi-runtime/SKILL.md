---
name: upgrade-pi-runtime
description: 检查并安全升级 Novex 本地个人 AI 工作台的 Pi Runtime 依赖。用户询问当前 Pi 是否有更新、要求升级 Pi、同步 @earendil-works/pi 三个 npm 包、审查 Pi 上游 release/API 兼容性，或验证升级后的 SQLite Session、SSE、Tool Loop 和跨服务回归时使用。
---

# 升级 Pi Runtime

把 Pi 升级视为 Runtime 基座迁移。先确定用户只要版本检查还是明确要求执行升级；检查保持只读，实际升级走独立 OpenSpec change 并完成全部验证。

## 固定边界

- 在 `/server/ai-agent` 工作，先完整读取 `MEMORY.md`、`CLAUDE.md` 和相关主题 memory/OpenSpec。
- 以仓库和实际命令结果为准；禁止使用 GitNexus。
- 同步处理以下直接依赖，不允许单包升级：
  - `@earendil-works/pi-agent-core`
  - `@earendil-works/pi-ai`
  - `@earendil-works/pi-storage-sqlite-node`
- 三包在 `package.json` 中使用相同的稳定版精确版本，不使用 `^`、`~`、npm tag、GitHub 默认分支或 commit 依赖。
- 只有三个 npm 包均发布目标版本且 GitHub 存在 `v<version>` tag 时，才进入正常升级流程。源码领先于 npm 时等待 release，不临时改用 git 依赖。
- 保留用户已有修改。未经明确确认，不执行 `git add`、`git commit`、`git push` 或 OpenSpec archive。
- 不调用真实模型，不提交真实 `model_id` 到 `prompt`/`compact`，不触发视频生成或平台发布。
- 不执行 `npm audit fix`，不执行 `docker compose down -v`，不删除或覆盖原 SQLite volume。

## 选择模式

### 只读检查

当用户只问“有没有更新”“现在是什么版本”或“能否跟随上游更新”时：

1. 检查工作区状态，但不要求干净：

   ```bash
   git status --short
   ```

2. 运行联网检查：

   ```bash
   .agents/skills/upgrade-pi-runtime/scripts/inspect_pi_versions.sh
   ```

3. 网络不可用且用户只需本地一致性时，运行：

   ```bash
   .agents/skills/upgrade-pi-runtime/scripts/inspect_pi_versions.sh --offline
   ```

4. 报告当前三包版本、lockfile 状态、npm latest、GitHub 最新稳定 tag 和候选版本。明确“GitHub 更新不会自动进入项目；只有执行受控依赖升级并生成新 lockfile 后才会采用”。
5. 到此停止。不创建升级 change，不修改依赖，不安装包，不重建服务。

脚本返回 `0` 表示信息完整且本地一致；发现较新候选版本仍返回 `0`。返回 `1` 表示版本或上游发布不一致，不足以升级；返回 `2` 表示参数、目录或工具等前置条件错误。

### 实际升级

仅当用户明确要求升级时继续后续全部步骤。若未指定版本，使用检查脚本确认的三个包共同 `npm latest`；不得只凭 GitHub 默认分支决定版本。

## 1. 预检目标版本

运行：

```bash
.agents/skills/upgrade-pi-runtime/scripts/inspect_pi_versions.sh --target <target-version>
```

停止升级并说明证据，若出现任一情况：

- manifest、lock spec、resolved 版本不一致；
- 三个 npm 包没有共同目标版本；
- 缺少对应 GitHub tag；
- 目标低于当前版本；
- 网络失败导致发布状态不足以定论；
- 用户修改与 `services/agent-runtime/package.json`、`package-lock.json` 或需调整的适配代码重叠，且无法无损合并。

记录升级前的 `git status --short` 和直接相关文件 diff。不要清理、暂存或还原无关修改。

## 2. 审查上游差异

在仓库外的 `mktemp -d` 中完成审查，并用 `trap` 清理临时目录：

1. 对照 `v<current>` 与 `v<target>` 的 GitHub release、changelog、提交 diff 和相关源码。
2. 分别对三个包的当前版和目标版执行 `npm pack <package>@<version> --json --pack-destination <dir>`。
3. 使用 JSON 解析 `npm pack` 输出并解压 tarball；比较：
   - `package/package.json` 的 `exports`、`engines` 和内部依赖；
   - `dist/**/*.d.ts` 的公开类型与函数；
   - Agent Harness、Provider、Session/SQLite、ExecutionEnv 和 tool 相关构建产物；
   - `createReadTool`、`createWriteTool`、`createEditTool`、`createBashTool` 等 factory 是否真正存在并导出。
4. 以 npm tarball 为实际消费证据。即使 GitHub 源码已有 factory，只要 npm 产物未导出，就继续保留 `services/agent-runtime/src/tools.ts` 的 `AgentTool + NodeExecutionEnv` 适配器。
5. 若目标 npm 已提供兼容 factory，先做影响分析并把“保留自有适配器”或“切换上游 factory”的决定写入升级 change；不得静默替换。

把以下结论写入 design/spec/tasks：破坏性 API、Node.js engine、Provider 请求/事件变化、SQLite schema/migration、tool factory 发布状态、所需代码调整、数据迁移与回滚路径。

## 3. 创建独立 OpenSpec Change

检查 `openspec list --json`。为该目标版本创建或继续一个专用 change，例如：

```text
upgrade-pi-runtime-to-0-82-0
```

使用项目的 `openspec-propose` 流程补齐 `proposal.md`、`design.md`、`specs/**/*.md` 和 `tasks.md`，再用 `openspec-apply-change` 实施。不要复用“创建本 Skill”的 change，也不要把升级塞入无关 change。

按项目规则从四个角度覆盖：

- `DDD`：Pi Runtime、Rust Kernel、业务工作台和 SQLite Session 的边界是否变化。
- `BDD`：既有 Session 重建后可读、SSE 顺序、Tool Loop、控制命令和错误语义。
- `SDD`：三包版本、exports/API、migration、兼容性、非目标和回滚约束。
- `TDD`：升级前失败测试、Runtime 合同测试、Compose 恢复和跨服务回归。

每完成一项立即勾选 `tasks.md`。

## 4. 隔离生成依赖变更

不要先在 Runtime 工作目录执行 `npm install`。按以下顺序生成候选 lockfile：

1. 对原 `package.json` 与 `package-lock.json` 计算 SHA-256，并复制到 `mktemp -d`。
2. 在临时目录执行：

   ```bash
   npm install --package-lock-only --ignore-scripts --save-exact \
     @earendil-works/pi-agent-core@<target-version> \
     @earendil-works/pi-ai@<target-version> \
     @earendil-works/pi-storage-sqlite-node@<target-version>
   ```

3. 对临时候选运行：

   ```bash
   .agents/skills/upgrade-pi-runtime/scripts/inspect_pi_versions.sh \
     --offline --runtime-dir <temporary-runtime-dir>
   ```

4. 审查 manifest/lockfile diff，确认三包精确同版、`resolved` 来自 npm registry、`integrity` 存在，且没有无法解释的依赖变化。
5. 再次校验原文件 SHA-256。若发生变化，停止并重新读取，不能覆盖并发编辑。
6. 用 `apply_patch` 修改 `services/agent-runtime/package.json`；把 npm 生成并已审核的 `package-lock.json` 应用到 Runtime。手工代码修改继续使用 `apply_patch`。
7. 立即重新运行联网检查并指定目标版本，确认工作区三包一致。

## 5. 验证 Runtime 产物

项目 Runtime 使用 Node.js 24，优先在 Docker 中验证，不用宿主机版本替代：

```bash
docker build --target test -t novex-agent-runtime-test services/agent-runtime
docker run --rm novex-agent-runtime-test npm run lint
docker run --rm novex-agent-runtime-test npm test
docker run --rm novex-agent-runtime-test npm audit --audit-level=high
```

镜像 build 已执行 `npm ci` 和 `npm run build`。测试必须继续覆盖：

- 模型映射与 OpenAI Responses/Chat Completions Provider；
- 凭据字段、Bearer、URL query 和已知 secret 脱敏；
- fake provider 下 SSE 事件顺序与唯一终态；
- workspace Tool Loop、并发拒绝、steer/follow-up/abort；
- SQLite Session create/open/list/entries/fork/tree/compact 和重启恢复。

high/critical audit 问题、lint/build/test 失败均阻止完成。不要用 `npm audit fix` 自动改依赖树。

## 6. 备份并验证 SQLite 持久化

在重建 Runtime 前保护现有数据：

1. 确认 `ai-agent-agent-runtime` 当前健康，调用只读 `GET /sessions` 保存已有 Session ID 清单；没有 Session 也要记录为空。
2. 用 `docker inspect ai-agent-agent-runtime` 从 `/data` mount 解析实际 source volume 名称，不猜测 Compose 前缀。
3. 停止 Runtime 后，将 source volume 只读挂载到临时容器，并完整复制到带时间戳的新 backup volume。验证 `agent-sessions.sqlite` 存在、备份非空且校验和可读。
4. 记录 backup volume 名称到升级 change。不得删除 source volume，不得运行 `down -v`。
5. 重建并强制重建 Runtime：

   ```bash
   docker compose -f /server/docker-compose.yml up -d --build --force-recreate ai-agent-agent-runtime
   curl -fsS http://127.0.0.1:18184/health
   curl -fsS http://127.0.0.1:18184/ready
   ```

6. 再次调用 `GET /sessions`，确认升级前所有 Session ID 仍存在，并读取至少一个既有 Session/entries（若原来非空）。再执行一次容器重启并重复 health/ready 与 Session 检查。

若 migration、启动或数据读取失败，停止验证，保存日志并回滚升级前 manifest/lockfile 和 Runtime 镜像。恢复 backup volume 会覆盖数据，属于破坏性操作；先报告 source、backup 和失败证据，取得用户最终确认后再执行，不得自动覆盖。

## 7. 验证跨服务边界

先检查测试代码不会访问真实外部服务，再运行：

```bash
docker compose -f /server/docker-compose.yml config --services
docker compose -f /server/docker-compose.yml exec -T ai-agent-api \
  sh -lc 'cd /app && /usr/local/cargo/bin/cargo test --workspace'
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker \
  sh -lc 'cd /app && pytest tests -q'
```

若容器未运行，先用 Compose 启动对应既有服务；不要改用宿主机临时运行时。任何直接相关回归失败都保留为未完成任务。

## 8. 同步事实并收口

升级及全部验证通过后：

1. 用 `rg` 查找旧 Pi 版本，只更新直接相关事实，至少复核：
   - `MEMORY.md`
   - `docs/memory/project-tech-stack.md`
   - `ARCHITECTURE.md`
   - 根 `README.md`
   - `services/agent-runtime/README.md`
2. 记录最终三包版本、上游 tag、tool factory 决策、备份 volume、测试结果和剩余风险。
3. 执行：

   ```bash
   openspec instructions apply --change "<change-name>" --json
   ```

4. 只有 `state` 为 `all_done` 且实际任务一致时才报告升级完成。
5. 报告 change 已可归档并等待用户命令；不自动 archive，不提交 Git。保留 backup volume，直到用户明确确认升级稳定并授权清理。
