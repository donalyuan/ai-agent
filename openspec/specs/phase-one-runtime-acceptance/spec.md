# phase-one-runtime-acceptance Specification

## Purpose
TBD - created by archiving change close-phase-one-mvp-a-runtime-gaps. Update Purpose after archive.
## Requirements
### Requirement:显式 migration 与 head-aware readiness

标准 Compose/deployment SHALL 提供可审计的显式 Alembic migration 步骤；API、Generation Worker 和 Media Worker readiness MUST 验证数据库 migration head、catalog bootstrap、Temporal queue 和本服务实际 runtime composition，而不只执行 `SELECT 1`。

#### Scenario:新数据库显式迁移后 ready
- **WHEN** PostgreSQL 从空卷执行 `alembic upgrade head` 并完成 catalog bootstrap
- **THEN** API/Worker readiness 报告 schema head 与运行模式，且服务可接收对应业务 operation

#### Scenario:旧 head 或迁移失败
- **WHEN** migration 未执行、head 落后或 migration 失败
- **THEN** readiness 为 not ready，API/Worker 不认领业务写入并保留原始 migration 诊断

### Requirement:标准 Compose 运行时能力配置

Compose SHALL 为 API/Generation/Media 提供显式 `DATABASE_URL`、Temporal、workspace 和 catalog profile；Media SHALL 提供 `FFMPEG_PATH`/`FFPROBE_PATH`，live TOS/Provider 凭据只能通过 Docker Secret/credential reference 注入，示例配置 MUST 不包含真实 secret。

#### Scenario:默认 Compose 无凭据
- **WHEN** 使用 `.env.example` 解析并启动标准 Compose
- **THEN** config 校验通过、Mock/Local 健康可用，标准镜像内的 ffmpeg/ffprobe probe 为 ready；真实 Provider/TOS 明确显示未配置且网络调用为零

#### Scenario:显式 credentialed sandbox
- **WHEN** 使用独立 allowlisted live profile、Docker Secret 和已通过的 provider/TOS/renderer probes
- **THEN** readiness 只报告这些实际 capability，日志/evidence 不含 secret 或完整私密 response

### Requirement:MVP-A credentialed sandbox 闭环

退出验收 SHALL 在显式 credentialed sandbox 中按固定顺序完成创建 Project/Episode、Text generation、TextReview、GPT Image candidate、image review/accept、Scenes image exact-CAS current、Agnes submit/poll/result（并验证 cancel 分支）、Video candidate、video review/accept、Scenes video exact-CAS current、MediaInspect/Derivative、Timeline、MP4/SRT/light export；每阶段 MUST 记录 owner、前置条件、operation identity、review decision、current revision/hash、结果和 artifact/hash 证据。Media activity MUST 按 `uploaded_source|asset_center|generated_candidate` discriminator 校验 admission：普通已验证上传/source/audio AssetVersion 可执行 inspection/通用 derivative，generated candidate 和 Timeline/Render/Export handoff 才要求 accepted current 与匹配 provenance。

#### Scenario:完整链路成功
- **WHEN** 各 provider、TOS、renderer capability 均 ready，Agnes submit/poll/result 完成 terminal result，用户按固定顺序分别接受 image/video candidate，Scenes owner 以 candidate/AssetVersion/provenance 的 exact CAS 写入 current 后再派发 generated-candidate Media
- **THEN** 产生可验证的 text review、image/video AcceptDecision 与 current revision、ready inspection/derivatives、TimelineVersion 和独立 verified MP4/SRT/light artifacts，退出报告为 `ready`

#### Scenario:普通上传素材 inspection 不被候选门阻断
- **WHEN** 同项目普通 uploaded/source/audio AssetVersion 已完成 Storage verify，但没有 Provider candidate、review decision 或 Scenes current
- **THEN** MediaInspection 与通用 proxy/thumbnail/waveform 可以完成并可复验 S08a/S08b；该素材仍不能直接进入 Timeline/Render/Export current handoff

