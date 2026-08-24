# workflows-runs Specification

## Purpose
TBD - created by archiving change implement-workflows-runs-slice. Update Purpose after archive.
## Requirements
### Requirement:固定 Skill/Provider resolve 与 video review verb
`drama-mvp-a-default` SHALL 仅绑定 approved `novel-writing`、`drama-skills`；后六项 candidate 只能在 `allowedSkills`、`requiredCapabilities`、`selectionMode=fixed|inherit` 匹配时按需读取，MUST NOT 成为 Worker 启动/默认 Run 前置。operation 仅在 `adapterInstalled=true`、catalog `approval=approved`、成功 probe 的 capability snapshot、`runnable=true`、`featureGate=MVP-A` 时可 resolve；首次 connection-test/probe 仅需 installed/approved/MVP-A、explicit live opt-in/profile/credential/timeout 并在成功后冻结 snapshot，不需既有 snapshot 或 `runnable=true`。MVP-B candidate、TTS/ASR、MiniMax H3、Seedance 2.5、Agnes 未选中 mode MUST 零外部调用。

视频 SHALL 严格执行 verified Provider terminal result/storage validation -> immutable candidate + existing AssetVersion -> 人工 `accept` -> scenes exact current CAS -> MediaInspect/derivatives -> Timeline handoff。基础 result 安全验证仅为 download/MIME/checksum/size/duration/dimension/StoredObjectRef，不是 MediaInspect derivative generation；candidate/pending_review 不携带 derivative readiness 作为 accept gate，accept 后 Media Worker 才生成 metadata/proxy/thumbnail/keyframe/waveform。derivative `pending|failed|stale` 仅阻断 Timeline/preview/export，MUST NOT 撤销 accepted/current。review Signal/HTTP DTO/audit event 的 `decision` 仅允许 `accept|reject|retake`；legacy `approve` 与 unknown MUST validation 且零 current/retake/Run/NodeRun/Outbox/Provider 副作用，`accept` 只可触发一次 scenes exact CAS。

#### Scenario:旧 review verb 不推进媒体 workflow
- **WHEN** workflow 接收 legacy `approve`、未知 action 或未 ready derivative
- **THEN** 系统返回 validation 或保持 handoff 阻断，不变更 current/retake/Run/NodeRun/Outbox/Provider 状态

### Requirement:Phase-one traceability and run-domain ownership
系统 SHALL 将本 capability 追溯到 `plan-phase-one-drama-mvp-a` 的任务 **2.2**，并遵守共享任务 **5.1**、**5.2**、**5.3**、**5.5**。总体协调 change MUST NOT 成为运行时代码依赖；`workflows/runs` MUST 是 WorkflowRun、NodeRun 与 RunEvent 的唯一业务事实所有者。完整非目标是拥有 Provider/Profile/Model 配置或 ProviderCall 账本、真实 Provider SDK/adapter/模型调用、文本 AgentScope 业务、图片/视频/音频生成、FFmpeg/媒体渲染、Timeline 与前端；本 capability MUST NOT 把 Temporal 内部表、进程内事件或 SSE 连接当作业务事实源，也不得复制 RunEvent 历史。

#### Scenario:implement the owned run slice without coordination leakage
- **WHEN** 实施方添加运行领域、Temporal 或 SSE 行为
- **THEN** 实现仅直接依赖阶段 0 架构/Schema 契约和已发布 WorkflowVersion，不读取或导入总体协调 change，也不要求 Scene、catalog、文本或媒体 Provider 业务先存在

#### Scenario:reject duplicate provider event history
- **WHEN** ProviderCall 或 Provider adapter 尝试保存与 RunEvent 相同的 sequence、SSE 来源或独立业务事件账本
- **THEN** 架构/持久化合同测试失败，ProviderCall 仅保留 `run_id`、`node_run_id`、`correlation_id` 关联和自身审计

