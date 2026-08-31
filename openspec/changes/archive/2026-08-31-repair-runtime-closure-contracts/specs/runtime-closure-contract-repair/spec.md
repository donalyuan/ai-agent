## ADDED Requirements

### Requirement:跨进程冻结 admission 与 route 身份
系统 SHALL 在 owner 持久化 intent 前冻结并校验 `projectId`、`runId`、适用的 `nodeRunId`、`logicalOperation`、owner revision/hash、catalog/capability/resource admission references、`executionRoute`、`workflowType`、`taskQueue` 与 `schemaVersion`。dispatcher 与 activity MUST 只从该 durable ledger 读取该身份，且在任何外部副作用前拒绝缺失、foreign、stale、drift 或 legacy-new 混用；不得从当前 catalog、环境变量或 activity 内存重新推导，也不得 fallback。

#### Scenario:重启后的 matching frozen operation 被单次派发
- **WHEN** API 已提交完整 frozen intent，进程重启后 Generation 或 Media Worker 接收同一 outbox operation
- **THEN** Worker 使用 ledger 中原始 identity 和稳定 workflow ID 执行或 reconcile，重复 delivery 不创建第二个 intent、ProviderCall、UploadSession、AssetVersion、ExportJob 或外部提交

#### Scenario:不完整或漂移 admission 在派发前拒绝
- **WHEN** payload 缺少任一冻结字段、scope/revision/hash/route/queue/schemaVersion 不匹配，或 legacy route 试图接收新 intent
- **THEN** 系统返回稳定 diagnostic，不启动 activity、不写新的 outbox 或 owner result，且零 provider/storage/render side effect

### Requirement:Media dispatch、activity 与 production reachability
系统 SHALL 仅把 matching owner intent/outbox 派发到已注册的 Generation/Media workflow、activity 与冻结 task queue。generated image/video 的 Media dispatch MUST 先验证 accepted candidate、Scenes exact-current eligibility、AssetVersion/provenance revision/hash；普通 `uploaded_source|asset_center` inspection/proxy 只按其 own Storage/Asset authorization 验证，MUST NOT 被伪装为 generated candidate。未注册 worker/activity、queue 不可达、admission 不完整或生成候选未接受均必须 fail closed。

#### Scenario:已接受 current 生成媒体进入正确 activity
- **WHEN** matching generated candidate 已由 owner accept 并通过 Scenes exact CAS，且 frozen Media route 的 worker/activity/queue 可达
- **THEN** dispatcher 只启动一次 matching Media workflow/activity，activity 只获得 typed immutable references，随后 handoff 仍由对应 owner command 登记

#### Scenario:不满足 dispatch gate 或 production wiring 时零副作用
- **WHEN** candidate 为 pending/rejected/stale/foreign、current/provenance 不匹配，或 activity/worker/queue binding 缺失
- **THEN** 系统返回可诊断 gate 或 `not_ready`，不创建 derivative、Clip、ExportJob、ProviderCall、Storage write 或替代 route

### Requirement:Storage 与 Export 的 typed owner handoff
Storage activity SHALL 只以 frozen operation identity 执行/reconcile 并在校验 MIME、size、checksum、ownership 和适用 capability 后返回 immutable verified `StoredObjectRef`。Assets owner SHALL 是 result AssetVersion 的唯一 append 方；Export owner SHALL 是 ExportArtifact 的唯一 append 方，并且仅能从 matching verified reference、ExportJob 和 packaging subphase 登记。Storage、Worker、adapter 或 direct SQL 路径 MUST NOT 直接创建 AssetVersion/ExportArtifact 或把 bare object key 当作 handoff。

#### Scenario:response-loss 后复用同一 verified reference
- **WHEN** Storage/Export activity 在 remote completion 后丢失响应并由新进程重试
- **THEN** 系统先按同一 frozen operation/revision reconcile，返回同一 StoredObjectRef 或既有 owner artifact，且不重新上传、渲染、登记第二个版本或 artifact

#### Scenario:typed handoff 缺失或不匹配时保持 owner 状态
- **WHEN** handoff 缺少 verified reference、project/operation/revision/hash 不匹配，或 Export packaging subphase 不允许登记
- **THEN** receiver 返回稳定 validation/conflict，保留原 UploadSession/ExportJob diagnostic，且不改变 AssetVersion、ExportArtifact、current、Timeline 或外部操作

### Requirement:显式 local live composition 与 no-fallback
runtime SHALL 只在显式 local live profile 完整提供 approved/runnable frozen capability、credential reference、adapter、Storage/renderer capability 及 worker/activity/queue binding 时组合对应 port。缺失、禁用、未批准、probe/capability 不支持、credential resolver 不可用、renderer 缺失或 worker 不可达时 MUST 保留脱敏原始原因并返回 `unconfigured` 或 `not_ready`。Mock/Local 仅能由明确模式选择；live failure MUST NOT 自动切换 Mock、Local、另一 Provider/Profile 或伪造 terminal success。

