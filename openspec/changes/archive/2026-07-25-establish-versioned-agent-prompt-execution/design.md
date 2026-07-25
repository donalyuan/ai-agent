## Context

当前 Rust 生产 LLM 调用分散在项目策略草稿、脚本生成与修改、选题生成、质量评审与重写、主题组评审、声音推荐和作品 Agent 等模块，Prompt 直接由 Rust builder 构造并通过 `LLMClient` 调用；`agent_runs` 只保存运行级模型快照。Pi Runtime 则把可选 `system_prompt` 放入 Session metadata，直接构造 `AgentHarness`，普通 Turn、Tool Loop 后续 Turn、compaction 和 branch summarization 都缺少统一 Definition 与调用级审计。

本变更跨 Rust、TypeScript、PostgreSQL 与 Pi SQLite。仓库级定义必须是唯一治理事实源，数据库只能保存不可变发布记录与实际运行快照；Pi 仍负责通用 Turn/Tool Loop/Session Tree，Rust 仍负责视频领域状态、Run/Step 和 Gate。常规迁移与验证不能调用真实模型。

Pi `0.82.0` 已公开 `AgentHarness` 构造参数、`on()` hook、`subscribe()`、运行控制方法和 `Models` 接口。可用 hook 包括 `before_agent_start`、`context`、`before_provider_request`、`before_provider_payload`、`after_provider_response` 与 `tool_call`，足以在组合式 wrapper 中接入编译、审计和 Gate，无需访问私有字段或复制上游循环。

## Goals / Non-Goals

**Goals:**

- 为全部现有 Rust/Pi 生产 LLM 节点建立统一、版本化且随代码发布的 Agent/Prompt 定义。
- 固定 Session/Conversation 使用的定义和模型行为，消除任意 system prompt、静默 Prompt 升级和模型行为漂移。
- 在任何外部模型请求前保存脱敏完整逻辑输入，并为每次调用与重试保留独立终态。
- 提供跨 Rust/Pi 一致的审计读取、导出、dry-run replay 与有预算评测契约。
- 在不改变现有 Rust Prompt 语义、输出 Schema、视频业务结果与领域 Gate 的前提下完成一次性迁移。
- 保持 Pi 上游可升级性，只使用官方公开 API/hook 和组合式 wrapper。

**Non-Goals:**

- 不建设 Context Compiler 的优先级、token 预算和裁剪策略；现有 Context 只改为结构化编译输入。
- 不建设正式长期 Memory、Planner、虚拟制作团队或多 Agent 自由讨论。
- 不迁移视频领域执行到 Pi，不改变付费、发布、删除等领域 Gate。
- 不提供 Prompt/Definition 在线编辑、Admin UI 或数据库反向覆盖代码定义的能力。
- 不在未确认预算时执行真实模型评测，也不把 dry-run replay 伪装为真实质量评测。

## Decisions

### 1. 使用仓库级 Definition Registry 作为唯一治理事实源

新增仓库级 `agent-definitions/`，按统一 schema 保存结构化 manifest、独立 Prompt 模板、输出 Schema 引用和版本索引。`AgentDefinition` 至少包含 `agent_key`、语义版本、执行器 owner、角色/目标/约束、模型能力要求、允许的 Tool/profile、节点到 `PromptDefinition` 的精确版本引用；`PromptDefinition` 包含 `prompt_key`、版本、System/User 模板、变量声明、信任等级、输出契约和 token 上限。

Rust 与 TypeScript 各自实现强类型只读 loader，但读取同一目录、schema 与 canonical digest。构建和启动均校验未知字段、重复 key/version、跨 owner 引用、缺失模板、hash 不一致和非法状态；任一错误阻止服务就绪。发布流程把定义 key/version/digest/status写入不可变 release 记录，运行时不得从数据库加载模板正文或覆盖 manifest。