#### Scenario:reject non-goal ownership leakage
- **WHEN** 本切片尝试承担任一列明的非目标，或将 Temporal/SSE/总体协调文档作为业务事实源
- **THEN** 架构依赖/契约测试失败，且不写 Draft、Version、Run、NodeRun、RunEvent、审计或 Outbox

### Requirement:Canonical workflow schema version mapping
系统 SHALL 以数据库与共享 Schema 的 `schema_version` 作为 WorkflowDraft、WorkflowVersion、WorkflowRun、NodeRun 及版本化 RunEvent 表示的唯一版本事实。HTTP DTO 的 `schemaVersion` MUST 只映射同一个 canonical 值，且实现 MUST NOT 独立持久化或推导第二个版本事实。

#### Scenario:map the canonical workflow version to HTTP
- **WHEN** API 序列化或反序列化有效的 workflow/run DTO
- **THEN** `schemaVersion` 与 canonical `schema_version` 值相同，且持久化层只保存一个版本事实

#### Scenario:reject missing or conflicting workflow version without writes
- **WHEN** 请求缺少必需版本、同时提供冲突的 `schema_version` 与 `schemaVersion`，或实现尝试分别赋值
- **THEN** API 在 UoW 前返回稳定 validation error，且不写 Draft、Version、Run、NodeRun、RunEvent、审计或 Outbox

### Requirement:固定默认工作流来源与 scope
系统 SHALL 仅通过受控 ensure/bootstrap 创建或校验唯一 `templateKey=drama-mvp-a-default` 的已发布 `WorkflowVersion` 和 project-scoped binding。其 `scopeType` 只能为 `project`、`episode`、`scene` 或 `shot`，scopeIds 必须非空、去重且全部归属 project；definition、scope、SHA-256 `contentHash`、schema/revision 均冻结。既有 `WorkflowDraft` 仅可只读兼容或作为 bootstrap 内部冻结来源，不构成 MVP-A 可编辑能力。

#### Scenario:显式生成时幂等确保固定已发布来源
- **WHEN** 用户显式文本生成的 owner command 为 project 调用 ensure，且固定 source 的 scope 与 hash 有效
- **THEN** 系统创建或返回同一已发布 WorkflowVersion/binding，不创建第二 Version，冻结 definition/scope/hash，并写入该受控 bootstrap 所需的审计/Outbox

#### Scenario:拒绝所有通用 Draft 或图 mutation
- **WHEN** 调用方尝试 Draft create/edit/save、node/edge mutation、连接校验、publish 或版本升级 command/API，或固定 source 的 scope/hash 无效
- **THEN** 前者统一返回 `unsupported`，后者返回稳定 validation/conflict；两者均不写 Draft、Version、binding、Run、NodeRun、RunEvent、审计或 Outbox

### Requirement:Versioned run and node state machine
系统 SHALL 只从已发布 WorkflowVersion 启动 `WorkflowRun`，并记录 `NodeRun`。WorkflowRun 状态 MUST 仅为 `queued`、`running`、`waiting_review`、`succeeded`、`failed`、`cancel_requested` 或 `cancelled`；其终态 MUST 仅为 `succeeded`、`failed`、`cancelled`，合法转移 MUST 仅为 `queued -> running|failed|cancel_requested`、`running -> waiting_review|succeeded|failed|cancel_requested`、`waiting_review -> running|failed|cancel_requested`、`cancel_requested -> cancelled`。NodeRun 状态 MUST 仅为 `pending`、`running`、`waiting_review`、`succeeded`、`failed`、`cancel_requested`、`cancelled` 或 `skipped`；其终态 MUST 仅为 `succeeded`、`failed`、`cancelled`、`skipped`，合法转移 MUST 仅为 `pending -> running|skipped|cancel_requested`、`running -> waiting_review|succeeded|failed|cancel_requested`、`waiting_review -> running|failed|cancel_requested`、`cancel_requested -> cancelled`。只要任一 NodeRun 为 `running`，Run MUST 保持 `running`；没有运行节点且至少一个必需节点为 `waiting_review` 时，Run MUST 在同一事务转为 `waiting_review`。`accept` MUST 原子地令目标 NodeRun 和等待中的 Run 都转回 `running`；后续节点成功后只从 `running` 重算为 `succeeded`、`waiting_review` 或保持 `running`，MUST NOT 直接执行 `waiting_review -> succeeded`。`reject` MUST 原子地令目标必需 NodeRun 与 Run 转为 `failed`。

