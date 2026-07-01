## 1. 变更冻结与基线确认

- [x] 1.1 记录 `script-agent-mvp` 当前进度，明确本 change 完成前暂停继续实现脚本 Agent 业务任务。
- [x] 1.2 执行当前基线验证，记录后端、worker、前端和 OpenSpec 的迁移前状态。

## 2. Novex 顶层目录骨架

- [x] 2.1 创建 `admin/`、`apps/`、`crates/`、`services/`、`templates/`、`infra/`、`docs/` 顶层目录。
- [x] 2.2 新增 `apps/video-agent/` 应用说明，声明 video-agent 是 Novex 的视频内容生产业务应用。
- [x] 2.3 新增 `docs/` 架构入口，指向 `ARCHITECTURE.md` 并说明后续以 Novex 基座结构为准。

## 3. 应用与服务迁移

- [x] 3.1 将现有 `frontend/` 迁移为 `admin/`，保留 Next.js 配置、页面、样式和测试/构建命令。
- [x] 3.2 将现有 `python-worker/` 迁移为 `services/video-worker/`，保留 FastAPI 健康检查和 pytest。
- [x] 3.3 更新 `.gitignore`、README、memory 中所有旧路径引用。

## 4. Rust workspace 与 crates 边界

- [x] 4.1 将 Rust 工程调整为 workspace，包含 `backend` 与 `crates/*`。
- [x] 4.2 创建最小可编译 crates：`novex-ai-core`、`novex-model`、`novex-agent`、`novex-rag`、`novex-tools`、`novex-memory`、`novex-eval`。
- [x] 4.3 保留当前脚本 Agent 已验证代码，并确认后续迁移归属写入 `apps/video-agent` 说明或 OpenSpec 衔接文档。

## 5. Compose 与环境口径更新

- [x] 5.1 更新 `docker-compose.yml` 服务名和构建路径为 `novex-api`、`novex-video-worker`、`novex-admin`。
- [x] 5.2 确认顶层 `/server/docker-compose.yml` 可识别新服务名。
- [x] 5.3 更新 API 与 worker 健康检查响应中的服务名。

## 6. OpenSpec 与文档同步

- [x] 6.1 更新 `script-agent-mvp` artifacts 中受路径迁移影响的说明，保证恢复开发时路径不误导。
- [x] 6.2 更新 `MEMORY.md` 与相关 `docs/memory/*.md`，将 Novex 基座结构设为新的长期约束。
- [x] 6.4 将完整需求文档整理到 `docs/requirements/`，并保留根 `openspec/` 作为 OpenSpec CLI 工作区。
- [x] 6.3 更新 README 的项目状态、目录导航和常用验证命令。

## 7. 验证

- [x] 7.1 运行 `openspec validate --all`。
- [x] 7.2 在容器内运行 Rust 后端及 workspace 测试。
- [x] 7.3 在容器内运行 `services/video-worker` pytest。
- [x] 7.4 在容器内运行 `admin` lint/build。
- [x] 7.5 运行 Compose 服务列表检查和必要健康检查。
- [x] 7.6 运行 `git diff --check` 并确认无空白错误。
