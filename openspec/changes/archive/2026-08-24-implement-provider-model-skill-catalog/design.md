# Provider、Model 与 Skill Catalog 设计

## Registry 与 runnable gate

Registry 固定保存八项 candidate 的 `provenance`、`approval`、`enabled`：`drama-skills` 和 `novel-writing` 为 `verified_snapshot`、`approved`、`true`；`zy-cinematic-realism`、`seedance-2.0`、`storyboard-tiktok-video-skill`、`hell-grind/cinedance-higgsfield`、`hell-grind/acting`、`hell-grind/lira` 为 `pending_provenance`、`not_approved`、`false`。仅前两项 approved revision 可进入 `drama-mvp-a-default` binding。Registry index/approved metadata 可在 Worker 启动读取，但 disabled/pending candidate 不得成为启动 lock 或默认 Run 前置；节点仅在 `allowedSkills`、`requiredCapabilities`、`selectionMode=fixed|inherit` 都匹配时按需加载。

catalog 的 runnable gate 不得创造别名：operation 必须分别保存并校验 `adapterInstalled`、catalog `approval`、`operationCapabilitySnapshot`、`runnable`、`featureGate`。首次显式 connection-test/probe 只要求 installed、`approval=approved`、`featureGate=MVP-A`、explicit live opt-in、已选 profile、可解析 credential 和 timeout，不要求已有 snapshot 或 `runnable=true`，也不因 disabled-for-run 拒绝；成功才冻结 successfully probed snapshot。snapshot 缺失、`runnable=false` 与 disabled-for-run 只阻断 enable/default、Run resolution 与 live operation invocation，后者需要 installed、approved、该成功 snapshot、`runnable=true`、`featureGate=MVP-A` 全部成立。MVP-B candidate、uninstalled、not-approved 或缺 explicit opt-in/profile/credential/timeout 一律不允许首次 probe 或外部调用；TTS/ASR、MiniMax H3、Seedance 2.5、Agnes 未选中 mode 不可运行，默认测试使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），explicit live opt-in 不改变。

## 上下文

阶段 0 已有六个 Port 合约、`Mock Provider +` 显式 Local test/offline profile、进程内 `ProviderCatalog` 与确定性的 Skill registry/router 行为，但没有持久化 Provider 配置、项目选择、冻结能力、credential 处理或调用/usage 审计。目标架构要求 `interfaces -> application -> domain`；一个写 Command 使用一个 UoW，外部副作用只在提交后发生。

## 总体计划追溯、依赖与非目标

本设计落实 `plan-phase-one-drama-mvp-a` 任务 `2.3`，总体计划只定义协调关系，不形成运行时代码依赖。直接依赖阶段 0 的 Provider/Skill foundation；后续 `integrate-agentscope-text-skills`、`integrate-gpt-image-provider`、`integrate-agnes-video-provider`、编辑与时间线 change 依赖本 catalog。catalog 与 `implement-workflows-runs-slice` 可并行实施；与 Run 仅以 `run_id`、`node_run_id`、`correlation_id` 关联。

完整非目标是实现真实 Provider SDK/adapter 或外部调用、让 credential 解密越过 adapter boundary、引入 Provider 特定业务逻辑、billing settlement 或无来源 usage 归一化、冻结最终 HTTP path/error envelope、修改阶段 0 的 `Mock Provider +` 显式 Local test/offline profile 配置，以及拥有或实现 WorkflowRun/NodeRun/RunEvent 状态机与事件历史。通用 Provider Credential 的 AES-256-GCM、Docker Secret 主密钥、真实 Provider 缺主密钥返回 503、`Mock Provider +` 显式 Local test/offline profile 继续可用由 catalog/security owner 负责；本 change 不实现 Provider-specific KMS 或 SDK 内部加密。

## 目标

持久化 catalog 事实与 revision；只选择已启用且已配置的能力；保证参数符合 schema；记录最小调用/usage 审计；保留显式 disabled/unconfigured 失败；并让密钥在 adapter 边界之外始终掩码。

## 决策

