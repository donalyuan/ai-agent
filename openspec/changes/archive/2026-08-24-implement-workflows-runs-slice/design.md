## Context

## 固定运行顺序

`drama-mvp-a-default` 仅绑定 approved `novel-writing` 与 `drama-skills`；其余六项 registry candidate 为 `pending_provenance` 或 disabled，不作为 Worker 启动或默认 Run 前置，只有 `allowedSkills`、`requiredCapabilities`、`selectionMode=fixed|inherit` 满足时按需读取。Provider operation 仅同时满足 `adapterInstalled=true`、catalog `approval=approved`、成功 probe 的 capability snapshot、`runnable=true`、`featureGate=MVP-A` 时可解析；首次 connection-test/probe 只需 installed/approved/MVP-A、explicit live opt-in/profile/credential/timeout 并在成功后冻结 snapshot，不需旧 snapshot/`runnable=true`。MVP-B candidate、TTS/ASR、MiniMax H3、Seedance 2.5、Agnes 未选中 mode 零外部调用。

视频阶段只能执行 verified Provider terminal result/storage validation -> immutable candidate + existing AssetVersion -> 人工 `accept` -> scenes exact current CAS -> MediaInspect/derivatives -> Timeline handoff。基础 result 安全验证仅为 download/MIME/checksum/size/duration/dimension/StoredObjectRef，不是 MediaInspect derivative generation；candidate/pending_review 不携带 derivative readiness 作为 accept gate，accept 后 Media Worker 才生成 metadata/proxy/thumbnail/keyframe/waveform。derivative `pending|failed|stale` 仅阻断 Timeline/preview/export，绝不撤销 accepted/current。所有 review Signal、HTTP DTO 与 audit event 的 `decision` 只允许 `accept|reject|retake`；legacy/unknown `approve` 只返回 validation，零 Run/NodeRun/current/retake/Outbox/Provider 副作用，`accept` 只可一次触发 scenes exact CAS。

既有 `WorkflowDraft`/`WorkflowVersion` Schema 已定义 projectId、scope 和不可变内容 hash，SQLAlchemy 只有占位表；Worker 只注册 phase-zero health activity，`workflows=[]`。架构规定外部副作用在提交后发生，Temporal Workflow 只承担确定性分支、等待和重试，Activity 以 `run_id + logical_operation` 幂等。

## 与总体计划的实施追溯

本设计直接落实 `plan-phase-one-drama-mvp-a` 的任务 **2.2**，并执行共享任务 **5.1**（同事务 UoW/审计/Outbox）、**5.2**（幂等、稳定 ID、单调事件和 SSE replay）、**5.3**（版本/归属拒绝）、**5.7**（逐 change OpenSpec/status/strict）及 **5.8**（全量质量门）。直接实施输入是阶段 0 的架构和占位契约；总体 change 不被任何运行时代码导入、读取或调用。后续 catalog、文本、图片和视频 change 只能通过已发布的 WorkflowVersion、Run/NodeRun 标识和端口契约集成，不能取得本领域事件账本的所有权。

## Goals / Non-Goals

**Goals:**

- 冻结固定默认已发布 WorkflowVersion、scope、Run/NodeRun、事件和恢复的领域模型与 API；通用 Draft/图编辑/发布不属于 MVP-A。
- 将 WorkflowRun 与 NodeRun 的状态集合、终态、合法转移、`cancel_requested` 和人工审核 Signal 语义冻结为不需总体计划再作决定的合同。
- 实现持久事件单调序号、SSE replay、稳定 Temporal Workflow ID、AlreadyStarted 复用、取消/Signal 与重启恢复。

**Non-Goals:**