#### Scenario:未接受或不匹配生成候选零派发
- **WHEN** generated image/video candidate 为 pending、rejected、retake、stale 或 foreign，或 review decision、Scenes current、AssetVersion revision/hash、provenance 任一不匹配
- **THEN** Media outbox、MediaInspection、Derivative、Timeline 和 Export mutation 均为零，退出报告记录 admission failure 且不是 `ready`

#### Scenario:任一前置失败
- **WHEN** scope、review、asset readiness、provider/TOS、renderer 或 storage verify 任一步失败
- **THEN** 后续 owner mutation 不被伪造，报告包含稳定失败状态和 no-side-effect 证据，退出报告不是 `ready`

### Requirement:Catalog 与资源准入进入阶段退出门

credentialed sandbox SHALL 分别证明首次 probe gate、完整 live invocation runnable/ProviderOperationPolicy gate，以及 operations-resilience RuntimeResourceSnapshot/CapacitySnapshot pre-intent gate。阶段报告 MUST 记录 catalog/policy/resource snapshot identity、revision/hash、observed/required、admission result 和 no-side-effect；不得以 readiness 或 probe 成功替代每个 command 的准入。

#### Scenario:完整 catalog 与资源准入后执行
- **WHEN** operation 同时满足 installed/approved/probed/runnable/MVP-A/explicit opt-in、Provider concurrency/rate/quota policy，以及未过期且满足 minimum/hard-limit 的 resource/capacity snapshot
- **THEN** command 冻结全部 admission references 后才可创建 intent/ledger/outbox，并由 Worker 在外部副作用前复核同一 operation/snapshot

#### Scenario:任一 admission 失败时零副作用
- **WHEN** operation 为 uninstalled/not-approved/MVP-B/disabled/non-runnable，policy 超限/quota exhausted，resource probe unavailable/capability unsupported，或 capacity 达到 hard limit
- **THEN** 系统不创建 intent、ProviderCall、UploadSession、ExportJob、AssetVersion、Outbox 或外部请求，记录稳定 diagnostic，退出报告不是 `ready`

#### Scenario:重启后复用冻结 admission
- **WHEN** API/Worker 在已接收 operation 后重启，或 admission 状态无法确认
- **THEN** recovery 先读取同一 operation 与冻结 policy/resource/capacity references，继续、blocked 或保持 unknown，不把 snapshot 重置为可用、不生成第二个 intent/submit/outbox

### Requirement:执行路由切换与远端响应丢失验收

阶段验收 SHALL 证明每个新 Text/Image/Video operation 冻结唯一 `executionRoute/workflowType/taskQueue/schemaVersion`，前向切换后只由 Generation route 接收新 intent；Agent/direct Text HTTP legacy route 只能 drain/reconcile 已冻结存量，回滚不得跨 route 接管。Text、GPT Image 与 Agnes 还 MUST 分别验证 pre-submit durable attempt、按 capability 外发 correlation/idempotency、remote request association 及 lookup/no-lookup 终局。

#### Scenario:Agent 与 Generation 并存时只执行冻结路由
- **WHEN** legacy Agent Worker 与 Generation Worker 同时运行，前向切换后提交新 intent，并让一个已冻结 legacy operation 继续完成
- **THEN** 新 intent 只进入 Generation queue，legacy operation 只按原 route drain/reconcile；API 不同步等待模型，duplicate dispatch、Temporal `AlreadyStarted` 和 late completion 最终只产生一个 ProviderCall/TextReviewBatch 或对应 owner fact

#### Scenario:远端已接受但响应丢失
- **WHEN** approved sandbox fault injection 使 Text、GPT Image 或 Agnes 远端接受请求后响应或 Worker 进程丢失
- **THEN** 支持 remote lookup 的 adapter 以预先持久化 correlation 查询并幂等关联原结果；不支持 lookup 时 Text/GPT Image ProviderCall 稳定停在 `unknown`、Agnes VideoOperation 稳定停在 `submission_unknown` 并要求人工处置，且两种路径都禁止自动重新 submit、证明该 logical operation 的外部接受/计费次数至多一次