- `Provider`、`Profile` 与 `Model` 是独立的版本化 catalog 记录。`Profile` 提供 adapter key、auth-reference metadata、timeout/defaults；`Model` 保存参数 schema 与启用状态。因此增加兼容模型不需要修改业务流程代码，避免将无关生命周期耦合到一条扁平记录。
- `CapabilitySnapshot` 只追加，记录某一时点的 Provider/Profile/Model identity 与已观测 operation/parameter。调用绑定 snapshot id 而非重新读取可变 catalog，保证重试和审计可复现。
- `SkillRevision` 持久化已解析 manifest identity、按来源类型区分的 source identity、content digest、license 状态、allowed tools 和 input/output schemas。Git Skill 的 source identity 为 commit/digest；公开 Markdown Skill 的 source identity 为 archive URL/获取时间/digest/license status。`SkillRegistry` 继续负责解析，`SkillRouter` 继续保持确定性；catalog 持久化不替换它们的选择策略。
- 项目默认值与 workflow node 显式覆盖保存引用及已验证默认参数。解析顺序固定为 workflow node override、project default、enabled system default；缺失、禁用、过期或不兼容选择必须可诊断失败，绝不隐式转向其他真实 Provider。被解析的选择及参数在 Run 启动时冻结为 snapshot。
- credential 密文使用 AES-256-GCM，主密钥只通过 Docker Secret 提供；数据库仅保存 ciphertext、opaque secret reference 与 masked display metadata。仅 adapter 侧 secret resolver 能在显式 opt-in 外部调用时物化密钥；API 响应、ProviderCall 审计和日志均不得含 plaintext。
- credential 解析失败的错误归属固定为 catalog/security：真实 Provider operation 缺少 Docker Secret 主密钥时返回 HTTP 503 `credential_master_key_unavailable`，不得回退或伪造成功；Mock Provider 与显式 Local test/offline profile（adapter identity=`local_workspace`）不依赖主密钥，继续可用并留下可审计的非敏感状态。
- 每个第三方 SkillRevision 必须保存按来源类型固定的 source identity、content/manifest digest、license 状态、allowed tools，以及 network/subprocess/file/secret capability audit evidence；任何未授权访问或脚本执行请求在路由/执行前拒绝并留下拒绝证据。AgentScope 2.x runtime 依赖不作为 SkillRevision 或 Skill vendor 内容管理。
- `ProviderCall` 是 catalog 领域唯一持久化一次调用、费用与幂等账本，采用 `run_id + logical_operation` 幂等键，保存选定 id/snapshot、request fingerprint、intent、terminal outcome、provider request id、native provider usage 和有来源的 cost/unknown-cost 字段。`submission_unknown` 必须保留并先 reconciliation；重放复用已记录 terminal outcome，不重复扣费。审计记录持久化 retention policy/version/hold 状态。
- catalog 提供只读、project/run/node/logical-operation scoped 的 `ProviderCallSummary` projection，返回 call ID/schemaVersion/revision/status、operation、provider/profile/model identity/revision、capability snapshot ref、native usage、cost value/status/currency/source、timing 和脱敏 failure。它不返回 request fingerprint 原文、提示词/SourceMaterial、secret/credential、原始 Provider request/response、媒体 bytes、objectKey/workspace URI 或持久 URL；不创建/镜像 RunEvent。foreign scope、关联不完整、owner timeout 或 schema drift 返回 forbidden/partial/unavailable，不能伪装为无调用或零成本。
- projects owner 拥有版本化项目文本阈值；catalog 只读取并在确认记录中冻结 threshold snapshot ID/revision/hash/value/currency。catalog 拥有 `CostConfirmation` 审计：记录 operation kind/batch size、estimate/actual、currency、source、`cost_status=known|unknown`、request fingerprint、稳定 `user_uuid`、confirmationId 和 `run_id + logical_operation`。workflows/runs 只拥有执行时的 BudgetGate/Run 状态；threshold snapshot、参数、fingerprint、Run 或 logical operation 任一变化都使旧确认不可复用。
- catalog 只提供 SkillRevision 及其 provenance/approval/enabled/capability metadata；运行级 SkillRouteDecision、过滤/排序原因、歧义和人工 selection 属 text/Agent runtime，最终选择由 workflows/runs 冻结。catalog lifecycle mutation 不得自动满足或改写某次 route decision。
- catalog sync 只形成可审计 candidate diff，显式接受后才改变 enabled catalog；它不得直接替换模型、配置或已冻结 snapshot。
- `ProviderOperationPolicy` 按 Provider/Profile/operation 保存 `maxConcurrency`、rate window/limit、bounded queue/admission policy 和 429/`Retry-After` 处理规则；`ProviderQuotaSnapshot` 只追加记录 `known|unknown|exhausted`、provider-native remaining/reset/source/capturedAt。外部调用前必须以当前 policy revision 和冻结 snapshot admission；超限/耗尽不得创建第二 ProviderCall 或 fallback，unknown 必须保持可见并交给显式费用/运行确认规则，而不是伪造剩余额度。
- Model lifecycle 在删除前查询 CapabilitySnapshot、ProviderCall、Run、project default 与 workflow version 的历史引用。存在引用时 `delete` 必须返回 `model_in_use` 并提供 disable action；disable 只影响新解析，不覆盖 Model identity、历史 snapshot 或调用审计。只有 owner proof 明确无引用的从未使用 Model 才可按显式 command 物理删除。
- `RunEvent` 完全归 `workflows/runs` 领域所有。catalog 不创建、镜像或持久化 RunEvent；双方只通过 `run_id`、`node_run_id`、`correlation_id` 关联。
- 数据库与共享 Schema 的 `schema_version` 是 Provider、Profile、Model、CapabilitySnapshot、SkillRevision、项目默认值与 ProviderCall 表示的唯一版本事实；HTTP camelCase DTO 的 `schemaVersion` 只映射同一个 canonical 值。请求缺少必需版本、同时给出冲突的 `schema_version`/`schemaVersion` 或实现双独立赋值时，必须在 UoW 前返回稳定 validation error，不写 catalog、project default、SkillRevision、ProviderCall、usage audit 或 Outbox。

