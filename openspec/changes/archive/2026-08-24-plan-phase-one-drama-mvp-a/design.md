## Context

### Local profile 选择合同

阶段一所有默认测试与离线开发场景的组合 SHALL 是 `Mock Provider` + 显式选择的 `Local test/offline profile`；该 profile 的 adapter identity 固定为 `local_workspace`。`LocalWorkspaceAdapter` 不是 TOS 失败时的临时降级路径：运行开始时必须冻结 Adapter/Profile（含 profile revision），TOS 或 Local 失败只能重试、reconcile 或返回原始诊断。页面加载、设置读取、刷新和视图切换 MUST NOT 隐式创建、启用、停用或切换 profile；真实 TOS 只能由用户显式选择的 enabled profile/probe 进入。

## 跨 owner 规范补充

`SkillRegistry` 的八项 candidate 为 `drama-skills`、`novel-writing`、`zy-cinematic-realism`、`seedance-2.0`、`storyboard-tiktok-video-skill`、`hell-grind/cinedance-higgsfield`、`hell-grind/acting`、`hell-grind/lira`。前两项记录 `provenance=verified_snapshot`、`approval=approved`、`enabled=true`；其他六项记录 `provenance=pending_provenance`、`approval=not_approved`、`enabled=false`。`drama-mvp-a-default` 仅固定绑定前两项 approved SkillRevision。候选集合不是 Worker 启动 lock 或默认 Run 前置；只有 node `allowedSkills`、`requiredCapabilities` 和 `selectionMode=fixed|inherit` 均通过后才渐进读取所选 revision。

catalog 必须冻结 `adapterInstalled`、catalog `approval`、成功 probe 后的 capability snapshot、`runnable`、`featureGate`。首次 connection-test/probe 只在 installed、`approval=approved`、`featureGate=MVP-A`、explicit live opt-in、已选 profile、可解析 credential 与 timeout 的显式命令下执行，成功后才写 snapshot，不能把 snapshot、`runnable=true` 或 disabled-for-run 作为自身前置。snapshot-missing/`runnable=false`/disabled-for-run 只阻断 enable/default、Run resolve 与 live invocation，后者再要求 installed、approved、成功 snapshot、`runnable=true`、`featureGate=MVP-A`。MVP-B/uninstalled/not-approved 或缺 opt-in/profile/credential/timeout 的 operation 不得 probe 或外部调用；TTS/ASR、MiniMax H3、Seedance 2.5 和 Agnes 未选中 mode 均不可运行。默认测试组合为 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），并保持 explicit live opt-in；运行开始后 Adapter/Profile 冻结。

MVP-A 只消费固定、版本化、已发布的 `drama-mvp-a-default` WorkflowVersion；不创建、读取或恢复任何 Draft identity，所有 Draft lifecycle 或 graph mutation 都是 MVP-B。

同一 Run、同项目、Schema-valid、hash/revision/scope 全部匹配的 provisional upstream candidate 是构建完整 Text candidate graph 的唯一 provisional 入口；accepted owner fact 仍可直接读取，其他输入必须拒绝。所有成员完成后只冻结一次 immutable `TextReviewBatch`，以 successor/stale closure 和全有或全无 CAS 收口，不引入第二次审核。视频固定为 verified Provider terminal result/storage validation -> immutable candidate + existing AssetVersion -> 人工 `accept` -> scenes exact current CAS -> MediaInspect/derivatives -> Timeline handoff；基础 result 安全验证仅为 download/MIME/checksum/size/duration/dimension/StoredObjectRef，不是 MediaInspect derivative generation。candidate/pending_review 不携带 derivative readiness 作为 accept gate；accept 后 Media Worker 才生成 metadata/proxy/thumbnail/keyframe/waveform，derivative `pending|failed|stale` 只阻断 Timeline/preview/export，绝不撤销 accepted/current。review 的 Schema、Signal、HTTP DTO、UI action 与 audit event 只允许 `accept|reject|retake`，legacy/unknown `approve` 零 current/retake side effect。verified Provider terminal success 是 result `AssetVersion` 唯一 append 时点；retry/reconcile 返回同一 version/candidate，AssetEdit `accept` 只写 AcceptDecision/audit 和同一 version 的 scenes eligibility CAS。

运行恢复还区分 failed successor 与历史 snapshot rerun：前者只复用同一失败链中可证明成功的 evidence，后者必须从用户明确选择的 immutable `RunInputSnapshot` 创建新 Run、新 `rerunOfRunId` 与全新的 `run_id + logical_operation`，不得重启历史 Run、默认采用 current 或复用 predecessor operation。Episode UI 的 viewport/collapse/filter/selection/active Agent session 只按 `projectId + episodeId` 保存 presentation references，恢复时重新校验 owner scope/revision/message sequence/selection hash；不得保存 owner 正文、跨集 fallback、重发消息或重复生成 Plan/Provider operation。

Timeline 的 `SoundCue.track` 是 `dialogue|music|ambience|effects` 唯一 canonical 分类；每个 cue 还冻结 `startFrame`、`durationFrames`、`trigger=manual|scene_start|shot_start|shot_end`、0..100 `priority` 和只指向 AssetBible/Scene/Shot/ShotSpec 的 `continuityRefs`。MVP-A 只支持 static gain 与 linear fades，不支持 automation/keyframes。ExportJob 八态不变；`packaging` 内以 `uploading|verifying|registering` subphase 暴露 MP4/SRT/light 各自 Storage upload/stat/checksum/MIME/size verify 和 owner registration，失败通过 `ExportDiagnosticTarget` 指向 `timeline|clip|caption|sound_cue|asset_version|renderer|storage|artifact`，unknown 先 reconcile。

阶段退出使用精确 `2_147_483_648` bytes 的实际媒体 fixture 验证 multipart interruption/resume、stat/checksum/MIME、单一 AssetVersion 和 Media Worker inspection/proxy；2 GiB 不是平台最大值，logical-size fake 不得替代 actual-byte evidence。`implement-local-observability` 只以 W3C Trace Context、secret-free logs、低基数 metrics 和可选 diagnostics profile 关联 owner 事实，telemetry backend 失败不得改变业务 readiness、状态、重试或付费语义。MVP-A 默认与验收只监听 localhost/`127.0.0.1`；LAN exposure、simple password 和 reverse-proxy auth 延后 MVP-B。

阶段 0、`projects/episodes`、`assets/asset-versions` 和 objectKey repair 已完成并归档。当前可复用的事实包括稳定 UUID、revision、不可变 `AssetVersion`、metadata/reference-only storage object、六个 Provider Port、`DeterministicMockProvider`、`LocalWorkspaceAdapter`、`SkillRegistry`/`SkillRouter`、基础 Draft 2020-12 Schema、Project/Episode 的 UoW/HTTP 测试边界，以及目标模块化单体架构。

阶段 1 的目标能力大多只有 Schema、ORM 占位或 Worker health 骨架，尚未实现完整 domain/application/repository/HTTP/Worker 切片。此设计仅定义后续实现的整合契约；它不声称这些能力已经存在。

## Goals / Non-Goals

**Goals:**

- 为一个总体协调 change 与十八个 child 建立可追踪的交付顺序，允许无共享写入的切片并行；总体协调不是 child 实施运行时必须先应用的代码依赖。
- 固定 `ProjectPackage` 的公共 manifest 与 `exportProfile=light|portable` 分层：MVP-A 只交付 `light`，其中只保存 manifest 和可解析引用；MVP-B 才在相同 manifest 上增加媒体载荷。
- 固定 MVP-A 只消费 `templateKey=drama-mvp-a-default` 的已发布 WorkflowVersion；后端只可 ensure/bootstrap 和冻结 source snapshot，通用工作流图编辑、连线、保存、发布和版本升级 command/API/UI 属于 MVP-B。
- 冻结 `creationMode=original|adaptation` 的 SourceMaterial 边界：`original` 以 CreativeBrief 的主题、题材、受众、人物设想、时长和风格创作且无 SourceMaterial 前置；`adaptation` 必须提交 `materialType=novel|synopsis|existing_script` 与 `inputMode=inline_text|uploaded_file`，再导入、解析、校验、绑定和恢复该 SourceMaterial。
- 在 Episode Timeline 内冻结对白自动压低参数，并让预览与 FFmpeg 使用同一 canonical RenderPlan。
- 让五个业务页面通过共享 project-scoped 壳层和显式 Episode/selection handoff 形成可返回的真实导航闭环，并补齐 Run 详情/取消、完整 ShotCard 与 TimelineVersion 命名/preflight/显式发布。
- 固定 300 个投影项可操作、普通 localhost API P95 `<500ms` 和桌面 Chrome/Edge 的可复算非功能退出证据；外部生成、长连接、媒体传输和渲染不进入普通 API 指标。
- 固定历史 snapshot 新 Run、Episode presentation/session isolation、完整 SoundCue、ExportDiagnosticTarget/packaging handoff、实际 2 GiB 媒体链路与本地 observability 的可复算退出证据。
- 统一 DDD、BDD、SDD、TDD 规则，并明确每个后续 change 的职责、依赖、数据迁移和验证门。
- 保持阶段 0 的版本、安全对象引用、`Mock Provider + 显式 Local test/offline profile` 测试组合和后端分层约束不被重复或削弱。

