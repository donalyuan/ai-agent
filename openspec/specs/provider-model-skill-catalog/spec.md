# provider-model-skill-catalog Specification

## Purpose
TBD - created by archiving change implement-provider-model-skill-catalog. Update Purpose after archive.
## Requirements
### Requirement:总体计划追溯与领域边界
本 capability SHALL 反向追溯 `plan-phase-one-drama-mvp-a` 任务 `2.3`。总体计划仅协调依赖与集成，不构成运行时代码依赖。本 change 直接依赖阶段 0 的 Provider/Skill foundation，并且 MUST 保持 ProviderCall 属于 catalog 领域、RunEvent 属于 `workflows/runs` 领域；二者只能以 `run_id`、`node_run_id`、`correlation_id` 关联。完整非目标是实现真实 Provider SDK/adapter 或外部调用、让 credential 解密越过 adapter boundary、引入 Provider 特定业务逻辑、billing settlement 或无来源 usage 归一化、冻结最终 HTTP path/error envelope、修改阶段 0 的 `Mock Provider +` 显式 Local test/offline profile 默认值，以及拥有或实现 WorkflowRun/NodeRun/RunEvent 状态机与事件历史；通用 Provider Credential 的 AES-256-GCM、Docker Secret 主密钥、缺主密钥真实 Provider 503 和 `Mock Provider +` 显式 Local test/offline profile 可用性属于本 capability 的 catalog/security owner；Provider-specific KMS/SDK 加密仍为非目标。

#### Scenario:记录运行关联而不复制事件历史
- **当** catalog 为一次逻辑调用记录审计
- **则** 它持久化唯一 ProviderCall 账本及关联 id，但不创建或镜像 RunEvent

#### Scenario:拒绝非目标职责泄漏
- **当** catalog 实现尝试承担任一列明的非目标或把总体计划作为运行时依赖
- **则** 架构依赖/契约测试失败，且不发起外部调用、不改变 catalog、ProviderCall、RunEvent 或运行状态

### Requirement:Catalog Schema 版本单一事实源
系统 SHALL 以数据库与共享 Schema 的 `schema_version` 作为 Provider、Profile、Model、CapabilitySnapshot、SkillRevision、项目默认值与 ProviderCall 表示的唯一版本事实。HTTP DTO 的 `schemaVersion` MUST 只映射同一个 canonical 值，且实现 MUST NOT 独立持久化或推导第二个版本事实。

#### Scenario:向 HTTP 映射 canonical catalog 版本
- **当** API 序列化或反序列化有效 catalog、project-default 或 ProviderCall DTO
- **则** `schemaVersion` 与 canonical `schema_version` 值相同，且持久化层只保存一个版本事实

#### Scenario:版本缺失或冲突时无写入拒绝
- **当** 请求缺少必需版本、同时提供冲突的 `schema_version` 与 `schemaVersion`，或实现尝试分别赋值
- **则** API 在 UoW 前返回稳定 validation error，且不写 catalog、project default、SkillRevision、ProviderCall、usage audit 或 Outbox

### Requirement:持久化 catalog 与生命周期
系统 SHALL 持久化 Provider、Profile、Model、CapabilitySnapshot 和 SkillRevision，且均带稳定 UUID、`schema_version`、revision、enabled state 与 audit timestamps。Provider/Profile/Model 配置在适用时 MUST 包含 adapter key、timeout、default parameters 与 parameter schema；业务 workflow 不得硬编码 model id、base URL、bucket 或 region。

#### Scenario:选择已启用的 catalog model
- **当** 项目为支持的 operation 解析一个已启用且已配置的 model
- **则** application 返回持久化选择与已验证默认值，业务代码中没有 model constant

#### Scenario:拒绝禁用或缺失的 catalog 选择
- **当** 已选 Provider、Profile、Model 或必需 capability 被禁用或不存在
- **则** application 返回稳定、可诊断的 unavailable/unconfigured error，且不隐式 fallback 到真实 Provider

### Requirement:冻结 capability 与参数校验
系统 SHALL 创建只追加的 CapabilitySnapshot，使其绑定 Provider/Profile/Model identity 与已观测 operation/parameter constraints。Port 调用之前，请求 operation 及合并后的 defaults/overrides MUST 针对选定 snapshot 和 model parameter schema 校验。

