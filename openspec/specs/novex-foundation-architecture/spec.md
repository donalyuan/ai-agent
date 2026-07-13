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

### Requirement: Rust 后端必须建立明确的内部层次边界

Rust 后端 SHALL 将应用组装、HTTP 传输、应用用例、领域模型和持久化职责分离，并 SHALL 保持 `api -> application -> domain` 的主依赖方向。

#### Scenario: HTTP 请求通过分层调用完成

- **WHEN** 任一后端业务 API 处理 HTTP 请求
- **THEN** API 模块 SHALL 只负责参数提取、DTO 校验和 HTTP 响应转换
- **AND** Application Service SHALL 负责业务用例和跨 Repository 编排
- **AND** Domain 模块 SHALL 提供不依赖 Axum 和 SQLx 的业务实体与状态规则
- **AND** Repository SHALL 返回 Domain 对象而不是 HTTP DTO 或 Agent Runtime 类型

#### Scenario: 应用启动依赖被集中组装

- **WHEN** 后端构建运行时状态和 Axum Router
- **THEN** `bootstrap` SHALL 负责配置、数据库、Redis、模型解析器、Application Service 和 Router 的组装
- **AND** `lib.rs` SHALL NOT 包含具体业务 handler 实现

### Requirement: Rust API 必须按业务功能拆分

Rust API SHALL 按项目、选题、素材、对话、脚本、素材生成、模型管理和工作台等业务功能组织路由、handler 与 DTO，避免不同业务继续聚合在单一请求模型或路由实现文件中。

#### Scenario: 开发者定位业务 API 实现

- **WHEN** 开发者查看一个业务功能的 API 源码
- **THEN** 该功能的路由、handler 和 DTO SHALL 位于对应业务模块
- **AND** 业务模块 SHALL NOT 通过调用其他业务模块的 handler 实现跨模块协作
- **AND** 跨模块协作 SHALL 通过 Application Service、Repository 接口或 Domain 类型完成

#### Scenario: 简单 CRUD 不绕过应用层

- **WHEN** API 执行创建、读取、更新或状态修改操作
- **THEN** handler SHALL 调用对应 Application Service
- **AND** handler SHALL NOT 直接执行 SQL 或持有业务状态流转规则

### Requirement: Agent Runtime 必须保留统一入口并按能力拆分

后端 SHALL 保留统一 Agent Runtime 入口，并 SHALL 将脚本、选题生成、质量闸门和主题组评审实现拆分为独立能力模块。

#### Scenario: Runtime 处理一次 Agent 消息

- **WHEN** Runtime 收到已支持 Agent 类型的用户消息
- **THEN** 统一入口 SHALL 加载会话上下文并识别 Agent 类型
- **AND** 统一入口 SHALL 将执行委派给对应能力模块
- **AND** 系统 SHALL 保持既有消息、run、step、模型快照和失败收尾语义

#### Scenario: Agent 能力模块保持职责单一

- **WHEN** 开发者查看脚本、选题生成、质量闸门或主题组评审实现
- **THEN** 每类能力 SHALL 位于独立模块
- **AND** 单一 Runtime 聚合文件 SHALL NOT 同时包含全部能力的 Prompt、输出解析、重试和业务规则

### Requirement: Rust 模块路径迁移必须完整且唯一

本次分层重构 SHALL 将仓库内生产代码与测试全部切换到新 Domain、Application 和 API 模块路径，并 SHALL NOT 保留旧公共路径兼容层。

#### Scenario: 重构完成后检查旧路径

- **WHEN** 开发者搜索旧 `agents::models` API DTO 路径和旧 `conversational_runtime` 路径
- **THEN** 生产代码和测试 SHALL NOT 再引用这些旧路径
- **AND** `lib.rs` 或其他模块 SHALL NOT 使用 `pub use` 重新暴露旧路径

### Requirement: 分层重构必须保持外部行为不变

分层重构 SHALL 保持现有 HTTP、数据库、模型调用和 Agent 运行行为，不得把结构调整转化为隐式协议变更。

#### Scenario: API 回归验证

- **WHEN** 重构后的后端执行现有路由集成测试
- **THEN** URL、HTTP 方法、状态码、请求字段、响应字段和错误结构 SHALL 与重构前一致
- **AND** 系统 SHALL NOT 要求新的数据库 migration

#### Scenario: Agent Runtime 回归验证

- **WHEN** 重构后的 Runtime 执行脚本、选题生成、质量闸门、主题评审和连续对话测试
- **THEN** 模型选择、同模型重试、Prompt 语义、消息 metadata、run 和 step 记录 SHALL 与重构前一致

### Requirement: 复杂分层与编排代码必须包含业务意图注释

Rust 后端 SHALL 为模块职责、公共 Application Service、Agent 入口、重试、幂等、质量闸门、主题组归一、失败收尾和事务顺序提供解释业务意图的注释。

#### Scenario: 开发者阅读复杂业务流程

- **WHEN** 开发者查看具有非显然约束的 Agent 编排或持久化流程
- **THEN** 代码 SHALL 说明该约束存在的原因或必须保持的不变量
- **AND** 注释 SHALL NOT 仅复述变量赋值、普通分支或显而易见 CRUD 行为