**Non-Goals:**

- 不在本 change 实现 Scene/Shot、Run、Provider catalog、AgentScope、GPT Image、Agnes、编辑审核、Timeline、FFmpeg 或 UI。
- 不要求在任一 child 开始实现前先应用、完成或归档本总体 change；总体 artifacts 只提供可追溯的协调约束。
- 不在本总体 change 接入真实 Provider、TOS 或生产凭据；真实 TOS 由 `integrate-tos-storage-provider` 独立实施，其他真实 Provider 仍只由各自 change 的显式 probe 实施。Fish Audio、Groq ASR、自动字幕对齐、专业 NLE、多人协作、移动端和发布平台不在 MVP-A。
- 不在总体 change 实现业务代码、Provider/TOS/FFmpeg adapter 或 UI；child owner 必须定义其所需的 HTTP path/error envelope、Run/Export 状态机、媒体编码白名单和通用 credential 安全实现。总体 change 只冻结跨 child 的不可变规则，不把未验证的 live profile 当作事实。
- MVP-A 不验收通用 Workflow graph editor、node/edge authoring、连线校验、draft save、publish 或版本升级 command/API/UI；这些能力只能作为 MVP-B 的后续 change。MVP-A 的工作流视图是固定 published WorkflowVersion 的只读运行/来源投影。
- MVP-A 不提供 SoundCue automation/keyframes，也不开放 LAN 监听、简单口令或 reverse-proxy auth；验收服务必须绑定 localhost/`127.0.0.1`，这些网络能力只能由 MVP-B 的独立安全 change 引入。

## Decisions

### 1. 以一个总体协调 change 与十八个 child 管理阶段 1

`plan-phase-one-drama-mvp-a` 只拥有总体 proposal/design/spec/tasks、矩阵、DAG、验收和待确认项。它提供协调与可追溯约束，不是 child 实施运行时必须先应用的代码依赖，也不要求 child 在其 tasks 完成或归档后才能实施。十八个 child 各自拥有其领域模型、接口、迁移、UI 或测试，不能将其他 child 的实体或运行时副作用纳入自身实现。

| Change | 总体任务号 | 唯一职责 | 前置依赖 | 并行/后置关系 |
| --- | --- | --- | --- |
| `plan-phase-one-drama-mvp-a` | 1.1-1.3、5.1-5.6 | 总体协调、追溯、共享约束与集成验收 | 无 | 不构成 child 实施运行时的代码依赖 |
| `extend-projects-episodes-creative-slice` | 2.0 | Project creationMode、CreativeBrief、项目创作设置/文本费用阈值、StorySpec current 与 Episode ScriptSpec current owner handoff | 已归档 Project/Episode | 与 AssetBible/scenes/workflows/catalog 并行；后置 text/workbench/run freeze |
| `implement-asset-bible-continuity-slice` | 2.1a | AssetBible typed entry/version、四层 override、resolved snapshot、impact analysis、ContinuityRevisionTask | Project/Episode + Scene/Shot owner query contracts + AssetVersion refs | 与 workflows/catalog 并行；后置 text handoff、图片、Agent、workbench |
| `implement-scenes-shots-storyboard-slice` | 2.1 | Scene/Shot、SpecVersion、AssetBible resolved snapshot reference、故事板/工作流双视图 | 阶段 0 Project/Episode/AssetVersion + AssetBible owner contract | 与 workflows/catalog 并行；后置文本、图片、视频、编辑、时间线 |
| `implement-workflows-runs-slice` | 2.2 | 固定默认 published WorkflowVersion/binding、run/node/event/outbox、受控 bootstrap、Temporal starter/Workflow/Activity | 阶段 0 Schema/架构 | 与 scenes/catalog 并行；后置文本、图片、视频、编辑、时间线；MVP-A 不含通用 graph mutation command/API/UI |
| `implement-provider-model-skill-catalog` | 2.3 | DB catalog、capability snapshot、SkillRevision、项目绑定、ProviderCall/usage 审计 | Provider/Skill foundation | 与 scenes/workflows 并行；后置文本、图片、视频、编辑、时间线 |
| `integrate-agentscope-text-skills` | 3.1 | AgentScope、Skill 路由裁决、TextModelPort、结构化生成、初始 AssetBible handoff 和确认 | project creative + AssetBible + scenes + workflows + catalog | 与图片/视频并行 |
| `integrate-gpt-image-provider` | 3.2 | GPT Image Adapter、AssetBible snapshot gate、能力校验、临时结果校验、StoragePort/AssetVersion | AssetVersion + AssetBible + workflows + catalog | 与文本/视频并行 |
| `integrate-agnes-video-provider` | 3.3 | Agnes submit/poll/cancel/result、冻结 snapshot、异步幂等、AssetVersion | AssetVersion + workflows + catalog | 与图片并行；后置编辑/时间线 |
| `implement-agent-asset-edit-review` | 3.4 | EditSession/Plan/Candidate、AssetBible snapshot/task gate、impact/stale/显式接受范围 | AssetBible + scenes + workflows + catalog + AssetVersion | 可与文本/图片/视频并行 |
| `implement-episode-timeline-audio-export` | 4.1-4.3 | 每集 Timeline/Cut/Clip、导入音频、MP4/SRT/light package | scenes + workflows + catalog + AssetVersion | 可与文本/图片并行；集成后验收 |
| `integrate-tos-storage-provider` | 3.5 | 真实 TOS `StoragePort` adapter、私有桶/短期 URL、显式 probe 与保留策略 | AssetVersion + catalog/security boundary | 与 UI 并行；Local 仍为测试/离线路径 |
| `implement-drama-creation-workbench-ui` | 6.1 | CreativeBrief、AssetBible 管理/影响/task、SourceMaterial、文本 Run/审核、Run 详情/取消、完整 ShotCard、共享 project-scoped 壳层和非功能 harness | project creative + AssetBible + scenes + workflows + catalog + text + StoragePort/AssetVersion | 后置 text；只读消费固定 published WorkflowVersion，不拥有后端事实或 graph editor UI |
| `implement-context-agent-candidate-review-ui` | 6.2 | Agent conversation/message/turn、AssetBible snapshot/task、从对话生成 Plan、primary selection、候选审查、CAS 接受和 Timeline replacement handoff | asset edit + AssetBible + scenes + workflows + timeline contract | 后置编辑审核；不泄漏会话上下文或自动改 Timeline |
| `implement-episode-timeline-editor-ui` | 6.3 | 每集 timeline、重拍 ReplaceClipSource、四类音轨、字幕、TimelineVersion 发布/比较与 `/projects/:projectId/exports` 多集逐集导出闭环 | timeline/audio/export | 后置时间线切片；发布只冻结 TimelineVersion，不是 Workflow/内容平台发布 |
| `implement-provider-model-skill-settings-ui` | 6.4 | Provider/Model/Skill 配置、候选 diff、费用确认与保留状态闭环 | catalog + provider/TOS changes | 后置 catalog；不保存或回显密钥 |
| `implement-project-asset-center` | 6.5 | 项目级上传恢复、目录筛选、Asset 元数据/版本/授权/派生状态、音频试听和只读使用位置闭环 | 已归档 Assets + TOS storage + MediaInspection/Derivative + 各引用 owner | 与其他 UI 并行；Timeline 素材箱复用其 selector/query，不复制 owner 事实 |
| `implement-operations-resilience` | 6.6 | 跨 Local/Worker/数据库/对象存储的 CPU、内存、容量/磁盘预检、软/硬阈值保护、拒绝诊断、手工备份恢复 runbook 与 checksum/ETag 恢复演练 | workflows + catalog/security + TOS object contract + timeline/export artifact contract | 与 UI 并行；不得把跨边界职责塞进 TOS adapter |
| `implement-local-observability` | 6.7 | W3C trace 传播、secret-free JSON logs、低基数 metrics、可选本地 diagnostics 与 E2E owner 对账 | workflows/outbox/Temporal + catalog/provider + Storage/TOS + Timeline/FFmpeg + operations owner contracts | 与 UI 并行；只观察 owner 事实，telemetry failure 不阻断业务 |

依赖 DAG 为：

