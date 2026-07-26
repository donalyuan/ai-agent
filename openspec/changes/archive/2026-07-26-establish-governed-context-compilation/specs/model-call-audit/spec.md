## ADDED Requirements

### Requirement: ModelCall 必须引用已持久化的成功 ContextSnapshot

每个实际模型调用的 prepared ModelCall SHALL 引用同一拥有者下已成功持久化的 ContextSnapshot ID/digest；Context 编译失败 SHALL 只产生 ContextCompileAttempt，不得产生 ModelCall。

#### Scenario: Context 成功后准备模型调用
- **WHEN** Runtime 完成最终 token 复核并持久化 ContextSnapshot
- **THEN** prepared ModelCall SHALL 保存 context_snapshot_id、digest、Policy/Profile 版本和预算摘要
- **AND** PromptSnapshot 中的最终动态输入 SHALL 与 ContextSnapshot 采用项和顺序一致
- **AND** 任一引用不一致 SHALL 阻止 provider 调用

#### Scenario: Context 编译失败
- **WHEN** Runtime 保存一个终态 ContextCompileAttempt
- **THEN** ModelCall repository SHALL 不存在对应虚假 prepared 记录
- **AND** Agent Run/Step 或 Session 错误 SHALL 可关联该 CompileAttempt ID

### Requirement: Context 审计必须进入统一读取、导出、回放和删除合同

Rust 与 Pi 的模型调用详情、导出和 dry-run SHALL 通过统一 schema 返回其 ContextSnapshot 证据，并 SHALL 对 ContextCompileAttempt 提供独立摘要与脱敏详情入口。

#### Scenario: 查询或导出 Context 审计
- **WHEN** 操作者查询 ModelCall 详情或独立 ContextCompileAttempt
- **THEN** 响应 SHALL 包含 Policy/Profile、预算账本、采用/排除 decision、最终顺序、digest 和脱敏内容边界
- **AND** 列表 SHALL 只返回摘要，不默认返回采用项全文
- **AND** Rust/PostgreSQL 与 Pi/SQLite 导出 SHALL 使用相同 schema_version 和字段语义

#### Scenario: dry-run Context replay
- **WHEN** 操作者对历史 ModelCall 请求 dry-run
- **THEN** replay SHALL 先验证 ContextSnapshot，再验证 PromptSnapshot 和结构化 diff
- **AND** SHALL 保证零模型、零 Tool、零领域写入和零 Session/Run 变化

#### Scenario: 删除拥有者
- **WHEN** 操作者明确删除拥有者 Session 或 Run
- **THEN** ContextSnapshot、ContextCompileAttempt 和 ModelCall SHALL 按同一所有权规则级联删除
- **AND** revoke、rollback、fork 或 rebind SHALL NOT 删除历史 Context 证据