- 不拥有 Provider/Profile/Model 配置或 ProviderCall 账本，不实现真实 Provider SDK/adapter/模型调用、文本 AgentScope 业务、图片/视频/音频生成、FFmpeg/媒体渲染、Timeline 或前端。
- 不把 Temporal 内部表、进程内事件或 SSE 连接当作业务事实源，不让 ProviderCall/Provider adapter 复制 RunEvent 历史，也不把总体协调 change 当作运行时依赖。
- MVP-A 不实现通用 Workflow graph editor、节点/边编辑、连线校验、草稿保存或发布 command/API/UI；默认 Workflow 的 ensure/bootstrap 和 published source snapshot 是后端运行合同，工作台只读消费。既有 `WorkflowDraft` 仅作只读技术兼容或 bootstrap 内部冻结来源。

## Decisions

### 固定默认来源、冻结与 scope

MVP-A 仅允许受控 ensure/bootstrap 创建或校验唯一 `templateKey=drama-mvp-a-default` 的已发布 `WorkflowVersion` 与 project-scoped binding。其 definition、scope、SHA-256 `contentHash`、schema/revision 均冻结；scopeType 只能为 `project`、`episode`、`scene` 或 `shot`，scopeIds 必须非空、去重且全部属于 project，系统不推断“当前项目全部对象”。既有 `WorkflowDraft` Schema/记录仅可只读兼容，或在 bootstrap 内部生成冻结来源；任何通用 Draft create/edit/save、node/edge mutation、连接校验、publish 或版本升级 command/API 统一返回 `unsupported` 且零写入。

### 冻结的 Run/NodeRun 状态机、幂等和取消

`WorkflowRun` 的完整状态集合为 `queued`、`running`、`waiting_review`、`succeeded`、`failed`、`cancel_requested`、`cancelled`；终态只有 `succeeded`、`failed`、`cancelled`。合法转移固定为：`queued -> running|failed|cancel_requested`，`running -> waiting_review|succeeded|failed|cancel_requested`，`waiting_review -> running|failed|cancel_requested`，`cancel_requested -> cancelled`。任何终态没有出边；Run 不能被重启或回退。

`NodeRun` 的完整状态集合为 `pending`、`running`、`waiting_review`、`succeeded`、`failed`、`cancel_requested`、`cancelled`、`skipped`；终态只有 `succeeded`、`failed`、`cancelled`、`skipped`。合法转移固定为：`pending -> running|skipped|cancel_requested`，`running -> waiting_review|succeeded|failed|cancel_requested`，`waiting_review -> running|failed|cancel_requested`，`cancel_requested -> cancelled`。Run 协调器在所有必需 NodeRun 为 `succeeded|skipped` 时将 Run 推进为 `succeeded`；任一必需 NodeRun 为 `failed` 时将 Run 推进为 `failed`，但已进入 `cancel_requested` 的 Run 只能等待 `cancelled`，不得接受新的成功或失败结果。

Run 的非终态聚合规则固定为：只要任一 NodeRun 为 `running`，Run 保持 `running`；当没有 NodeRun 为 `running` 且至少一个必需 NodeRun 为 `waiting_review` 时，Run 在同一事务由 `running` 转为 `waiting_review`。`accept` 将目标 NodeRun 从 `waiting_review` 转为 `running`，并在 Run 当前为 `waiting_review` 时于同一事务将 Run 转回 `running`；若其他 NodeRun 已在运行，Run 原本即保持 `running`。目标 NodeRun 后续成功时，协调器只从 `running` 重算：全部必需 NodeRun 为 `succeeded|skipped` 则 `running -> succeeded`；若没有运行节点但仍有等待节点则 `running -> waiting_review`；否则保持 `running`。`reject` 将目标 NodeRun 转为 `failed`，并按“任一必需 NodeRun 失败”规则在同一事务将 Run 从 `waiting_review|running` 转为 `failed`。因此不需要也不允许 `waiting_review -> succeeded`，且 `waiting_review` 具有可观察、可恢复的唯一聚合语义。

`cancel_requested` 是非终态且先持久化的取消意图：对 `queued`、`running` 或 `waiting_review` Run 的取消 Command 必须在同一事务写 Run 状态、受影响活跃 NodeRun 的 `cancel_requested`、RunEvent、审计和 Outbox；提交后才发送 Temporal Signal。Workflow/Activity 只在确定性安全点把对应 NodeRun 和 Run 变为 `cancelled`，并且在提交业务结果前检查取消意图。非法源/目标状态、终态重启、或并发 expected revision 不匹配分别返回稳定的 `InvalidWorkflowRunTransition`、`InvalidNodeRunTransition` 或 conflict 错误，且不得改写状态、Outbox 或 RunEvent。

