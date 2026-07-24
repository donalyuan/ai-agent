## MODIFIED Requirements

### Requirement: Agent Runtime 必须保留统一入口并按可注册能力拆分

系统 SHALL 保留统一 Agent Runtime 入口，并 SHALL 将可复用执行内核归入 `novex-agent` / `novex-ai-core`，通过 Registry 将脚本、选题、声音、作品及后续业务能力委派给 `backend` 中的独立 Adapter。执行内核 SHALL NOT 依赖 `backend`，也 SHALL NOT 持有具体业务 Repository 或业务类型分支。

#### Scenario: Runtime 处理一次 Agent 消息

- **WHEN** Runtime 收到已支持 Agent 类型的用户消息
- **THEN** 统一 Coordinator SHALL 加载会话上下文并解析稳定 `AgentKey`
- **AND** Coordinator SHALL 从 Registry 解析对应 Adapter
- **AND** Coordinator SHALL 将执行委派给该 Adapter
- **AND** 系统 SHALL 保持既有消息、run、step、模型快照和失败收尾语义

#### Scenario: Agent 能力模块保持职责单一

- **WHEN** 开发者查看脚本、选题生成、质量闸门、主题组评审、声音或作品实现
- **THEN** 每类业务能力 SHALL 位于独立 Adapter 或其专属业务模块
- **AND** 单一 Kernel 聚合文件 SHALL NOT 同时包含全部能力的 Prompt、输出解析、重试和业务规则
- **AND** 业务 Adapter SHALL 只持有自身执行所需的 Repository 或 Application Service

#### Scenario: 基座依赖方向保持单向

- **WHEN** 开发者检查 Rust workspace 模块依赖
- **THEN** `novex-agent` SHALL 依赖 `novex-ai-core` 并可依赖 `novex-model`
- **AND** `novex-agent` 与 `novex-ai-core` SHALL NOT 依赖 `backend`
- **AND** `backend` SHALL 负责具体 Adapter、持久化 port 实现和 Bootstrap 注册

#### Scenario: Agent Runtime 重构保持外部行为

- **WHEN** Registry 和 Adapter 架构替换现有业务分派
- **THEN** 系统 SHALL 保持现有 HTTP URL、方法、状态码、请求字段、响应字段和错误结构
- **AND** 系统 SHALL NOT 要求新的数据库 migration
- **AND** 模型选择、Prompt 语义、消息 metadata、业务结果、run 和 step 记录 SHALL 与重构前一致