```text
plan（协调/追溯，非实施运行时代码依赖）
 ├─ project creative ─┬─ text skills ──────────────┐
 ├─ AssetBible ───────┼─ image provider             ├─ drama creation UI
 ├─ scenes ───────────┼─ asset edit review ─────────┼─ context/candidate UI
 ├─ workflows ────────┼─ video provider             │
 ├─ provider catalog ─┤                              │
 ├─ AssetVersion ─────┘                              │
 ├─ TOS storage ──────── provider/settings UI        │
 ├─ AssetVersion + TOS + media derivatives ── project asset center
 └─ timeline/audio/export ── timeline editor UI <────┘
                           ├─ operations resilience
 workflows/catalog/TOS/timeline/operations ── local observability
```

替代方案是由一个大型 change 同时实现全部能力；因其会混合领域所有权、共享迁移和外部副作用，不能形成可验证的切片，故不采用。

### 2. 共享 DDD 规则

- 聚合所有权：`projects` 持有 Project、`creationMode`、CreativeBrief、项目级设置、文本费用阈值和 StorySpec current reference；`episodes` 持有 Episode 与 ScriptSpec current reference；`scenes` 持有 Scene、Shot、排序和 resolved snapshot reference；独立 AssetBible owner 持有 typed entry/version、override、resolved snapshot、impact 和 ContinuityRevisionTask。workflow 与 storyboard 是同一版本化事实的投影，不创建平行的真相来源。
- 发布、运行和导出只引用不可变版本。素材编辑必须生成 Schema-valid plan 和候选 `AssetVersion`，不得替换历史版本；接受时必须指定镜头、场、集或勾选范围，基础版本改变返回 409，已发布或历史版本只读。
- 每个写 Command 对应一个 UoW；领域写入、审计和 Outbox 在同一事务提交。Provider、Temporal、FFmpeg 等外部副作用只在提交后执行。
- 所有可重试 Activity 采用 `run_id + logical_operation` 幂等键；Temporal starter 使用稳定 Workflow ID 并显式处理 `AlreadyStarted`。事件在持久化后以单调序号发布，SSE 用 `Last-Event-ID` 补发。指定历史 `RunInputSnapshot` 的 rerun 必须创建新 Run、新 logical operations 和 `rerunOfRunId`，不得重启历史 Run、隐式 rebase/current upgrade 或复用 failed-successor evidence。
- 所有新增可交换结构带稳定 UUID、`schema_version` 与 revision；`schema_version` 是持久化与 manifest 的 canonical 字段。HTTP DTO 使用 `schemaVersion` 时只映射同一个 `schema_version` 值，不能形成双事实源。参数、模型、Skill 和 Provider 调用保存冻结版本或 capability snapshot。密钥仅在 Adapter 边界解密，接口和审计只呈现掩码。
- `projects` owner 的 `CreativeBrief` 必含六项创作语义 `subject`、`genre`、`audience`、`characterPremise`、`style`、每集目标 `episodeDurationSeconds`，三个精确产出计数 `episodeCount`、`scenesPerEpisode`、`shotsPerScene`，以及 canonical `schema_version` 与 revision；时长不是项目总时长。`StorySpec` 属于项目级，`ScriptSpec` 属于单集；Scene/Shot 是稳定 ID 实体，`ShotSpec` 是版本化事实。最小结构字段由 `id`、`schema_version`、`version`、所属稳定 ID、排序/编号、内容字段和不可变上游引用构成：Story 为 logline/characters/conflict/beats/continuity，Script 为 episode goal/conflict/scene order，Scene 为地点/时间/角色/道具/目标/情绪/对白/shot order，Shot 为 durationFrames/framing/camera/action/dialogue/first-last-frame/audio/continuity。MVP-A 的文本产物以这些结构化规格为终点，不生成小说正文或章节草稿。
- `AssetBible` 按 project -> episode -> scene -> shot 显式覆盖，由 AssetBible owner 生成 immutable accepted resolved snapshot；`ShotSpec`、Agent 会话、Run 与图片 operation 只冻结 snapshot ID/revision/hash 和 owner refs，不复制 entry/override。下游 stale 只创建 `ContinuityRevisionTask`/标记，不自动替换。
- 一次文本运行可把已通过 Schema 校验的上游候选作为同一运行内的 provisional input，直到生成完整 StorySpec、各集 ScriptSpec、Episode/Scene/Shot/ShotSpec 候选图及其实际引用的初始 AssetBible typed entry specs；逐对象校验不插入人工暂停。完整候选图冻结为一个 `TextReviewBatch` 后只执行一次必需的批量审核。批次内修改上游候选必须使依赖候选 stale，补齐并重新校验前不得接受；任何付费媒体 operation 在 TextReview 的 accepted handoff 与 Project/Episode/Scene/Shot/AssetBible 各自 typed command/ack 全部完成前暂停。批次接受必须携带完整 candidate IDs/hashes、stable target/reference IDs 与 expected revisions；文本 owner 在同一事务只追加 accepted handoff、candidate/TextReview 状态、审计与 Outbox，各 aggregate owner 以自己的 typed command/CAS/ack 落地 accepted 事实，旧版本、发布版本和历史运行只读。
- MVP-A storyboard 只允许 Scene 在同一 Episode 内排序、Shot 在同一 Scene 内排序；命令必须提交该唯一父 scope 的完整成员顺序与 expected revision，不能新增、删除或改变归属。storyboard insert/copy、Scene split/merge、Shot 跨场 move 和批量编辑属于 MVP-B；MVP-A 不暴露其 command/API/UI，兼容请求返回 `unsupported_feature` 且零 owner mutation。该边界不影响 Timeline 聚合内的 `SplitClip`。
- `creationMode` 与 CreativeBrief 归 `projects` owner；text owner 只消费已校验 immutable CreativeBrief snapshot，并拥有 adaptation SourceMaterial、文本候选与 TextReview。`original` 必须使用 snapshot 中的六项创作语义、三个精确计数、schema/revision 继续，无 SourceMaterial 前置或 snapshot；`adaptation` 必须以 `materialType=novel|synopsis|existing_script` 和 `inputMode=inline_text|uploaded_file` 创建 SourceMaterial。保存 CreativeBrief source reference 时冻结 `CreativeBriefSourceBindingSnapshot={projectId, sourceMaterialId, sourceMaterialRevision, sourceContentHash, creativeBriefId, creativeBriefRevision, creativeBriefPayloadHash, parseStatus, validationStatus, bindingStatus, bindingVersion}`；创建 Run 后冻结 `TextRunSourceBindingSnapshot` 的全部相同字段并增加 `runId`、`runRevision`。Run 分配前不得伪造 run identity，任一 snapshot 缺字段或 hash/revision/status 不匹配均不得生成；恢复只复用同一 source/brief revision，不隐式换源。
- 只有 adaptation 的 `inputMode=uploaded_file` 使用唯一 `sourceMaterialUploadKey=source-material-upload:{projectId}:{sourceMaterialId}:{sourceMaterialRevision}`。Text owner 创建冻结 projects owner 返回的 creation mode/brief revision、`materialType/inputMode`、source revision/contentHash 与 asset/version reservation 的 `SourceMaterialUploadIntent`；Storage owner 仅负责 UploadSession、MIME/size/SHA-256/ETag verification 和 immutable `VerifiedStoredObjectHandoff`；Assets owner 在自己的 UoW 以同一 key append 一次 AssetVersion；Text owner 再以 SourceMaterial revision CAS 绑定 `assetVersionId/revision/contentHash`。相同 key/fingerprint 重试返回同一 session/ref/version/binding，冲突返回 diagnostic；`run_id + logical_operation`（若存在）只能映射该 key。handoff 状态为 `uploading -> verified -> asset_registration_pending -> bound`，unknown 先 reconcile，registration 失败保留未引用对象/operation，不重新上传；binding 409/foreign/stale 不换源、不启动付费 Run。adaptation 的 `inline_text` 直接保存 immutable revision/contentHash、parse/validation，并产生上述精确 brief/run binding snapshots，不创建 storage session、StoredObject 或 AssetVersion。
- Timeline 的对白 ducking 归 Episode 聚合：持久化 `enabled`、从对白 SoundCue/显式区间解析并合并的 `dialogueIntervals`、正值 `attenuationDb`、非负整数 `attackFrames`/`releaseFrames` 和 `targetTracks=music|ambience|effects`。RenderPlan 将衰减量映射为目标轨道负增益；对白轨道不被 duck，重复/越界/非整数区间在 owner command 前拒绝。
- Agent 可执行编辑只覆盖 image/video `AssetEditPlan` 与候选审核；story/script 只能生成 TextReview successor/stale closure，audio/Timeline 只能提交 Timeline editor owner commands。没有 typed command、版本、权限、执行和恢复闭环时，任何 UI 或 Agent manifest 都不得声称支持对应类型编辑。
- MVP-A 的 image/video `AssetEditPlan` 只能绑定一个完整 base AssetVersion 和显式 reference AssetVersion 集合；不接受图片 mask/选区或视频/音频时间范围作为可执行 target。上述局部/时间范围编辑属于 MVP-B，兼容请求必须在 intent/ProviderCall 前拒绝。
- Provider/Profile 按 operation 保存并发上限、速率窗口/额度、429/`Retry-After` 行为与可查询 quota snapshot/status；admission 在外部调用前执行，未知配额不得伪报可用。Model 存在 CapabilitySnapshot、ProviderCall、Run 或项目/工作流历史引用时只能停用，不能物理删除。
- 项目资产中心扩展 Asset 目录元数据和 `AssetVersionReservation`，但 UploadSession/StoredObject、MediaInspection/MediaDerivative 及 Scene/Shot/Timeline/Export 引用仍由各 owner 持有。使用位置只聚合精确 owner query；owner 不可用时返回 partial/unavailable，不得伪报未使用。Timeline 素材箱复用该 selector/query。
- Timeline MVP-A 每个 Episode 只有一个 mutable current Cut，不创建 `TimelineDraft`，也不提供多个 mutable Cut 的创建、选择或切换；用户可发布、命名并只读比较多个不可变 TimelineVersion。command surface 固定为 `TrimClip`、`SplitClip`、`ReorderClips`、`DeleteClip`、`SetClipTransform`（仅静态 position/scale/opacity）、`SetSoundCueMix`、`SetDuckingPolicy` 和 `UpsertManualCaption`；均携带 `expectedRevision`，成功立即持久化 current Cut revision，409 为零部分写入并由客户端回滚后 refetch。关键帧、独立 autosave、undo/redo、version restore、subtitle style、Narration/TTS、loop、speed、track lock 均为 MVP-B 非目标。
- 已接受且 derivative ready 的重拍结果不得自动写 Timeline；Review 只交付 replacement handoff，Timeline owner 以 exact old/new source/eligibility/derivative/frame/revision 执行 `ReplaceClipSource`，成功只更新 current Cut，用户另行发布新 TimelineVersion。项目导出由 `EpisodeExportBatch` 接受显式 Episode + published TimelineVersion 集合，为每集创建独立 ExportJob 与 MP4/SRT/light artifacts；不得自动选择 current、扩大集合或跨集拼接。