### 失败节点继续创建 successor Run

用户所见“从失败节点继续”不是 `failed -> running`，而是 `CreateSuccessorRunFromFailure` command。command 必须绑定 predecessorRunId/revision、失败 nodeRunId/revision、当前 CreativeBrief/source/workflow/selection revisions、显式 reuse evidence 集合和稳定 actor UUID。predecessor 及其 NodeRun/Event/selection snapshot 永久只读；application 在一个 UoW 中创建新 runId、`predecessorRunId`/`resumeFromNodeKey` lineage、新 Run/NodeRuns/RunEvent/Outbox，并冻结当前可用的新 input/Provider/Model/Skill/capability snapshot。

只有 predecessor 中 `succeeded` 且 output owner reference/hash/revision 仍可验证、未 stale、与新 scope/input 兼容的节点可标记为 `reused` evidence；successor 不伪造这些节点再次成功，也不再次执行或收费。失败节点及其所有未成功依赖者在 successor 中使用新的 `logicalOperation` 和幂等键。任一 reuse evidence、scope、revision 或新 selection 不可验证时，整个 command 409/422 且零 successor/Outbox/Temporal/ProviderCall；不得退化为重启 predecessor。`submission_unknown` 仍属于原 Run/原 operation 的 reconciliation，只有确认原 operation terminal failed/not-submitted 后，用户另行明确创建 successor；未知期间不得创建新收费 operation。

### 从历史输入快照创建 rerun Run

“从某版本重新运行”由 `CreateRunFromHistoricalSnapshot` 拥有，输入必须是用户明确选择且仍可解析的 immutable `RunInputSnapshot`，包含 source Run ID/revision、CreativeBrief/SourceMaterial、固定 published WorkflowVersion、scope 和全部 owner reference IDs/revisions/hashes。command 创建带 `rerunOfRunId` lineage 的新 Run；source Run、候选、审核、事件和 current pointers 永久不变。它不采用 `CreateSuccessorRunFromFailure` 的成功节点 reuse evidence，所有执行节点使用新的 `run_id + logical_operation`，并重新进行 capability/runnable、权限、资源与费用确认。历史 Provider/Model/Skill selection 不再 runnable 时必须在提交前阻断，或由用户显式选择并确认新的 selection snapshot；系统不得 fallback、默认升级为 current input 或静默重基历史引用。

### 预算闸门、费用确认与本地身份

每个需要付费的 NodeRun 在进入外部 Activity 前必须有不可变 `BudgetGate` snapshot，包含项目阈值、批量 operation 标识、estimated/actual cost、currency、cost source、`cost_status`（`known|unknown`）、confirmation id、稳定本地 `user_uuid`、`run_id`、`logical_operation` 与 `retention_policy/version/hold`。图片/视频批量生成在提交前必须进入 `pending_confirmation`；文本项目估算超过阈值必须使 Run/NodeRun 进入 `waiting_review`；`cost=unknown` 即使未超阈值也必须明确确认。确认只能批准同一 `run_id + logical_operation`，过期、重复、范围变化或恢复后的不同 operation 必须重新闸门，不能重用确认或重复收费。诊断 payload 至少保留 30 天，长期审计事实由 hold 保护。

审核 Signal 必须包含目标 `nodeRunId`、`correlationId`、当前 node revision、`decision`（仅 `accept|reject|retake`）和审核人标识。它只可作用于同一 Run 中 `waiting_review` 的指定 NodeRun：`accept` 令该 NodeRun 转为 `running` 并按上述规则将等待中的 Run 原子恢复为 `running`；`reject` 令其转为 `failed`；`retake` 仅用于视频 take 并创建 successor logical operation。目标不存在/不属于 Run、correlation/revision 不匹配、非等待状态、重复、legacy `approve` 或未知 decision、或 Run 已 `cancel_requested`/终态，返回稳定 `InvalidReviewSignal` validation/conflict 错误；除无状态错误响应/诊断日志外，不得写 Run、NodeRun、RunEvent、业务审计、幂等记录、current 或 Outbox，也不得触发 Temporal/Provider 副作用。