#### Scenario:start and complete a run
- **WHEN** starter 接受一个已发布 Version、显式 scope snapshot 和新的 runId
- **THEN** 系统持久化 queued/running Run、关联 NodeRun 和单调事件，并在所有节点成功后转为 succeeded

#### Scenario:reject forbidden state transition
- **WHEN** 调用方试图从终态重启/回退、越过 `cancel_requested` 直接取消、或在 Run 已取消请求后写成功/失败结果
- **THEN** 系统返回稳定 `InvalidWorkflowRunTransition` 或 `InvalidNodeRunTransition` conflict，且不改变 Run/NodeRun、RunEvent 或 Outbox

#### Scenario:resume an accepted review and complete through running
- **WHEN** Run 因没有运行节点且至少一个必需 NodeRun 等待审核而处于 `waiting_review`，审核人 accept 目标节点，且该节点随后成功
- **THEN** accept 在同一事务将 NodeRun 与 Run 转为 `running`；节点成功后协调器从 `running` 转为 `succeeded`，或在仍有等待节点时转回 `waiting_review`

### Requirement:失败节点继续必须创建 successor Run
系统 SHALL 将“从失败节点继续”实现为 `CreateSuccessorRunFromFailure`，MUST NOT 将 terminal `failed` Run 或 NodeRun 重启、回退或原地覆盖。command MUST 绑定 predecessor Run/node IDs 与 revisions、当前 CreativeBrief/source/workflow/selection revisions、显式 reuse evidence 集合和稳定 actor UUID。成功时 SHALL 创建新的 runId、predecessor lineage、`resumeFromNodeKey`、新 Run/NodeRuns/RunEvent/Outbox 与冻结 selection snapshot；只有前驱 `succeeded`、owner reference/hash/revision 仍有效、non-stale 且 scope/input 兼容的节点可作为只读 reused evidence。失败节点及未成功依赖者 MUST 使用新的 `run_id + logical_operation`，reused 节点不得再次执行或收费。

#### Scenario:从失败节点创建 successor
- **WHEN** 用户明确选择继续，predecessor 为 failed，失败点、当前 inputs/selections 和全部 reuse evidence 均精确有效
- **THEN** 系统保留 predecessor 终态和历史不变，原子创建 queued successor；可复用成功节点只记录 evidence，待执行节点使用新 operation，且没有重复 ProviderCall/收费

#### Scenario:拒绝过期或未知提交上的继续
- **WHEN** predecessor/reuse evidence/scope/revision 已过期，或任一待判定 operation 为 `submission_unknown`
- **THEN** 系统返回 409/422 并要求先 reconciliation，零 successor Run/NodeRun/RunEvent/Outbox/Temporal/ProviderCall，且不重启 predecessor

### Requirement:从明确历史输入快照创建新 Run
系统 SHALL 通过 `CreateRunFromHistoricalSnapshot` 支持用户从一个明确选择的 immutable `RunInputSnapshot` 重新运行。请求 MUST 绑定 source Run ID/revision、CreativeBrief/SourceMaterial、固定 published WorkflowVersion、scope、全部 owner reference IDs/revisions/hashes、目标 selection snapshot 和稳定 actor UUID；成功 MUST 创建新的 runId 与 `rerunOfRunId` lineage。source Run、RunEvent、候选、审核决定和 current pointers MUST 保持只读。历史 rerun MUST NOT 复用 failed-successor evidence；每个执行节点 MUST 使用新的 `run_id + logical_operation` 并重新通过 capability/runnable、权限、资源和 BudgetGate。系统 MUST NOT 默认选择 current、隐式升级输入、静默重基引用或 fallback 到其他 Provider/Model/Skill。