PostgreSQL 将 Definition 内容首次发布证据与代码 Registry 生命周期快照分离：`definition_releases` 按 key/version 固定内容 digest，`definition_release_manifests` 与 entries 按 registry digest 固定 candidate/active/supported/revoked 完整快照。同一 registry digest 重复部署幂等，不同生命周期 manifest 追加新快照；两类记录均不可更新或删除，也不保存模板正文。

选择共享 registry 而不是 Rust 常量、Pi 目录或数据库表，是为了避免多语言事实源分叉，并让 Definition 变更与代码评审、回滚和发布保持同一治理单元。发布镜像必须包含同一 registry digest，Compose 挂载只用于开发，不能成为生产镜像的隐式依赖。

### 2. 定义版本显式建模，历史版本永不原地修改

版本状态固定为 `candidate -> active -> supported -> revoked`。同一 `agent_key` 只有一个 active 版本；旧 active 在新版本发布后转为 supported。candidate 只可用于静态验证、dry-run 与显式 EvalRun；revoked 版本禁止继续发起模型调用，但历史读取、导出和 dry-run 不受影响。

manifest 中已发布的 key/version 内容发生变化视为校验失败，必须增加新版本。数据库 release 记录用 digest 证明运行镜像与已发布定义一致，不承担状态在线编辑。回滚通过发布一个把既有 supported 版本重新标为 active 的代码版本完成，不改写历史 snapshot。

Definition 内容 digest SHALL 排除生命周期 `status` 字段；`status` 由包含它的 registry digest 与代码发布 manifest 单独证明。这样 active/supported/revoked 切换会改变 registry digest，但同一 key/version 的 Definition 内容 digest、历史 binding 与评测引用保持不变。

### 3. PromptCompiler 固定 System/User 分层并实行 fail-closed

编译输入使用带 `schema_version` 的结构体，不接受 Adapter 拼接完成后的任意 Prompt。System 层只允许平台规则、Agent 角色与约束、节点职责、输出契约以及 Tool/能力边界；User 层承载 `confirmed_fact`、带来源的 `reference`、`user_instruction`、`steer`/`follow-up` 和 `candidate` 等动态片段。每个片段保存稳定 ID、信任等级、来源、内容或资产引用。

Compiler 只替换 definition 声明的变量，并校验必填/未知变量、类型、大小、定义引用、输出 Schema 与 Tool 声明。对于随请求变化但属于既有 provider 合同的 JSON Schema 约束，Definition 可声明强类型变量，并以完整值占位符参数化 output schema；Compiler 必须以原始 JSON 类型替换，禁止字符串拼接或未声明占位符。动态字符串不能写入 System 模板；任何异常都在创建 ModelCall 前失败，不使用空值、旧 Prompt、默认 system prompt 或非结构化拼接降级。

编译输出形成不可变 `PromptSnapshot`，包含 definition 版本/digest、编译输入、System/User 逻辑消息、输出 Schema、Tool Schema 和非敏感参数。第一版保持现有 Context 内容和裁剪方式，以 golden regression 证明迁移后的逻辑 Prompt 与当前节点等价。

### 4. Pi 与 Rust 分别建立固定 Session 绑定

Pi `POST /sessions` 改为接收 `agent_key`、`model_id` 与 tool profile，不再接受 `system_prompt`。创建时解析 active `AgentDefinition`、其全部 Prompt 精确版本、模型能力和 `behavior_fingerprint`，原子写入 Session binding；普通 Turn、Tool Loop 后续 Turn、compaction、branch summarization 和 fork 都读取该 binding。普通 fork 继承绑定；升级 fork 必须显式指定目标 agent/version/model，并记录来源。

Rust Conversation 创建 API 当前没有 `model_id`，而已确认视频 Conversation URL、字段和响应保持不变。因此新 Conversation 在创建时固定 Agent/Prompt 版本，模型绑定则在第一次可能调用模型的既有消息请求中、外部调用前原子建立；后续消息仍保留 `model_id` 字段，但必须与 binding 一致。提交不同模型时返回稳定 `model_rebind_required`，只有显式 rebind/fork 操作可创建新绑定，任何模型调用都不得在未绑定状态执行。