### 3. Run 状态机已由 child 冻结

`implement-workflows-runs-slice` 已冻结 Run 状态机；总体 change 不再将其列为待确认。Run 只可为 `queued`、`running`、`waiting_review`、`succeeded`、`failed`、`cancel_requested` 或 `cancelled`，合法转移与 NodeRun 规则由该 child 的 `workflows-runs` spec 唯一拥有。总体层只复用其终态表：

| 类别 | 状态 |
| --- | --- |
| 成功终态 | `succeeded` |
| 失败终态 | `failed` |
| 取消终态 | `cancelled` |

`queued`、`running`、`waiting_review` 与 `cancel_requested` 均不是终态；总体 change 不增加新的转移或终态。

failed Run 的“从失败节点继续”不是状态转移，而是显式创建 successor Run。predecessor 永久保持 `failed`；successor 冻结新的 selection/input snapshot，只复用前驱成功节点的精确 owner evidence，待执行节点使用新的 `run_id + logical_operation`，不得重新收费执行 reused 节点。`submission_unknown` 仍必须先在原 Run reconciliation。

Skill 路由按 `deterministic_filter -> lexical_rank -> optional_semantic_adapter -> policy_decide` 生成带 revision 的 `SkillRouteDecision`。唯一高置信候选可直接冻结；并列、低置信或 semantic adapter unavailable 必须返回 `needs_human_selection`，用户只可从当前候选集合显式提交 `SkillRouteSelection`。选择完成前不得创建或启动 Run/NodeRun/TextModel/Provider，不得默认选择第一项。

### 4. 共享 BDD 与 SDD 规则

- 每项后续 capability 的 spec 至少有一个可观察的 `#### Scenario`，并覆盖成功路径与其关键拒绝/恢复路径。
- 所有跨项目/跨集引用、过期 revision、隐式范围扩大、重复逻辑操作、无效 Signal、能力参数不匹配、密钥回显、MIME/hash 不符、缺素材、越界裁剪和非整数帧都必须显式拒绝或可诊断失败，不能静默降级。MVP-A Agnes 只允许 submit/poll/cancel/result 与 `submission_unknown` reconciliation；callback/webhook 不属于本阶段。
- 接口、schema 和迁移采用 additive-first 兼容策略，直到对应 change 以证据冻结 HTTP path、错误 envelope 和数据映射；任何不兼容变更必须声明迁移和回滚策略。
- 新数据表或约束只能由拥有该聚合的 change 新增。每个 change 只创建自己的 Alembic revision，`down_revision` 以当时已验证 head 为准；不得重写归档 migration、不得将并行分支假定为线性 head。合并 revision 仅在集成时由拥有迁移边界的后续 change 明确提出、测试升级和降级路径后创建。

### 5. 共享 TDD 与运行策略

- 每个 change 先以 domain/application/adapter/integration/contract/BDD 测试表达它拥有的正反场景，再实现代码；完成后先运行定向测试，再运行 `pnpm run check`。
- 默认测试配置必须使用 `Mock Provider +` 显式选择的 Local test/offline profile（adapter identity=`local_workspace`）。真实 Provider、AgentScope 和 FFmpeg 只允许在显式 opt-in probe 中执行；未配置、不可用或未验证必须保留原始错误与受影响范围，且不得切换 profile。
- `ProjectPackage` 公共 manifest 必须声明 `schema_version`、version、`exportProfile`、选定 Episode/TimelineVersion、`AssetVersion` 引用、模型/Skill/参数/成本来源、音频结构、授权和响度报告。`exportProfile` 只可为 `light|portable`；MVP-A 仅实现 `light`，其仅含 manifest 与可解析引用，`portable` 仅由 MVP-B 在相同公共字段上增加载荷，不能改变引用或审计语义。不得并存 `profile` 与 `exportProfile` 两套事实。
- Provider 解析优先级固定为 workflow node override > project default > enabled system default；每个 Run 在启动时冻结选中的 Provider/Profile/Model/Skill/capability 参数 snapshot，缺失或禁用即失败，不隐式 fallback。`submission_unknown` 必须先 reconciliation，禁止盲目重提；取消后的晚到媒体结果登记为未引用候选，不成为当前引用。
- 所有付费 operation 使用逻辑 operation 幂等键、持久化 intent/ProviderCall 与费用确认；账本保存 native provider usage，未知成本显式为 `unknown`。catalog 拥有通用费用策略、确认记录和 ProviderCall，workflows 拥有 Run 级预算闸门；图片/视频批量操作必须在提交前确认，文本项目按冻结阈值超限时进入 `waiting_review`，`cost=unknown` 无论阈值均需明确确认，确认必须绑定同一 `run_id + logical_operation`。model sync 只产生候选 diff，须显式接受；通用 Provider Credential 使用 AES-256-GCM，Docker Secret 提供主密钥，缺少主密钥时真实 Provider 返回 503，`Mock Provider +` 显式 Local test/offline profile 继续可用。
- AgentScope 2.x 作为 Agent Worker 独立 runtime dependency，由依赖清单与 lock 管理，不放入 Skill vendor 目录。第三方 Skill 先经审计并按来源类型固定：Git Skill 使用 commit/digest，公开 Markdown Skill 使用 archive URL/获取时间/digest/license status；Worker 启动只读取 Registry index/approved metadata，路由后才按需读取 `SKILL.md` 和 references，绝不执行第三方脚本。必须留下 network、subprocess、file、secret 访问证据，任何未授权能力均拒绝。诊断保留 30 天，长期审计事实按 `retention_policy/version/hold` 保留；RunEvent、AcceptDecision、CapabilitySnapshot、ProviderCall 摘要和被引用 AssetVersion 受长期 no-GC 规则保护，本地审核/操作人使用稳定 UUID，不能以显示名代替。
- GPT Image 限制为最多 8 个 reference、合计 32 MiB、最大边长 8192，输入图片只允许 PNG/JPEG/WebP，edit mask 只允许 PNG；URL 必须命中配置 allowlist、禁止重定向、loopback、私网、链路本地、保留和 metadata service 地址。Agnes 在实施 probe 时优先从 v2.0 稳定候选中选择，但 probe 前不硬编码 model/mode ID；最终只冻结已配置账号实测通过的一个 image-to-video mode，并明确排除 2.5 preview。输入必须绑定当前 storyboard 的 AssetVersion、ShotSpec、显式 duration 和 aspect ratio；MVP-A 只保留 submit/poll/cancel/result 与 `submission_unknown` reconciliation，不实现 callback/webhook。
- MVP-A timeline 固定支持 9:16、16:9、1:1、1080p、30fps 整数帧、H.264 `yuv420p`、AAC 48 kHz 与 UTF-8 SRT。`light` 只包含 manifest 与可解析引用，MP4/SRT 单独输出且 MVP-A 不回导。音轨为 dialogue/music/ambience/effects；`SoundCue.track` 是唯一 cue 分类，并带 `startFrame`、`durationFrames`、受限 trigger、priority 与 continuity refs。仅支持静态 gain、mute、solo、线性淡入淡出、master limiter，任意 automation/keyframes 拒绝；交付目标为 -14 LUFS-I/-1 dBTP。ExportJob 状态仅为 `queued`、`preflighting`、`rendering`、`packaging`、`succeeded`、`failed`、`cancel_requested`、`cancelled`；`packaging` 内部 subphase 为 `uploading|verifying|registering`，不增加第九个状态。

