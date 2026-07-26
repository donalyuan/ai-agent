## Why

Novex 已完成版本化 Agent/Prompt 执行与调用级审计，但当前 Rust Adapter 仍自行选择、拼接和按字符裁剪 Context，Pi 也只把完整 Harness context 作为单一 reference blob 记录，无法统一证明来源优先级、有效期和 token 预算。现在必须在建设正式 Memory、Planner 和虚拟制作团队之前建立跨 Rust/Pi 的受治理 Context Compiler，否则后续来源和角色增长会放大静默丢失、上下文溢出、历史漂移与不可回放风险。

## What Changes

- 建立共享、版本化的 `ContextPolicyDefinition`、`TokenizerProfile`、`ContextCandidate`、`ContextSnapshot` 与 `ContextCompileAttempt` 合同，并由 Rust/TypeScript 使用相同 schema、canonical digest 和确定性决策语义。
- 为每个 LLM node 固定精确 Context Policy，为每个文本模型显式绑定版本化 tokenizer profile；优先使用精确 tokenizer，无法精确映射时只允许明确声明算法、适用范围和安全余量的保守策略，禁止静默回退。
- 建立独立的 trust、priority、required 和 freshness 治理维度，以及 P0-P4 默认保留层级、确定性去重、版本替代、有效期和冲突失败规则。
- 将 Context 拆为语义 fragment、消息和不可拆分 Tool request/result 组，按固定输出预留和完整请求开销计算 token 预算；必需内容无法容纳时明确失败，不截断 JSON、Tool 调用链或已确认事实。
- 在模型调用前执行固定 Prompt 预编译、Context 选择、最终 token 复核、Context 审计持久化和 ModelCall 持久化；供应商仍报告 context overflow 时作为 tokenizer/profile 兼容性缺陷阻断，不临时重裁剪或透明重试。
- 成功编译保存不可变 ContextSnapshot 并由 ModelCall 引用；失败编译保存不可变 ContextCompileAttempt 但不创建虚假 ModelCall。采用项保存脱敏逻辑内容，排除项只保存最小化的身份、来源、hash、token 数和 decision code。
- **BREAKING**：行为不等价的历史 Session/Conversation 不得继续使用旧 Context 装配，必须标记 `context_migration_required` 并显式 fork/rebind；只有 golden regression 证明完全等价的 baseline Policy 才能审计地自动绑定。
- 一次迁移全部 Rust 与 Pi 生产 LLM 节点，删除 Adapter 手工裁剪、整段预格式化 Prompt fragment、Pi 单一 context blob 及其他旧 Context 装配入口，不保留两套执行实现。
- 提供 Context 摘要、脱敏详情、统一导出和零模型/零 Tool/零领域写入的 dry-run replay 合同；Context Policy/tokenizer 行为变化纳入不可变评测与激活门禁。
- 保持现有业务 API、Prompt 角色与输出契约、领域 Run/Step、Pi SSE/Session Tree、Tool Gate 和正式 Memory 边界；本变更不新增 Memory、RAG、Planner、虚拟制作团队或 Admin UI。

## Capabilities

### New Capabilities

- `agent-context-compilation`: 定义候选 Context、版本化 Policy/tokenizer、确定性选择与冲突、token 预算、快照/失败尝试及 dry-run replay 的核心合同。

### Modified Capabilities

- `agent-definition-registry`: Registry 新增 ContextPolicyDefinition、TokenizerProfile 和 Agent node 的精确 Policy 引用及生命周期约束。
- `model-routed-ai-execution`: 文本模型显式绑定 tokenizer profile，并将 profile/version 纳入跨语言模型行为 fingerprint 与调用前兼容性校验。
- `agent-runtime-kernel`: Rust 全部生产 LLM 节点改为提交原子 ContextCandidate 并只通过统一 Context Compiler 执行。
- `personal-agent-runtime`: Pi wrapper 通过公开 context hook 对实际消息和 Tool 组执行受治理选择，并保持 Harness 生命周期与 SSE 行为。
- `local-agent-session-persistence`: Pi SQLite 保存固定 Context binding、ContextSnapshot/CompileAttempt、迁移事件和所有权删除语义。
- `conversational-agent-runtime`: Rust Conversation 固定 Context Policy/tokenizer binding，并提供等价迁移、`context_migration_required` 与显式 fork/rebind 语义。
- `model-call-audit`: ModelCall 引用成功 ContextSnapshot，审计 API、导出、删除和 replay 扩展统一 Context 证据。
- `agent-prompt-evaluation`: Context Policy 与 tokenizer candidate 纳入静态、历史回放、预算、安全和行为等价门禁。
- `novex-foundation-architecture`: 明确 Context Compiler 在 Registry、Rust Kernel、Pi Runtime、PostgreSQL 与 SQLite 间的单向边界及禁止双轨规则。

## Impact

- 影响 `agent-definitions/`、`crates/novex-ai-core`、`crates/novex-agent`、`crates/novex-model`、`crates/novex-eval`、`backend/` 和 `services/agent-runtime/`。
- PostgreSQL 需要增加 Context binding、快照、失败尝试与 tokenizer 配置持久化；Pi SQLite 需要增加 namespaced Context binding、快照、失败尝试、迁移和删除记录。
- AgentDefinition/Registry schema、`ai_models` 文本模型行为配置、Session/Conversation binding、ModelCall 详情/导出与 replay schema 会发生版本化扩展。
- 需要跨 Rust/TypeScript tokenizer 与 Context Compiler contract tests、全生产节点 golden fixtures、历史迁移、静态入口扫描、fake-provider、SQLite/PostgreSQL 持久化和跨服务回归；常规验证不得调用真实模型或产生外部费用。
