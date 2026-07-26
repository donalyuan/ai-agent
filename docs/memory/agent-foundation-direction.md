---
name: agent-foundation-direction
description: Novex Agent 基座、角色分化和虚拟制作团队的长期方向
metadata:
  type: project
---

# Agent 基座与虚拟制作团队方向

## 基座执行模式

- Novex 定位为本地单用户、多领域个人 AI 工作台；视频生产是首个领域应用，不建设未确认的多租户客户交付能力。
- 新工作台的通用执行由 `services/agent-runtime` 中的 Pi Harness `0.82.0` 承担，包括 Turn、Tool Loop、SSE、steering、follow-up、abort、Session Tree 和 compaction；Runtime 使用 `toolContext + AgentHarnessTool` 契约，并保留 Novex 自有 `read/write/edit/bash` schema，避免改变既有工具 transcript。
- Novex 不修改或 fork Pi 上游源码；使用 Novex 自有组合式 wrapper 持有 `AgentHarness`，通过 Pi 官方公开 hook 和公开方法接入 Prompt、Context、模型调用审计与 Tool Gate。禁止通过继承、monkey patch、复制 Tool Loop 或未导出的内部路径改变 Pi 私有生命周期，以保持后续 Pi 升级能力。
- 领域 Agent 默认采用“受控工作流型 Agent”：业务状态机、固定检查点、成本限制、权限和最终写入权由代码控制。
- 自主 `Planner` 作为可选的局部能力，只在调研、开放式分析、失败诊断等授权节点中提出或执行有限子计划；不得绕过预算、权限、质量闸门、人工确认或业务状态规则。
- 最终形态采用“受控工作流外壳 + 局部 Planner”，不以多个 Agent 自由群聊作为默认架构。

## Agent 角色与 Prompt