## Current / Defined / Pending Matrix

| 范围 | 当前已实现 | 已定义但未实现 | 本阶段待后续 change 实现 |
| --- | --- | --- | --- |
| 基线 | Project/Episode、Asset/append-only AssetVersion、对象引用、revision、UoW、HTTP、Alembic `0001`-`0006`、`Mock Provider +` 显式 Local test/offline profile | 模块化单体、Ports/Adapters、Outbox/Temporal/Worker 边界 | 不重复阶段 0；仅复用契约 |
| 项目创作配置 | 基础 Project/Episode 已实现 | creationMode、CreativeBrief/设置版本、Story/Script current refs | projects creative owner typed handoff/ack |
| AssetBible | 无 | typed entry/version、override、resolved snapshot、impact/task | 独立 owner 与 Workbench/Agent/Image consumer contracts |
| 场景与分镜 | 基础 Schema/ORM 占位 | Project/Episode/Scene/Shot 关系与版本投影 | Scene/Shot、SpecVersion、AssetBible snapshot reference、同父 scope 排序、双视图 API；insert/copy、Scene split/merge、Shot 跨场 move 为 MVP-B |
| 工作流运行 | Worker queue 和 health Activity | run/node/event/outbox、持久事件与 SSE、确定性 Temporal 边界 | 发布、启动、取消、Signal、审核等待、恢复、SSE replay |
| Provider/Skill | 六个 Port、进程内 catalog、SkillRegistry/Router | DB catalog、snapshot、usage 审计 | catalog/项目默认、参数 schema、SkillRevision、ProviderCall |
| 生成 | Mock 边界 | Text/Image/Video Adapter 的目标边界 | AgentScope 文本、GPT Image、Agnes Video，均以 Mock 默认 |
| 编辑与导出 | Timeline 基础 Schema/ORM 占位 | 候选编辑、每集独立 Timeline、整数帧、ducking、受控 FFmpeg | Edit review、音频/字幕、MP4/SRT/light package |
| 本地可观测性 | 基础日志/health 输出 | W3C trace、secret-free log envelope、低基数 metric 与可选 diagnostics profile | `implement-local-observability` 关联 HTTP/Outbox/Temporal/Worker/adapter，且不成为业务事实源 |

## Risks / Trade-offs

- [PRD 同时并列 light 与完整包] → 采用端到端验收的明确时序：MVP-A 只实现 `light`，MVP-B 实现 `portable`；公共 manifest 提前固定以避免格式分叉。
- [PRD 将工作流图编辑误列为 MVP-A] → MVP-A 仅 ensure/freeze 固定已发布默认 WorkflowVersion 并提供只读运行/来源投影；通用 graph editor、连线、草稿保存、发布和版本升级 command/API/UI 归 MVP-B。
- [并行 change 竞争 Alembic head] → 每个聚合拥有独立 revision，禁止假设线性顺序；集成前实际核验 revision graph，并在需要时用单独 merge revision。
- [Provider 调用的费用和外部不确定性] → 以 capability snapshot、`run_id + logical_operation` 幂等键和 Mock 默认控制；真实调用仅显式 opt-in probe。
- [目标架构被误当作现状] → 矩阵显式区分“当前已实现”“已定义”“待实现”，每个后续 change 在实施前以代码和测试重新取证。
- [Schema/ORM/HTTP 映射不完整] → 由各拥有 change 先定义 additive mapping、迁移和负例测试；不得从现有占位字段推断最终契约。

## Migration Plan

1. 十八个 child 可按 DAG 独立创建和实施；本总体 change 的 artifacts 提供协调与追溯，不要求先应用、完成或归档其 tasks。并行 change 不共享目标目录和 Alembic revision。
2. 每个 change 在实现前重查实际 Alembic head、Schema 和相关 API，先完成所属迁移与测试，再接入 application/adapter/interface。
3. 在改变持久数据前，提供可测试的升级与降级路径；无法安全降级的数据变换必须在对应 change 中显式说明并请求确认。
4. 完成全部依赖后，执行全 change strict validation、定向测试与 `pnpm run check`；真实外部能力仍以显式 probe 的结果单独报告。

## 待提供的 probe 输入

- 真实 TOS、GPT Image、Agnes 的显式 opt-in profile、Docker Secret、账号许可、allowlist、provider 原始 usage 字段和 retention profile；未提供时必须保持 `unconfigured`，而非替代为成功。MVP-A 不需要 callback/webhook 认证输入。
- Agnes 已配置账号的 image-to-video 候选 probe 请求/响应、精确 model/mode ID、限制与取消语义；首选考察 v2.0 稳定候选，2.5 preview 不属于 probe 候选。
- 实际 `ffmpeg`/`ffprobe` 二进制路径与版本。Timeline 输出参数已冻结，未配置 renderer 时返回 `renderer_unconfigured`，不把 probe 缺失写成媒体成功。

## DDD / BDD / SDD / TDD

- **DDD**：总体 plan 仅协调十八个 child 的领域所有权与依赖，不成为运行时模块；projects 持有 Project/creationMode/CreativeBrief/项目设置/预算与 Story/Script current refs，AssetBible 独立持有 entry/version/override/snapshot/impact/task，text 只读消费 validated brief snapshot并持有 adaptation SourceMaterial/candidate/TextReview；AssetEdit 只拥有 image/video，TextReview 通过 accepted handoff + aggregate owner typed command/ack 落地，Timeline editor 保持独立 owner；Asset metadata/reservation、Storage session/object、Media derivative 与 usage query projection 分离；settings UI、operations resilience 与 local observability 的 owner 边界固定，telemetry 不拥有业务状态。
- **BDD**：验收覆盖审核暂停、Run 详情/显式取消及晚到结果、failed successor 与指定历史 snapshot 新 Run、Episode 状态隔离、完整 ShotCard、共享项目导航、TimelineVersion 命名/preflight/发布/409、完整 SoundCue、精确 CAS、无重复付费、StorageProfile CRUD/connection-test、SourceMaterial upload/ref/register/bind success/failure/reconcile、项目资产中心上传恢复/取消/实际 2 GiB/筛选/试听/usage、导出失败定位和 upload/verify/register、observability fail-open 与五个前端/业务闭环。
- **SDD**：本工件固定 19-change DAG、版本/Provider/媒体/安全契约、`sourceMaterialUploadKey` 与通用 `asset-upload` handoff/error/state matrix、TextReview/AssetBible owner handoff/ack、Timeline `ReplaceClipSource`/`EpisodeExportBatch` typed command surface、historical snapshot、SoundCue/ExportDiagnosticTarget、W3C observability 和 localhost-only 边界。
- **TDD**：每个 child 先覆盖其正反场景，集成以真实页面导航的 Mock 2x2x3、跨 owner SourceMaterial E2E、历史重跑、实际 2 GiB、trace/log/metric 对账、恢复、live 1x1x1、300 个投影项、普通 API P95、localhost bind、Chrome/Edge 与 strict validation 验收。

## Current / Defined / Todo

- **Current**：阶段 0 已实现；19 个阶段 1 change 均未实施。
- **Defined**：本总体 plan 的冻结契约、DAG、退出条件和 probe 输入。
- **Todo**：按 DAG 实施 18 个 child，并完成集成验证。

## 阶段一闭合矩阵