#### Scenario:从选定历史快照重新运行
- **WHEN** 用户查看完整历史快照和新费用影响后明确提交，全部引用仍可解析且目标 selection 当前可运行
- **THEN** 系统原子创建 queued rerun Run、新 NodeRuns/RunEvent/Outbox 和新 logical operations，保留 `rerunOfRunId`，source Run 与历史引用不变且没有隐式 reuse 或重复提交

#### Scenario:拒绝模糊、过期或不可运行的历史重跑
- **WHEN** 请求只写“当前/上一版”、缺少精确 snapshot/revision/hash，引用 foreign/missing，operation 为 `submission_unknown`，或历史 selection 已不可运行且用户未明确选择新配置
- **THEN** 系统返回 409/422 与逐项 diagnostic，零新 Run/NodeRun/RunEvent/Outbox/Temporal/ProviderCall，且不改用 current input 或其他 Provider/Model/Skill

### Requirement:Idempotent Temporal start, cancellation and signal
系统 SHALL 为每个 Run 派生稳定 Temporal Workflow ID，并以 `run_id + logical_operation` 去重 Activity。starter MUST 将 Temporal AlreadyStarted 解析为原 Run 的可重试结果；取消 MUST 先在一个事务中持久化 Run/活跃 NodeRun 的 `cancel_requested`、RunEvent、审计和 Outbox，再发送 Signal。

#### Scenario:repeat start request
- **WHEN** 相同 runId 与相同 Version/scope/输入重复请求启动或 Temporal 返回 AlreadyStarted
- **THEN** 系统返回原 Run 和稳定 Temporal reference，不创建第二个 Run 或重复执行 logical operation

#### Scenario:cancel a running run
- **WHEN** 调用方取消 queued、running 或 waiting_review Run
- **THEN** 系统先写 cancel_requested 状态/事件与 Outbox，再 Signal Workflow，并在确定性安全点写 cancelled；取消后的 Activity 结果不得令 Run 成功或失败

### Requirement:脱敏的 Run/NodeRun 详情与允许动作
系统 SHALL 提供只读 Run/NodeRun detail projection。Run detail MUST 包含 stable ID/revision/schemaVersion/status、WorkflowVersion/scope snapshot reference、created/started/ended timestamps、owner elapsed/source、allowed actions、最近持久 RunEvent sequence 和 failure `code/message/retryable`；NodeRun detail MUST 包含 stable ID/revision/node key/status、来自冻结 node scope 的 project/episode/scene/shot stable IDs/revisions `scopeRefs`、correlation、started/ended/elapsed、allowlisted input/output summary 和 failure。UI MUST NOT 从标题、顺序或相邻事件推断 scope。summary MUST 仅包含安全标量、数量、类型、状态与 owner ID/revision/hash/reference，MUST NOT 返回 secret/credential、提示词或 SourceMaterial/剧本全文、媒体 bytes、objectKey/workspace URI、持久下载 URL或原始 Provider request/response。`allowedActions.cancel` MUST 仅对 `queued|running|waiting_review` 为 true，最终 cancel command 仍以 expectedRevision/correlation 权威校验；detail query MUST 零 mutation。

#### Scenario:读取可诊断的运行详情
- **WHEN** 用户读取同项目 Run 及其 NodeRun detail
- **THEN** API 返回 owner 时间/耗时、脱敏摘要、最近 event、失败与允许动作，且读取不写 RunEvent/Outbox、不启动 Temporal/Provider 或重试 Activity

#### Scenario:拒绝详情泄露或跨项目读取
- **WHEN** 请求属于其他项目，或 summary 来源含 secret、全文、媒体 bytes、objectKey、持久 URL 或原始 Provider payload
- **THEN** API 返回 forbidden/validation 或将不安全字段排除并记录稳定诊断，不泄露内容、不复制 ProviderCall ledger、不产生业务 mutation