非会话型 Rust LLM 操作以一次业务 Run 作为固定作用域：入口解析 Definition 与模型绑定，内部生成、评审、重写及重试继承该绑定。这样既保持现有 API，又消除同一 Session/Run 内的静默切换。

### 5. 用 behavior_fingerprint 区分凭据轮换与模型行为变化

`AgentDefinition` 只声明文本、Tool Calling、结构化输出、视觉、reasoning 与最小 context window 等能力，不复制 `ai_models` 部署配置。`behavior_fingerprint` 对协议、请求根地址的规范化非敏感身份、上游模型、reasoning、输出上限、context window 和其他影响行为的 settings 计算 canonical hash；API Key、Authorization、Cookie 和其他凭据不参与 hash。

每次调用仍从 PostgreSQL `ai_models` 解析当前配置。只发生凭据轮换且 fingerprint 不变时可继续；fingerprint 改变、模型停用/删除或能力不兼容时必须在请求前阻断并要求显式 rebind/fork，不回退默认模型或其他供应商。

### 6. ModelCall 是最小审计单元，持久化失败即禁止调用

Rust 在 PostgreSQL 保存 `model_calls`，Pi 在其 SQLite 中保存 namespaced `novex_model_calls`；两者使用相同 `schema_version`、字段语义和状态机。每条记录关联拥有者 Session/Conversation/Run、父调用或重试根、node key、attempt、Agent/Prompt 版本、PromptSnapshot、Context/Memory 来源、Tool Schema、模型 fingerprint、非敏感参数和资产引用。

调用状态从 `prepared` 单向进入 `succeeded`、`failed` 或 `aborted`。外部请求前必须完成脱敏、schema 校验和 `prepared` 持久化；失败时不发请求。输出、usage、错误与终态只能写入一次，不复用或覆盖旧记录。业务层及 Runtime 层每次重试创建新的 ModelCall，并以 root/attempt 关联；底层透明重试必须关闭或提升到可审计 wrapper，避免一次记录对应多个不可见请求。

文本逻辑输入和输出保存脱敏全文；图片、音频、视频仅保存不可变资产 ID、版本/hash、MIME 与必要元数据，不保存 base64。secret 标记字段、凭据、认证头、Cookie、原始请求头和带敏感查询参数的 URL 在持久化前统一剔除。Agent Run/Step 与 Pi entries 只保存 `model_call_id` 摘要关联，不复制另一份可漂移的完整快照。

### 7. Pi 采用组合式 Novex wrapper 与公开 hook/API

新增 Novex-owned wrapper 持有而不继承 `AgentHarness`。wrapper 负责加载 binding/definition、注册公开 hook、代理 `prompt`、`steer`、`followUp`、`compact`、`navigateTree`、`abort` 和订阅事件；`fork` 不属于 Pi `AgentHarness`，由同一 Novex 组合层的 `SessionCoordinator` 代理公开 `SessionRepo.fork`。Pi 继续独占 Turn/Tool Loop/SSE/Session Tree 生命周期。

- `before_agent_start` 与 `context`：注入编译后的 System/User 逻辑输入并捕获实际 Context。
- `before_provider_request` / `before_provider_payload`：校验模型 fingerprint，在请求发送前创建并持久化 ModelCall；持久化异常通过 hook 失败终止请求。
- `after_provider_response` 与 Harness 事件：完成调用终态、usage、输出/错误关联。
- `tool_call`：继续执行 Novex Tool Gate；插话通过 Pi 公开 queue 方法进入可审计动态 User 输入。

公开 `Models`/Provider 边界由 Novex audited wrapper 组合包装，以关闭不可见透明重试并为每个显式 attempt 分配 ModelCall。禁止修改 Pi 源码、导入未导出路径、继承私有实现、monkey patch、反射私有字段或复制 agent/tool loop。静态架构测试扫描这些禁用模式，并对允许的公开 import 建立 allowlist。