总体 DAG 的验收主线为 `E2E-MVPA-001`：project create/select -> project-scoped shell 与 zero-episode workbench -> original CreativeBrief 或 adaptation SourceMaterial -> fixed published Workflow/Skill route -> Run create/start/reconcile/cancel、failed successor 或用户指定 `RunInputSnapshot` 的新 Run -> 一次完整 TextReviewBatch 与全部 owner ack -> AssetBible snapshot/impact/task -> 显式 Episode 选择及隔离的 presentation/session state -> ShotCard/image/video candidate/AssetEdit/provider/review -> MediaInspect -> 资产中心普通上传与实际 2 GiB interruption/resume/register/inspect -> 显式 `ReplaceClipSource` -> 完整 SoundCue/static transform/caption/ducking -> TimelineVersion publish/compare -> EpisodeExportBatch 为每集分别 render、upload、verify、register MP4/SRT/light 并支持安全失败定位 -> resilience -> observability owner 对账。Workbench/Review/Assets/Timeline/Exports/Settings 必须由真实导航往返并保留 project/Episode/selection，不能用直接 URL 代替导航证据；MVP-A 服务只监听 localhost/`127.0.0.1`。Provider result 在 candidate 前只做基础安全验证，candidate/pending_review 不要求 derivative readiness；未经 video accept 或 derivative ready 不得进入 Timeline。每步记录 owner、exact prerequisite、success evidence、focused failure 和 no-side-effect invariant。

### E2E-MVPA-001 canonical stage evidence matrix

下表以原 `S01`-`S11` 加不重编号的 `S03a historical rerun`、`S04a asset bible continuity`、`S08a asset center`、`S08b 2 GiB media chain` 与 `S11a observability` 组成唯一验收矩阵。`success evidence` 必须是可读取的 owner response、持久化 snapshot、candidate/derivative 状态或导出 artifact；`failure diagnostic` 必须保留稳定 ID 和原始错误；任何前置失败均不得执行该行之后的外部副作用。

