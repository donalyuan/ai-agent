# phase-one-drama-mvp-a-integration-plan Specification

## Purpose
TBD - created by archiving change plan-phase-one-drama-mvp-a. Update Purpose after archive.
## Requirements
### Requirement:跨 owner 运行合同
系统 SHALL 登记八项 `SkillRegistry` candidate：`drama-skills`、`novel-writing`、`zy-cinematic-realism`、`seedance-2.0`、`storyboard-tiktok-video-skill`、`hell-grind/cinedance-higgsfield`、`hell-grind/acting`、`hell-grind/lira`。前两项 SHALL 为 `provenance=verified_snapshot`、`approval=approved`、`enabled=true`，`drama-mvp-a-default` SHALL 只绑定其 approved revisions；其余六项 SHALL 为 `provenance=pending_provenance`、`approval=not_approved`、`enabled=false`，不得成为 Worker 启动或默认 Run 前置，除非 node `allowedSkills`、`requiredCapabilities` 与 `selectionMode=fixed|inherit` 均通过。Git Skill 的 provenance SHALL 使用 commit/digest，公开 Markdown Skill 的 provenance SHALL 使用 archive URL、获取时间、digest 与 license status；AgentScope 2.x SHALL 作为 Agent Worker 独立 runtime dependency 由依赖清单与 lock 管理。Worker 启动 SHALL 只读取 Registry index/approved metadata，路由后才按需读取选中 Skill 的 `SKILL.md` 和 references。Provider/Model/Profile 的首次 connection-test/probe SHALL 仅要求 `adapterInstalled=true`、catalog `approval=approved`、`featureGate=MVP-A`、explicit live opt-in、已选 profile、可解析 credential 与 timeout，成功后 SHALL 冻结 capability snapshot，MUST NOT 以前次 snapshot、`runnable=true` 或 disabled-for-run 为前置；snapshot-missing/`runnable=false`/disabled-for-run 只阻断 enable/default/Run resolve/live invocation，后者 SHALL 额外要求成功 snapshot 与 `runnable=true`。MVP-B/uninstalled/not-approved 或缺 opt-in/profile/credential/timeout MUST 零 probe/外部调用，TTS/ASR、MiniMax H3、Seedance 2.5、Agnes 未选中 mode 不可运行。

同一 Run、同项目、Schema-valid、hash/revision/scope 匹配的 provisional upstream candidate SHALL 可构造完整 Text candidate graph；只有既非 accepted owner fact 又不满足该 provisional 条件的输入才拒绝。完整图只能产生一次 immutable `TextReviewBatch`，并用 successor/stale closure 与全有或全无 CAS 收口。视频 SHALL 按 verified Provider terminal result/storage validation -> immutable candidate + existing AssetVersion -> 人工 `accept` -> scenes exact current CAS -> MediaInspect/derivatives -> Timeline handoff 运行；基础安全验证仅含 download/MIME/checksum/size/duration/dimension/StoredObjectRef，不是 derivative generation，candidate/pending_review 不携带 derivative readiness 作为 accept gate。accept 后 Media Worker 才生成 metadata/proxy/thumbnail/keyframe/waveform；derivative `pending|failed|stale` 仅阻断 Timeline/preview/export，不撤销 accepted/current。review Schema、Signal、HTTP DTO、UI action、audit event 只允许 `accept|reject|retake`；`approve`/unknown 均 validation 且零 current/retake side effect。verified Provider terminal success 是 result `AssetVersion` 唯一 append 时点；retry/reconcile 返回同一 version/candidate，AssetEdit accept 仅追加 AcceptDecision/audit 与同一 version 的 scenes current eligibility CAS，reject/stale/foreign accept 不改变 AssetVersion 数、current 或 Timeline。

MVP-A SHALL 只消费固定、版本化、已发布的 `drama-mvp-a-default` WorkflowVersion，MUST NOT 产生任何 Draft identity、可编辑 Draft lifecycle 或 graph mutation。

#### Scenario:只让完整 gated candidate 进入后续 owner
- **WHEN** Worker 解析 Registry/Provider、构造文本图、审核视频或接受 AssetEdit
- **THEN** 系统按本 Requirement 的 exact gate、顺序与零副作用规则执行，任何 pending/unknown/legacy 输入均不绕过 owner contract

#### Scenario:Worker 启动与 Skill 渐进加载分离
- **WHEN** Worker 启动，或路由器为节点选中一个 approved Git/公开 Markdown Skill revision
- **THEN** 启动阶段只读取 AgentScope runtime lock、Registry index 和 approved metadata；路由前不读取 `SKILL.md`/references，路由后仅读取选中固定 revision 的正文与必要 references，并记录 source identity

### Requirement:阶段 1 MVP-A 总体集成边界
系统 SHALL 将阶段 1 剧集创作 MVP-A 组织为一个总体协调 change 与十八个职责单一的 child OpenSpec change，并在总体 change 中记录每个 child 的总体任务号、唯一职责、前置依赖、并行边界与非目标。除既有 child 外，系统 MUST 包含 `extend-projects-episodes-creative-slice`、`implement-asset-bible-continuity-slice`、`integrate-tos-storage-provider`、五个前端/业务闭环（其中一个为独立 `implement-project-asset-center`）、`implement-provider-model-skill-settings-ui`、独立 `implement-operations-resilience` 与独立 `implement-local-observability`。总体 change MUST 将阶段 0、`projects/episodes`、`assets/asset-versions` 和 objectKey repair 识别为已完成且不可重复实现的范围。总体协调 change MUST NOT 被描述为 child 实施运行时必须先应用的代码依赖。

#### Scenario:后续切片以总体契约开始
- **WHEN** 维护者开始任一阶段 1 后续 change
- **THEN** 其 proposal、design、specs 和 tasks 能追溯到总体 change 中的唯一职责与 DAG 前置，并且不把阶段 0 已完成能力列为新实现

#### Scenario:总体协调不阻塞 child 实施运行时
- **WHEN** 维护者按已声明的 child 依赖开始实施一个 child
- **THEN** 该实施不要求先应用、完成或归档总体 change 的 tasks；总体 artifacts 只提供可追溯的协调约束

### Requirement:共享版本、幂等与副作用规则
系统 SHALL 要求阶段 1 的写入使用稳定 UUID、`schema_version` 和 revision；发布、运行和导出 MUST 引用不可变版本。`schema_version` MUST 是持久化与 manifest 的 canonical 字段；HTTP DTO 使用 `schemaVersion` 时 MUST 只映射该同一值，不得形成双事实源。每个写 Command MUST 在一个 UoW 中提交领域写入、审计和 Outbox；Provider、Temporal、FFmpeg 等外部副作用 MUST 在提交后执行。可重试操作 MUST 使用 `run_id + logical_operation` 幂等键，持久事件 MUST 使用单调序号，SSE MUST 支持 `Last-Event-ID` 补发。