## 风险与取舍

- [实际 Provider 能力与 catalog 不一致]：同步到新 snapshot，在途/已审计工作保留旧 snapshot，并在调用前拒绝不支持参数。
- [credential 暴露]：解密仅限 adapter 边界，所有对外表示掩码，并测试 serialization/logging 路径。
- [usage 单位因 Provider 而异]：保存 Provider 声明的 unit/value/currency 与 source metadata；确认前不归一或虚构账单换算。
- [工作期间禁用]：新选择失败；已绑定 snapshot 的调用依其持久化 retry policy/audit state 处理，绝不静默改路由。
- [分布式并发计数漂移]：admission 使用持久 policy/revision、active operation ledger 与可恢复 lease/expiry，不以进程内计数作为唯一事实。
- [Provider 不返回 quota]：保存 `unknown` 和原始来源，不推导或虚构统一额度；是否继续仍受显式确认与 policy 控制。
- [前端为 ShotCard 直接拼接 ProviderCall 表]：只允许消费脱敏 `ProviderCallSummary` query；scope/revision/关联不匹配时 fail visible，不把 unknown/partial 推断为未调用或零成本。

## 迁移计划

增加 normalized catalog/revision/default/audit 表以及 immutable snapshot/call 约束，采用 additive migration。仅在显式配置时 seed `Mock Provider +` Local test/offline profile-compatible records，不自动导入 credential。诊断至少保留 30 天；长期审计记录 `retention_policy/version/hold`，`CapabilitySnapshot` 与脱敏 `ProviderCall` 摘要不进入诊断过期、temporary/derivative cleanup、容量维护、恢复或 GC 的删除/覆盖/静默压缩候选，本地操作人使用稳定 `user_uuid`。回滚仅在部署策略确认无需保留 catalog 数据后删除未使用的新表/列，且不得以自动回滚或维护流程绕过长期事实保护。

## 待确认

精确 HTTP path/error envelope、native usage 字段到内部报表的映射与各 provider 的 retention duration 仍须按 adapter contract 验证；它们不得改变 AES-256-GCM、Docker Secret、candidate diff 或保留 policy/version/hold 事实。

## DDD / BDD / SDD / TDD

- **DDD**：catalog 拥有 ProviderCall/usage，RunEvent 仍属于 workflows/runs。
- **BDD**：覆盖优先级、禁用/未配置、密钥掩码、candidate diff、费用与重试。
- **SDD**：固定 AES-256-GCM/Docker Secret、snapshot、保留和 canonical schema mapping。
- **TDD**：先写 lifecycle/selection/secret/ledger 负例，再验证 persistence、HTTP 与 adapter。

## Current / Defined / Todo

- **Current**：只有进程内 catalog、六个 Port、Mock Provider、显式 Local test/offline profile 和 SkillRegistry/Router。
- **Defined**：持久化 catalog、冻结选择、通用 credential 加密/503 失败边界、Skill 访问审计、稳定本地 user UUID 和保留策略。
- **Todo**：完成此 change 的未勾选任务和 catalog 定向验证。

Provider/Profile/Model/Skill 的 create/edit/enable/disable 均接受 expectedRevision/If-Match，冲突以 409 零写入。Skill 内容变化只追加只读 `SkillRevision`，状态切换不覆盖已冻结 run snapshot。Credential envelope 固定为 AES-256-GCM algorithm/ciphertext/12-byte nonce/16-byte authTag/keyVersion/aadVersion/profile/credential-bound canonical AAD，并约束 `(keyVersion,nonce)` 唯一。Docker Secret 提供版本化 32-byte keyring；rotation/re-encrypt 以 cursor 恢复、可幂等，旧 key 仅无 envelope 引用才退役。
