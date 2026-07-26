## Context

`establish-versioned-agent-prompt-execution` 已把 Rust/Pi 生产模型调用统一到 Definition、PromptCompiler、固定模型 binding 和 ModelCall 审计，但它刻意保留了旧 Context 选择、顺序和裁剪。当前 Rust 仍由各 Adapter 预格式化完整 Prompt、使用字符上限截断局部字段，再把整段字符串作为一个 DynamicFragment；Pi wrapper 则只在公开 `context` hook 中复制完整消息列表，并把 canonical JSON 作为单一 reference 写入审计，实际 Harness context 没有被治理。

本变更跨仓库级 Registry、Rust 公共 crate/backend、TypeScript Pi Runtime、PostgreSQL 和 SQLite。Context 编译必须早于 ModelCall，必须和实际 provider 逻辑输入一致，还必须保留 Pi 官方 Harness 生命周期、Rust 领域 Gate、数据所有权和零费用常规验证。

2026-07-25 对当前 PostgreSQL 的只读盘点显示，两条 enabled 文本模型使用 `openai_responses` 和不透明上游名 `gpt-5.6-luna`，`settings.context_window` 为空。该事实不足以证明 context window 或精确 tokenizer，因此迁移不得推断或自动回填；实现和发布前必须重新盘点，并要求操作者显式配置。

Pi `0.82.0` 的公开 `context` hook 支持返回替换后的 `AgentMessage[]`，足以接入 Context Compiler；Pi 自带 compaction `estimateTokens()` 使用 `chars/4` 启发式，对中文等输入可能低估，只能继续服务 Pi 内部 compaction 兼容，不能作为 Novex 模型调用前预算证据。

## Goals / Non-Goals

**Goals:**

- 为 Rust/Pi 全部生产 LLM node 建立同 schema、同决策语义、可版本化和可回放的 Context Compiler。
- 把 Context 来源、信任、优先级、必需性、有效期、替代、冲突、原子组和 token 预算从业务字符串构造中分离。
- 通过精确 tokenizer 或明确声明的保守策略，在 provider 调用前证明最终逻辑输入不超过模型窗口。
- 为成功与失败 Context 编译保存最小必要、脱敏且不可变的审计证据，并与 ModelCall、Session/Run 所有权一致。
- 一次切换唯一生产入口，删除旧手工裁剪和 blob 装配，不改变领域输出契约、Run/Step、Pi SSE/Session Tree 或 Gate。

**Non-Goals:**

- 不建设正式长期 Memory、RAG/向量召回、Planner、虚拟制作团队或新的领域角色。
- 不使用模型执行语义去重、冲突裁决或 Context 摘要；Pi 既有 compaction 继续是独立有损 Session Context。
- 不提供 Context Policy/tokenizer 在线编辑或 Admin UI；定义仍随代码发布，模型只选择已发布 Profile。
- 不迁移视频领域执行到 Pi，不改变付费生成、发布、删除和正式写入 Gate。
- 不为当前不透明模型猜测 context window、底层 tokenizer 或模型家族。

## Decisions

### 1. Registry schema v2 增加 Policy/Profile，并保留已发布 v1 定义

`agent-definitions/` 升级为向后可读的 schema v2，新增：

- `ContextPolicyDefinition`：key/version/status、允许 executor owner、允许来源、P0-P4 规则、required/freshness/冲突/排序/预算策略。
- `TokenizerProfile`：key/version/status、`exact` 或 `conservative` 模式、算法与实现版本、协议封装规则、适用协议/模型范围、安全余量和可选 tokenizer 资产 digest。
- 新 node 引用：同时精确引用 PromptDefinition 与 ContextPolicyDefinition。

已发布 Agent/Prompt 内容不得原地改变。v2 schema 允许读取旧 node 形态，但只允许旧 AgentDefinition 处于 supported/revoked；所有 active AgentDefinition 必须发布新版本并使用完整 node 引用。Definition release index 扩展 Policy/Profile 内容证据和生命周期 manifest，数据库仍只保存不可变发布证据，不保存可覆盖模板或算法的正文。