#### Scenario:重试逻辑操作不产生重复副作用
- **WHEN** 同一 `run_id` 和 `logical_operation` 的已提交外部操作被重试
- **THEN** 系统复用已记录的操作结果或返回可诊断状态，且不重复提交 Provider 请求、重复扣费或覆盖原素材

### Requirement:ProjectPackage profile 与 MVP-A 范围
系统 SHALL 为工程包使用具有 `schema_version`、version 与 `exportProfile` 的公共 `ProjectPackage` manifest。`exportProfile` MUST 仅为 `light` 或 `portable`，且系统 MUST NOT 并存 `profile` 与 `exportProfile` 两套事实。公共 manifest MUST 将选定 Episode/TimelineVersion、AssetVersion 引用、Model/Profile/CapabilitySnapshot、SkillRevision、实际参数、usage/cost value/status/source、音频结构、每项素材/音频授权和响度测量报告全部设为必填；`cost=unknown` 也必须包含来源。MVP-A MUST 只实现 `exportProfile=light`，且该 profile MUST 只包含 manifest 与可解析引用；`exportProfile=portable` MUST 仅由 MVP-B 在相同公共 manifest 上增加媒体载荷，且不得改变公共引用或审计字段。

#### Scenario:MVP-A 导出轻量工程包
- **WHEN** MVP-A 的每集时间线通过导出预检并请求工程包
- **THEN** 系统产出 `schema_version` 与 `exportProfile=light` 的 manifest 和可解析引用，不内嵌全部素材或生成结果，并保留所选版本与音频审计信息

#### Scenario:缺少 light manifest 必填审计字段
- **WHEN** authorization、loudness、Model/Skill/parameters/cost source 任一缺失或空值，或 unknown cost 无来源
- **THEN** Schema 与导出预检失败，不生成成功工程包，并报告精确缺失字段

### Requirement:独立迁移和测试边界
系统 SHALL 要求每个后续 change 只拥有其聚合的数据表、约束和 Alembic revision，并在创建 revision 前重新核验实际 head。并行 change MUST NOT 假设线性 migration 顺序或修改彼此的 revision；需要合并分支时 MUST 以独立、已测试的 merge revision 明确处理。每个 change MUST 以 domain、application、adapter、integration、contract 与 BDD 测试覆盖其正反场景，并保持 `Mock Provider +` 显式选择的 `Local test/offline profile`（adapter identity=`local_workspace`）为默认测试组合；Local 不是 TOS 失败 fallback，运行开始时冻结 Adapter/Profile。

#### Scenario:并行切片出现多个 Alembic head
- **WHEN** 两个独立阶段 1 change 已生成不同的 Alembic 分支
- **THEN** 集成工作显式核验 revision graph，并通过单独的 merge revision 与升级/降级测试解决分支，而不重写任一已共享 revision

### Requirement:未决接口和真实外部能力必须保持可见
系统 SHALL 将除已冻结 StorageProfile owner API/DTO/error matrix 外的实际 HTTP path/error envelope 兼容、Provider/TOS/FFmpeg explicit probe 输入、native usage 字段和 retention profile 记录为待验证；MVP-A 不包含 Agnes callback/webhook。CreativeBrief/StoryScriptSceneShot 最小字段、ExportJob 状态、Timeline 编码、credential AES-256-GCM/Docker Secret、缺少主密钥时真实 Provider 503、Skill source-type provenance/访问审计规则、费用确认与预算闸门、profile 优先级、退出矩阵与项目包 light 引用语义已冻结，不得重新列为待确认。`implement-workflows-runs-slice` 已冻结 Run 状态机：状态仅为 `queued`、`running`、`waiting_review`、`succeeded`、`failed`、`cancel_requested` 或 `cancelled`，终态仅为 `succeeded`、`failed` 与 `cancelled`；总体 change MUST NOT 将它重新列为待确认或增加转移。真实 Provider、AgentScope 和 FFmpeg MUST 仅在显式 opt-in probe 中调用，并对未配置、不可用或未验证状态保留原始错误。

#### Scenario:未配置真实 Provider 的默认测试
- **WHEN** 阶段 1 测试在没有真实 Provider 凭据的环境运行
- **THEN** 测试使用 `Mock Provider +` 显式选择的 `Local test/offline profile` 完成可验证行为，并将真实 Provider 未配置状态显式报告而不静默回退为成功；profile 选择和 adapter identity 在运行开始后保持不变

### Requirement:费用确认、预算闸门与审计保留
系统 SHALL 将费用策略、估算/实际成本、币种、来源、`cost=unknown`、确认人稳定本地 UUID、`run_id + logical_operation`、`retention_policy/version/hold` 作为可审计事实。图片/视频批量生成前 MUST 经过一次明确确认；文本项目超过配置阈值 MUST 进入 `waiting_review`；`cost=unknown` MUST 在任何阈值下再次明确确认；确认或拒绝不得脱离原 logical operation 重放。

#### Scenario:阻止未经确认的付费批量操作
- **WHEN** 图片/视频批量操作缺少确认、文本估算超过项目阈值、成本未知或确认绑定的 run/logical operation 不匹配
- **THEN** 系统保持预算闸门等待/拒绝状态，不创建第二个 ProviderCall、不扣费且保留原始诊断

