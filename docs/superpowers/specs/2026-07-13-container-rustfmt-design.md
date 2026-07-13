# API 容器 rustfmt 安装设计

## 目标

将 `rustfmt` 固化到 `ai-agent-api` 镜像，使容器重建后仍可执行 `cargo fmt`，不依赖对运行中容器的临时安装。

## 设计

- 在 `backend/Dockerfile` 的 Rust 工具链组件安装层中同时安装 `clippy` 和 `rustfmt`。
- 不修改 Compose 服务定义、Rust 版本、应用启动命令或业务依赖。
- 通过顶层 `/server/docker-compose.yml` 重建并重新创建 `ai-agent-api`。

## 四维审视

- **DDD**：不涉及领域模型、领域状态或业务规则。
- **BDD**：开发者进入重建后的 API 容器时，可以直接运行 `cargo fmt`；API 继续正常提供健康检查。
- **SDD**：镜像构建必须执行 `rustup component add clippy rustfmt`；不得使用只对当前容器生效的临时安装。
- **TDD**：构建后验证 `cargo fmt --version`、`cargo fmt --all --check` 和 `/health`；同时确认容器状态为 healthy。

## 风险与边界

- 重建期间 API 会短暂不可用。
- `cargo fmt --all --check` 若失败，表示仓库现有 Rust 源码格式不符合工具输出，应单独报告，不自动改写业务代码。
- 本次不安装其他 Rust 组件，不触发 Worker 或任何模型供应商调用。