#### Scenario:取消后的晚到结果保持 owner 终态
- **WHEN** detail/SSE 在 Run 已 `cancel_requested|cancelled` 后观察到 Activity 或 Provider 晚到成功
- **THEN** projection 保持取消状态并显示关联 diagnostic，不把 allowed action 变为重试/成功、不改写 Run/NodeRun/Event

### Requirement:Frozen provider selection and paid-media review gate
系统 SHALL 在 Run 启动时按 workflow node override > project default > enabled system default 解析 Provider/Profile/Model/capability 参数，并将实际 Adapter/Profile identity 与 profile revision 持久化到 immutable selection snapshot。Skill MUST 来自 text/Agent runtime 当前 immutable `SkillRouteDecision`：只有唯一 selected，或 `needs_human_selection` 已有匹配 expected revisions 的 `SkillRouteSelection` 时才可创建/启动 Run；snapshot MUST 冻结 decision/selection IDs/revisions、SkillRevision ID/digest 和 route reason summary。设置页 enabled 状态、候选顺序或默认第一项不得代替 selection。缺失、禁用、过期、非候选或不兼容选择 MUST 返回可诊断失败，MUST NOT 隐式 fallback；运行开始后不得因 TOS/Local 失败切换 adapter/profile。结构化文本 NodeRun MAY 连续生成并逐对象校验同一 Run 的候选，但 MUST NOT 在 Story/Script/Scene/Shot 层之间要求人工 Signal；Run MUST 只为完整 `TextReviewBatch` 创建一个必需的文本审核边界。付费媒体 NodeRun MUST 等待该 batch 的一次显式 `accept`、handoff 内 candidate/source hashes、payload hash、expected revisions 及 Project/Episode/Scene/Shot/AssetBible 全部 matching idempotent owner ack；`submission_unknown` MUST 先 reconciliation，不得盲目重提。

#### Scenario:selection or review gate blocks chargeable work
- **WHEN** 节点没有有效冻结选择、TextReviewBatch 尚未批准或含 stale/partial 成员，或已提交 intent 处于 `submission_unknown`
- **THEN** Run/NodeRun 保持可诊断等待或失败状态，不产生第二个可收费 Provider request

#### Scenario:Skill 路由未裁决时不启动
- **WHEN** route decision 为 `needs_human_selection` 且无当前 selection，或 selection/candidate revision/digest 已过期
- **THEN** starter 返回 waiting/409 diagnostic，零 Run/NodeRun/Temporal/TextModel/Provider 副作用且不默认选择首项

### Requirement:Constrained human review Signal
系统 SHALL 只接受包含 `nodeRunId`、`correlationId`、当前 node revision、`decision`（`accept|reject|retake`）和审核人标识的人工审核 Signal。Signal MUST 只作用于同一 Run 的 `waiting_review` NodeRun：accept MUST 转为 `running`，reject MUST 转为 `failed`，retake 仅对 video take 创建 successor logical operation；legacy `approve` 或未知 decision MUST validation 且零持久化副作用，随后 Run 协调器按 NodeRun 结果推进 Run。

#### Scenario:accept or reject a waiting node
- **WHEN** 审核人向仍在运行的 Run 的 matching waiting_review NodeRun 提交当前 revision 的 accept 或 reject Signal
- **THEN** 系统分别将 NodeRun 转为 running 或 failed，追加单调 RunEvent，并按全部成功/任一失败规则推进 Run

#### Scenario:reject illegal review signal
- **WHEN** Signal 的 node 不属于 Run、correlation/revision 不匹配、node 不在 waiting_review、decision 未知或重复，或 Run 为 cancel_requested/终态
- **THEN** 系统返回稳定 `InvalidReviewSignal` validation/conflict；除无状态错误响应/诊断日志外，不写 Run、NodeRun、RunEvent、业务审计、幂等记录或 Outbox，也不触发 Temporal/Provider 副作用

