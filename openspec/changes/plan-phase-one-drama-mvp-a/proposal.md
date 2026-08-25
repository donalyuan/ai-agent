## Why

## 跨 owner 运行合同

阶段一 Registry 固定登记八项 candidate：`drama-skills`、`novel-writing`、`zy-cinematic-realism`、`seedance-2.0`、`storyboard-tiktok-video-skill`、`hell-grind/cinedance-higgsfield`、`hell-grind/acting`、`hell-grind/lira`。前两项为 `provenance=verified_snapshot`、`approval=approved`、`enabled=true`，且 `drama-mvp-a-default` 只绑定这两个 approved revision；其余六项为 `provenance=pending_provenance`、`approval=not_approved`、`enabled=false`。Git Skill 使用 commit/digest，公开 Markdown Skill 使用 archive URL/获取时间/digest/license status；AgentScope 2.x 作为 Agent Worker 独立 runtime dependency 管理。Worker 启动和默认 Run 不得把后六项当作前置；仅当 node 的 `allowedSkills`、`requiredCapabilities` 与 `selectionMode=fixed|inherit` 均满足时，才可按需读取 `SKILL.md` 和 references。

Provider/Model/Profile 的首次显式 connection-test/probe 只要求 `adapterInstalled=true`、catalog `approval=approved`、`featureGate=MVP-A`、explicit live opt-in、已选 profile、可解析 credential 与 timeout，成功后冻结 successfully probed capability snapshot；它不以前次 probe、`runnable=true` 或 disabled-for-run 为前置。snapshot-missing/`runnable=false`/disabled-for-run 只阻断 enable/default/Run 解析与 live invocation，后者还必须同时具有该成功 snapshot 和 `runnable=true`。MVP-B/uninstalled/not-approved 或缺 opt-in/profile/credential/timeout 的 operation 零 probe/外部调用；TTS/ASR、MiniMax H3、Seedance 2.5 与 Agnes 未选中 mode 保持不可运行。默认测试组合为 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），并保持 explicit live opt-in；运行开始后 Adapter/Profile 冻结。

MVP-A 只绑定固定、版本化、已发布的 `drama-mvp-a-default` WorkflowVersion；不存在 Draft identity、Draft lifecycle 或 graph mutation，后两者均属于 MVP-B 且不得进入 Run。

文本同一 Run、同项目、Schema-valid 且 hash/revision/scope 匹配的 provisional upstream candidate 可构造完整候选图；其余输入拒绝。视频严格按 verified Provider terminal result/storage validation -> immutable candidate + existing AssetVersion -> 人工以 exact candidate/source/ShotSpec facts `accept` -> scenes exact current CAS -> MediaInspect/derivatives -> Timeline handoff。基础 result 安全验证仅含 download/MIME/checksum/size/duration/dimension/StoredObjectRef，不是 MediaInspect derivative generation；candidate/pending_review 不携带 derivative readiness 作为 accept gate，accept 后 Media Worker 才生成 metadata/proxy/thumbnail/keyframe/waveform。derivative pending/failed/stale 只阻断 Timeline/preview/export，不阻断或撤销 accepted/current。所有 review 外部/domain/UI/audit verb 统一为 `accept|reject|retake`；`approve` 和未知旧 verb 均 validation 且零 current/retake side effect。verified Provider terminal success 是 result AssetVersion 的唯一 append 时点；AssetEdit `accept` 仅追加 AcceptDecision/audit 和同一 version 的 scenes eligibility CAS。

阶段 0 已提供项目、剧集、素材版本、Mock Provider、显式 Local test/offline profile 边界和基础 Schema，但阶段 1 的场景分镜、工作流、Provider、生成、编辑审核和时间线导出仍是互相依赖的待实现能力。需要先以一个总体 change 固定 MVP-A 的范围、集成顺序和共享工程约束，避免各切片重复阶段 0、把目标架构误写为现有实现，或在产品需求冲突处做出不兼容的实现。