| 阶段 | Owner | 精确前置条件 | 成功证据 | 定向失败诊断 | 无副作用不变量 |
| --- | --- | --- | --- | --- | --- |
| `S01 project` | `projects/episodes` + Workbench/Review presentation state | 空数据环境；用户显式 create/edit/select；`If-Match` 与 project scope 有效；Episode 切换使用 `projectId + episodeId` | project id/revision、显式 selected project response 和共享 project-scoped shell；zero-episode 仅显示 original/adaptation；往返 Episode 后恢复其 viewport/collapse/filter/selection/active Agent session refs | `F01 project_scope_conflict` / foreign or stale `If-Match`、missing project scope、跨集 session/selection、stale sequence/hash 或模板入口出现 | 页面加载、深链解析、selection/state restore 或 zero-episode 不创建 Episode、WorkflowRun、RunEvent、ProviderCall、消息、Plan、模板或付费 intent；stale/foreign refs 被清除而非 fallback |
| `S02 source/brief` | `projects/episodes`（Project、creationMode、CreativeBrief、项目设置/预算）+ `integrate-agentscope-text-skills`（仅 adaptation SourceMaterial/binding、文本输入校验）+ `integrate-tos-storage-provider`（仅 adaptation uploaded_file 的 StorageProfile/UploadSession/verified ref）+ 已归档 Assets owner（仅 uploaded_file AssetVersion append） | `S01` selected project；projects owner 先保存 `creationMode`。original 的 valid CreativeBrief 含六项创作语义、三个精确计数及 schema/revision且无 SourceMaterial 前置；adaptation 必须为 `materialType=novel|synopsis|existing_script` 和 `inputMode=inline_text|uploaded_file` 的 valid SourceMaterial。uploaded_file 使用同一 `sourceMaterialUploadKey`、profile snapshot、asset/version reservation；parse/validation 可恢复 | original 读取 projects owner 的无 source CreativeBrief snapshot；adaptation inline 读取 `CreativeBriefSourceBindingSnapshot` 的 project/source/brief IDs、revisions、content/payload hashes、parse/validation/binding status/version，且无 storage session/StoredObject/AssetVersion；Run 创建后读取增加 run ID/revision 的 `TextRunSourceBindingSnapshot`；adaptation upload 还逐项读取 `uploadSessionId/operationKey`、verified `StoredObjectRef`（provider/profile/bucket/objectKey/status/checksum/ETag）、AssetVersion id/revision/contentHash/storage ref与 SourceMaterial bound facts | `F02 source_parse_or_validation_failed`、`F02a storage_upload_or_verify_failed`、`F02b asset_registration_failed`、`F02c source_binding_conflict` / invalid enum/scope/revision、foreign、invalid、stale、checksum/MIME/size、timeout/unknown 或 409 diagnostic | original 无 SourceMaterial 不得阻断 TextModel；adaptation 任一 source/upload/verify/register/bind/parse/validation 或 exact snapshot 失败不得启动 TextModel/付费 Provider。unknown 先同 key reconcile；失败 registration 保留未引用 object/operation，不 re-upload、不重复 AssetVersion、不换源；original 或 inline 的 upload intent 必须零 storage mutation；text 不得写 Project/creationMode/CreativeBrief owner state |
| `S03 workflow/run` | `implement-workflows-runs-slice` + `integrate-agentscope-text-skills`（Skill route） | `S02` 的 valid original CreativeBrief（无 SourceMaterial）或 valid adaptation Brief/source binding snapshot；用户显式 generate command；published `drama-mvp-a-default` 可用；Skill route 唯一或人工 selection 已确认 | immutable WorkflowVersion/SkillRouteDecision/Selection snapshot、Run/NodeRun ids、`run_id + logical_operation`；UI 可读节点安全摘要/耗时/最近事件/失败诊断、`cancel_requested|cancelled`；failed continuation 创建新 successor runId 并记录 reused evidence | `F03 workflow_unconfigured_or_source_conflict` / missing、non-published、route ambiguous/stale/non-candidate、stale binding、successor reuse mismatch、`run_cancel_conflict`、终态/foreign/stale cancel 或晚到结果 | page/view access 不 ensure；路由未裁决不创建 Run/NodeRun/TextModel/Provider；binding/snapshot/successor/取消失败不写额外 Run/NodeRun/ProviderCall，predecessor 保持 failed，晚到结果不覆盖 cancelled |
| `S03a historical rerun` | `implement-workflows-runs-slice` + Workbench UI | 用户显式选择同项目可读的 immutable `RunInputSnapshot`；snapshot owner refs/revisions/hashes 与所选历史 Run 匹配；预算/能力重新准入 | 新 runId、`rerunOfRunId`、原 snapshot identity 与全新的 `run_id + logical_operation` 集合；UI 在提交前展示精确历史输入且新旧 Run 均可读 | `F03a historical_snapshot_rerun_conflict` / foreign、stale、missing snapshot、隐式 current upgrade/rebase、operation reuse 或 budget/capability diagnostic | 不重启或改写历史 Run，不复用 failed-successor evidence/ProviderCall，不默认 current，不创建部分 NodeRun/付费 operation |
| `S04 text review` | `integrate-agentscope-text-skills` + Project/Episode/Scene/Shot/AssetBible owners | `S03` running Run；完整 Story/Script/Scene/Shot candidate graph及实际引用的初始 AssetBible entry specs 逐对象 Schema-valid；无 stale/partial/foreign member | immutable accepted `TextReviewBatch`、handoff 的 candidate/source hashes、payload hash、expected revisions/correlation 与 Project/Episode/Scene/Shot/AssetBible 全部 owner idempotent ack | `F04 text_schema_invalid_or_batch_stale` / successor closure、batch CAS 或 owner-ack diagnostic | batch 未 accepted 或任一 owner ack 缺失/失败均不创建或提交任何付费 image/video operation |
| `S04a asset bible continuity` | `implement-asset-bible-continuity-slice` | `S04` AssetBible ack；typed entries/current versions/assignments 有效；影响 owner projections 完整 | accepted resolved snapshot ID/revision/hash 与完整 override chain；impact actual Episode/Scene/Shot target set/hash；显式 successor AcceptDecision 和可查询 `ContinuityRevisionTask` | `F04a asset_bible_impact_or_snapshot_conflict` / incomplete、foreign、stale、set/hash/revision mismatch 或 pending task diagnostic | 失败不创建 successor/pointer/task/ProviderCall；旧 ShotSpec/current media/Timeline 保持原 snapshot且不自动重生成 |
| `S05 image candidate` | `integrate-gpt-image-provider`（transport/result）+ `implement-scenes-shots-storyboard-slice`（eligibility） | `S04` accepted batch 与全部 ack；`S04a` accepted resolved snapshot 无 pending task；image capability/selection/cost gate；project/episode/target 与 provenance/hash/revision 匹配 | 冻结 AssetBible snapshot 的 immutable image AssetVersion/candidate compare result；accepted storyboard eligibility exact CAS | `F05 image_unaccepted_or_provenance_mismatch` / continuity incomplete/stale/pending、foreign、hash or revision conflict | snapshot/Plan/candidate/current CAS 失败不创建 ProviderCall/外部请求或新 current storyboard |
| `S06 AssetEdit/provider` | `implement-agent-asset-edit-review`（intent/Candidate/reconcile）+ provider child（transport/result） | 已完成的 Agent 回复 turn；计划目标为图片时已接受 `S05` base/reference；通过 Schema 校验的 pending plan；冻结的 accepted AssetBible resolved snapshot ID/revision/hash 且无 pending continuity task；冻结的 base/selection/fee/capability snapshot；已绑定 Run/node/logical operation | 一项绑定同一 AssetBible snapshot 的 execution intent、ProviderCall/result handoff、一个不可变 result AssetVersion 和已登记 candidate；结果接受复用 scenes eligibility 的同一精确 CAS；unknown state 已完成 reconcile | `F06 asset_edit_plan_conflict_or_submission_unknown` / continuity incomplete/stale/pending，无效、stale、foreign plan，重复或未知提交 | snapshot/task/plan/fee/binding 不匹配不创建 intent、Outbox、ProviderCall、AssetVersion 或 candidate |
| `S07 video take` | `integrate-agnes-video-provider`（transport/result）+ `implement-workflows-runs-slice`（review/retake） | 已接受的 image eligibility；匹配的 ShotSpec/duration/aspect snapshot；通过 budget gate；已完成的视频结果和检查输入 | 不可变 `VideoTakeCandidate`，带 `accept\|reject\|retake`；接受 take 以精确 CAS 写入 scenes current-video；retake 使用 successor logical operation | `F07 video_take_rejected_or_retake_conflict` / stale take、foreign target、取消后的晚到结果或 CAS conflict | reject/retake/late poll 不覆盖 predecessor 或 current reference，且不进入 Timeline |
| `S08 media inspect` | 由 `implement-episode-timeline-audio-export` 拥有的 Media Worker | S07 accepted current exact CAS；existing AssetVersion/StoredObjectRef 的 source hash/revision 稳定；worker 可读取 bytes | `MediaInspection` 加上状态为 ready 的 proxy/thumbnail/keyframe index/waveform，以及绑定 source fingerprint 的 normalized metadata | `F08 media_inspection_failed_or_derivative_stale` / MIME、checksum、probe、worker retry 或 source mismatch diagnostic | failed/stale derivative 不得显示为 ready 或启用 Timeline handoff、preview、export，且不得改变已 accepted/current；重试必须幂等 |
| `S08a asset center` | `implement-project-asset-center`（目录/编排）+ Assets/Storage/Media/usage reference owners | `S01` selected project；显式 Local/TOS profile；有效 Asset metadata/reservation；owner query 可用或明确 partial/unavailable | 同一 reservation/operation 的 upload/resume/cancel/reconcile、单一 AssetVersion、筛选结果、ready/failed media projection、音频试听 grant、usage exact refs 与 Timeline selector handoff | `F08a asset_center_upload_or_projection_failed` / foreign、stale、duplicate、authorization、registration、derivative、grant 或 usage owner unavailable diagnostic | 页面加载/筛选/试听/usage 不创建 UploadSession、ProviderCall、RunEvent、AssetVersion 或 derivative；失败/取消/late result 不创建第二对象/版本或 Timeline 引用 |
| `S08b 2 GiB media chain` | `integrate-tos-storage-provider` + Assets owner + Media Worker + Project Asset Center | StorageProfile object/part limits 与 CPU/memory/capacity 支持精确 `2_147_483_648` bytes；actual-byte fixture、reservation 与 interruption point 已冻结 | 实际 streaming multipart interruption/resume、part manifest、stat/checksum/MIME/size verify、单一 AssetVersion、MediaInspection/proxy 及 profile revision evidence；logical-size fake 单独标记为快测 | `F08b media_2gib_capability_or_resume_failed` / profile limit、capacity、part mismatch、resume、checksum、registration 或 inspect diagnostic | 不支持时在 UploadSession/part/workspace read-write 前拒绝；恢复不重复 object/part/AssetVersion；不得把 2 GiB 声明为平台最大值或以 fake evidence 通过退出门 |
| `S09 timeline handoff` | `implement-episode-timeline-audio-export`（Timeline）+ `implement-scenes-shots-storyboard-slice`（current eligibility） | 已接受的 current video/image eligibility；`S08` 所有必需 derivative 均 ready；相同 project/episode/target；有效 30fps frame/ducking；current Cut revision/name 有效；SoundCue track/start/duration/trigger/priority/continuityRefs 有效；重拍替换还需 exact old/new source/eligibility/derivative fingerprint | Timeline Clip 引用 current immutable AssetVersion 和匹配 derivative fingerprint；重拍经用户确认 `ReplaceClipSource` 后只更新 current Cut；完整 SoundCue/static gain/linear fades；proxy/player parity；用户命名、preflight、显式发布新增不可变 TimelineVersion并只读比较 | `F09 timeline_foreign_or_unaccepted_media` / 未接受、跨 project/episode、stale、derivative-not-ready、old/new source mismatch、frame bounds、非法 trigger/ref/priority、双 cue 分类、automation/keyframe、非法名称、preflight failure、`timeline_publish_conflict` 或 409 diagnostic | accept 不自动改 Timeline；failed cue/assembly/replacement/publish 不修改 Clip/SoundCue/current Cut/既有 TimelineVersion，不创建新 Version 或提交 export |
| `S10 export` | `implement-episode-timeline-audio-export` + StoragePort owner | 用户显式选择非空去重的 Episode + published TimelineVersion 集合；全集合 canonical RenderPlan/artifact/authorization/storage preflight；有效 format/audio/subtitle/exportProfile=`light` | 一个 `EpisodeExportBatch`，每集独立 ExportJob；`packaging` 中记录 `uploading|verifying|registering`；MP4/SRT/light 分别经 upload/stat/checksum/MIME/size verify 后登记 ExportArtifact；失败返回可重新校验的 `ExportDiagnosticTarget` 与授权短期 grant | `F10 render_plan_or_export_failed` / duplicate/foreign/current-or-unpublished selection、命名、renderer、parity、upload/verify/register unknown、diagnostic target scope、artifact authorization、TTL 或 hold diagnostic | 全集合 preflight 失败零 batch/job；unknown 先 reconcile，不 rerender/duplicate/fallback；运行中单集失败不伪报整体成功，不自动重试/扩集/拼接，不暴露 objectKey/workspace URI或产生 portable payload |
| `S11 resilience` | `implement-operations-resilience` | `S01`-`S10` 与 `S08a` owner facts；已配置 CPU、内存、容量/磁盘 capability snapshot；版本化 manual runbook 输入；显式 drill fixture | resource/capability probe、soft warning、hard-threshold refusal diagnostic、restart/reconcile evidence、runbook fingerprint 和 checksum/ETag restore drill artifact | `F11 resilience_capacity_or_restore_failed` / CPU/memory/capability unavailable、hard limit、缺失 backup、checksum/ETag mismatch、foreign object、manifest drift 或 TOS-owner leakage | refusal/restore 失败不创建部分 intent、paid call、current reference 或 success state；TOS adapter 保持仅 transport |
| `S11a observability` | `implement-local-observability` | `S01`-`S11` owner facts；in-memory exporter 或可选 diagnostics profile；W3C context 与 log/metric allowlist 已冻结 | 一个文本 Run、image/video operation、multipart resume 与 export 的连续 root/child lineage、secret-free logs、低基数 metric delta 与 owner 对账；exporter unavailable 单独有 diagnostic | `F11a trace_metric_or_redaction_failed` / invalid header、parentage gap、secret leak、high-cardinality label、metric mismatch、viewer scope 或 exporter unavailable | telemetry 失败不改变 UoW/readiness/Run/Export/Provider/Storage/FFmpeg 结果，不创建重复业务事件、付费 operation 或第二事实账本 |

实现者必须在 E2E report 中按 `S01`-`S11` 加 `S03a`、`S04a`、`S08a`、`S08b`、`S11a` 逐行填入实际 owner response、前置 snapshot、成功 evidence 和 failure diagnostic；只给出链路截图或最终绿色状态不满足验收。

Mock 维度固定为 2 Episodes x 2 Scenes/Episode x 3 Shots/Scene；live 维度固定为 1 Episode x 1 Scene x 1 Shot，仅 explicit opt-in provider/storage/renderer probe，未配置记录 `unconfigured`。默认 Playwright 不使用真实 FFmpeg 作为 oracle；preview 与 final 只在 media adapter/probe 以 canonical RenderPlan parity 验证，目标为 SSIM >= 0.98、总时长/字幕边界/音频 onset-sync <= 1 frame，明确不承诺逐像素/逐采样、4K/专业监看/复杂特效。

### 共享前端与非功能退出证据