Run 只引用已发布 Version。starter 由 `project_id + workflow_version_id + run_id` 派生稳定 Temporal Workflow ID，并以 `(run_id, logical_operation)` 去重外部 Activity。启动前按 workflow node override > project default > enabled system default 解析 Provider/Profile/Model/Skill/capability 参数，并将实际 Adapter/Profile identity 与 profile revision 写入 immutable selection snapshot；缺失、禁用或参数不匹配返回稳定 unconfigured/validation，绝不隐式 fallback。默认测试只解析 `Mock Provider +` 显式选择的 Local test/offline profile（adapter identity=`local_workspace`），TOS 失败只能重试/reconcile/报告诊断。结构化文本节点可在同一 Run 内连续生成并逐对象校验候选，不插入逐层人工门；唯一文本审核 NodeRun 等待完整 `TextReviewBatch` 的一次 batch 决定。需要付费媒体的 NodeRun 只有在冻结的 batch snapshot 获得显式批准后才可离开等待边界。相同启动请求返回原 Run/Temporal reference；不同参数使用稳定 conflict。

### 事件、SSE 与 Temporal

Run 的 Skill 选择必须来自 text/Agent runtime 的 immutable `SkillRouteDecision`：唯一 selected，或 `needs_human_selection` 已产生当前 `SkillRouteSelection`。selection snapshot 冻结 route decision/selection IDs/revisions、SkillRevision ID/digest 和 route reason summary；未选择、过期、disabled/unapproved/非候选时必须在 Run/NodeRun/Temporal/TextModel/Provider 前失败，设置页 enabled 状态和候选排序都不能代替本次选择或默认第一项。successor Run 也必须基于新 launch 重新核对/冻结 route selection，不继承可变 decision。

`workflows/runs` 是 `RunEvent` 的唯一领域所有者。RunEvent 持久化 `run_id`、从 1 开始且按 Run 单调递增的 `sequence`、类型、时间、脱敏 payload 与 `correlation_id`；同一 transaction 写 Run/NodeRun、审计、Outbox、事件，并以 `(run_id, sequence)` 唯一约束防止重放或错序。SSE endpoint 以 `Last-Event-ID` 在数据库读取 sequence 后补发，再持续转发；过期/非法 cursor 返回明确错误。

ProviderCall 只保存 `run_id`、`node_run_id`、`correlation_id` 作为关联键及其自身请求/账务审计，MUST NOT 复制同一业务事件的独立事件历史、sequence 或 SSE 来源。Agnes MVP-A 只通过 submit/poll/cancel/result 观察，必须按关联键归一化为本领域追加的 RunEvent；callback/webhook 不属于本阶段。Temporal Workflow 不访问数据库或 Provider；Activity 只执行已提交的 port operation 并记录幂等结果。

### 只读 Run/NodeRun detail projection

workflows/runs owner 提供 secret-free detail query，返回 Run 的 stable ID/revision/schemaVersion/status、WorkflowVersion/scope snapshot reference、created/started/ended timestamps、owner 计算或标明来源的 elapsed、allowed actions、最近持久 RunEvent sequence 与 failure `{code,message,retryable}`；每个 NodeRun 返回 stable ID/revision/node key/status、显式 `scopeRefs`（project/episode/scene/shot stable IDs/revisions）、correlation、started/ended/elapsed、allowlisted `inputSummary`/`outputSummary` 和 failure。`scopeRefs` 必须来自冻结 Run/node scope，不得由前端从顺序或标题推断；summary 只可包含安全标量、数量、类型、状态及 owner ID/revision/hash/reference，不包含提示词全文、SourceMaterial/剧本正文、secret、credential、媒体 bytes、objectKey/workspace URI、持久 URL 或原始 Provider request/response。

