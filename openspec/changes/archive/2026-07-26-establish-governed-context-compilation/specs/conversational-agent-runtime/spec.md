## ADDED Requirements

### Requirement: Rust Conversation 必须固定 Context Policy 与 Tokenizer Profile

Rust Conversation SHALL 在 Definition binding 中固定各 LLM node 的 Context Policy，并在首次模型 binding 时固定 Tokenizer Profile；后续消息和内部步骤 SHALL 使用同一组合。

#### Scenario: Conversation 首次执行模型
- **WHEN** 已支持 Conversation 第一次处理可能调用模型的消息
- **THEN** Runtime SHALL 在外部调用前原子固定 Context Policy 与 Tokenizer Profile binding
- **AND** 同轮全部内部 node SHALL 使用各自 Definition 引用的固定 Policy 和同一模型 Profile
- **AND** binding 持久化失败 SHALL 不创建模型调用或领域写入

#### Scenario: 后续 Context 行为发生变化
- **WHEN** 当前 active Policy/Profile 已不同于 Conversation binding
- **THEN** 既有 Conversation SHALL 继续使用 supported 版本或在 revoked/不兼容时阻断
- **AND** SHALL NOT 因新发布静默改变 Context
- **AND** 行为变化 SHALL 要求显式 fork/rebind

### Requirement: Rust 历史 Conversation 必须按 Context 等价证据迁移

历史 Conversation 的 Context binding SHALL 幂等迁移；只有完整 golden 证据证明旧装配等价时才能自动绑定 baseline Policy，否则 SHALL 标记 `context_migration_required`。

#### Scenario: 历史 Conversation 等价迁移
- **WHEN** 已知 agent_type/node 和历史 fixture 足以证明最终 Prompt 等价
- **THEN** 迁移 SHALL 固定 baseline Policy 并保存证据
- **AND** SHALL 保留原 Conversation、消息、Run/Step 和模型 binding

#### Scenario: 历史 Conversation 需要显式迁移
- **WHEN** 历史 Context、裁剪或来源证据不足以证明等价
- **THEN** Conversation SHALL 保持可读并拒绝继续调用模型
- **AND** API SHALL 返回稳定 `context_migration_required`
- **AND** 显式 fork/rebind SHALL 保留来源关联并创建新绑定