- 每类 Agent 使用版本化 `AgentDefinition`，当前 schema 描述角色、目标、约束、模型能力要求、允许的 Tool/profile，以及每个 LLM node 的 Prompt 与 Context Policy 精确引用；正式 Memory 策略仍属于后续独立能力。
- 每个 LLM 节点使用独立、版本化的 `PromptDefinition`。实际模型输入由平台规则、Agent 角色、节点职责、动态上下文、用户指令和输出契约共同编译，形成不可变 `PromptSnapshot`。
- `PromptCompiler` 使用固定 System/User 分层：System 只包含平台规则、Agent 角色与约束、节点职责、输出契约及 Tool/能力边界；已确认领域事实、带来源的参考 Context、当前用户指令和插话进入 User 层。动态内容必须按 `confirmed_fact`、`reference`、`user_instruction`、`candidate` 等信任等级结构化隔离，禁止把动态字符串直接插入 System 模板；缺失必填变量、未知变量、无效版本引用或其他编译异常必须明确失败，不得以空值或旧 Prompt 降级继续。
- 仓库级版本化 Definition Registry 是 `AgentDefinition`、`PromptDefinition`、`ContextPolicyDefinition` 与 `TokenizerProfile` 的唯一事实源，使用结构化 manifest、独立模板和统一 schema 随代码发布；每个 Agent 只能声明一个执行器 owner，Rust 与 Pi Runtime 通过各自的强类型 loader 只读加载自己拥有的定义。PostgreSQL 与 Pi SQLite 只保存不可变发布记录和运行快照，数据库内容不得反向覆盖运行定义。
- 同一 Agent 的生成、评审和重写节点可以使用不同专业角色。例如选题生成使用策划角色，质量闸门使用独立评审角色，避免一个万能 Prompt 同时承担冲突职责。
- 读取资源、状态流转、持久化等确定性步骤不需要角色 Prompt，也不得伪装成模型推理。
- 第一版角色和 Prompt 随代码版本治理，不提供后台任意在线编辑；数据库可以保存由代码发布流程写入的不可变版本和运行快照，但不得原地覆盖已生效 Prompt。Prompt 行为变化必须经过版本、回放和评测。
- Agent/Prompt 版本生命周期为 `candidate -> active -> supported -> revoked`；`candidate` 只能用于静态验证、历史快照 dry-run 回放和显式评测，普通 Session 不得选择。行为变化版本必须在相同模型条件下通过安全、结构化输出、核心质量与 token/成本阈值并生成不可变 `EvalReport` 后，才能由后续代码发布标记为 `active`；任何真实模型评测必须先明确案例数、最大 token 和成本上限并取得用户确认，未获批时不得激活行为变化版本。首次 v1 迁移若以 golden regression 证明与当前 Prompt 字节或语义等价，可据此建立基线而不额外产生付费调用。
- Agent Session 创建时固定当时生效的 `AgentDefinition` 及其引用的 `PromptDefinition` 版本，运行期间不得静默升级；新版本默认只用于新 Session，旧 Session 只能通过显式 fork 迁移。存在安全问题的旧版本可以撤销并阻断继续执行；版本撤销、切换或回滚不得删除或替换历史定义和运行快照，只有用户明确删除拥有者 Session/Run 时才能按数据删除规则级联清理对应快照。
- `AgentDefinition` 只声明模型选择策略与文本、Tool Calling、结构化输出、视觉、reasoning、最小 context window 等能力要求，不复制 `ai_models` 的部署配置或凭据。enabled 文本模型必须显式保存 `context_window` 与 Tokenizer Profile key/version；Session 固定 `model_id` 与由协议、上游模型、请求地址、reasoning、输出上限、context window、Profile 及行为相关 settings 计算的非敏感 `behavior_fingerprint`。每轮仍从 `ai_models` 解析最新配置，凭据轮换不改变 fingerprint，可透明继续；窗口、Profile 或其他模型行为配置变化必须阻断并要求显式重新绑定或 fork。模型不存在、停用、删除、配置缺失或 Profile 不兼容时明确失败，不得根据模型名猜测或回退默认 Profile。
- 历史 Pi Session 中未设置自定义 `system_prompt` 的会话在首次打开时可审计地绑定行为等价的 `personal.general@1`；保存了自定义 `system_prompt` 的会话只能读取，必须显式 fork 到选定 `agent_key`，并由用户选择丢弃旧 Prompt 或将其降级为可见的普通用户指令，旧 Prompt 永不得再次作为 system prompt 执行。Rust 历史 Conversation 按已知 `agent_type` 确定性回填对应 v1 Definition；缺少准确 Prompt/Context 的历史 Run 标记为 `legacy_partial_audit`，不得伪造完整 `ModelCall` 快照。
- 用户在 Agent 运行中插话时，该内容作为可审计的动态用户输入，按 `steer`、`follow-up` 或领域修改请求在安全执行边界影响后续生成；插话不得改写版本化 Agent/Prompt 定义、直接覆盖已确认产物、自动写入正式 Memory，或绕过成本、权限、质量与发布 Gate。

## Context 与 Memory