detail projection 只读取 workflows/runs 自有事实及其他 owner 提供的安全引用，不复制 ProviderCall ledger、AssetVersion 或候选事实。`allowedActions.cancel` 仅在 Run 为 `queued|running|waiting_review` 且调用方 scope/revision 有效时为 true；`allowedActions.createSuccessor` 仅在 Run=`failed`、失败点与 reuse evidence 可解析且无 `submission_unknown` 时为 true。取消或 successor 提交仍由 command 权威校验。SSE gap、Provider/Activity 晚到或关联 owner 暂不可用只追加 diagnostic/partial marker，不能把 `cancel_requested|cancelled` 改回 succeeded/failed，也不能由 query 触发重试或补偿。

### DB/API/兼容性与测试

增加 run/node_run/run_event/idempotency/outbox 表及固定 workflow version/binding 的 project/scope/hash/唯一约束，保留阶段 0 JSON definition 的只读兼容。数据库与共享 Schema 的 `schema_version` 是 WorkflowDraft、WorkflowVersion、WorkflowRun、NodeRun 及版本化 RunEvent 表示的唯一版本事实；HTTP camelCase DTO 的 `schemaVersion` 只映射同一个 canonical 值。请求缺少必需版本、同时给出冲突的 `schema_version`/`schemaVersion` 或实现双独立赋值时，必须在 UoW 前返回稳定 validation error，不写 Draft、Version、Run、NodeRun、RunEvent、审计或 Outbox。HTTP 同时使用 If-Match/expected revision 和明确 409/422/404 错误，不替换既有错误 envelope。TDD 从状态机、固定 source bootstrap/拒绝通用图 mutation、Repository、starter、Activity、SSE replay、Provider event normalization、版本字段映射和 BDD 负例开始。

## Risks / Trade-offs

- [Temporal AlreadyStarted 或重启重复启动] -> 稳定 Workflow ID、starter 查询与持久幂等记录。
- [失败 Run 被错误复活或复用旧收费 operation] -> 终态无出边；继续只能创建带 lineage 的 successor Run，成功证据逐项校验，待执行节点使用新 logical operation。
- [未知提交状态被盲目重试] -> `submission_unknown` 先按冻结 selection snapshot 和已存 intent/reconciliation 查询；无确定状态不得创建第二个可收费 submit。
- [取消与 Activity 竞态] -> 先持久化 cancel_requested，Activity 在写结果前检查状态和 logical operation；取消后的结果不能把 Run 推回成功或失败。
- [详情页泄露模型输入、凭据或媒体位置] -> detail DTO 采用字段白名单与脱敏 fixtures，只返回 owner IDs/revisions/hashes 和安全摘要；Provider 原始 payload/secret/objectKey 永不进入 projection。
- [事件丢失或错序] -> DB sequence 唯一约束、同事务追加和 Last-Event-ID replay。
- [工作流图语义越界] -> MVP-A 只冻结唯一固定 source、状态和 SSE 规则；节点业务解释由后续 change 定义，通用图 mutation 返回 `unsupported`。
- [将默认 Workflow 误作可编辑草稿] -> 以固定 `templateKey` 的 published source snapshot 作为 MVP-A 唯一来源，页面加载和视图切换零 workflow mutation；编辑/连线/保存/发布 command/API/UI 留到 MVP-B。

## Migration Plan

1. 先加入 contracts 与失败测试，定义固定默认 source、run 状态、事件序号和 scope 验证。
2. 以 additive Alembic revision 创建 run/node/event/idempotency/outbox，并补齐 fixed version/binding 的 hash、project、scope 约束。
3. 部署 domain/application/adapter/HTTP，再注册 starter、Workflow/Activity；默认测试使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）。
4. downgrade 删除新增运行记录路径；不可变发布版本与审计数据不原地改写。验证 SQLite/PostgreSQL upgrade/downgrade cycle。

## 待实现取证