## What Changes

- 建立阶段 1 剧集创作 MVP-A 的总体协调 change 与集成契约，不实现业务代码；它不是十九个 child 实施运行时必须先应用的代码依赖。
- 固定本总体 change 与十九个 child 的唯一职责、总体任务号映射、依赖 DAG、并行边界和独立 Alembic 迁移边界；其中包括 `extend-projects-episodes-creative-slice`、`implement-asset-bible-continuity-slice`、五个前端/业务闭环、独立 TOS、独立运维 resilience 和独立 `implement-local-observability`。共享前端组件基线由 `implement-drama-creation-workbench-ui` 建立，其余 UI change 只消费，不新增原型 change。
- 建立 DDD、BDD、SDD、TDD 共享规则：版本、幂等、持久事件、`Mock Provider +` 显式 Local test/offline profile、外部副作用和测试分层。
- 采用统一 `ProjectPackage`：canonical manifest 字段为 `schema_version` 与 `exportProfile`；`exportProfile` 仅取 `light|portable`，MVP-A 仅实现 `light`，`portable` 及媒体载荷属于 MVP-B。
- MVP-A 只消费固定、版本化、已发布的 `templateKey=drama-mvp-a-default` WorkflowVersion；默认 Workflow 的 ensure、source snapshot 和 Run 绑定由后端 `workflows/runs` 完成，通用工作流图编辑、连线、保存、发布和版本升级 command/API/UI 明确延后到 MVP-B。
- MVP-A 冻结 `creationMode=original|adaptation`：`projects` owner 持有 Project、`creationMode`、CreativeBrief、项目设置和预算阈值；CreativeBrief 固定六项创作语义、三个精确计数字段及 schema/revision。text owner 只消费其已校验 immutable snapshot，并仅拥有 adaptation 的 `SourceMaterial`、文本候选和 `TextReviewBatch`。`original` 不要求 `SourceMaterial`；`adaptation` 必须提交 `materialType=novel|synopsis|existing_script` 与 `inputMode=inline_text|uploaded_file` 的 `SourceMaterial`，并保留 import/parse/validation/binding/recovery owner contract。
- 由独立 AssetBible owner 持有 Character/Look/Location/SceneVisual/Prop/VisualStyle 的稳定 entry、不可变 version、project -> episode -> scene -> shot override、accepted resolved snapshot、精确影响分析和 `ContinuityRevisionTask`；TextReview handoff 只包含叙事候选实际引用的初始 entry specs，Project/Episode/Scene/Shot/AssetBible 全部 ack 前媒体门关闭。
- MVP-A 保留对白自动压低：`enabled`、对白作用区间、衰减量、attack/release 和目标音轨必须冻结进 TimelineVersion，并由预览与 FFmpeg 共用的 canonical RenderPlan 映射。
- 冻结费用/预算闸门、通用 AES-256-GCM/Docker Secret credential owner、第三方 Skill 按来源类型的访问审计、GPT Image SSRF/格式上限、Agnes storyboard 输入绑定与纯 submit/poll/cancel/result 生命周期，以及 30 天诊断/长期审计/稳定本地 UUID。
- 补齐五个前端/业务闭环：CreativeBrief 创建/启动/重生成/恢复文本 Run 及 Run 详情/显式取消，完整 ShotCard，Agent conversation/message/turn 到 AssetEditPlan，每集 Timeline 的命名/preflight/发布不可变 TimelineVersion，Provider/Model/Skill 设置，以及项目资产中心的上传恢复、目录筛选、版本/授权/派生状态、试听和只读使用位置。
- 补齐恢复与路由裁决：failed Run 只能显式创建 successor Run 并复用精确成功 evidence；Skill 并列/低置信时必须人工选择当前 candidate revision，选择前零 Run/NodeRun/TextModel/Provider 副作用。
- 补齐媒体进入成片的显式边界：已接受且 derivative ready 的重拍只生成 Timeline replacement handoff，用户再以 `ReplaceClipSource` 精确替换既有 Clip 并单独发布新 TimelineVersion；项目多集导出使用 `EpisodeExportBatch`，每集独立 MP4/SRT/light artifacts，不自动选择 current 或跨集拼接。
- 由创作工作台 child 交付共享 project-scoped 壳层和 selection handoff，使 Workbench、Review、Assets、显式 Episode Timeline 与项目设置可经真实导航往返；zero-episode 只提供 original/adaptation，不提供已延后 MVP-B 的模板入口。
- 固定阶段一非功能验收：含 300 个 fixed published Workflow node、冻结 scope 及必要 Scene/Shot 关联的确定性只读投影可操作，普通 localhost API P95 `<500ms`（排除外部生成、长连接、媒体传输与渲染），桌面 Chrome/Edge 均完成关键闭环并记录版本。
- 新增独立 `implement-project-asset-center` child：Assets owner 只扩展 Asset 目录元数据、`AssetVersionReservation` 与 append-only AssetVersion；Storage 继续拥有 UploadSession/StoredObject，Media Worker 继续拥有 MediaInspection/MediaDerivative，使用位置只聚合各 owner query，不形成第二事实源。
- 记录当前已实现、已定义和待实现能力的追踪矩阵，以及不可由阶段 0 证据推断的待确认事项。
- 冻结两个跨 change MVP-A 合同：StorageProfile 专属 settings lifecycle/connection-test，以及 `SourceMaterialUploadIntent -> verified StoredObjectRef -> AssetVersion -> SourceMaterial binding` 的唯一 owner、同一幂等键、reconcile、失败恢复与 E2E 证据。
- 收口 Agent DDD：可执行 `AssetEditPlan` 与候选审核只覆盖 image/video；story/script 只走 TextReview successor/stale closure；audio/Timeline 只走 Timeline editor typed commands。UI 不得宣称有 story/script/audio/TimelineVersion 编辑闭环；如要扩展多类型 Agent，必须先新增各类型 owner contract、typed command、版本和执行闭环。
- 收口 Timeline MVP-A：保留静态 position/scale/opacity、trim/split/reorder、明确 `DeleteClip`、四类音轨、静态音量、mute/solo、淡入淡出、ducking 与手工字幕文本/时间编辑；每个成功编辑 command 立即持久化，409 必须回滚乐观状态并重新读取 authoritative revision，静态变换不得引入关键帧。
- 新增独立 `implement-operations-resilience` child，负责跨 Local、Worker、数据库和对象存储的 CPU、内存、容量与磁盘预检、软/硬阈值保护、拒绝/诊断语义、手工备份/恢复 runbook 与一次 checksum/ETag 恢复演练；该职责不放入 TOS adapter。
- 冻结范围补充：文本生成只交付结构化 StorySpec/ScriptSpec/Scene/Shot/ShotSpec，不生成小说正文或章节草稿；AssetEdit 在 MVP-A 只接受完整 image/video AssetVersion 与显式引用集合，图片 mask/选区和视频/音频时间范围编辑延后 MVP-B；Provider catalog 还必须提供按 operation 的并发/限流、配额状态与历史引用模型只能停用的规则。
- 补齐运行恢复与 Episode UI 状态合同：用户可从指定历史 `RunInputSnapshot` 创建全新的 Run 和全新 logical operations，但不得重启历史 Run、默认采用 current 或复用 failed-successor evidence；Workbench/Review 按 `projectId + episodeId` 隔离并恢复视口、折叠、筛选、选择和 active Agent session，恢复时重新校验 owner scope/revision/sequence/hash，绝不重发消息或付费 operation。
- 补齐 SoundCue 与导出交付合同：四类 cue 的 canonical 分类只使用 `track`，并冻结 `startFrame`、`durationFrames`、受限 trigger、0..100 priority、owner-only continuity refs、静态 gain 与 linear fades；任意 automation/keyframes 延后 MVP-B。导出失败返回 `ExportDiagnosticTarget`，`packaging` 内显式记录 `uploading|verifying|registering`，三个 artifact 分别上传、校验并登记。
- 将实际 2 GiB multipart resume/registration/Media Worker 链路纳入阶段退出证据；2 GiB 是验收 fixture 而非平台最大值，logical-size fake 只能用于快测，不能替代 actual-byte evidence。
- 新增 `implement-local-observability` child，以 W3C Trace Context、secret-free JSON logs、低基数 metrics 和可选本地 diagnostics profile 关联 Web/API/Outbox/Temporal/Worker/Provider/Storage/FFmpeg；telemetry 只观察 owner 事实，失败不得改变业务结果。
- 明确网络边界：MVP-A 默认与验收只监听 localhost/`127.0.0.1`；LAN 暴露、简单口令和反向代理认证属于 MVP-B，不得以无认证广域监听替代。