- `Context` 是本轮动态装配的数据。当前 Rust 与 Pi 全部生产文本 node 已统一进入同 schema、同 canonical digest 和同决策语义的 Context Compiler；来源 Adapter 只提交原子 `ContextCandidate`，旧手工字符裁剪、整段预格式化 Prompt fragment、Pi 单一 Context blob 和未治理消息入口已从生产路径删除。
- Context Compiler 的 token 计数采用“精确 tokenizer + 明确声明的保守策略”：模型优先绑定版本化的精确 tokenizer；无法精确映射时，只能显式选择具备算法、适用范围和安全余量定义的保守策略，禁止静默回退。模型未绑定可用策略、策略版本失效或计数失败时必须在外部模型调用前明确失败。tokenizer/profile 选择及其版本属于模型行为和 Context 审计快照的一部分，变化不得让既有 Session/Run 静默漂移。
- Context Compiler 以语义 fragment、消息及 Tool request/result 组作为不可拆分的基础裁剪单元，优先整体排除低优先级候选，不得从中间截断 JSON、Tool 调用链、已确认事实或其他需要保持结构完整的内容。来源 Adapter 必须在进入 Compiler 前提供可独立治理的原子候选，不得继续用整段预格式化 Prompt 隐藏内部优先级。必需 Context 在合法裁剪后仍超出预算时必须明确返回 `context_budget_exceeded`，不得静默丢失、截短或降级执行。
- Context 候选的 `trust`、`priority`、`required` 与 `freshness` 是相互独立的治理维度，信任等级不得直接充当裁剪顺序。默认保留层级由版本化 `ContextPolicyDefinition` 声明：P0 为当前用户指令及插话、节点必需执行状态和不可拆分的 Tool 调用组；P1 为当前有效的已确认事实；P2 为当前分支近期消息和用户本轮明确指定的参考；P3 为普通参考与有损摘要；P4 为候选、过期及重复内容并优先排除。具体 node 可以通过精确版本引用收紧来源与必需项，但不得绕过 P0 保护；同层选择和排除必须使用确定性稳定排序并写入 Context 审计决策。
- 每项 Context 候选必须携带稳定身份、来源及来源版本、`observed_at`、可选 `valid_until`、可选 `supersedes` 和内容 hash。超过有效期的候选直接失去入选资格，不得仅因预算充足继续使用；节点必需来源过期时明确失败。相同权威来源内只选择最新有效且未被替代的版本；多个仍有效的已确认事实发生不可消解冲突时返回 `context_conflict`，不得按时间或排序擅自选择。普通 reference/candidate 的冲突可以并存，但必须保留来源与冲突标记。确定性去重只使用稳定身份、版本关系和内容 hash，不调用 LLM 做语义去重。
- Context token 预算由固定模型 binding、`PromptDefinition`、精确版本的 `ContextPolicyDefinition` 与 `TokenizerProfile` 共同确定，Adapter 不得自行声明预算。动态 Context 预算固定为模型 context window 减去 System Prompt、User 模板固定部分、Tool Schema、输出 Schema、协议封装、最大输出 token 和声明式安全余量；必须先完整保留最大输出预算，禁止为容纳更多 Context 静默借用。执行顺序固定为“解析 binding/policy -> 编译固定 Prompt 部分 -> 计算动态预算 -> 选择 Context -> 完成 Prompt -> 最终 token 复核 -> 持久化 ContextSnapshot -> 持久化 prepared ModelCall -> 外部调用”。固定部分或必需 Context 超限统一返回带具体原因的 `context_budget_exceeded`；最终复核失败不得临时重裁剪或透明重试。供应商仍报告 context overflow 时记录为 tokenizer/profile 兼容性缺陷并阻断，不得即时缩短请求继续调用。
- Context 编译成功后生成不可变 `ContextSnapshot`；编译失败时生成不可变 `ContextCompileAttempt`，但不得创建未实际调用模型的 `ModelCall`。`ModelCall` 只引用成功快照的 ID 与 digest。快照必须记录单次固定时钟、Context Policy 与 tokenizer/profile 精确版本、模型窗口、完整预算账本、稳定排序键、每项选择决策、最终顺序和 digest。采用项保存脱敏后的完整逻辑内容或不可变资产引用；排除项按数据最小化原则只保存稳定身份、来源及版本、内容 hash、token 数和稳定 decision code，不额外保存未发送全文。Rust/PostgreSQL 与 Pi/SQLite 使用相同 `schema_version` 和字段语义；dry-run 使用历史精确版本重建并校验相同结果，不读取后来变化的来源覆盖历史快照。删除拥有者 Session/Run 时，Context 记录按既有所有权规则级联删除。
- `ContextPolicyDefinition` 使用 `candidate -> active -> supported -> revoked` 生命周期，`TokenizerProfile` 使用不可变 key/version，算法、安全余量或适用范围变化必须发布新版本。新版 `AgentDefinition` 的每个 LLM node 精确引用一个 Context Policy；Session、Conversation 与非会话 Run 固定 Policy 和 tokenizer/profile，运行期间不得静默变化。新作用域选择当前 active 组合；revoked 版本阻断后续模型调用但保留历史审计与 dry-run；行为变化只能通过显式 fork/rebind。历史绑定只有在 golden regression 能证明选择、顺序、裁剪与最终 Prompt 等价时，才可审计地绑定 baseline Policy；无法证明等价时标记 `context_migration_required` 并限制为只读，必须显式 fork/rebind。本项能力必须一次迁移全部 Rust/Pi 生产节点，删除 Adapter 手工裁剪、整段预格式化 Prompt fragment 和 Pi 单一 context blob 等旧装配路径；多个 Policy 版本共用唯一 Context Compiler，不保留两套执行实现。
- `Memory` 只保存已确认、稳定、可复用的信息，例如账号策略、用户明确偏好、已确认反馈和长期约束；瞬时数据、未确认推断和临时对话摘要不得自动升级为长期记忆。
- 记忆必须具备作用域、来源、置信度、状态、更新时间、失效和删除语义。模型提出的新记忆先作为候选，经过人工确认或确定性规则后才能生效。
- 当前已保存模型、Prompt、Context 和 Tool 相关必要快照，以支持审计、回放和评测；正式 Memory 尚未落地，因此不得声明或伪造 Memory 快照。
- 每次实际模型调用必须建立独立、不可覆盖的 `ModelCall` 记录，并在发起外部请求前持久化脱敏后的完整逻辑输入；生成、评审、重写、Tool Loop 和每次重试分别建档，失败也保留终态。快照包含实际 Agent/Prompt 版本、编译输入、Context/Memory 来源、输出与 Tool Schema、非敏感模型参数和结果，但不得保存凭据、认证头、Cookie、原始请求头或标记为 secret 的内容；快照持久化失败时不得继续调用模型。
- Rust 与 Pi 的模型调用记录使用统一、带 `schema_version` 的读取与导出格式：列表默认只返回摘要，详情返回脱敏后的完整逻辑输入与输出；文本可保存脱敏全文，图片、音频、视频等大对象只保存不可变资产 ID、版本/hash、MIME 与必要元数据，不得写入 base64 二进制。回放默认是零模型调用、零 Tool、零领域写入的 `dry_run`，任何模型对比必须新建显式预算的 `EvalRun`，不得覆盖源快照。版本撤销、回滚和 fork 不得删除历史记录；用户明确删除拥有者 Session/Run 时，对应 ModelCall 级联删除，EvalReport 只可保留不含原始内容的聚合指标并标记来源已删除。
- Pi SQLite 保存 Session Tree、消息、工具结果、分支、compaction，以及 Novex namespaced 的固定 binding、`ContextSnapshot`、`ContextCompileAttempt`、`ModelCall`、迁移事件和可恢复删除意图；PostgreSQL 继续保存领域事实、模型配置和 Rust Context/ModelCall/评测证据。Pi summary 不自动写入正式长期 Memory。