### Requirement:通用凭据、Skill 访问审计与诊断保留
系统 SHALL 由 catalog/security owner 实现通用 Provider Credential 的 AES-256-GCM，Docker Secret 仅提供主密钥；真实 Provider 缺少主密钥 MUST 返回 503，`Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）MUST 继续可用。第三方 Skill 的 source-type provenance 以及 network、subprocess、file、secret 访问 MUST 有审计证据且未授权访问拒绝。诊断至少保留 30 天，长期审计受 `retention_policy/version/hold` 保护，本地用户使用稳定 UUID。

#### Scenario:缺少主密钥不影响 Mock
- **WHEN** live Provider profile 缺少 Docker Secret 主密钥，或第三方 Skill 请求未授权的 network/subprocess/file/secret 能力
- **THEN** live operation 返回 503 或稳定拒绝且无外部副作用；Mock Provider 与显式 Local test/offline profile 仍可运行，访问审计被保留

### Requirement:跨 owner retention/no-GC
系统 SHALL 将 `RunEvent`、`AcceptDecision`、`CapabilitySnapshot`、脱敏 `ProviderCall` 摘要和仍被引用的 `AssetVersion` 标记为长期保留事实。诊断至少 30 天的窗口、Worker temporary/derivative cleanup、capacity probe、恢复维护和 GC MUST NOT 删除、覆盖或静默压缩这些事实；只有明确无引用且符合 retention policy 的临时对象可清理。`retention_policy/version/hold`、稳定 user UUID、source/revision/hash 和引用关系 MUST 可用于验收追溯。

#### Scenario:长期事实不被自动清理
- **WHEN** 构造超过 30 天、不同 hold 状态、跨 owner 引用以及 Worker/API restart/reconcile 的记录，并运行所有 temporary/derivative cleanup、capacity maintenance 和 GC 路径
- **THEN** `RunEvent`、`AcceptDecision`、`CapabilitySnapshot`、脱敏 `ProviderCall` 摘要和被引用 `AssetVersion` 保持可读取、append-only 且不被删除、覆盖或静默压缩；cleanup 仅处理明确无引用的临时对象

#### Scenario:GC 误删尝试保持零副作用
- **WHEN** cleanup/GC 试图删除仍被引用的 AssetVersion 或任一长期事实
- **THEN** 系统拒绝或跳过该删除并留下稳定诊断，引用、审计、RunEvent sequence 和 owner revision 均不变

### Requirement:冻结的创作、候选和 Provider 决策
`projects` owner SHALL 持有 Project、`creationMode`、CreativeBrief、项目设置、文本费用阈值和 StorySpec current reference，Episode owner SHALL 持有每集 ScriptSpec current reference；text owner MUST 只读消费已校验 CreativeBrief snapshot，并只拥有 adaptation SourceMaterial、文本候选和 `TextReviewBatch`，不得写 projects owner state。CreativeBrief SHALL 精确包含六项创作语义 `subject`、`genre`、`audience`、`characterPremise`、`style`、每集目标 `episodeDurationSeconds`，三个精确计数 `episodeCount`、`scenesPerEpisode`、`shotsPerScene`，以及 canonical `schema_version` 和 revision。StorySpec MUST 为项目级，ScriptSpec MUST 为单集级，Scene/Shot MUST 保持稳定 ID，ShotSpec MUST 版本化。独立 AssetBible owner SHALL 持有 typed entry/version、project -> episode -> scene -> shot override、resolved snapshot、impact analysis 和 ContinuityRevisionTask；consumer 只能冻结 snapshot ID/revision/hash 与 owner refs。MVP-A 的文本产物 MUST 以这些结构化规格和叙事候选实际引用的初始 AssetBible entry specs 为终点，MUST NOT 生成小说正文或章节草稿。一次文本运行 MUST 先逐对象完成 Schema 校验并生成完整候选图，再冻结一个 `TextReviewBatch` 并只执行一次必需的批量人工审核；不得在 Story/Script/Scene/Shot 层之间插入必需人工门。批次内上游修改 MUST 将依赖候选标记 stale，且完整集合重新校验前 MUST NOT 接受或启动付费媒体。文本 owner 的批次接受 MUST 携带精确 candidate/reference 集合、candidate/source hashes、payload hash、实际引用的 AssetBible specs、correlation 与 expected revisions，并以 CAS 在一个 UoW 中全有或全无地追加 accepted handoff、candidate/TextReview 状态、审计与 Outbox；Project/Episode/Scene/Shot/AssetBible 只能由各自 owner 的 typed command/CAS/idempotent ack 落地。任一 owner ack 缺失/失败时付费媒体继续阻断；旧版本、发布版本和历史运行 MUST 只读。Provider 优先级 MUST 为 workflow node > project default > enabled system default，Run MUST 冻结 selection snapshot，缺失选择 MUST NOT 隐式 fallback。

#### Scenario:未经审核或精确 CAS 的候选不能驱动付费媒体
- **WHEN** `TextReviewBatch` 仍待审核、包含 stale/缺失/未通过 Schema 的成员、接受集合缺少 candidate hash 或目标 revision，或 Provider 选择缺失
- **THEN** 系统不生成媒体、不改写任何引用，并返回可诊断的 review/conflict/unconfigured 状态

### Requirement:AssetBible owner、影响确认与 consumer gate
MVP-A SHALL 由 `implement-asset-bible-continuity-slice` 唯一拥有 Character、Look、Location、SceneVisual、Prop、VisualStyle 的稳定 entry/不可变 version、project -> episode -> scene -> shot assignment/override、accepted `ResolvedContinuitySnapshot`、精确 `ContinuityImpactAnalysis` 和 `ContinuityRevisionTask`。修改 current entry 前 MUST 预览完整实际 Episode/Scene/Shot target set/revisions/reasons/hash；只有用户以 expected revisions 显式全有或全无接受后才可创建 successor/current pointer/AcceptDecision/tasks。Scene/Shot、Run、GPT Image 和 Agent MUST 只读冻结 accepted snapshot ID/revision/hash 与必要 owner refs，不得复制/写入 entry/override 或自行解析 chain。snapshot incomplete/stale/foreign/hash mismatch 或 pending task MUST 在 ProviderCall、Agent execution 或 Timeline handoff 前阻断，且旧 ShotSpec/current media/Timeline 不自动变化。

#### Scenario:显式接受 AssetBible successor
- **WHEN** impact analysis 完整，实际 target set/hash 和所有 expected revisions 匹配，用户确认全部范围
- **THEN** AssetBible owner 原子追加 successor、移动 current、记录 AcceptDecision 并为精确目标创建任务；旧版本与下游引用保持可读

#### Scenario:连续性冲突阻断 consumer
- **WHEN** analysis incomplete、set/hash/revision 过期、snapshot foreign/stale，或 target 有 pending `ContinuityRevisionTask`
- **THEN** accept 或 consumer operation 返回稳定 diagnostic，且零 successor、ProviderCall、Agent execution、ShotSpec/current media/Timeline 自动 mutation

### Requirement:MVP-A 媒体、导出和受控外部副作用
系统 SHALL 对付费 operation 保存幂等 intent；`submission_unknown` MUST 先 reconciliation，取消后的晚到结果 MUST 成为未引用候选。GPT Image 输入 MUST 仅为 PNG/JPEG/WebP、mask 仅 PNG，URL MUST 禁止 redirect 与 loopback/private/link-local/reserved/metadata 地址。Agnes MVP-A MUST 只实现 submit/poll/cancel/result，并把当前 storyboard AssetVersion、对应 ShotSpec、显式 duration/aspect ratio 冻结为提交输入；callback/webhook 不属于 MVP-A。MVP-A timeline MUST 固定 9:16、16:9、1:1、1080p、30fps、H.264 `yuv420p`、AAC 48 kHz 和 UTF-8 SRT；`light` 仅包含 manifest 与可解析引用，MP4/SRT 单独输出且不得回导。ExportJob 状态 MUST 仅为 `queued`、`preflighting`、`rendering`、`packaging`、`succeeded`、`failed`、`cancel_requested`、`cancelled`；`packaging` 内 MUST 以 `uploading|verifying|registering` subphase 分别跟踪 MP4/SRT/light 的 StoragePort upload、stat/checksum/MIME/size verify 和 ExportArtifact registration，不得增加第九个状态。失败 MUST 返回安全且可重新校验的 `ExportDiagnosticTarget`，其 targetType 只允许 `timeline|clip|caption|sound_cue|asset_version|renderer|storage|artifact`。真实 TOS 由独立 child 接入，Local MUST 继续仅用作测试/离线路径；Timeline child MUST 包含真实 `ffmpeg`/`ffprobe` adapter 与显式 probe，并验证 H.264/AAC decoder/encoder、`yuv420p`、MP4/container 支持，缺失任一能力时在 preview/export 前稳定阻断。

#### Scenario:未配置外部能力不会伪造交付
- **WHEN** TOS、Provider 或 renderer 的 explicit opt-in/probe 输入缺失
- **THEN** 系统记录原始 `unconfigured` 诊断，且不重复付费、不生成媒体或导出成功状态

#### Scenario:拒绝不安全图片输入或过期 Agnes 分镜输入
- **WHEN** 图片格式/mask/URL 违反安全上限，或 Agnes 引用的 storyboard AssetVersion、ShotSpec、duration/aspect ratio 缺失、过期或跨 scope
- **THEN** 系统在外部请求前拒绝，不创建 ProviderCall、StorageObject、AssetVersion 或成功 RunEvent

### Requirement:StorageProfile 设置和显式 connection-test contract
MVP-A SHALL 提供由 TOS/catalog owner 支撑的专属 StorageProfile 设置界面。该 contract MUST 覆盖 `StorageProfile/BucketBinding` 的 Bucket/Region/Endpoint/private policy/credential reference/status/timeouts/presign TTL/project scope fields、携带 `expectedRevision`/`If-Match` 的 create/edit/enable/disable、`409 storage_profile_revision_conflict` 的零写入语义、携带 profile snapshot 与 `probeCorrelationId` 的显式 connection-test，以及 masked credential status。Settings UI MUST 只消费该 owner state；页面加载或 probe 失败时，MUST NOT 启用 profile、fallback 到 Local，或创建 object/AssetVersion。

#### Scenario:接受或拒绝 StorageProfile 设置变更
- **WHEN** 用户提交有效的 StorageProfile fields，或提交 stale/foreign lifecycle mutation
- **THEN** 有效 create/edit/enable/disable 返回新的 owner revision 和 masked status；stale/foreign 输入返回带 expected/current revision 的 owner conflict diagnostic，且不写 config/session/adapter

#### Scenario:观察显式 StorageProfile connection test
- **WHEN** 用户对 configured、disabled、unconfigured 或 master-key-unavailable profile 点击 connection-test
- **THEN** UI 针对精确 profile snapshot 显示 pending 后 connected，或显示脱敏的 `unconfigured|validation|authentication|network|timeout`/503 diagnostic；不得隐式切换 adapter、写入 object/AssetVersion 或变更 config revision

### Requirement:MVP-A 固定 published workflow source 与只读边界
MVP-A SHALL 只解析并冻结 versioned、published 的 `templateKey=drama-mvp-a-default` WorkflowVersion。Backend ensure/bootstrap 只可创建或校验其固定 immutable version/binding；legacy `WorkflowDraft` 仅作只读兼容或内部冻结来源。通用 Workflow graph node/edge editing、connection validation、draft save、publish 和 version upgrade command/API/UI 均为 MVP-B capability，MUST NOT 进入 MVP-A implementation 或 acceptance path。

#### Scenario:无 graph mutation 展示固定 source
- **WHEN** 用户进入 MVP-A workbench 并请求 workflow view
- **THEN** UI 读取当前 published WorkflowVersion/source snapshot；页面加载和视图切换产生零 workflow mutation，任何通用 Draft/graph/publish/version-upgrade request 均为 `unsupported` 且零写入

### Requirement:failed successor Run 与 Skill 人工路由裁决
failed Run MUST 永久保持终态；“从失败节点继续” SHALL 显式创建 successor Run，冻结新 selection/input snapshot，只复用 predecessor 成功节点的精确 owner evidence，并为待执行节点分配新的 `run_id + logical_operation`。`submission_unknown` MUST 先在原 Run reconciliation。Skill routing SHALL 使用 `deterministic_filter -> lexical_rank -> optional_semantic_adapter -> policy_decide` 并保存 revisioned `SkillRouteDecision`；唯一高置信候选可冻结，并列、低置信或 semantic adapter unavailable MUST 返回 `needs_human_selection`。用户只能从当前 candidate set 提交 `SkillRouteSelection`；选择前 MUST 零 Run/NodeRun/TextModel/Provider mutation，不得默认选择第一项。

#### Scenario:从 failed Run 创建 successor
- **WHEN** owner 证明 predecessor failed、reuse evidence/inputs/revisions 匹配且用户明确继续
- **THEN** 系统创建新 runId 与新 logical operations，predecessor 保持 failed，reused 节点不重新执行或收费

#### Scenario:歧义 Skill route 等待用户
- **WHEN** 候选并列、低置信、semantic adapter unavailable，或 selection revision/候选不匹配
- **THEN** owner 返回 `needs_human_selection` 或 stable conflict，且不创建/启动 Run、NodeRun、TextModel 或 ProviderCall

#### Scenario:拒绝不可用的固定 source
- **WHEN** default binding 缺失、non-published、cross-project、stale 或 hash-mismatched
- **THEN** owner 返回 `workflow_unconfigured`、`workflow_version_unavailable` 或 `workflow_source_conflict`，且不创建 Run/NodeRun/Temporal/Provider side effect

### Requirement:指定历史 RunInputSnapshot 只创建新 Run
MVP-A SHALL 提供 `CreateRunFromHistoricalSnapshot`。用户 MUST 先选择并确认同项目、可读取、不可变的 `RunInputSnapshot` 精确 ID/revision/hash 与历史 runId；command MUST 重新执行预算和 capability admission，并创建新 runId、`rerunOfRunId` 与全新的 `run_id + logical_operation` 集合。该语义 MUST 与 failed successor 分离：不得重启或修改历史 Run，不得默认采用 current、隐式升级/rebase Provider/Model/Skill/Workflow/owner refs，也不得复用 failed-successor evidence、ProviderCall 或外部提交。新旧 Run MUST 保持独立可读。

#### Scenario:从指定历史 snapshot 重跑
- **WHEN** 用户确认一个同项目且 owner refs/revisions/hashes 完整的历史 `RunInputSnapshot`
- **THEN** 系统创建一个引用该 snapshot 的新 Run 和新 logical operations，保留原 Run/NodeRun/events/selection，不读取 current 替换冻结输入

#### Scenario:拒绝歧义或过期的历史重跑
- **WHEN** snapshot foreign、missing、hash/revision 不匹配，command 试图复用 operation/evidence，或预算/capability admission 未通过
- **THEN** 系统返回 `historical_snapshot_rerun_conflict` 或 owner diagnostic，不创建部分 Run/NodeRun/ProviderCall/外部提交，也不改变历史 Run

### Requirement:SourceMaterial 导入、校验与 text-run 绑定
MVP-A SHALL 冻结 `creationMode=original|adaptation`，其 canonical owner 为 `projects`。`original` SHALL 使用 projects owner CreativeBrief 的 `subject`、`genre`、`audience`、`characterPremise`、`style`、`episodeDurationSeconds`、`episodeCount`、`scenesPerEpisode`、`shotsPerScene`、schema/revision，MUST NOT 要求、创建或冻结 SourceMaterial。`adaptation` SHALL 要求 `materialType=novel|synopsis|existing_script` 与 `inputMode=inline_text|uploaded_file` 的 valid SourceMaterial，并由 text owner 负责 SourceMaterial import、validation、binding 和 recovery。保存 brief source reference 时 MUST 冻结 `CreativeBriefSourceBindingSnapshot={projectId, sourceMaterialId, sourceMaterialRevision, sourceContentHash, creativeBriefId, creativeBriefRevision, creativeBriefPayloadHash, parseStatus, validationStatus, bindingStatus, bindingVersion}`；创建 Run 后的 `TextRunSourceBindingSnapshot` MUST 冻结全部相同字段并增加 `runId`、`runRevision`，Run 分配前不得伪造这些字段。adaptation 的 `inline_text` SHALL 直接形成上述 snapshots，MUST NOT 创建 storage session、StoredObject 或 AssetVersion。只有 adaptation 的 `uploaded_file` 上传 bytes：text owner 创建冻结 projects owner creation/brief snapshot、input/material、source revision/contentHash 与 reservation 的唯一 `SourceMaterialUploadIntent`，key 为 `sourceMaterialUploadKey=source-material-upload:{projectId}:{sourceMaterialId}:{sourceMaterialRevision}`；Storage owner 校验并返回 `VerifiedStoredObjectHandoff`，Assets owner 在其 UoW 中 append 一个 AssetVersion，然后 text owner 以 CAS 绑定精确 `assetVersionId/revision/contentHash`；`run_id + logical_operation`（若存在）只映射到该 key。无效 enum、scope、revision、hash 或 status/version MUST 在 TextModel、付费 Provider 和 Storage mutation 前拒绝；恢复只复用同一 source/brief revision，不隐式换源。

#### Scenario:导入并绑定 adaptation source
- **WHEN** 用户提交当前 PRD 明确保留的 SourceMaterial input，且 parsing 与 validation 成功
- **THEN** upload 在 SourceMaterial binding 前记录同一 upload session/operation、verified StoredObjectRef 和 AssetVersion append response；inline input 不记录 storage session/AssetVersion；两者均由 projects owner 保存精确 brief source snapshot，并在 Run 创建后冻结包含 project/source/brief/run IDs、revisions、content/payload hashes 和 parse/validation/binding status/version 的 text Run snapshot，全部字段匹配后才允许 structured generation

#### Scenario:文本生成前拒绝无效或 foreign source
- **WHEN** parsing/validation 失败、upload/verification 失败、source 或 AssetVersion 为 foreign/stale/unverified、AssetVersion registration 失败、binding 返回 409，或 adaptation 无有效 source
- **THEN** 系统展示原始 diagnostic，不进行 downstream binding/paid text Run，为受控 recovery 保留任何 unreferenced verified object/operation，且不静默替换另一个 source

#### Scenario:恢复 source parsing 和文本生成
- **WHEN** source upload/verification/registration/binding、parse 或 bound text Run 失败，或页面/API/Worker 重启
- **THEN** recovery 复用同一 SourceMaterial revision/contentHash 和 `sourceMaterialUploadKey`（以及存在时的 `run_id + logical_operation`），先 reconcile unknown state，且不重新上传、不 append 重复 AssetVersion、不绑定不同 source、不提交重复 paid operation

### Requirement:Dialogue ducking 是 MVP-A Timeline contract
MVP-A SHALL 支持持久化 dialogue ducking：`enabled`、已合并的整数帧 `dialogueIntervals`、正值 `attenuationDb`、非负整数 `attackFrames`/`releaseFrames`，以及只含 `music|ambience|effects` 的 `targetTracks`。Dialogue 自身 MUST NOT 被 duck。canonical RenderPlan MUST 将 `attenuationDb` 映射为每条 target track 的负 gain，FFmpeg compiler MUST 消费相同的冻结值；重叠 interval MUST 确定性合并。

#### Scenario:配置 ducking 并保持 preview/render parity
- **WHEN** 用户为 current Episode TimelineVersion 配置有效 ducking parameters
- **THEN** Timeline、proxy preview 和 FFmpeg RenderPlan 携带相同 interval、attenuation、attack/release 与 target tracks，audio regression 确认 dialogue 不变而 target 被衰减

#### Scenario:拒绝不安全或歧义的 ducking
- **WHEN** interval 非整数、为空、越界或不可合并，attenuation 非正，attack/release 为负，或 targetTracks 含 dialogue/unknown track
- **THEN** command 和 preflight 在 TimelineVersion/RenderPlan/ExportJob mutation 前失败并返回原始 validation diagnostic

### Requirement:Agent edit 所有权按类型闭合
MVP-A SHALL 只允许 image/video 的 executable `AssetEditPlan`，并提供 owner typed command、version、execution/reconcile path 和 candidate review。每个 Plan MUST 绑定完整 image/video base AssetVersion 与显式完整 AssetVersion reference 集合；MUST NOT 接受图片 mask/选区、视频/音频时间范围或局部片段作为可执行输入。story/script MUST 保持在 TextReview successor/stale closure；audio/Timeline MUST 保持在 Timeline editor typed commands。对于缺少 owner contract 的类型，UI MUST NOT 声称提供可编辑 Agent surface。

#### Scenario:拒绝未拥有的 Agent edit 类型
- **WHEN** 用户或 UI 试图为 story、script、audio 或 TimelineVersion 生成或执行 AssetEditPlan
- **THEN** 系统返回 typed owner diagnostic 或打开对应的 read-only/TextReview/Timeline editor surface，不创建 AssetEditPlan/Outbox/ProviderCall/AssetVersion/Timeline mutation，也不暗示支持

### Requirement:MVP-A 立即持久化的 Timeline edit 闭合
MVP-A SHALL 保留 `TrimClip`、`SplitClip`、`ReorderClips`、`DeleteClip`、`ReplaceClipSource`、静态 `position`/`scale`/`opacity`、四条 SoundCue track、静态 gain、mute/solo、linear fade、ducking 和手工 caption text/time editing。`SoundCue.track` SHALL 是 PRD `cueType` 的唯一 canonical 分类，MUST NOT 同时接受第二个 cueType 字段；每项 cue MUST 持久化非负整数 `startFrame`、正整数 `durationFrames`、`trigger=manual|scene_start|shot_start|shot_end`、0..100 整数 `priority` 与只含同 Episode AssetBible/Scene/Shot/ShotSpec owner refs 的 `continuityRefs`。非 manual trigger MUST 绑定对应 Scene/Shot ID/revision 与整数 offset；priority 只决定同轨重叠的稳定排序。MVP-A MUST 只接受 static gain 与 linear fades，任意 automation/keyframes MUST 返回 `unsupported_feature`。每个成功 command MUST 立即持久化 owner revision；409 MUST 零部分写入并返回 authoritative state 供 UI rollback/refetch。静态 transform MUST NOT 接受 keyframe。AssetEdit/VideoTake accept MUST NOT 自动调用 `ReplaceClipSource`；只有 accepted-current、derivative ready 且用户比较 exact Clip old/new source 后才可提交，成功只更新 current Cut，既有 TimelineVersion 不变。

#### Scenario:持久化或恢复立即 edit
- **WHEN** current Cut 接收有效或 stale/delete/keyframe edit command
- **THEN** 有效 command 返回其立即持久化的 revision；stale 或 keyframe 输入返回 conflict/validation 且无部分 mutation；AssetVersion 和 published TimelineVersion 保持不变

#### Scenario:显式替换重拍来源后另行发布
- **WHEN** 用户确认同项目/集/Shot 的 exact old/new AssetVersion/hash/derivative/frame facts 并提交当前 Cut revision
- **THEN** `ReplaceClipSource` 只更新该 Clip source 和 Cut revision，保留编辑属性；用户必须另行命名/preflight/publish 新 TimelineVersion

### Requirement:MVP-B 边界必须明确
MVP-A MUST NOT 包含 storyboard insert/copy、Scene split/merge、Shot 跨场 move、storyboard batch generate/batch retake/batch review、Timeline standalone autosave、undo/redo、version restore、subtitle style、SoundCue automation/keyframes、Run pause、review comments/timecodes/reminders、Narration/TTS、loop、speed、track lock、project-package reimport、LAN exposure、simple password 或 reverse-proxy auth。该边界 MUST NOT 禁止 Timeline 聚合内的 `SplitClip`。MVP-A 默认与验收环境 MUST 只监听 localhost/`127.0.0.1`，不得为 LAN 使用改成无认证广域监听。

#### Scenario:拒绝延后的 editing 或 review feature
- **WHEN** MVP-A request 包含被延后的 feature command 或 UI affordance
- **THEN** 它不存在，或返回 `unsupported_feature` 且零 owner mutation

### Requirement:Provider operation 限流、配额与历史模型保护
MVP-A SHALL 为每个 Provider/Profile operation 保存可版本化的 concurrency limit、rate-limit window/quota、429/`Retry-After` policy 和 quota snapshot/status。每次 live admission MUST 在 ProviderCall/external submit 前读取冻结配置；超限 MUST 返回稳定 retryable diagnostic，quota unknown MUST 保持 unknown 而不得伪报可用。Model 存在 CapabilitySnapshot、ProviderCall、Run、项目默认或 workflow 历史引用时 MUST 只允许 disable，MUST NOT 物理删除或替换历史 identity。

#### Scenario:限流或历史引用阻止不安全 mutation
- **WHEN** operation 达到并发/速率上限、quota unknown/exhausted，或用户请求删除仍有历史引用的 Model
- **THEN** 系统在外部提交或 catalog delete 前返回稳定 diagnostic；不创建额外 ProviderCall、不切换模型，且历史 Model/snapshot/reference 保持可读

### Requirement:项目资产中心是独立业务闭环
MVP-A SHALL 通过 `implement-project-asset-center` 提供项目级 Asset 上传/续传/取消/恢复、目录分页与 kind/catalogRole/tag/source/authorization/processing 过滤、不可变版本、授权、MediaInspection/Derivative 状态、音频试听和精确只读 usage。Assets owner 只拥有 Asset metadata、AssetVersionReservation 与 AssetVersion append；Storage 只拥有 UploadSession/StoredObject，Media Worker 只拥有 inspection/derivative，各业务 owner 只拥有真实引用。usage MUST 聚合 owner query，owner unavailable MUST 返回 partial/unavailable，Timeline 素材箱 MUST 复用资产中心 selector/query。

#### Scenario:资产中心上传恢复并交给 Timeline selector
- **WHEN** 用户以同一 reservation/operation 恢复图片或音频上传、筛选目录、试听并查看 usage 后显式选择同项目 AssetVersion
- **THEN** 系统至多登记一个 AssetVersion，返回精确 owner revisions/hashes/usage 和 selector handoff；页面读取、失败、取消或 late result 不创建第二对象/版本、ProviderCall、RunEvent、derivative 或 Timeline reference

### Requirement:Operations resilience 是独立的跨边界 child
MVP-A SHALL 通过 `implement-operations-resilience` 实现 CPU、内存、容量/磁盘 capability probe、disk soft/hard threshold protection、稳定 refusal/diagnostic semantics、manual backup/restore runbook 和一次 checksum/ETag restore exercise。该 contract 跨越 Local workspace、Worker temporary/derivative files、database backup metadata 与 object-storage manifest/reference state；MUST NOT 被 TOS adapter 吸收。

#### Scenario:hard threshold 或 restore failure 可见且可恢复
- **WHEN** CPU/内存/容量 capability 不可满足、capacity 到达 configured hard threshold，或 restore checksum/ETag 不匹配
- **THEN** 相关 write/export operation 以稳定 diagnostic 被拒绝且零部分成功；runbook 记录 manual recovery evidence，不 fallback 或声称 restore 成功

### Requirement:实际 2 GiB 素材链路是阶段退出证据
MVP-A SHALL 使用精确 `2_147_483_648` bytes 的实际媒体 fixture 验证 `StorageProfile` object/part limits、CPU/memory/capacity admission、streaming multipart interruption/resume、part manifest、stat/checksum/MIME/size verification、单一 AssetVersion registration 以及 Media Worker inspection/proxy。2 GiB SHALL 是验收规模而非平台最大值；默认快测 MAY 使用 logical-size fake，但 MUST 单独标记且不得替代 actual-byte evidence。能力不支持时 MUST 在 UploadSession、part、workspace file read/write 或 reservation side effect 前拒绝。

#### Scenario:实际 2 GiB 中断后恢复到单一版本
- **WHEN** profile/resource preflight 支持精确 2 GiB，上传在已记录 part 后中断并以同一 reservation/operation 恢复
- **THEN** evidence 记录实际 bytes、part manifest、checksum、profile revision、单一 StoredObject/AssetVersion 与 ready inspection/proxy，且不重复 part、object 或 version

#### Scenario:不支持 2 GiB 时前置拒绝
- **WHEN** object/part limit、capacity 或 worker inspection capability 不支持该 fixture
- **THEN** 系统返回 `media_2gib_capability_or_resume_failed`，在 session/file/reservation mutation 前停止，并且不得把 logical-size fake 或更小文件报告为阶段退出成功

### Requirement:Local observability 是独立且非阻断的 child
MVP-A SHALL 通过 `implement-local-observability` 使用 W3C `traceparent`/`tracestate` 关联 Web、FastAPI、Outbox、Temporal、三类 Worker、Provider/Storage adapter 与 FFmpeg。它 SHALL 输出 secret-free allowlisted JSON logs、低基数 metrics 与可选本地 diagnostics profile，并在授权的 Run/NodeRun/ProviderCall/Upload/Export projection 中提供 canonical trace ID。Telemetry MUST NOT 复制或拥有 RunEvent、ProviderCall、usage/cost、ExportJob、AssetVersion 或审核事实；SDK/exporter/collector/viewer 失败 MUST NOT 改变业务 readiness、UoW、重试、状态、Adapter/Profile 或付费 operation。

#### Scenario:跨服务证据可与 owner facts 对账
- **WHEN** 一个文本 Run、image/video operation、multipart resume 与 Timeline export 在 in-memory exporter 或可选 diagnostics profile 下完成
- **THEN** report 包含连续 parent-child lineage、脱敏 logs、无实体 ID 高基数 label 的 metric delta 与 owner facts 对账，且不存在第二业务账本

#### Scenario:Telemetry 失败不改变业务结果
- **WHEN** trace header 非法、exporter/collector/viewer 不可用、队列已满，或输入包含 secret/full text/path/raw payload
- **THEN** 系统使用安全新 root 或记录 `telemetry_export_unavailable`/redaction diagnostic，业务按原 owner contract收敛且不重复事件、调用或费用

### Requirement:五个业务页面形成项目内导航与控制闭环
MVP-A SHALL 由 `implement-drama-creation-workbench-ui` 提供共享 project-scoped 壳层，连接 Workbench、Candidate Review、Project Asset Center、显式 Episode Timeline 和项目模型设置；所有导航 MUST 保留并校验 projectId，Timeline MUST 要求用户显式选择 episodeId，selection handoff MUST 只携带 owner stable IDs/revisions/hashes。Workbench/Review MUST 以 `projectId + episodeId` 隔离并恢复 viewport、collapsed scenes、status/model/review filters、Shot/Asset selection 和 active Agent session ID；恢复 MUST 重新校验 owner scope/revision、message sequence 与 selection hash，清除 stale/foreign references，MUST NOT 保存 message/turn/Run/candidate/AssetVersion owner 正文、跨集 fallback、重发消息、重复生成 Plan 或重复 Provider operation。Workbench MUST 显示 Run/NodeRun 的脱敏输入输出摘要、耗时、最近事件、失败诊断和允许动作，并对 `queued|running|waiting_review` 提供显式 cancel；必须显示 `cancel_requested|cancelled`、并发冲突和取消后的晚到结果。完整 ShotCard MUST 显示 Scene、角色/场景引用、image/video current/candidate、时长、提示词摘要、模型 revision、成本来源和 generation/review/derivative 状态，并提供 owner-validated 跨页入口。zero-episode MUST 只提供 `original|adaptation`，MUST NOT 展示 MVP-B 推荐模板入口。route load/back/tab/breadcrumb MUST 零业务 mutation。

#### Scenario:经真实导航完成项目内业务往返
- **WHEN** 用户从项目入口进入 Workbench，再由 ShotCard 或共享导航进入 Review、Assets、显式 Episode Timeline、项目设置并返回
- **THEN** 所有页面保持相同 owner-validated project 和必要 Episode/selection；不得以全局 `/assets|/runs|/settings` 或直接 URL 拼接替代项目内导航证据，也不产生额外 Run/Provider/upload/review/Timeline/settings mutation

#### Scenario:取消运行时保留 owner 终态
- **WHEN** 用户取消可取消 Run，且随后出现重复点击、stale revision、Activity/Provider 晚到结果或 SSE 重连
- **THEN** UI 只提交一次 cancel command，以 authoritative snapshot 收敛到 `cancel_requested|cancelled`，显示冲突/诊断且不报告 succeeded、不重复付费

#### Scenario:空项目不提供延后模板
- **WHEN** 用户进入没有 Episode 的项目
- **THEN** Workbench 只显示 original/adaptation 当前入口，不创建/推荐/启动模板、Workflow、Run 或 Episode

### Requirement:TimelineVersion 发布是 Timeline UI 的显式闭环
MVP-A SHALL 在 current Cut UI 提供 TimelineVersion 名称、owner preflight、显式 publish 和只读比较。publish MUST 携带 current Cut `expectedRevision`，成功时只追加一个不可变 TimelineVersion；名称/preflight/foreign scope/409 失败 MUST 零版本写入并要求刷新，不得自动重试、修改 current Cut/既有 Version 或启动 ExportJob。该 publish 仅冻结 TimelineVersion，MUST NOT 被解释为 Workflow 发布或内容平台分发。

#### Scenario:发布后再导出
- **WHEN** 用户为 current Cut 输入合法名称、preflight 通过并以当前 revision 显式发布
- **THEN** UI 读取新增 Version ID/name/sourceCutRevision/scope/schema/revision 并可只读比较；只有该已发布版本随后可进入 export preflight

#### Scenario:发布冲突阻断导出
- **WHEN** 名称无效、preflight 失败或提交时 expectedRevision 已过期
- **THEN** UI 显示 owner diagnostic、丢弃过期 preflight 并 refetch；不创建 TimelineVersion、RenderPlan、ExportJob 或 artifact

### Requirement:项目多集导出按集独立
MVP-A SHALL 在 `/projects/:projectId/exports` 提供 `EpisodeExportBatch`。请求 MUST 由用户显式提交非空、去重且有序的 Episode + published TimelineVersion ID/revision 集合及每集安全唯一 output base name；创建任何 job 前 MUST 完成全集合归属、发布、renderer、authorization、artifact 与 RenderPlan preflight。成功 SHALL 为每集创建独立 ExportJob、MP4、SRT 和 light artifact，并汇总 `succeeded|partially_failed|failed|cancelled`；MUST NOT 自动选择 current、扩大集合、合并 RenderPlan、跨集拼接或自动重试失败集。

#### Scenario:多集批次逐集输出
- **WHEN** 用户显式选择的全部 published TimelineVersion 通过 preflight
- **THEN** 系统创建一个 batch 和逐集独立 jobs/artifacts，稳定文件名可追溯到 Episode/Version，且没有合并视频

#### Scenario:任一成员预检失败则零提交
- **WHEN** 集合重复、foreign、stale、未发布、命名冲突或任一 preflight 失败
- **THEN** 系统返回逐项 diagnostic，不创建 batch/job/artifact，也不替换失败项为 current

#### Scenario:导出上传或登记失败可定位并恢复
- **WHEN** 任一 Episode ExportJob 在 `packaging` 的 upload、stat/checksum/MIME/size verify 或 artifact registration 中失败、响应丢失或状态 unknown
- **THEN** owner 保留八态 ExportJob 和精确 subphase，返回同项目且重新校验过的 `ExportDiagnosticTarget`；recovery 先 reconcile 同一 operation，不重新渲染、重复上传/登记、fallback 或自动替换 TimelineVersion

### Requirement:阶段一非功能退出证据
阶段一 SHALL 在确定性本地 fixture 上验证包含 300 个 fixed published Workflow node、冻结 scope 及必要 Scene/Shot 关联的只读投影可完成加载、浏览、滚动或分页、筛选、选择、详情和五页面真实导航，且不得借机恢复 MVP-B graph authoring。MVP-A 默认与验收服务 MUST 只绑定 localhost/`127.0.0.1`；声明的普通 localhost API 成功请求 P95 MUST `<500ms`。报告 MUST 记录监听地址、route、样本量、warm-up、环境、数据量、成功/失败数和 percentile，并排除 Provider/Agent/Temporal 等待、SSE 长连接、上传下载、媒体 probe/preview/render/export。项目入口到导出前的关键浏览器闭环 MUST 分别在桌面 Chrome 与 Edge 执行并记录实际版本；缺少任一证据时不得通过阶段一验收。

#### Scenario:非功能证据可复算且范围明确
- **WHEN** 维护者提交阶段一性能和兼容性报告
- **THEN** 报告能重放 300-node 只读投影交互、复算普通 API P95，证明只监听 localhost/`127.0.0.1`，并分别列出 Chrome/Edge 版本与结果；外部或媒体耗时未混入普通 API 指标

### Requirement:阶段 1 退出标准
系统 SHALL 将退出验证固定为：Mock 2x2x3、恢复且不重复付费、显式 live 1x1x1、指定历史 snapshot 新 Run、Episode 状态隔离、完整 SoundCue、导出失败定位及 upload/verify/register、实际 2 GiB 素材链路、W3C trace/log/metric owner 对账、300 项前端投影、普通 localhost API P95 `<500ms`、localhost-only bind、桌面 Chrome/Edge，以及所有 19 个 change 的 strict validation。五个前端/业务 child MUST 分别完成创作、image/video 上下文候选审查、时间线编辑、Provider/Model/Skill 设置和项目资产中心闭环；创作闭环必须含 projects creative owner、AssetBible entry/version/override/impact/task、Brief 到文本 Run 的 create/start/regenerate/reconcile/failed successor/historical rerun、Skill 人工路由、Run detail/cancel、Episode presentation/session isolation、完整 ShotCard 与共享项目导航；上下文闭环必须含 image/video conversation/message/turn、跨集恢复拒绝、AssetBible snapshot/task gate 到 Schema-valid AssetEditPlan 及显式 Timeline replacement handoff；Timeline 闭环必须含 ReplaceClipSource、完整 SoundCue、命名/preflight/publish/只读比较、`ExportDiagnosticTarget` 和 `/projects/:projectId/exports` 多集逐集导出；工作流/设置界面必须含阈值、批量/unknown cost 确认与精确 run/logical operation 绑定；资产中心必须含上传恢复/取消/实际 2 GiB/筛选/试听/usage/Timeline selector；resilience child 必须有 resource/capacity 拒绝、runbook 和 checksum/ETag 演练证据；observability child 必须证明 telemetry fail-open。

#### Scenario:集成验收拒绝缺口
- **WHEN** 任一 mock/live 矩阵、恢复/幂等证据、UI/resilience/observability 闭环、实际 2 GiB/localhost bind 证据或 19 个 strict validation 缺失
- **THEN** 阶段 1 验收失败并保留缺失证据，不报告 MVP-A 完成

### Requirement:空系统可追溯性和浏览器验收
系统 SHALL 以 `E2E-MVPA-001` 追溯空系统到逐集 MP4/SRT/light：project create/select -> shared shell/zero-episode -> original Brief 或 adaptation SourceMaterial -> fixed Workflow/Skill route -> Run create/reconcile/cancel、failed successor 或 `S03a` historical snapshot 新 Run -> one TextReviewBatch/all owner acks -> `S04a` AssetBible -> Episode list/select 与隔离的 presentation/session state -> ShotCard/image/video/AssetEdit/review -> MediaInspect -> `S08a` asset center 和 `S08b` actual 2 GiB resume/register/inspect -> explicit `ReplaceClipSource` -> typed Clip 与完整 SoundCue/static transform/caption/ducking -> TimelineVersion publish/compare -> EpisodeExportBatch per-Episode render/upload/verify/register 与 `ExportDiagnosticTarget` -> `S11` resilience -> `S11a` observability owner 对账。Workbench/Review/Assets/Timeline/Exports/Settings MUST 通过共享壳层真实导航往返，不得用 direct URL 替代；服务 MUST 只绑定 localhost/`127.0.0.1`。未经 video accept 或 derivative ready 不得进入 Timeline。Mock `2x2x3` 必须明确为 2 Episodes x 2 Scenes/Episode x 3 Shots/Scene；live `1x1x1` 是显式 opt-in probe，非默认 Playwright。

该 E2E 必须按总体 `design.md` 的 canonical stage matrix `S01`-`S11` 加 `S03a historical rerun`、`S04a asset bible continuity`、`S08a asset center`、`S08b 2 GiB media chain` 与 `S11a observability` 验收；每行 SHALL 记录 owner、exact prerequisites、success evidence、对应 `F01`-`F11`、`F03a`、`F04a`、`F08a`、`F08b` 或 `F11a` focused diagnostic 和 no-side-effect invariant。矩阵中的 owner 是 DDD owner，UI、Playwright harness 和 report 只能观察其 response/state，不能重新定义或拼接领域事实。

#### Scenario:默认 E2E 保持确定性
- **WHEN** 维护者运行 `pnpm run test:e2e`
- **THEN** harness 使用 deterministic reset、Web/API/worker lifecycle、Mock Provider、显式 Local test/offline profile（adapter identity=`local_workspace`）和 Mock preview；真实 FFmpeg/Provider/TOS/AgentScope 均不作为 browser oracle，且页面加载/视图切换不创建或切换 profile

#### Scenario:stage evidence matrix 完整
- **WHEN** 为 `S01`-`S11` 加 `S03a`、`S04a`、`S08a`、`S08b`、`S11a` 生成 E2E-MVPA-001 report
- **THEN** 每行均包含 owner response 或 persisted artifact、精确 prerequisite snapshot、稳定 focused failure diagnostic，以及对应 no-side-effect invariant 已成立的 assertion；缺少行或仅有泛化 final-green assertion 均使验收失败

### Requirement:跨 change contract 可追溯性
总体集成 MUST 追溯 candidate provenance/CAS、successor stale closure、catalog CAS、credential envelope/key rotation、独立 ExportArtifact 和 cut/crossfade RenderPlan parity；任何未满足的前置都 MUST 阻断后续副作用。

#### Scenario:Contract gate 阻断 downstream work
- **WHEN** 任一 candidate、revision、归属、凭据、artifact grant 或 render-plan parity 校验失败
- **THEN** 下游 ProviderCall、external submit、导出下载或成功状态均不得产生，且保留可诊断原始错误。