#### Scenario:校验允许的参数请求
- **当** 请求使用已启用 model，且值被其冻结 capability snapshot 与 parameter schema 接受
- **则** resolved call context 包含 snapshot id 与已验证参数

#### Scenario:拒绝不支持参数或过期 capability
- **当** 请求声明不支持 operation、未知参数、无效值或与其 model revision 不兼容的 snapshot
- **则** 在 Provider invocation 前校验失败，记录 diagnostic reason，且不计 usage

### Requirement:项目默认值与显式覆盖
系统 SHALL 持久化 project-scoped defaults 与 workflow node explicit overrides，内容为 catalog references 加 schema-valid parameters。解析顺序 MUST 是 workflow node override、project default、enabled system default；调用方必须能识别胜出的来源，并在 Run 启动时冻结 resolved selection snapshot。

#### Scenario:解析 workflow node 覆盖
- **当** workflow node 对 operation 有有效且已启用的 override
- **则** 它优先于 project/system defaults，audit 记录 selection source 并将选择冻结到 Run snapshot

#### Scenario:阻止跨项目或无效覆盖
- **当** 项目引用另一项目的 override、禁用目标或超出目标 schema 的参数
- **则** Command 失败，且不持久化 changed default

### Requirement:SkillRevision 保持 registry 边界
系统 SHALL 从已解析本地 manifest metadata 持久化只追加的 SkillRevision，包括 name、version、按来源类型区分的 source identity、content digest、license 状态、stages、capabilities、input/output schemas 与 allowed tools。Git Skill 的 source identity MUST 包含 commit/digest；公开 Markdown Skill 的 source identity MUST 包含 archive URL、获取时间、digest 与 license status。AgentScope 2.x runtime dependency MUST 由 Agent Worker 依赖清单与 lock 单独管理，不得作为 SkillRevision 或 Skill vendor 内容。catalog 操作 MUST NOT 使 disabled、unlicensed 或 incompatible Skill 变为可路由。

#### Scenario:记录已解析的 skill revision
- **当** SkillRegistry 为 catalog synchronization 解析有效且已启用的 local manifest
- **则** 系统保存固定 revision identity，router 可审计它而不跟随可变外部分支

#### Scenario:拒绝无效 skill revision
- **当** manifest 缺少必需 metadata、违反 policy 或已禁用
- **则** synchronization 保持其不可路由并返回 diagnostic reason

### Requirement:密钥掩码与调用/usage 审计
系统 SHALL 以 AES-256-GCM 保存 credential ciphertext，主密钥 MUST 仅来自 Docker Secret；plaintext secret 只能在 adapter boundary 解析，MUST NOT 出现在 API responses、ProviderCall、usage audit、logs 或 errors。ProviderCall MUST 只追加，并绑定 `run_id + logical_operation`、选定 catalog ids/snapshot、request fingerprint、intent、outcome、存在时的 provider request id、native provider usage 与有来源的 usage/cost 或 unknown-cost 字段；审计 MUST 保存 retention policy/version/hold 状态。

#### Scenario:重复逻辑操作
- **当** 记录 terminal ProviderCall 后再次提交相同 `run_id + logical_operation`
- **则** 系统返回记录 outcome 或 explicit in-progress state，且不发起第二次可收费 invocation

#### Scenario:序列化带 credential 的 profile
- **当** API 或 audit response 暴露已配置 Profile
- **则** 它只包含 opaque secret reference policy 与 masked display metadata，绝不含 plaintext credential material

#### Scenario:真实 Provider 缺少主密钥而 Mock 继续可用
- **当** live Provider profile 缺少 Docker Secret 主密钥
- **则** 真实 Provider API 返回 HTTP 503 `credential_master_key_unavailable`，不创建外部请求或成功 ProviderCall；`DeterministicMockProvider` 与显式 Local test/offline profile（adapter identity=`local_workspace`）仍可运行，且不会把密钥错误写入日志