#### Scenario:回滚不跨 schema 接管
- **WHEN** 新 Generation dispatch 被停用并执行运行时回滚
- **THEN** 各 operation 仅由其冻结 route 完成或 reconcile，legacy Worker 不认领新 schema operation，owner ledger 与诊断保持可查询

### Requirement:API 与 Worker restart recovery

credentialed sandbox E2E SHALL 在至少一个已提交的 Generation/Media operation 中重启 API，并在至少一个未完成或 unknown operation 中重启 Generation 或 Media Worker；恢复后 MUST 继续或 reconcile 原 operation。

#### Scenario:API 重启后恢复查询与提交
- **WHEN** API 在 outbox 和对应 owner ledger 已提交后重启
- **THEN** operation status、owner scope 和 workflow id 可恢复，且不会产生第二个 Provider submit 或 owner fact

#### Scenario:Worker 重启后恢复媒体
- **WHEN** Generation/Media Worker 在 activity 或 multipart/export upload 中重启
- **THEN** 新实例使用稳定 workflow/operation identity、冻结 execution route 与 outbound correlation 继续或 reconcile，最终状态与 verified object/artifact 数量保持幂等

### Requirement:unconfigured 不是退出成功

`unconfigured`、`renderer_unconfigured`、缺凭据、缺许可、缺 binary 或 sandbox 未执行 SHALL 只能表示明确前置缺失；它们 MUST NOT 被计为 MVP-A 业务成功或退出门通过。

#### Scenario:缺凭据的默认验证
- **WHEN** 默认 CI 只能执行 Mock/Local 且 live profile 未配置
- **THEN** 记录稳定 `unconfigured` 与 zero live request 证据，退出报告标记 `not_ready`，同时默认回归仍可通过

#### Scenario:禁止将未配置转成功
- **WHEN** export 或 provider path 返回 `unconfigured`/`renderer_unconfigured`
- **THEN** 不创建 succeeded ExportArtifact/accepted media，阶段报告不能写入成功结果

### Requirement:阶段证据可重验证且无秘密

退出证据 SHALL 使用固定 schema 记录 prerequisite、owner、operation id、observed result、focused failure、no-side-effect、restart/reconcile 和 artifact hashes；证据 MUST 可在 sandbox 重跑且不得包含 secret、token、认证头、完整 Provider response 或媒体二进制。

#### Scenario:重跑得到一致证据结构
- **WHEN** 同一 sandbox 重新执行固定阶段并发生一次明确失败
- **THEN** 报告仍能按 stage/owner/operation 查询对应结果和原始脱敏诊断，且不把临时日志路径当作业务成功

#### Scenario:发现证据泄露
- **WHEN** 证据生成器检测到 secret、token 或未脱敏 response
- **THEN** 证据生成失败并阻断退出门，原文件不得作为验收报告发布

### Requirement:阶段范围与 no-GC 保护

验收 MUST 保留 RunEvent、ProviderCall 脱敏摘要、CapabilitySnapshot、AcceptDecision、仍被引用 AssetVersion 和已登记 ExportArtifact；清理、恢复或 GC 不得删除、覆盖或静默压缩这些诊断事实。

#### Scenario:恢复和清理后仍可追溯
- **WHEN** 完成 Worker restart/reconcile、临时目录清理和 retention/GC 维护
- **THEN** owner 可读取 operation、snapshot、review decision、provider diagnostic 和 artifact provenance，且引用对象仍可验证

#### Scenario:范围外功能被拒绝
- **WHEN** E2E 请求 workflow 图编辑、批量操作、portable/full 回导、TTS/ASR、移动端或多租户行为
- **THEN** 系统以稳定 non-goal/validation 结果拒绝，不把范围外行为计入 MVP-A 证据