选择 Registry 而不是数据库 Policy 表，是为了让 Context 行为变化继续经过代码评审、digest、评测和发布；`ai_models` 只选择 Profile key/version，不定义算法。

### 2. ContextCompiler 与 PromptCompiler 使用 prepare/finalize 边界

`novex-ai-core` 定义跨业务的共享逻辑类型：

- `ContextCandidate` 与 `ContextAtomicGroup`
- `PreparedPromptEnvelope`
- `ContextCompileRequest/Result`
- `ContextDecision`、`BudgetLedger`
- `ContextSnapshot`、`ContextCompileAttempt`
- `LogicalModelInput`

调用顺序为：

1. PromptCompiler `prepare` 解析固定 Agent/Prompt/node、变量、System、User 模板固定片段、Tool/输出 Schema。
2. ContextCompiler 校验候选，固定一次编译时钟，执行资格与预算决策，返回 `CompiledContext`。
3. PromptCompiler `finalize` 把 CompiledContext 渲染为实际逻辑消息并生成 PromptSnapshot v2。
4. 同一 TokenizerProfile 对最终 `LogicalModelInput` 复核。
5. 持久化 ContextSnapshot，再持久化 prepared ModelCall，随后才调用 provider。

PromptCompiler 继续拥有 System/User 信任边界和模板/Schema 校验；ContextCompiler 不渲染角色 Prompt，也不读取业务库。这样避免把两个不同生命周期的职责合并成一个万能 Compiler。

### 3. Candidate payload 保留实际消息类型，Tool 交换组成原子组

ContextCandidate payload 使用 tagged union：文本 fragment、Pi/Rust 逻辑消息、不可变资产引用。候选保存 source kind/id/version、fact key、trust、priority、required、时间、supersedes、content hash、稳定 `render_order` 和可选 atomic group。`priority` 与 `required` 只决定预算竞争和必保语义；`render_order` 决定入选候选在最终逻辑输入中的展示位置，二者不得混用。

Rust Adapter 把账号策略字段、脚本/分镜、历史消息、候选选题和用户指令拆成可独立治理的候选，不再先拼成完整 Prompt。Pi wrapper 把公开 hook 的 AgentMessage 转换为候选；同一 assistant toolCall 与对应 toolResult 使用 call ID 形成原子组。Compiler 只能整体采用或排除组，不能切断 JSON、Tool 调用或消息结构。

采用 tagged payload 而不是统一字符串，是为了让 Pi 返回原生 AgentMessage[]、Rust 渲染现有 User 模板，同时让两者共享选择算法和审计 schema。

### 4. 决策流水线固定且不调用模型

ContextCompiler 按以下顺序运行：

1. schema、来源 allowlist、owner 和原子组完整性校验；
2. 固定 `compiled_at`，排除过期项并检查 required freshness；
3. 应用 supersedes，按稳定身份/version/hash 去重；
4. 检测 confirmed fact 冲突，reference/candidate 冲突只打标；
5. 使用 TokenizerProfile 计算每个候选/组 token；
6. 先保留全部 required/P0，再按 priority 与 Policy 稳定排序填充剩余预算；
7. 对入选候选按 `render_order` 和 Policy 稳定键排序，生成最终采用顺序、排除 decision code、预算账本和 canonical digest；
8. finalize 后对完整 LogicalModelInput 再计数并验证不变量。

预算选择的稳定排序依次使用 Policy 声明排序字段、source kind、source ID、source version、candidate ID；最终渲染先按 `render_order`，再使用同一稳定键消除并列。两者都不使用数据库返回顺序或 HashMap/对象插入顺序。所有失败使用稳定 code，不用模型裁决，也不降级为旧 helper。

### 5. 精确 tokenizer 使用双语言成熟库与共享资产合同