## 虚拟制作团队

- Video Agent 的长期目标是受控的 `Virtual Production Crew Agent`，参考 `Emily2040/seedance-2.0` 的“主路由器 + 按需专业 Skill + 共享项目状态 + Prompt Compiler + Gate”思想，但不把其文档式 Skill 包直接当作后端多 Agent Runtime。
- 第一版采用一个 `ProductionOrchestrator`、多个版本化 `RoleDefinition`、共享 `ProductionState`、结构化阶段产物、固定 Gate 和 `PromptCompiler`，而不是多个独立 Agent 自由讨论。
- 专业角色方向包括制片人、编剧、导演、摄影指导、表演指导、剪辑师、声音指导和 QC。简单短视频应走 Fast Lane，不强制启动完整制作团队。
- “演员”优先建模为持久化 `CharacterBible` 与受约束的 `PerformanceDirector`；需要角色视角校验时可以增加 `CharacterCritic`，但角色不得拥有发布、付费生成、删除或正式记忆写入权。
- 共享制作状态至少逐步覆盖 `CreativeBrief`、`StoryBible`、`CharacterBible`、`ScriptDraft`、`DirectorialTreatment`、`ShotContract`、`PerformanceBrief`、`SoundPlan`、`ContinuityLedger` 和 `TakeReview`。
- 各角色只拥有自己的结构化产物，通过新版本或修改建议协作，不直接覆盖其他角色的已确认产物。实际生成结果经接受后覆盖计划状态，成为后续镜头的连续性事实来源。

## 实施顺序与边界

1. 新的通用 Agent 会话统一进入 Pi Runtime，不在 Rust Kernel 中建设第二套通用 Tool Loop。
2. PostgreSQL `ai_models` 是唯一模型配置来源；Session/Conversation/Run 固定 `model_id + behavior_fingerprint`，Runtime 每次调用前重新解析配置并把非敏感快照保存到独立 `ModelCall`，不回退环境变量或 Pi 模型目录。
3. `chat` profile 无本地工具；`workspace` profile 才启用固定工作目录下的 `read/write/edit/bash`。
4. 现有视频 Conversation API、Rust Adapter、Run/Step 和领域状态保持不变；后续迁移必须以类型化领域 Tool 为前提，并在独立 change 中删除重复执行路径。
5. 付费生成、正式发布、删除领域数据等动作继续由 Rust 领域 Gate 管理，禁止通过通用 bash 绕过。