## Capabilities

### New Capabilities

- `phase-one-drama-mvp-a-integration-plan`: 定义阶段 1 MVP-A 的集成范围、共享工程契约、变更依赖、验证门和非目标。

### Modified Capabilities

无。

## Impact

- 受影响的后续规划范围：`scenes/shots`、`workflows/runs`、Provider/Model/Skill catalog、文本/图片/视频生成、素材编辑审核、项目资产中心、每集时间线/音频/导出与本地可观测性。
- 后续 change 将触及 `services/api`、`workers/*`、`packages/contracts`、`apps/web`、Alembic 和测试套件；本 change 本身不修改这些代码或运行配置。
- 真实 Provider、AgentScope、Temporal 与 FFmpeg 只可由各自后续 change 在显式 opt-in probe 中验证；默认开发和测试路径继续使用 `Mock Provider +` 显式 Local test/offline profile。真实 TOS 只由独立 change 在显式配置下接入，Local 不是 TOS 失败 fallback。

## 空系统到交付追溯

总体 change 追踪 `E2E-MVPA-001` 的 owner、前置、成功证据和失败诊断，从 `/projects` 经共享 project-scoped 壳层进入无模板入口的 zero-episode workbench、projects owner 的 original CreativeBrief 或 adaptation SourceMaterial binding、默认 published WorkflowVersion、Skill 人工路由裁决、Run 详情/显式取消/failed successor/指定历史 snapshot 新 Run、一次文本批量审核与 Project/Episode/Scene/Shot/AssetBible owner ack、AssetBible resolved snapshot/impact/task、显式 Episode 与隔离的 presentation/session state、完整 ShotCard、image/video AssetEdit execute/provider/candidate review、image candidate accept/reject、video Take accept/reject/retake、media inspection/derivatives、项目资产中心上传恢复及实际 2 GiB 链路/筛选/试听/usage/selector、显式重拍 `ReplaceClipSource`、typed eligible timeline assembly、完整 SoundCue、proxy/player、ducking、TimelineVersion 命名/preflight/显式发布/只读比较、项目 `EpisodeExportBatch` 的逐集 MP4/SRT/light upload/verify/register、失败定位、resilience 与 observability evidence。共享 UI 基线、300-node 只读投影和五个正式页面的组件复用由五个正式 UI child 协同验证，页面间证据必须来自真实导航而非直接 URL。它为 1 overall + 19 child，共 20 个未归档 changes；Mock `2x2x3` 和 explicit live `1x1x1` 的维度含义与默认 Playwright/真实 probe 边界在 design/spec/tasks 中一致。