#### Scenario:完整 local composition 只构造匹配 frozen port
- **WHEN** 显式 local live profile 与其 frozen capability、adapter、queue 和所有运行前置完整匹配
- **THEN** runtime 构造对应 typed port/activity binding，并将同一 frozen identity 传入 owner ledger，而不读取未选 profile

#### Scenario:未配置 composition 不发生外部调用
- **WHEN** 任一 live profile 前置缺失或不匹配
- **THEN** 系统返回 `unconfigured`/`not_ready` 与脱敏 diagnostic，记录零 external accept，不发送 Provider/TOS/付费请求，也不回退到其他模式

### Requirement:Text success 与 ambiguous submit 的 owner 映射
Text terminal success SHALL 仅由 matching frozen `ProviderCall`、Text owner intent 和 schema-valid response 通过 idempotent owner command 映射为一次 immutable candidate graph/TextReviewBatch handoff。网络响应不确定、remote accepted-response-loss 或无可确认 lookup 的 submit MUST 保持 `ProviderCall.unknown`；reconcile 只能使用 frozen correlation、remote request id 与 frozen lookup capability，不能通过重新 submit 猜测成功。

#### Scenario:Text success 只登记一次完整 batch
- **WHEN** matching ProviderCall 得到可验证 terminal success，且 response 与 frozen scope/schema/count/hash 一致
- **THEN** Text owner 至多登记一个 immutable candidate graph/batch 及对应 handoff，重复 result/restart 返回同一 owner facts，且不产生第二次提交

#### Scenario:Text ambiguous submit 保持 unknown
- **WHEN** submit 结果不确定且 ledger 没有可用 remote identity 或 frozen lookup capability
- **THEN** ProviderCall 保持 `unknown` 并暴露人工处置 diagnostic，不自动 retry/re-submit、不创建 candidate/batch/accepted handoff，也不改变媒体 gate

### Requirement:Image 与 Video 的 frozen retry/reconcile
Image retry/reconcile SHALL 复用 matching frozen `ProviderCall`、correlation 与 capability contract，并在无法确认时保持 `ProviderCall.unknown`。Video retry/reconcile SHALL 复用 matching `VideoOperation`、provider request identity 与 frozen poll/cancel/result contract，并在无法确认时保持 `submission_unknown`。二者 MUST 在 retry 前 reconcile；late terminal result 只能按对应 owner 的 immutable candidate/result rules 登记，不能覆盖 current、Timeline、AssetVersion 或 ExportArtifact。

#### Scenario:可 lookup 的 Image/Video 在重启后 reconcile 同一 operation
- **WHEN** Image 或 Video activity 在已有 remote identity 后重启，且 frozen capability 允许 lookup
- **THEN** activity 先查询并以同一 ledger transition 归一结果，最多产生一个对应 candidate/result handoff，且不产生第二个收费 submit

#### Scenario:无 lookup 的 Image/Video 停在 owner-specific unknown
- **WHEN** retry 前无法从 frozen correlation 确认 remote acceptance，或 capability 明确不支持 lookup
- **THEN** Image 保持 `ProviderCall.unknown`、Video 保持 `VideoOperation.submission_unknown`；系统不伪造成功、不改为另一个状态值、不自动重提或替换 current

### Requirement:B1-B7 失败优先验证与外部证据边界
本 change 的实施 SHALL 先为 B1-B7 编写并运行 focused tests：cross-process/restart frozen admission、Media dispatch/activity/queue reachability、Storage/Export typed handoff、local explicit composition、Text success/ambiguous mapping、Image/Video retry/reconcile。默认运行 MUST 使用 Mock/Local 或无网络 fixture。credentialed E2E、真实 Provider/TOS/付费调用和 MVP-A exit closure MUST 记录为 `not_ready`/`unconfigured`，并包含缺失前置与 `externalAcceptCount=0`；这些记录不得作为成功或关闭旧 `sol_max_closure` 的依据。

#### Scenario:本地测试覆盖 B1-B7 而不访问外部服务
- **WHEN** 执行该 change 的 focused contract、worker、migration 和 Compose/readiness 验证
- **THEN** 输出可运行的通过/失败证据，测试配置不含 live credential，任何外部调用尝试使测试失败

#### Scenario:凭据前置缺失时诚实报告未就绪
- **WHEN** `MVP_A_CREDENTIAL_SANDBOX` 的账号、许可、secret、allowlist 或 renderer 前置未提供
- **THEN** evidence 记录 `result=unconfigured`、`readiness=not_ready` 和 `externalAcceptCount=0`，不启动真实请求、不宣称 live ready、MVP-A exit closed 或旧 lineage 已关闭