### 已确认第一步

- 第一项正式变更命名为 `stabilize-agent-runtime-kernel`，目标是完成外部行为不变的 `Agent Runtime Kernel v1` 重构。
- 本步建立 `AgentKey`、`AgentAdapter`、`AgentRegistry`、`AgentExecutionContext`、`AgentRunCoordinator` 和 Run/Step 记录契约，以注册机制替代 Runtime 核心中的业务类型分派。
- 业务 Repository 归对应 Adapter 持有；Kernel 不认识选题、脚本、声音或作品等业务概念，缺失依赖在 Bootstrap 注册阶段失败，不延迟到运行时。
- 本步不引入 Memory、Tool Loop、自主 Planner、虚拟制作团队业务流或新的角色 Prompt；只为后续 `AgentDefinition` 的版本和能力声明保留稳定边界。
- 本步不得改变现有 HTTP API、数据库结构、模型选择、Prompt、消息 metadata、Run/Step 审计语义及选题、脚本、声音、作品 Agent 的业务结果。

### 已确认第二步

- 第二项正式变更为 `establish-versioned-agent-prompt-execution`，一次覆盖 Rust 项目策略、脚本、选题、质量评审与重写、主题组评审、声音、作品等全部现有生产 LLM 节点，以及 Pi 普通 Turn、Tool Loop 后续 Turn、compaction 和 branch summarization；不得只迁移示范 Agent，也不得保留新旧 Prompt 或未审计模型调用双轨。
- 本步建立 Definition Registry、Rust/TypeScript 强类型 loader、`AgentDefinition`、`PromptDefinition`、`PromptCompiler`、模型调用级快照及对应 PostgreSQL/Pi SQLite 持久化，并以 `agent_key` 取代 Pi Session 创建接口的任意 `system_prompt` 输入。
- 本步提供模型调用摘要、脱敏详情、统一导出、dry-run replay 和评测入口，但不新增 Admin 前端页面；前端审计界面必须另行经过设计上下文、Pencil 原型和用户确认。
- 本步保持现有 Rust Prompt、输出 Schema、模型参数、业务结果、视频 Conversation API 和领域 Gate 不变，不把现有视频业务执行迁移到 Pi。
- Context Compiler 的优先级选择、token 预算与裁剪作为紧接着的独立 change 推进；本步保持现有 Context 装配行为，但以结构化输入进入 Prompt Compiler 并完整快照。正式 Memory、Planner 和虚拟制作团队不纳入本步。

### 已确认第三步

- 第三项正式变更为 `establish-governed-context-compilation`，已一次覆盖 Rust 与 Pi 全部生产文本 node，并建立版本化 `ContextPolicyDefinition`、`TokenizerProfile`、原子 `ContextCandidate`、确定性 Context Compiler、`ContextSnapshot`、`ContextCompileAttempt`、统一导出与无副作用 replay。
- Rust 使用 PostgreSQL、Pi 使用 namespaced SQLite 保存固定 Context binding 与审计证据；每个实际 provider step 和显式 retry 分别对应一个 `ContextSnapshot + ModelCall`，编译失败只保存 CompileAttempt。两个 Runtime 不建立跨库事务或同请求双执行。
- 历史 Session/Conversation 只有在全节点 golden 证明等价时才自动绑定 baseline Policy；证据不足时保持只读 `context_migration_required` 并要求显式 fork/rebind。回滚只重新发布 supported Registry/二进制并保留向前兼容审计表，不执行破坏性逆迁移。
- 文本模型必须由操作者显式提供 context window 和适用的 Tokenizer Profile，系统不得根据不透明模型名推断。配置缺失会继续阻断生产模型切换，但不影响 Context Compiler、迁移、审计和零费用验证的已落地事实。
- 本步没有新增正式 Memory、RAG、Planner、虚拟制作团队业务流或 Admin Context/审计 UI；这些能力仍需独立 OpenSpec 与相应前端原型确认。

## 暂缓范围

- 网络热门视频采集与总结暂缓，不纳入本轮 Agent 基座方向；后续如恢复，必须另行确认数据来源、合规边界和证据时效规则。