### Requirement:脱敏且按 scope 的 ProviderCall summary projection
Catalog SHALL 提供 project/run/node/logical-operation scoped 的只读 `ProviderCallSummary` query，返回 ProviderCall stable ID/schemaVersion/revision/status、operation、Provider/Profile/Model identity/revision、CapabilitySnapshot reference、native usage、cost value/status/currency/source、timing 和脱敏 failure。响应 MUST NOT 包含提示词或 SourceMaterial 全文、secret/credential、原始 Provider request/response、媒体 bytes、objectKey/workspace URI、持久 URL 或可重放认证信息，也 MUST NOT 创建、镜像或替代 RunEvent。foreign scope MUST fail closed；关联 owner unavailable、timeout 或 schema drift MUST 返回 partial/unavailable diagnostic，MUST NOT 伪报无调用、零 usage 或零成本。

#### Scenario:为 Run detail 或 ShotCard 读取安全调用摘要
- **WHEN** 同项目调用方按 run/node/logical operation 读取 ProviderCall summary
- **THEN** API 返回冻结模型 revision、状态、native usage、cost source 和脱敏 failure，读取零 ProviderCall/RunEvent/外部调用 mutation

#### Scenario:拒绝不安全或不完整的调用摘要
- **WHEN** 请求跨项目，或候选 summary 含原始 payload、全文、secret、媒体位置，或关联事实不可验证
- **THEN** API 返回 forbidden/validation/partial/unavailable，移除不安全内容且不把未知值变为零、不复制事件或触发 reconciliation

### Requirement:Third-party Skill access audit and refusal
系统 SHALL 为每个固定 SkillRevision 保存按来源类型区分的 source identity、content/manifest digest、license 状态、allowed tools 和 network/subprocess/file/secret access audit evidence。运行时 MUST 只允许 manifest 声明且策略批准的访问；未授权网络、子进程、文件或密钥访问以及第三方脚本执行 MUST 在调用前拒绝并记录稳定拒绝原因。

#### Scenario:reject an unauthorized Skill capability
- **当** Skill 请求 manifest 未声明的 network/subprocess/file/secret 能力或尝试执行脚本
- **则** SkillRouter/adapter 返回稳定 refusal，不调用 TextModelPort/Provider，不写候选或 ProviderCall，并保留 redacted audit evidence

### Requirement:Diagnostic retention and stable local identity
系统 SHALL 为 provider/skill 诊断至少保留 30 天；`CapabilitySnapshot` 与脱敏 `ProviderCall` 摘要 SHALL 作为长期审计事实保持可读取和 append-only，诊断窗口到期、temporary/derivative cleanup、容量维护、恢复或 GC MUST NOT 删除、覆盖或静默压缩它们。长期审计 MUST 记录 `retention_policy`、`version` 和 `hold`，且这些字段不得被解释为允许自动清理长期事实；本地操作人 MUST 使用稳定 `user_uuid`，而非每次请求生成或用显示名替代。

#### Scenario:retain diagnostics under a hold
- **当** 诊断或调用审计进入 30 天窗口、被 retention policy/version/hold 标记或等待 reconciliation
- **则** 系统保留可审计记录和稳定 user_uuid；`CapabilitySnapshot` 与脱敏 `ProviderCall` 摘要不删除、不覆盖、不静默压缩

#### Scenario:拒绝清理长期 catalog 事实
- **当** cleanup、容量维护、恢复或 GC 尝试清理已超过诊断窗口的 `CapabilitySnapshot` 或脱敏 `ProviderCall` 摘要
- **则** catalog 拒绝或跳过清理并留下稳定诊断，原记录、关联 id、revision 与 hold 状态保持不变

### Requirement:Candidate model synchronization
系统 SHALL 将 provider model synchronization 产物保存为 candidate diff；只有显式接受 Command 才可创建或更新 enabled catalog record。sync MUST NOT 自动替换已冻结 snapshot 或 project default。

#### Scenario:inspect an unaccepted model diff
- **当** adapter 返回新增、修改或移除模型的同步结果
- **则** 系统保存可审计 candidate diff，既有 model selection 与 Run snapshot 保持不变