### Requirement:Run-scoped cost and budget gate
系统 SHALL 在任何付费 Activity 前持久化不可变 `BudgetGate` snapshot，包含项目文本阈值、批量 operation、estimated/actual cost、currency、cost source、`cost_status`、确认人稳定本地 UUID、confirmation id、`run_id + logical_operation` 和 `retention_policy/version/hold`。图片/视频批量生成 MUST 在提交前确认；文本估算超过项目阈值 MUST 进入 `waiting_review`；`cost=unknown` MUST 明确确认。确认只对相同 run/logical operation 有效，不能通过重试、恢复或参数变化复用。

#### Scenario:pause over-budget or unknown-cost operation
- **WHEN** 付费 NodeRun 是图片/视频批量操作、文本估算超过阈值、成本未知或确认缺失/绑定不匹配
- **THEN** Run/NodeRun 保持 `waiting_review` 或明确拒绝，不创建第二个 ProviderCall，不产生可收费外部请求，并保留原始诊断

#### Scenario:resume an exact confirmed operation
- **WHEN** 用户以稳定本地 UUID 明确确认当前预算闸门，且 confirmation 绑定当前 `run_id + logical_operation`
- **THEN** 系统在同一事务记录确认和 RunEvent，随后只允许对应 Activity 执行；重复确认返回既有结果，不重复收费

### Requirement:Persistent events and SSE replay
系统 SHALL 为每个 Run 持久化由 `workflows/runs` 唯一拥有、从 1 开始单调递增的 RunEvent `sequence`，并提供支持 `Last-Event-ID` 的 SSE 读取。RunEvent MUST 以 `(run_id, sequence)` 唯一约束；ProviderCall 只可用 `run_id`、`node_run_id`、`correlation_id` 关联，MUST NOT 重复保存同一事件历史。Agnes MVP-A 的 submit/poll/cancel/result 业务观察 MUST 归一为 RunEvent，callback/webhook 不属于本阶段。重连 MUST 从下一 sequence 补发，不得依赖进程内内存、Temporal 内部表或 Provider event ledger。全部 RunEvent SHALL 长期保持可读取和 append-only；诊断窗口到期、temporary/derivative cleanup、容量维护、恢复或 GC MUST NOT 删除、覆盖或静默压缩其事件历史。

#### Scenario:replay missed events
- **WHEN** 客户端以某个已持久化 sequence 作为 Last-Event-ID 重连
- **THEN** SSE 先按 sequence 顺序发送其后的所有可见事件，再持续发送新增事件

#### Scenario:invalid event cursor or foreign run
- **WHEN** Last-Event-ID 非法、指向其他 Run，或客户端请求其他项目 Run
- **THEN** API 返回稳定 validation/not-found/forbidden，且不泄露事件 payload

#### Scenario:normalize an Agnes provider event once
- **WHEN** Agnes adapter 收到具有 matching runId/nodeRunId/correlationId 的 submit、poll、cancel 或 result 业务事件
- **THEN** adapter 经 workflows/runs application command 追加唯一的下一个 RunEvent sequence，ProviderCall 不写入同一事件的平行历史

#### Scenario:拒绝清理长期 RunEvent
- **WHEN** cleanup、容量维护、恢复或 GC 尝试清理超过诊断窗口或带任意 hold 状态的 RunEvent
- **THEN** workflows/runs 拒绝或跳过该操作，事件 payload、sequence、revision 与 SSE replay 结果保持不变并留下稳定诊断