- 节点端口输入/输出的完整类型系统、统一 HTTP 错误 envelope 与状态历史保留策略待后续实现验证；它们不改变 frozen selection snapshot、幂等与审核暂停规则。
- 审核人身份与权限来源可由后续协作 change 扩展，但不得改变本 change 已冻结的 Signal 字段、可接受 decision、状态前置条件或转移。

## DDD / BDD / SDD / TDD

- **DDD**：WorkflowRun、NodeRun 与 RunEvent 是本切片唯一的运行事实。
- **BDD**：覆盖固定 source bootstrap、通用图 mutation 拒绝、状态转移、审核、取消、SSE replay、冻结选择和未知提交恢复。
- **SDD**：固定状态机、Temporal 边界、snapshot、事件所有权和 schema 映射。
- **TDD**：先定义状态/Signal/幂等失败测试，再扩展到持久化、HTTP、Temporal 和 BDD。

## Current / Defined / Todo

- **Current**：只有 Workflow Schema/ORM 占位和 health Activity。
- **Defined**：运行状态机、selection snapshot、审核暂停和事件契约。
- **Todo**：完成此 change 的未勾选任务及 `Mock Provider +` 显式 Local test/offline profile 验证。

## 默认已发布 WorkflowVersion

**DDD**：`ProjectDefaultWorkflowBinding` 属于 project scope，指向 immutable published WorkflowVersion；Run 冻结 `workflowVersionId`、`versionNumber`、`contentHash`、definition/scope 和 `bindingRevision`。更新 binding 不影响历史 Run。**BDD**：空系统首次 ensure 产生唯一固定模板、已发布 Version/binding；重复 ensure 返回同一版本。**SDD**：启动前完整验证 binding/source：缺失为 `workflow_unconfigured` 422，missing/non-published/cross-project/scope/hash mismatch 为 `workflow_version_unavailable` 409，stale/noncurrent binding 为 `workflow_source_conflict` 409；通用 Draft/graph/publish mutation 返回 `unsupported`，上述路径均在 Run/NodeRun/RunEvent/audit/Outbox/Temporal 前零写入。Provider 选择失败仍无 fallback。**TDD**：覆盖 bootstrap 幂等、source snapshot、每个拒绝码、通用 mutation 拒绝和零写入。

固定 definition 至少声明 `text.generate` 输出 `TextReviewBatch`，其批准才使 `media.generate` 可消费 storyboard/reference immutable AssetVersion，`timeline.handoff` 只输出 accepted media/port facts；端口 contract 由 owner payload/schema 定义，UI 仅显示。

`text.generate` regenerate 只能消费通过 schema/count/scope/hash 的 successor closure batch；`media.generate` 只能消费 scenes owner 以 candidate/provenance/AssetVersion id/revision/hash/target 精确 CAS 的 current eligibility。任何 stale/partial/foreign/mismatch 在 Run/NodeRun/RunEvent/audit/Outbox/Temporal、ProviderCall 与 remote submit 前阻断。

默认媒体阶段进一步拆为 `media.generate.image|video`、`media.review.image|video`、`media.inspect` 与 `timeline.handoff` stages。外部兼容的父 logical operation 可为 `media.generate`，但每个 stage 必须有独立 `nodeRunId`、`logicalOperation` 和 owner contract。视频 review 只能为 `accept|reject|retake`，不复用 text batch 的 complete semantics：Provider terminal result candidate 后，`accept` 先校验 exact candidate/source/ShotSpec facts，再只触发一次 scenes current-video exact CAS，随后才允许 MediaInspect/derivatives；`reject` 保留 rejected take；`retake` 创建 successor logical operation。derivative pending/failed/stale 仅阻断 timeline.handoff/preview/export，不阻断或撤销 accepted/current；legacy `approve` 零副作用。

**DDD/BDD/SDD/TDD**：Workflow owns stage orchestration/RunEvent and BudgetGate；Provider owns transport；media worker owns inspection/derivatives；Scenes owns accepted current eligibility；Timeline owns consumption. E2E 必须覆盖 stage split、video take review/retake、unaccepted gate、derivative failure 与恢复不重复收费。
