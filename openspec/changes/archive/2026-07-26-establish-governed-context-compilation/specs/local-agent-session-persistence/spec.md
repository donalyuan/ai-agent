## ADDED Requirements

### Requirement: Pi SQLite 必须持久化固定 Context binding 与 namespaced 审计记录

Pi SQLite SHALL 在 Novex namespaced 表中保存 Session 的 Context Policy/tokenizer binding、ContextSnapshot、ContextCompileAttempt 和迁移事件，并 SHALL NOT 修改或依赖 Pi 私有表 schema。

#### Scenario: 重启后恢复 Context binding
- **WHEN** Runtime 重启并重新打开可执行 Session
- **THEN** SHALL 恢复 Agent/Prompt、Context Policy、Tokenizer Profile、model_id 与 behavior_fingerprint 的完整固定 binding
- **AND** 任一版本缺失、revoked 或不兼容 SHALL 在模型请求前失败
- **AND** SHALL NOT 按当前 active 版本静默重写 binding

#### Scenario: 保存 Pi Context 记录
- **WHEN** Pi Context 编译成功或失败
- **THEN** SQLite SHALL 按统一 schema 保存不可变 Snapshot 或 CompileAttempt
- **AND** 成功 ModelCall SHALL 引用 ContextSnapshot ID/digest
- **AND** namespaced 写入失败 SHALL 阻止模型调用

### Requirement: 历史 Pi Session 必须按可证明等价性迁移 Context

历史 Pi Session 只有在 golden regression 证明 Context 选择、顺序、裁剪和最终 Prompt 等价时，才 SHALL 审计地绑定 baseline Policy；否则 SHALL 标记为只读 `context_migration_required`。

#### Scenario: 历史 Session 可等价迁移
- **WHEN** 迁移程序证明一个历史 Session 的旧装配可由 baseline Policy 等价重建
- **THEN** SHALL 幂等写入固定 Context binding 和迁移证据
- **AND** SHALL 保留原 Session Tree、entries 与既有 Definition/model binding

#### Scenario: 历史 Session 无法证明等价
- **WHEN** 缺少准确 Context、旧裁剪证据或重建结果不等价
- **THEN** Session SHALL 保持可读但拒绝继续模型调用
- **AND** 显式 fork/rebind SHALL 创建使用新 active 组合的新 Session

### Requirement: 删除 Pi Session 必须清理其 Context 证据

Pi Session 明确删除流程 SHALL 使用既有 durable deletion intent 协调 namespaced Context binding、Snapshot、CompileAttempt、ModelCall 和迁移事件清理。

#### Scenario: 删除包含 Context 记录的 Session
- **WHEN** 操作者明确删除 Session
- **THEN** 其所有 Context 原始记录 SHALL 按所有权级联清理
- **AND** 相邻 fork 的 binding、ContextSnapshot 和 ModelCall SHALL 保留
- **AND** 中断恢复 SHALL 继续使用公开 SessionRepo 与 deletion reconciliation

