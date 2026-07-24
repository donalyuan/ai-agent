## ADDED Requirements

### Requirement: Novex 必须定位为本地单用户个人 AI 工作台基座
系统 SHALL 以 local-first、单用户、可扩展多个领域工作台作为长期产品定位；视频生产 SHALL 是首个领域应用，而不是整个基座的唯一用途。

#### Scenario: 新增非视频工作台
- **WHEN** 后续新增编程、知识研究或其他个人工作台
- **THEN** 该工作台 SHALL 位于 `apps/*` 的独立领域边界
- **AND** SHALL 复用通用 Agent Runtime、模型配置和本地会话能力
- **AND** SHALL NOT 复制视频领域的 ProductionState 或业务规则

#### Scenario: 不建设未确认的多租户交付能力
- **WHEN** 开发者规划通用基座能力
- **THEN** 系统 SHALL NOT 把多租户、客户模板、企业 RBAC 或插件市场作为默认必选范围
- **AND** 本地付费调用、发布和删除确认 SHALL 作为单用户安全规则独立保留

## MODIFIED Requirements

### Requirement: Rust 可复用能力必须有 workspace crate 边界
系统 SHALL 保持 Rust 领域能力的 workspace crate 边界，并 SHALL 允许 `services/agent-runtime` 使用 TypeScript Pi Agent Harness 承担通用 Turn/Tool/Session 执行；Rust backend SHALL NOT 重复实现同职责的第二套通用 Tool Loop。

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

#### Scenario: Pi Runtime 保持独立服务边界
- **WHEN** 开发者查看通用 Agent Turn、Tool Loop、Session Tree 或 Context Compaction 实现
- **THEN** 新实现 SHALL 位于 `services/agent-runtime`
- **AND** Rust `novex-agent` SHALL 保留已存在的业务 Adapter 与 Run 生命周期合同
- **AND** 两个边界 SHALL NOT 对同一请求执行双写或双模型调用
