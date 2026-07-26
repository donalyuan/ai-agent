## ADDED Requirements

### Requirement: Context Compiler 必须作为跨 Runtime 可复用基座能力

Context Policy、Tokenizer Profile、候选/快照 schema 和确定性 Compiler SHALL 归入仓库级 Definition Registry、`novex-ai-core`、`novex-agent`、`novex-model`、`novex-eval` 与 Pi Runtime 对应适配层；领域 Adapter SHALL 只拥有来源读取和业务含义映射。

#### Scenario: Rust 与 Pi 编译 Context
- **WHEN** Rust 视频 node 或 Pi 通用 Turn 准备模型调用
- **THEN** 两者 SHALL 加载同一 Registry 合同并使用同语义 Compiler
- **AND** Rust SHALL 在 PostgreSQL 保存其 Context 证据
- **AND** Pi SHALL 在 namespaced SQLite 保存其 Context 证据
- **AND** SHALL NOT 建立跨库双写事务或同请求双执行

#### Scenario: 新增领域 Context 来源
- **WHEN** 后续业务新增项目事实、资产、Memory 或其他 Context 来源
- **THEN** 来源 SHALL 先转换为版本化 Policy 允许的 ContextCandidate
- **AND** SHALL 复用统一预算、冲突、审计与回放合同
- **AND** SHALL NOT 在业务路由或 Prompt builder 中建立第二套选择和裁剪逻辑

### Requirement: Context Compiler 迁移必须删除旧执行双轨

本变更 SHALL 一次覆盖全部 Rust/Pi 生产 LLM node，并 SHALL 删除旧手工裁剪、整段 Prompt fragment、Pi 单一 context blob 和绕过 ContextSnapshot 的路径。

#### Scenario: 完成唯一入口切换
- **WHEN** 全节点 contract、golden、迁移和回归通过
- **THEN** 生产执行 SHALL 只保留“Context Compiler -> PromptCompiler -> ModelCall”路径
- **AND** SHALL NOT 通过 feature flag、兼容 helper 或 legacy Runtime 保留旧装配
- **AND** Pi Harness、Rust 领域 Gate 和各自数据所有权边界 SHALL 保持不变