### Requirement:Cost policy and exact confirmation audit
projects owner SHALL 持久化项目文本费用阈值；catalog MUST NOT 保存或编辑第二份阈值。catalog SHALL 读取并在 `CostConfirmation` 中冻结 threshold snapshot ID/revision/hash/value/currency，同时保存 operation kind/batch size、estimated/actual cost、currency、source、`cost_status=known|unknown`、request fingerprint、稳定本地 `user_uuid`、confirmationId、`run_id + logical_operation`、revision 与 retention policy/version/hold。图片/视频批量生成 MUST 在提交前确认；文本估算超过项目阈值和任何 `cost=unknown` MUST 要求确认。workflows/runs 拥有 BudgetGate/Run 状态，catalog MUST NOT 复制 Run 状态机。

#### Scenario:record an exact paid-operation confirmation
- **当** 用户确认当前 Run 的图片/视频批量 operation 或超阈值/unknown-cost 文本 operation
- **则** catalog 追加绑定精确 threshold snapshot/run/logical operation/fingerprint/revision/user_uuid 的确认，workflows 可引用 confirmationId 解除对应 BudgetGate

#### Scenario:reject confirmation reuse after operation changes
- **当** Run、logical operation、batch、parameters、fingerprint、cost source 或 revision 与已有确认不一致
- **则** 系统拒绝复用旧确认，不创建第二个收费 ProviderCall，并要求新的明确确认或保持 waiting_review

#### Scenario:拒绝 catalog 复制项目阈值
- **当** catalog command/migration 尝试创建、修改或推导项目文本费用阈值，而不是读取 projects owner snapshot
- **则** architecture/contract test 失败且零 catalog/CostConfirmation/BudgetGate 写入

### Requirement:Skill catalog 与运行路由裁决分离
Catalog SHALL 只拥有 SkillRevision identity/provenance/approval/enabled/capability/schema/tool metadata，并为 SkillRouter 提供只读 candidate metadata。运行级候选集合、过滤/排序原因、歧义状态和人工 `SkillRouteSelection` MUST 由 text/Agent runtime 拥有，最终 selected SkillRevision MUST 由 workflows/runs 冻结。catalog create/edit/enable/disable MUST NOT 自动解决或改写某次 launch/node decision。

#### Scenario:设置生命周期不代替路由选择
- **WHEN** 用户在设置页启用 Skill，且某次 launch 仍处于 `needs_human_selection`
- **THEN** catalog 只更新 lifecycle revision；该 decision 仍等待 runtime selection，零 Run/NodeRun/TextModel/Provider 调用

### Requirement:Catalog CAS and immutable SkillRevision
Provider、Profile、Model 和 Skill command MUST 支持携带 expectedRevision/If-Match 的 create/edit/enable/disable。revision conflict 返回 409 且零写入；Skill content 变更追加 immutable SkillRevision，绝不覆盖历史 snapshot。

#### Scenario:Credential key rotation is recoverable
- **WHEN** rotation 或 re-encryption 从 cursor 恢复，或 legacy replacement 失败
- **THEN** 操作保持 idempotent/recoverable，校验 AES-256-GCM envelope fields，且仅在旧 key 的 envelope reference count 为零后将其 retire。

### Requirement:Per-operation concurrency、rate limit 与 quota snapshot
Catalog SHALL 为每个 Provider/Profile operation 持久化带 revision 的 `ProviderOperationPolicy`，至少包含 maxConcurrency、rate window/limit、bounded admission 和 429/`Retry-After` policy；并 SHALL 只追加 `ProviderQuotaSnapshot`，保存 `known|unknown|exhausted`、provider-native remaining/reset/source/capturedAt。每次 live invocation MUST 在 ProviderCall/external submit 前 admission；超并发、超速率或 quota exhausted MUST 返回稳定 retryable/blocked diagnostic，quota unknown MUST 保持 unknown 且不得伪报可用或跨 Provider 归一。

#### Scenario:operation 超限时不外部提交
- **WHEN** 同一 Provider/Profile operation 达到 maxConcurrency/rate limit，或最新可信 quota snapshot 为 exhausted
- **THEN** catalog 返回包含 policy revision、operation 和可用 retry time 的稳定 diagnostic，不创建新的 ProviderCall/external submit、不重复收费且不 fallback 到其他模型