### 8. Rust 使用统一 AuditedModelExecutor 迁移所有生产节点

在 `novex-agent`/`novex-ai-core` 定义与业务无关的 compiled call、binding 和审计 port，在 backend 实现 PostgreSQL repository 与 Definition bootstrap。`AuditedModelExecutor` 组合 `LLMClient`，要求调用方提交 node key 和结构化编译输入，负责 compile、persist-before-call、provider 调用与终态；Adapter 不再直接构造 `LLMPrompt` 后调用裸 `LLMClient`。

一次迁移清单至少覆盖：项目策略草稿及重试、脚本 metadata/单镜/完整生成、脚本生成意图与分镜修改、选题生成、质量评审、重写后的再评审、主题组评审、声音推荐、作品意图/规划，以及仓库搜索发现的其他生产 `generate_script` 调用。测试 fake client 与底层 provider contract 可保留直接 `LLMClient` 使用，但生产装配不得暴露绕过 audited executor 的路径。

现有 `AgentRunCoordinator`、Run/Step 类型与顺序、业务 Repository、输出解析、事务、领域写入和 Gate 不变；ModelCall 作为更细粒度审计记录通过 ID 与现有 Run/Step 关联。

### 9. 审计 API、导出与 replay 使用统一版本化合同

Rust backend 与 Pi Runtime 分别为自己拥有的数据提供相同字段语义的列表、详情、导出和 replay 入口。列表默认只返回调用 ID、拥有者、node、版本、模型、状态、token/cost 摘要和时间；详情才返回脱敏后的完整逻辑输入/输出。导出 envelope 固定 `schema_version`、source runtime 和校验 hash，支持离线合并，不要求跨 PostgreSQL/SQLite 双写事务。

默认 replay 为 `dry_run`：只加载历史快照、验证 definition 可解析、重新编译并生成结构化 diff；禁止模型调用、Tool、领域写入和 Session/Run 变更。需要与真实模型比较时必须创建独立 EvalRun，并引用而不修改源 ModelCall。

用户明确删除拥有者 Session/Run 时，ModelCall 按现有所有权规则级联删除；Definition release 和版本 hash 永久保留。PostgreSQL 在同一所有权事务中级联并标记关联 EvalReport source deleted。Pi 公开 `SessionRepo.delete` 不提供跨扩展表事务 hook，且禁止依赖其私有表，因此 SQLite 采用 Novex namespaced durable deletion intent：先持久化删除意图，再调用公开 Session 删除，随后在 namespaced 事务中清理 binding、ModelCall 与迁移事件；中断时由重试或启动 reconciliation 根据公开 Session 列表完成清理，成功响应前必须清除意图。不得通过私有表 trigger、外键、SQL 拦截或复制 Pi 删除逻辑伪造单事务。EvalReport 可保留不含原始内容的聚合指标，但必须标记 source deleted。撤销、回滚、fork 或 rebind 不删除历史记录。

### 10. candidate 激活由不可变 EvalReport 门禁

candidate 发布先执行 schema/引用/安全静态检查和历史 snapshot dry-run，再执行结构化输出、安全、核心质量、token 与成本阈值评测。EvalRun 固定 candidate、基线版本、模型 fingerprint、case set 版本、最大 case 数、最大 token 和成本上限；EvalReport 保存逐项结论的脱敏结果与聚合指标，完成后不可修改。

真实模型 EvalRun 在创建前必须有显式预算确认；无确认时只能运行零费用检查，不能把 candidate 标为 active。首次 v1 若 golden regression 证明与当前 Prompt 字节或规范化语义等价，可建立 baseline 而不产生付费调用。激活和回滚只能随后续代码发布改变 manifest 状态，评测 API 不能直接在线改 active 定义。

### 11. 历史迁移遵循“可证明才回填”