精确 Profile 第一版支持明确 encoding 的 byte-level BPE，例如 `cl100k_base`、`o200k_base`。Rust 使用固定版本 `tiktoken-rs`，TypeScript 使用固定版本 `js-tiktoken`；Profile 引用同一仓库级 encoding/协议 framing 定义和 asset digest，跨语言 fixture 覆盖 ASCII、中文、emoji、JSON、Tool Schema、reasoning 内容和多消息封装。

仅凭上游模型名不得自动选择 encoding。Profile 的 applicability 必须明确匹配当前 api_protocol 和由操作者确认的模型家族；依赖升级或资产变化必须发布新 Profile 版本并通过 contract。

选择成熟 tokenizer 库而不是手写 BPE，是为了避免词表、特殊 token 和 Unicode 处理错误；选择双语言本地实现而不是独立 tokenizer 服务，是为了避免为每次调用增加跨服务可用性和延迟，同时由共享资产/digest 与 fixtures 约束一致性。

### 6. 保守 Profile 使用显式 UTF-8 byte upper bound

为不透明但可确认属于 byte-level tokenizer 家族的模型提供 `utf8-byte-upper-bound@1`：文本 token 上界按 UTF-8 byte 数计算，消息/Tool/输出 Schema 和 provider framing 使用 Profile 中声明的固定/按项开销，再加显式 safety reserve。该策略牺牲可用窗口以换取调用前安全边界，审计中必须标记 `mode=conservative`。

该 Profile 不适用于无法确认 tokenizer 基本性质的任意模型；此时模型保持不可执行。禁止把 Pi `chars/4`、平均字符/token、provider 返回的上一次 usage 或失败后的缩短请求作为保守策略。

选择 byte upper bound 而不是经验比例，是因为中文、emoji 和代码会让经验比例严重低估；选择显式 applicability 而不是全局兜底，是为了不对未知 tokenizer 作无法证明的承诺。

### 7. 模型配置显式增加窗口与 Profile 引用

`ai_models` 为文本模型增加显式、可空迁移列：`context_window`、`tokenizer_profile_key`、`tokenizer_profile_version`。新建/编辑 enabled 文本模型时应用层要求三者完整且 Profile 可解析；历史缺失记录保留但运行时返回稳定配置错误，直到操作者显式补齐。图片/视频模型保持为空。

`ModelBehavior` 与 behavior_fingerprint 加入 context window 和 Profile key/version。凭据轮换不改变 fingerprint；窗口、Profile 或预算相关 settings 变化必须 rebind/fork。数据库不自动根据 `upstream_model` 回填，环境导入也必须得到显式 Profile 配置才能创建可执行文本模型。

列允许历史 null 而不在 migration 中停用或猜测现有模型；彻底性由管理写入校验、Runtime fail-closed、迁移报告和发布 readiness 共同保证，而不是破坏性修改未知配置。

### 8. 预算账本使用固定输出预留和完整逻辑输入

有效 `max_output_tokens` 取 PromptDefinition node 上限与模型部署上限的较小非空值；两者都缺失或不合法时拒绝执行。动态预算公式为：

```text
model_context_window
- system_prompt_tokens
- user_template_fixed_tokens
- tool_schema_tokens
- output_schema_tokens
- protocol_envelope_tokens
- effective_max_output_tokens
- tokenizer_profile_safety_reserve
= dynamic_context_budget
```

每项写入 `BudgetLedger`。不得为了容纳输入借用输出预算。最终复核使用完整 LogicalModelInput；若失败，返回 `context_budget_exceeded`，不重新裁剪。provider 仍返回 overflow 时，ModelCall 失败并记录 `tokenizer_profile_incompatible`，阻断同一绑定后续调用，直到显式更换 Profile/rebind。

### 9. PostgreSQL/SQLite 使用同逻辑 schema、各自本地事务

PostgreSQL 新增：

- Conversation/Run 的 `context_policy_bindings` 与 tokenizer binding 字段；
- `context_snapshots`，保存成功快照、digest、owner/node 和脱敏 payload；
- `context_compile_attempts`，保存失败阶段、code、预算和最小化 decision；
- `model_calls.context_snapshot_id` 外键及 schema v2 数据。