`E2E-MVPA-001` 保留规范阶段 `S01`-`S11`，并新增不重编号的 `S03a historical rerun`、`S04a asset bible continuity`、`S08a asset center`、`S08b 2 GiB media chain` 与 `S11a observability`。原 focused failures 保留 `F01`-`F11`，新增 `F03a historical_snapshot_rerun_conflict`、`F04a asset_bible_impact_or_snapshot_conflict`、`F08a asset_center_upload_or_projection_failed`、`F08b media_2gib_capability_or_resume_failed` 和 `F11a trace_metric_or_redaction_failed`；每行必须同时记录唯一 owner、精确前置、成功 artifact/assertion、focused failure diagnostic 和失败时的 no-side-effect invariant。完整矩阵以总体 `design.md` 为规范来源，`spec.md` 与 `tasks.md` 只允许引用这些 ID，不得重新解释 owner 或跳过拒绝证据。

## 阶段一合同补充（DDD/BDD/SDD/TDD）

- **DDD**：GPT Image 结果先登记为未引用 `AssetVersion` candidate；scenes/storyboard eligibility owner 才能记录 accepted provenance、`candidateId`、AssetVersion id/revision/hash、target/project/episode。只有精确 CAS 接受后才成为 current storyboard reference。Text 上游编辑必须产生 successor candidate，并按依赖闭包标记旧 candidate/batch stale；StorageProfile/UploadSession/StoredObjectRef、AssetVersion 与 SourceMaterial binding 的 owner 不得混写。
- **BDD**：Agnes submit 在任何 `ProviderCall`/external submit 前校验 exact accepted image candidate/provenance/eligibility projection、`ShotSpec`、duration/aspect-ratio snapshot；未接受、stale、foreign、hash/revision mismatch 均零外部副作用。Text partial/stale/foreign/duplicate batch、StorageProfile CRUD/connection-test、SourceMaterial upload/ref/register/bind、资产中心 upload/resume/cancel/reconcile/usage projection 的失败恢复、catalog revision conflict、credential 不可解封和跨归属 artifact 下载均可观察拒绝。
- **SDD**：catalog 的 Provider/Profile/Model/Skill create/edit/enable/disable 使用 `expectedRevision`/`If-Match`，冲突返回 409 且零写入；Skill 内容变化只追加 immutable `SkillRevision`。Provider/Profile 按 operation 保存并发/限流与配额 snapshot，历史引用模型只能停用。Credential envelope 固定 AES-256-GCM 字段和 canonical AAD；Export MP4/SRT/light manifest 是三个独立 artifact；MVP-A transition 仅 `cut|crossfade`、30fps 整数帧，共用 canonical RenderPlan/compiler。
- **SDD**：Project、`creationMode`、CreativeBrief、项目设置和预算由 `projects` owner 持有；text owner 只读消费 validated CreativeBrief snapshot，并拥有 adaptation SourceMaterial、文本候选和 TextReview；AssetBible owner 独立持有 entry/version/override/resolved snapshot/impact/task。`original` 的 CreativeBrief/Run 不携带 source；`adaptation` 的 brief source snapshot 固定 project/source/brief IDs、revisions、content/payload hashes 和 parse/validation/binding status/version，Run snapshot 再固定 run ID/revision。`uploaded_file` 额外只引用已验证的 AssetVersion，`inline_text` 不创建 storage session、StoredObject 或 AssetVersion。TextReview 只追加完整 accepted handoff，含 candidate/source hashes、payload hash、实际引用的初始 AssetBible specs、expected revisions 和 correlation；Project/Episode/Scene/Shot/AssetBible 只能由各自 owner typed command/idempotent ack 落地，全部 ack 后才可进入媒体。Ducking 使用 `enabled`、合并后的整数帧 dialogue intervals、`attenuationDb`、`attackFrames`、`releaseFrames` 和 `targetTracks`，并由 canonical RenderPlan/FFmpeg filter graph 消费。
- **TDD**：总体验收必须包含上述拒绝矩阵、SourceMaterial parse/validation/recovery、rotate/re-encrypt/recovery、artifact 归属/TTL/hold、preview/FFmpeg parity 和 ducking 音频回归失败测试；全量 `openspec` strict/status/instructions 与 unchecked task 扫描是关闭证据。