#### Scenario:429 和未知 quota 保持可恢复事实
- **WHEN** Provider 返回 429/`Retry-After`，或无法取得 quota
- **THEN** 系统保存原生 observation/source，按有界 policy 恢复或保持 `quota_unknown`，重启后不重置为可用、不盲目重提

### Requirement:被历史引用的 Model 只能停用
Model delete command SHALL 在 mutation 前查询 CapabilitySnapshot、ProviderCall、Run、project default 与 WorkflowVersion 历史引用。存在任一引用或无法证明无引用时 MUST 拒绝物理删除，并只允许 disable；disable MUST 只影响新 selection，MUST NOT 改写历史 Model identity、snapshot 或 audit。

#### Scenario:拒绝删除已被引用的模型
- **WHEN** 用户删除存在历史引用或 reference proof unavailable 的 Model
- **THEN** owner 返回 `model_in_use`/`reference_proof_unavailable` 和 disable action，Model 及全部历史引用保持可读，零默认值或 snapshot 覆盖

### Requirement:八项 Skill Registry 与 runnable feature gate
系统 SHALL 为 `drama-skills`、`novel-writing`、`zy-cinematic-realism`、`seedance-2.0`、`storyboard-tiktok-video-skill`、`hell-grind/cinedance-higgsfield`、`hell-grind/acting`、`hell-grind/lira` 保存 provenance、approval、enabled。`drama-skills` 与 `novel-writing` SHALL 为 `provenance=verified_snapshot`、`approval=approved`、`enabled=true`；其他六项 SHALL 为 `provenance=pending_provenance`、`approval=not_approved`、`enabled=false`。`drama-mvp-a-default` SHALL 只绑定前两项 approved revision；后六项 MUST NOT 成为 Worker 启动或默认 Run 前置，只有 node `allowedSkills`、`requiredCapabilities` 与 `selectionMode=fixed|inherit` 都满足时才可按需读取。

每个 Provider/Profile/Model operation SHALL 保存 `adapterInstalled`、catalog `approval`、`operationCapabilitySnapshot`、`runnable`、`featureGate`。首次显式 connection-test/probe 必须具备已安装 adapter、`approval=approved`、`featureGate=MVP-A`、用户 explicit live opt-in、已选 profile、可解析 credential 和显式 timeout，但 MUST NOT 要求既有 snapshot 或 `runnable=true`，也 MUST NOT 因 disabled-for-run 被拒绝；成功时 SHALL 冻结 successfully probed `operationCapabilitySnapshot`。snapshot 缺失、`runnable=false` 与 disabled-for-run 仅阻断 enable/default、Run resolve 和 live operation invocation；后者仅可用于同时满足 installed、approved、successfully probed snapshot、`runnable=true`、`featureGate=MVP-A` 的 operation。MVP-B candidate、uninstalled、not-approved 或缺 explicit live opt-in/profile/credential/timeout 的 operation MUST 零 probe/外部调用；TTS/ASR、MiniMax H3、Seedance 2.5 和 Agnes 未选中 mode SHALL `runnable=false`。默认测试组合 MUST 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），并保持 explicit live opt-in；运行开始后 Adapter/Profile MUST 冻结。

#### Scenario:首次 probe 不由完整 runnable gate 拒绝
- **WHEN** 用户对已安装、approved、`featureGate=MVP-A` 的 candidate 以 explicit live opt-in、已选 profile、可解析 credential 和 timeout 发起首次 connection-test/probe，且尚无 snapshot 或 `runnable=false`
- **THEN** 系统仅执行该显式 probe；成功后冻结 successfully probed snapshot，不启用/default/Run resolve，失败保留原始诊断且不产生任何其他外部调用

#### Scenario:拒绝不具备首次 probe 前置或完整 runnable action 的 operation
- **WHEN** Worker 或设置命令对 uninstalled/not-approved/MVP-B candidate、或缺 explicit live opt-in/profile/credential/timeout 的 operation 请求首次 probe，或对 snapshot-missing/`runnable=false`/disabled-for-run 的 operation 执行 enable/default/Run resolve/live invocation
- **THEN** 系统保留 catalog 状态并返回 validation/unconfigured；前一类不发起 connection-test 或外部调用，后一类不 resolve 或 invoke，且均不读取非允许 Skill
