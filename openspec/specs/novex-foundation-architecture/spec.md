# novex-foundation-architecture Specification

## Purpose
定义 Novex AI Agent Foundation monorepo 的长期目录边界、业务应用归属、Rust workspace crate 边界、Python sidecar 服务边界和管理后台边界。
## Requirements
### Requirement: 仓库必须采用 Novex 基座 monorepo 边界

系统 SHALL 以 Novex AI Agent foundation 作为仓库根定位，并提供稳定的 `backend`、`admin`、`apps`、`crates`、`services`、`templates`、`infra`、`docs` 顶层目录边界。

#### Scenario: 顶层目录反映基座结构

- **WHEN** 开发者查看仓库根目录
- **THEN** 仓库 SHALL 包含 `backend`
- **AND** 仓库 SHALL 包含 `admin`
- **AND** 仓库 SHALL 包含 `apps`
- **AND** 仓库 SHALL 包含 `crates`
- **AND** 仓库 SHALL 包含 `services`
- **AND** 仓库 SHALL 包含 `templates`
- **AND** 仓库 SHALL 包含 `infra`
- **AND** 仓库 SHALL 包含 `docs`

### Requirement: video-agent 必须作为业务应用保留并迁移

系统 SHALL 将 video-agent 作为 Novex 基座下的视频内容生产业务应用保留在 `apps/video-agent`，并将正式视频生产工作台归入该目录，而不是继续作为仓库根级身份或 `admin/` 管理后台功能扩展。

#### Scenario: video-agent 应用存在于 apps 边界

- **WHEN** 开发者查看 `apps/`
- **THEN** 系统 SHALL 包含 `apps/video-agent`
- **AND** `apps/video-agent` SHALL 说明它是 Novex 的视频内容生产应用
- **AND** `apps/video-agent` SHALL 承载 `VEDIO-AGENT` / “视频工作台”的正式前端入口
- **AND** 当前 `script-agent-mvp` 已完成的业务能力 SHALL 有迁移后的继续开发入口

#### Scenario: 业务开发不得继续绑定旧根级结构或 admin 边界

- **WHEN** 后续新增 video-agent 业务能力
- **THEN** 新业务 SHALL 归属于 `apps/video-agent` 或明确归属到 `crates/*` 的可复用基座能力
- **AND** 新业务 SHALL NOT 以仓库根级 `frontend` 或 `python-worker` 结构继续扩展
- **AND** 视频生产工作台 SHALL NOT 继续作为 `admin/` 的正式业务页面扩展

### Requirement: Rust 可复用能力必须有 workspace crate 边界

系统 SHALL 为 Novex 的可复用 AI 基座能力建立 Rust workspace crate 边界，避免模型、Agent、RAG、工具、记忆和评测能力继续堆叠在 `backend/src`。

#### Scenario: 基座 crates 可被 workspace 识别

- **WHEN** 开发者查看 `crates/`
- **THEN** 系统 SHALL 包含 `novex-ai-core`
- **AND** 系统 SHALL 包含 `novex-model`
- **AND** 系统 SHALL 包含 `novex-agent`
- **AND** 系统 SHALL 包含 `novex-rag`
- **AND** 系统 SHALL 包含 `novex-tools`
- **AND** 系统 SHALL 包含 `novex-memory`
- **AND** 系统 SHALL 包含 `novex-eval`

#### Scenario: workspace 构建覆盖基座 crates

- **WHEN** 开发者在仓库根或 Rust workspace 入口执行 Rust 构建/测试命令
- **THEN** 构建 SHALL 覆盖 `backend` 和 `crates/*` 中已声明的 workspace 成员
- **AND** 最小 crate SHALL 可编译通过

### Requirement: Python sidecar 必须归入 services 边界

系统 SHALL 将 Python worker/runtime 类服务放入 `services/`，并将当前视频 worker 迁移为 `services/video-worker`。

#### Scenario: 视频 worker 位于 services

- **WHEN** 开发者查看 `services/`
- **THEN** 系统 SHALL 包含 `services/video-worker`
- **AND** 原 worker 健康检查 SHALL 在迁移后仍可测试

### Requirement: 管理后台必须归入 admin 边界

系统 SHALL 将 `admin/` 作为 Novex 控制面管理后台，承载平台管理、配置、运行状态和诊断能力；`admin/` SHALL NOT 作为视频内容生产工作台的正式承载位置。

#### Scenario: admin 保留可构建管理后台

- **WHEN** 开发者查看 `admin/`
- **THEN** 系统 SHALL 包含 Next.js 应用配置
- **AND** 管理后台 SHALL 保留基础管理页面或环境健康入口
- **AND** 管理后台 SHALL 可执行既有 lint/build 验证
- **AND** 管理后台 SHALL NOT 展示脚本生成、分镜时间轴、素材匹配、视频生成、发布排期或优化建议等日常视频生产流程

#### Scenario: 平台管理能力保留在 admin

- **WHEN** 开发者新增用户、权限、模型、MCP、工具、Worker、任务队列、日志、审计、成本、限额或健康检查功能
- **THEN** 该功能 SHALL 归属于 `admin/` 或后端控制面
- **AND** 该功能 SHALL NOT 混入 `apps/video-agent` 的生产工作台页面