### Requirement:Project default published workflow source
系统 SHALL 用唯一 `templateKey=drama-mvp-a-default`（或语义等价唯一 key）维护 revision/schema/contentHash，且以 project-scoped `ProjectDefaultWorkflowBinding` 选择 published WorkflowVersion。首次 ensure MUST 创建或校验 template、published Version 和 binding；重复 ensure MUST 不新增 Version。MVP-A MUST NOT 暴露版本升级或通用 Draft/graph/publish command/API；旧 Draft 仅作只读兼容或 bootstrap 内部冻结来源。Run MUST 冻结 workflowVersionId/versionNumber/contentHash/definition/scope/bindingRevision，binding 之后变化不得影响历史 Run。

#### Scenario:ensure an empty system default once
- **WHEN** 空系统在用户显式发起文本生成时，由 owner create/start command 为 project 调用 ensure
- **THEN** 系统幂等返回固定 published source，definition 明确 `text.generate`、accepted-media `media.generate` 和 accepted-media `timeline.handoff` ports，且不由 UI 推断

#### Scenario:reject unavailable workflow source before writes
- **WHEN** binding 缺失、source missing/non-published/cross-project/scope/hash mismatch 或 binding stale/noncurrent
- **THEN** 分别返回 `workflow_unconfigured` 422、`workflow_version_unavailable` 409 或 `workflow_source_conflict` 409，且 Run/NodeRun/RunEvent/audit/Outbox/Temporal 零写入

### Requirement:MVP-A 工作流只读边界
MVP-A UI SHALL 只消费 default binding 返回的 project-scoped published WorkflowVersion。它 MUST NOT 暴露 node/edge editing、connection authoring/validation、draft save 或 publish command/API；backend bootstrap/ensure 保持为显式 owner command，页面加载和视图切换 MUST 只读。

#### Scenario:查看固定默认工作流
- **WHEN** 用户打开 MVP-A project 的 workflow view
- **THEN** UI 展示 immutable published source、node/run status 与 diagnostic，但不创建或变更 WorkflowDraft/Version/binding

#### Scenario:拒绝来自 MVP-A 的通用工作流 mutation
- **WHEN** MVP-A 浏览器或 API 调用试图 edit node、connect edge、save draft、publish version 或升级 version
- **THEN** capability 标记为 MVP-B/`unsupported`，且不发送或执行 workflow mutation

### Requirement:Immutable 媒体 gate 输入
Media workflow node MUST 只消费精确 accepted storyboard eligibility projection 和 accepted、non-stale 的 TextReviewBatch successor closure。

#### Scenario:Gate rejection precedes run side effects
- **WHEN** text closure 或 image candidate/provenance/revision/hash/target 无效
- **THEN** 不创建 ProviderCall、external submit、RunEvent、Outbox 或 Temporal side effect。

### Requirement:默认媒体 workflow 拆分和视频审核
MVP-A 的 published `drama-mvp-a-default` WorkflowVersion SHALL 将媒体阶段表达为 `media.generate.image|video`、`media.review.image|video`、`media.inspect` 与 `timeline.handoff` 的显式 stages；外部兼容父 logical operation 可为 `media.generate`，但每 stage MUST 有独立 `nodeRunId`、operation snapshot 和 owner contract。`media.review.video` MUST 支持 `accept|reject|retake`：Provider terminal result candidate 后，accept 只触发一次 scenes current-video exact CAS，随后才可 MediaInspect/derivatives；reject 保留 rejected take，retake 创建 successor logical operation；derivative pending/failed/stale 仅阻断 Timeline/preview/export，不撤销 accepted/current。

#### Scenario:split media generation and review without implicit acceptance
- **WHEN** default workflow 接收 verified image/video candidate
- **THEN** generation stage 只产生 immutable candidate + existing AssetVersion/pending_review，review stage 等待显式 decision；accept 先完成 scenes exact current CAS，随后 Media Worker 生成 derivatives，timeline handoff 保持阻断直到 accepted current eligibility 与 ready derivative 均就绪

#### Scenario:retake is a new operation
- **WHEN** 用户使用 frozen successor input 提交 video retake
- **THEN** workflows owner 创建新的 logical operation/NodeRun 并保留 rejected 或 stale predecessor；不复用旧 confirmation/provider submit
