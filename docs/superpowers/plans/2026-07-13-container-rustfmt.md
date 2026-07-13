# API 容器 rustfmt 安装实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `rustfmt` 持久安装到 `ai-agent-api` 镜像，并验证格式检查和 API 健康状态。

**Architecture:** 在现有 Rust 官方基础镜像的组件安装层中追加 `rustfmt`，保持 Compose、Rust 版本和应用启动方式不变。通过顶层 Compose 只重建并重新创建 API 服务，不启动视频 Worker。

**Tech Stack:** Docker、Docker Compose、Rustup、rustfmt、Rust 1.x bookworm 镜像

---

### Task 1: 固化并验证 rustfmt

**Files:**
- Modify: `backend/Dockerfile:9`
- Reference: `docs/superpowers/specs/2026-07-13-container-rustfmt-design.md`

- [x] **Step 1: 验证当前镜像缺少 rustfmt**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc '/usr/local/cargo/bin/cargo fmt --version'
```

Expected: FAIL，输出包含 `cargo-fmt is not installed`。

- [x] **Step 2: 修改 Dockerfile 的 Rustup 组件安装层**

将：

```dockerfile
RUN rustup component add clippy
```

改为：

```dockerfile
RUN rustup component add clippy rustfmt
```

- [x] **Step 3: 重建并重新创建 API 服务**

Run:

```bash
docker compose -f /server/docker-compose.yml up -d --build --no-deps ai-agent-api
```

Expected: 镜像构建成功，`ai-agent-api` 容器重新创建并启动；`ai-agent-video-worker` 状态不变。

- [x] **Step 4: 验证 rustfmt 已安装**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc '/usr/local/cargo/bin/cargo fmt --version'
```

Expected: PASS，输出 `rustfmt` 版本。

- [x] **Step 5: 验证仓库 Rust 格式**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo fmt --all --check'
```

Expected: PASS；若失败，仅报告现有源码格式差异，不执行 `cargo fmt` 改写源码。

Actual: `cargo fmt --all --check` 已执行并发现现有 Rust 源码格式差异；未自动改写源码。

- [x] **Step 6: 验证 API 健康状态和 Worker 未启动**

Run:

```bash
docker compose -f /server/docker-compose.yml ps -a ai-agent-api ai-agent-video-worker
```

Expected: `ai-agent-api` 为 `healthy`，`ai-agent-video-worker` 仍为未运行状态。

- [x] **Step 7: 检查变更范围**

Run:

```bash
git diff --check
git status --short
```

Expected: 无空白错误；只包含 Dockerfile、设计规格和实施计划变更。未经用户明确授权，不执行 `git add`、`git commit` 或 `git push`。