Pi SQLite 增加对应 `novex_context_snapshots`、`novex_context_compile_attempts` 和 Session binding 字段。两个 Runtime 使用相同 DTO/schema_version，但各自在本地事务中保存 Snapshot + prepared ModelCall；不存在 PostgreSQL/SQLite 跨库事务。

Snapshot 保存采用项脱敏全文，排除项只保存身份、来源/version、hash、token 和 decision code。CompileAttempt 同样不保存未发送全文。成功编译后 ModelCall 持久化失败时保留孤立但有 owner 的 Snapshot 作为失败证据；只有 owner 明确删除时级联清理。

### 10. Pi 只使用公开 context hook 返回治理后的消息

Novex wrapper 在公开 `context` hook 中：

1. 读取 hook 提供的实际 AgentMessage[]；
2. 构建消息/Tool 原子候选；
3. 使用当前 phase 对应 node binding 编译 Context；
4. 保存 latest ContextSnapshot；
5. 返回 `{ messages: compiledMessages }`。

`before_provider_request` 只做 binding 复核和 Snapshot/ModelCall 事务准备，不再把完整 context 重新序列化为单一 fragment。普通 Turn、Tool follow-up、compaction 和 branch summary 分别使用精确 node Policy；steer/follow-up 保留各自 trust/source。wrapper 不修改 Pi 源码、不访问私有表，也不复制 Context/Tool Loop。

### 11. Rust Adapter 全量拆分候选，Executor 统一编译

`AuditedModelRequest` 从接收已装配 `PromptCompileInput.fragments` 改为接收 `ContextCompileRequest` 和 node 变量。`AuditedModelExecutor` 持有 ContextCompiler、PromptCompiler、tokenizer registry 和 Context audit repository port，统一执行两阶段编译与持久化顺序。

所有生产节点一次迁移；每个 Adapter 的 source provider 只负责从其 Repository/消息输入产生候选并声明稳定来源元数据。现有字符 truncate helper、完整 generation_prompt fragment 和其他生产 Prompt 拼接入口删除。PromptSnapshot v2 保存最终逻辑消息与 context_snapshot_id/digest，不再重复保存可漂移的候选列表；历史 v1 Snapshot 保持可读。

### 12. 审计 API 与 replay 先验证 Context 再验证 Prompt

Rust/Pi 增加 Runtime-local Context 编译摘要、脱敏详情与导出入口；ModelCall 详情嵌入或链接对应 ContextSnapshot 摘要。列表不返回采用项正文，详情/导出按所有权返回脱敏全文。

dry-run replay 顺序为 ContextSnapshot -> PromptSnapshot -> ModelCall envelope：使用历史固定时钟、Policy/Profile 版本和快照证据计算选择/digest，再输出结构化 diff。排除项没有全文，但算法只依赖其持久化身份、版本、时间、hash、token 和排序元数据，因此足以复核历史 decision；不得读取当前业务事实替换历史。历史实现版本缺失时明确返回 `historical_context_dependency_unavailable`。

### 13. Policy/Profile 复用既有评测生命周期

Policy/Profile candidate 使用既有 EvalRun/EvalReport 基础设施，增加 definition kind、Context case set、选择 diff、预算 ledger、tokenizer contract 和确定性指标。首次迁移使用零费用 fixtures/golden：能证明等价的 node 可产生 baseline evidence；行为变化可通过静态、安全、预算和 fake-provider case set产生零费用 EvalReport，只有确需真实模型质量比较时才按既有显式预算确认规则执行。

发布只通过代码 Registry 改变 active/supported/revoked；评测 API 不在线切换。Profile overflow 兼容缺陷通过发布新 Profile 或 revoke 阻断，不原地改算法。

### 14. 历史迁移只允许等价自动绑定，否则显式 fork/rebind

迁移先 dry-run 生成按 Runtime、Agent/node 和原因分组的报告。旧 AgentDefinition 保持原 digest；新 AgentDefinition 版本引用 active Policy。历史 Session/Conversation 的扩展 binding 单独保存 baseline Policy/Profile：