`implement-drama-creation-workbench-ui` 拥有共享 project-scoped shell 和非功能 harness，其他四个业务 UI 只消费导航/selection contract，不复制壳层或 owner state。Workbench/Review 的 Episode presentation slice 必须以 `projectId + episodeId` 隔离 viewport、collapsed scenes、status/model/review filters、Shot/Asset selection 和 active Agent session ID；恢复时重新校验 owner scope/revision/message sequence/selection hash，清除 stale/foreign refs，不保存 message/turn/Run/candidate/AssetVersion 正文、不跨集 fallback、不重发消息或操作。`E2E-MVPA-001` 必须从 `/projects` 经可见导航进入 Workbench、Review、Assets、显式 Episode Timeline 和项目设置并返回来源上下文；直接请求目标 URL 只可作为 route contract 的 focused test，不可替代业务导航证据。zero-episode 只显示 original/adaptation，固定 Workflow ensure 仍只由用户显式生成 command 触发，不存在模板入口副作用。

非功能报告使用确定性 Compose/PostgreSQL、Mock Provider 和显式 Local test/offline profile，所有 MVP-A 服务默认与验收均绑定 localhost/`127.0.0.1`；不得为满足 LAN 场景改为无认证广域监听。fixture 含 300 个 fixed published Workflow node、每个 node 的冻结 scope 及必要 Scene/Shot 关联，只覆盖其只读投影、筛选、选择、详情和页面切换，不恢复 MVP-B graph authoring。普通 API P95 `<500ms` 以 localhost HTTP 请求端到端耗时为准，报告记录 route、样本量、warm-up、数据量、环境、成功/失败数与 percentile；Provider/Agent/Temporal 等待、SSE、上传下载、媒体 probe/preview/render/export 均排除。桌面 Chrome/Edge 分别运行同一关键闭环并记录实际版本；任一浏览器、localhost bind 或原始报告缺失即未满足退出门。

### 六类合同统一落点

1. **Image -> storyboard -> Agnes**：GPT Image 成功仅产生未引用 candidate。eligibility owner 接受时必须原子记录 accepted provenance、candidateId、AssetVersion id/revision/hash、target/project/episode；精确 CAS 后才 current。Agnes command 在 ProviderCall/external submit 前重新读取 projection，并逐字段比较 candidate/hash/revision、ShotSpec、duration、aspect-ratio snapshot；任何 mismatch 直接拒绝且不写 ProviderCall、不发外部请求。
2. **Text successor closure**：上游 candidate 编辑产生 successor candidate，旧依赖闭包 stale。`regenerateStaleClosure`/`regenerateTextCandidate` 必须绑定 run/brief/batch revision、source candidate ids/hashes、expected revisions，并逐对象校验 schema/count/scope/hash；只成功生成新的 immutable TextReviewBatch，旧 batch/candidate 不变。partial/stale/foreign/duplicate 拒绝，接受 CAS 全有或全无。
3. **Catalog**：Provider/Profile/Model/Skill 均有 create/edit/enable/disable command/API；expectedRevision/If-Match 409 零写入。Skill 内容变更只能 append `SkillRevision`，旧 revision 只读，状态切换不覆盖 snapshot。
4. **Credential**：envelope 为 `algorithm`、`ciphertext`、12-byte `nonce`、16-byte `authTag`、`keyVersion`、`aadVersion` 和 profile/credential-bound canonical AAD；`(keyVersion,nonce)` 唯一。Docker Secret 是版本化 32-byte keyring；rotation/re-encrypt 带 cursor、可恢复幂等，旧 key 仅零 envelope 引用后退役。主密钥缺失的真实 provider 为 503 `credential_master_key_unavailable`，`Mock Provider +` 显式 Local test/offline profile 可用。
5. **Export**：MP4、UTF-8 SRT、light manifest 各自是 ExportArtifact，含 artifactId/type/status/object ref/retention/license。下载按 Project->Episode->TimelineVersion->ExportJob->Artifact 完整归属与稳定本地 actor UUID/project policy 授权，使用短 TTL read-only grant，不暴露 objectKey/workspace URI；foreign/expired/held/unauthorized fail closed。
6. **Transition/Ducking**：MVP-A 只实现 `cut|crossfade`，duration 有上下界，Clip adjacency/overlap 不变量在 preview 和 export 同一 canonical RenderPlan/compiler 校验；对白 ducking 以合并后的整数帧区间、衰减量、attack/release 和目标轨道写入 RenderPlan/FFmpeg filter graph，parity/音频回归失败不得报告成功。wipe/mask/auto/keyframe/audio crossfade 属于非目标。

### MVP-A scope closure and resilience handoff

- 固定默认 Workflow、SourceMaterial import、一次文本批量审核、Provider/Model/Skill 管理、费用确认、ducking 与 MP4/SRT/light 导出均保留。固定 Workflow 只读消费 `drama-mvp-a-default` published source；页面加载、视图切换和 selection 零 workflow mutation。
- Agent 的 image/video `AssetEditPlan` 仅在 owner 的 typed command、版本、execution/reconcile 和 candidate review 全部存在时可执行。story/script 继续使用 TextReview successor/stale closure，audio/Timeline 继续使用 Timeline editor typed commands。UI 不得将只读 selection 写成可编辑能力；新增类型必须先有独立 owner contract 后才可实施。
- Timeline 编辑只保留 static transform、trim/split/reorder/delete、四类音轨、静态音量、mute/solo、fade、ducking 和手工字幕 text/time。每个成功 command 立即持久化，409 回滚/refetch；关键帧不得以静态 transform 名义进入 MVP-A。
- Timeline UI 必须提供 current Cut 的 TimelineVersion 命名、owner preflight、显式 publish 与只读比较；失败零版本写入，publish 不等于 Workflow 发布或内容平台分发。
- 五个业务页面由共享 project-scoped shell 串联；zero-episode 只提供 original/adaptation，无 MVP-B 推荐模板入口。Run 详情/取消、完整 ShotCard 和真实导航均属于 MVP-A，而不是 presentation-only 非验收项。
- 以下全部是 MVP-B 非目标：故事板插入/复制/批量生成/批量重拍/批量审核、Timeline 独立 autosave、undo/redo、version restore、subtitle style、Run pause、review comments/timecodes/reminders、Narration/TTS、loop、speed、track lock、完整工程包回导。
- SoundCue arbitrary automation/keyframes，以及 LAN exposure、simple password、reverse-proxy auth 同属 MVP-B；MVP-A 只允许 static gain/linear fades 和 localhost-only 运行。
- 项目资产中心是 MVP-A 第五个前端/业务闭环；它负责上传恢复、目录、筛选、授权、版本、派生状态、试听和只读 usage，不实现自动语义/视觉质检、统一审核中心、物理删除/GC、语义搜索或批量标签。
- `implement-operations-resilience` 是独立 cross-boundary child：它协调 Local workspace、Worker temporary/derivative、数据库 backup metadata 与 object storage manifest/reference 的磁盘阈值与恢复规则。TOS adapter 只实现 StoragePort/Profile/object 生命周期，不能吞并磁盘阈值、全局拒绝语义、备份 runbook 或恢复演练。

### MVP-A media stages and ownership closure

默认 workflow 的媒体阶段按 `media.generate.image|video`、`media.review.image|video`、`media.inspect`、`timeline.handoff` 拆分；兼容父 `media.generate` 只作为 logical operation grouping，不拥有子节点事实。AssetEdit owner 负责 execute intent/reconcile/Candidate，Provider child 负责 submit/poll/cancel/result transport，Workflow owner 负责 review signal/RunEvent，Scenes owner 负责 accepted current eligibility，Media Worker 负责 canonical inspection、proxy、thumbnail、keyframe index、waveform，Timeline 只消费 accepted current + ready derivative。未经 video accept 或 derivative ready 不得进入 Timeline。

文本真实 adapter 归 `integrate-agentscope-text-skills`，Codex relay 默认、DeepSeek opt-in，必须通过 OpenAI-compatible `/v1/models` diff、认证、bounded retry、structured response parsing 与 explicit probe；StorageProfile/Bucket/Region/Endpoint、连接测试、启停和 masked credential status 属 `integrate-tos-storage-provider`/catalog/settings UI 的 MVP-A 范围，默认测试使用 `Mock Provider +` 显式 Local test/offline profile，真实 TOS 只显式 opt-in。

### Local observability closure

`implement-local-observability` 以 W3C `traceparent`/`tracestate` 把 Web -> FastAPI -> Outbox -> Temporal -> Worker -> Provider/Storage/FFmpeg 关联到同一 lineage，并输出 allowlisted secret-free JSON logs 与低基数 metrics。它只保存安全 trace/correlation reference，不复制 RunEvent、ProviderCall、usage/cost、ExportJob 或 AssetVersion；可选 collector/viewer/exporter 失败必须 fail-open for telemetry，不能改变业务 readiness、重试、状态或费用。`S11a` 同时证明 parentage、脱敏、metric delta、owner 对账与无业务副作用。