未保存自定义 `system_prompt` 的 Pi Session 首次打开时绑定行为等价的 `personal.general@1` 并写迁移事件；保存过自定义 system prompt 的 Session 标记为 read-only，必须显式 fork 到选定 `agent_key`，用户选择丢弃旧文本或把它降级为可见普通 user instruction，旧文本永不再进入 System。

Rust Conversation 按现有 `agent_type` 确定性回填 v1 Agent/Prompt binding；已有首轮模型记录时可据其 `model_id/model_snapshot` 建立 fingerprint，否则等待首个模型请求原子绑定。历史 Run 缺少准确 Prompt/Context 时只标记 `legacy_partial_audit`，不创建伪造 ModelCall。

迁移命令必须可重复执行、先报告计划与异常、后在事务中写入，并在切换生产入口前通过完整覆盖审计。任何无法确定映射的记录保持只读或 partial 状态，不猜测定义版本。

## Risks / Trade-offs

- [Definition 双语言解析产生差异] -> 使用同一 JSON Schema、canonical serialization、跨语言 fixture 和 digest contract test；启动时校验 release digest。
- [调用前持久化增加延迟和存储量] -> 只在详情保存脱敏逻辑全文，列表使用索引摘要，大对象只存引用；以审计完整性优先，不采用异步补写。
- [Pi hook 生命周期升级后变化] -> 只依赖公开类型和事件，增加上游 API compatibility test；Pi 升级必须先通过该门禁。
- [关闭透明重试改变供应商行为] -> 在公开 audited wrapper 中显式复现已批准的 attempt 上限和退避，每次 attempt 独立建档，禁止对已产生部分输出的流静默重试。
- [一次性迁移节点多，遗漏会形成旁路] -> 建立生产裸 `LLMClient`/`Models` 调用静态扫描 allowlist，并用 fake provider 断言所有已知节点都生成 ModelCall。
- [固定模型影响现有逐轮切换习惯] -> 保留 Rust 既有 `model_id` 字段用于一致性校验，提供显式 rebind/fork；不允许静默变化换取可复现性。
- [Prompt 脱敏误删业务语义或漏 secret] -> secret 使用 schema 标记与集中 redactor，保存前执行拒绝式扫描；用 canary secret 和结构化 fixture 验证，不在日志输出原文。
- [Definition 撤销导致旧 Session 无法继续] -> 保留历史读取、导出与 dry-run，允许显式 fork 到受支持版本；安全撤销优先于连续执行。

## Migration Plan

1. 建立共享 schema、registry v1、跨语言 loader/fixture 和所有现有 LLM 节点 inventory；用 golden tests 固化当前 Prompt、Schema、参数和业务结果。
2. 新增 PostgreSQL/SQLite namespaced 持久化、binding、ModelCall、EvalRun/EvalReport 与审计 API，先以 fake provider 验证调用前持久化和删除/回放语义。
3. 接入 Rust `AuditedModelExecutor` 并一次迁移 inventory 中全部生产调用；静态扫描阻止裸调用旁路。
4. 接入 Pi 组合式 wrapper、公开 hook 与 audited Models，覆盖 Turn、Tool Loop 后续 Turn、compaction、branch summary、steer/follow-up 和 fork。
5. 运行历史 migration dry-run，审查映射后执行幂等迁移；自定义 system prompt Session 保持只读。
6. 执行跨语言 schema、golden regression、fake-provider、迁移、审计 API、replay、安全、Rust workspace、Pi Runtime 和 Video Worker 回归，确认零真实模型调用。
7. 单次发布 registry、数据库 migration、Rust 与 Pi 入口；不保留 feature flag 双轨。部署失败时整体回滚二进制与 active manifest，保留向前兼容的新表和已写审计记录，不逆向删除数据。

## Open Questions

无。真实模型评测的 case 数、token 和成本预算属于每次 EvalRun 的运行时审批输入，不是本提案尚未决定的架构问题。
