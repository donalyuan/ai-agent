## ADDED Requirements

### Requirement: Context Policy 与 Tokenizer Profile candidate 必须通过激活门禁

行为变化的 ContextPolicyDefinition 或 TokenizerProfile candidate SHALL 通过 schema/引用、跨语言计数、历史 snapshot dry-run、确定性、安全、预算边界和核心 Prompt/业务回归，并生成不可变 EvalReport 后才可由代码发布标为 active。

#### Scenario: Context candidate 全部门禁通过
- **WHEN** candidate 在固定 case set、模型 fingerprint、Prompt 版本和编译时钟下满足全部阈值
- **THEN** EvalReport SHALL 记录 candidate/baseline、Tokenizer Profile、Policy、case set、预算差异、选择 diff 和逐项结论
- **AND** candidate SHALL 成为后续代码发布可激活版本
- **AND** 评测 API SHALL NOT 直接修改 Registry 生命周期

#### Scenario: Context candidate 任一门禁失败
- **WHEN** candidate 导致跨语言计数不一致、非确定性选择、required 丢失、冲突误判、预算溢出、安全回归或核心行为下降
- **THEN** EvalReport SHALL 明确失败项
- **AND** candidate SHALL NOT 被普通 Session/Run 选择

### Requirement: 首次 Context baseline 必须证明全节点等价或要求显式迁移

系统 SHALL 对全部现有 Rust/Pi 生产 LLM node 建立零真实模型调用的 Context golden baseline；自动历史绑定 SHALL 只引用证明选择、顺序、裁剪与最终 Prompt 等价的报告。

#### Scenario: baseline 完全等价
- **WHEN** 新 Context Compiler 对一个 node 的历史 fixtures 产生相同最终 Prompt、Schema、参数、调用次数和 fake-provider 结果
- **THEN** EvalReport SHALL 可标记该 node 为 `equivalent`
- **AND** 对应历史绑定 SHALL 可按迁移规格审计地绑定 baseline Policy

#### Scenario: baseline 存在差异
- **WHEN** 任一 fixture 的选择、顺序、裁剪、Prompt 或业务结果存在未批准差异
- **THEN** 报告 SHALL 标记该 node 非等价
- **AND** 历史作用域 SHALL 要求显式 fork/rebind
- **AND** 常规 baseline SHALL NOT 因此调用真实模型

