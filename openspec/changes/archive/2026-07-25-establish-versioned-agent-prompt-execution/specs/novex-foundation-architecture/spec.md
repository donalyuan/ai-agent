## MODIFIED Requirements

### Requirement: 仓库必须采用 Novex 基座 monorepo 边界

系统 SHALL 以 Novex AI Agent foundation 作为仓库根定位，并提供稳定的 `agent-definitions`、`backend`、`admin`、`apps`、`crates`、`services`、`templates`、`infra`、`docs` 顶层目录边界；`agent-definitions` SHALL 作为跨语言版本化 Agent/Prompt 定义唯一事实源。

#### Scenario: 顶层目录反映基座结构

- **WHEN** 开发者查看仓库根目录
- **THEN** 仓库 SHALL 包含 `agent-definitions`
- **AND** 仓库 SHALL 包含 `backend`
- **AND** 仓库 SHALL 包含 `admin`
- **AND** 仓库 SHALL 包含 `apps`
- **AND** 仓库 SHALL 包含 `crates`
- **AND** 仓库 SHALL 包含 `services`
- **AND** 仓库 SHALL 包含 `templates`
- **AND** 仓库 SHALL 包含 `infra`
- **AND** 仓库 SHALL 包含 `docs`
- **AND** 数据库与业务应用目录 SHALL NOT 保存可覆盖 Registry 的在线 Prompt 正文

## ADDED Requirements

### Requirement: Definition、执行与审计必须保持跨 Runtime 单向边界

仓库级 Registry SHALL 提供治理定义，Rust/Pi Runtime SHALL 各自只读加载自身 owner 的定义并写入自身拥有的运行审计存储；系统 SHALL NOT 建立定义数据库反向覆盖、跨库双写或同请求双执行。

#### Scenario: Rust 执行视频领域 Agent
- **WHEN** 现有视频业务 Adapter 执行其 Definition 声明的 LLM node
- **THEN** Rust SHALL 使用共享 Registry 与 PostgreSQL ModelCall repository
- **AND** Pi Runtime SHALL NOT 对该请求再次调用模型
- **AND** 领域 Run/Step、Repository 和 Gate SHALL 继续由 Rust 拥有

#### Scenario: Pi 执行通用个人 Agent
- **WHEN** 通用工作台 Session 执行 Pi Turn 或 Tool Loop
- **THEN** Pi SHALL 使用共享 Registry 与 SQLite ModelCall repository
- **AND** Rust Kernel SHALL NOT 实现或执行相同 Turn/Tool Loop
- **AND** PostgreSQL `ai_models` SHALL 继续是模型部署和凭据唯一来源

### Requirement: Pi 集成必须使用公开组合边界

`services/agent-runtime` SHALL 通过 Novex-owned 组合式 wrapper 持有 Pi `AgentHarness`，并 SHALL 只使用 Pi 官方公开 hook、`Models`、toolContext、Tool 和 Session API。

#### Scenario: Novex 接入 Prompt、审计和 Tool Gate
- **WHEN** Runtime 构造一个可执行 Pi Harness
- **THEN** wrapper SHALL 通过公开构造参数和 hook 注入固定 System/User Prompt、调用前审计与 Tool Gate
- **AND** wrapper SHALL 代理公开运行控制方法并保留 Pi SSE/Session 语义
- **AND** AgentHarness 私有生命周期 SHALL 保持由 Pi 实现

#### Scenario: Pi 上游升级兼容性检查
- **WHEN** Pi 依赖升级或公开 hook/API 发生变化
- **THEN** compatibility test SHALL 在发布前发现类型或行为不兼容
- **AND** 系统 SHALL NOT 通过继承私有实现、monkey patch、未导出路径或复制源码规避升级问题

### Requirement: Agent 审计与评测必须归入可复用基座能力

通用 binding、Prompt 编译、ModelCall schema 和评测合同 SHALL 归入 `novex-ai-core`、`novex-agent`、`novex-eval` 及 Pi Runtime 对应适配层，业务 Adapter SHALL 只拥有结构化输入、输出解析和领域规则。

#### Scenario: 新增业务 Agent node
- **WHEN** 后续业务应用新增一个需要 LLM 的 node
- **THEN** 开发者 SHALL 在 Registry 增加版本化 Definition
- **AND** SHALL 复用 PromptCompiler、AuditedModelExecutor 和 Eval 门禁
- **AND** SHALL NOT 在业务路由中新增裸 provider 调用或私有审计格式

#### Scenario: 第一版不提供 Admin UI
- **WHEN** 本变更完成审计 API、导出、replay 和 Eval 入口
- **THEN** 系统 SHALL NOT 同时新增 Prompt 在线编辑或审计 Admin 页面
- **AND** 后续前端 SHALL 通过独立 OpenSpec、设计上下文、Pencil 原型和明确确认推进