- 只有完整 golden 能证明旧来源选择、顺序、裁剪、最终 Prompt 等价时才自动绑定并记录 EvalReport/迁移事件；
- 无法证明的作用域标记 `context_migration_required`，保持读取/导出但阻断模型调用；
- 显式 fork/rebind 创建新 binding 并保留 parent/source 关联；
- 历史已完成 Run 不伪造 Snapshot，继续使用 `legacy_partial_audit`。

所有 Rust/Pi 路径、迁移和回归通过后单次删除旧 helper/入口。多个 Policy 版本由同一 Compiler 解释，不构成双实现。回滚重新发布既有 supported Registry/二进制并保留向前兼容表；不得逆向删除新审计数据。

## Risks / Trade-offs

- [保守 byte upper bound 会显著浪费可用窗口] -> Profile 明确标记 conservative，审计预算浪费量；确认精确 encoding 后通过新 Profile/rebind 提升利用率，不降低安全性。
- [双语言 tokenizer 库或 encoding 资产产生差异] -> 固定依赖、共享资产 digest、跨语言 fixture 和启动期 self-test；任一差异阻止 ready。
- [当前 enabled 文本模型缺少 context window/Profile，切换后不可调用] -> 发布前迁移报告明确列出；通过模型管理补齐显式配置，禁止根据不透明上游名自动猜测。
- [原子候选拆分可能改变现有 Prompt 空白或顺序] -> 先建立全节点 byte/normalized semantic golden；未批准差异阻止 baseline 和自动历史绑定。
- [Pi hook 选择后破坏 Tool 对话完整性] -> 按 tool call ID 建立不可拆分组，加入孤立 call/result 失败测试和 Pi high-level fake-provider 回归。
- [审计快照增加本地存储] -> 排除项不保存全文，采用项与 owner 生命周期一致，列表只返回摘要，并测试级联删除与孤立 fork 隔离。
- [固定输出预留降低动态 Context 容量] -> 这是可证明预算的必要成本；输出上限变化必须进入版本化 Prompt/模型行为，不允许运行时借用。
- [历史 Session 大量进入只读] -> 只允许有证据的自动迁移，提供 dry-run 报告和显式 fork/rebind；不以兼容为由保留旧执行路径。

## Migration Plan

1. 增加 Registry v2 schema、Policy/Profile fixtures、双语言 loader/tokenizer/Compiler contract，保持生产入口未切换。
2. 以可空列和 namespaced 表扩展 PostgreSQL/SQLite，部署只读 migration inventory；不修改现有模型配置或 Session binding。
3. 扩展模型管理与 Runtime 校验，要求新 enabled 文本模型显式 context window/Profile；操作者为现有模型补齐配置后才能通过 readiness/cutover 检查。
4. 为全部 Rust/Pi node 创建新 AgentDefinition/Policy 版本与零费用 baseline/candidate EvalReport。
5. 先迁移 Rust Adapter 候选来源与 AuditedModelExecutor，再迁移 Pi public context hook；在测试环境保持旧生产入口直到全路径 ready。
6. dry-run 历史迁移并备份 PostgreSQL/SQLite；仅对证明等价记录写入 baseline binding，其余标记 `context_migration_required`。
7. 一次切换唯一 Context Compiler 入口，删除旧 helper/blob/Prompt 拼接路径和临时切换代码。
8. 运行全量 zero-cost、跨语言、迁移、审计、安全、Compose readiness 与跨服务回归；确认无真实模型、视频生成或平台发布调用。

回滚通过发布前一个受支持 Registry/二进制完成；数据库只做向前兼容扩展，不执行破坏性 down migration。若新 active Policy/Profile 存在安全问题，发布 revoked manifest 阻断并重新激活已验证 supported 版本；历史 Context/ModelCall 证据保持不变。

## Open Questions

- 无架构开放问题。生产切换前仍需由操作者为当前文本模型提供真实 `context_window`，并明确选择经其模型家族确认的 exact 或 conservative TokenizerProfile；这是部署输入，不允许实现自行推断。
