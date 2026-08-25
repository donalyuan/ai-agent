## Context

Project/Episode 与 Asset/AssetVersion 已实现版本、归属、UoW 和不可变存储引用。`TimelineDocument` 仅有 Draft 2020-12 Schema/ORM 占位，没有 domain、application、API、音频或导出链。所有写入遵守一个 Command 一个 UoW 和同事务 Outbox；FFmpeg 是 commit 后由 media worker 调用的受控外部副作用。

## Goals / Non-Goals

**Goals:**

- 每个 Episode 维护唯一 mutable current Cut、多个可命名的不可变 TimelineVersion 和按整数帧排列的 Clip；成功 typed command 立即持久化 current Cut revision。
- 支持导入同集 dialogue/music/ambience/effects，持久化 `SoundCue`、轨道、静态 gain、mute、solo、线性淡入淡出、对白 ducking、master limiter（-14 LUFS-I/-1 dBTP）与手工字幕。
- 在导出前验证素材引用、帧边界、时间线完整性和 render 参数；通过受控真实 `ffmpeg`/`ffprobe` adapter 的显式 probe 生成独立 MP4、UTF-8 SRT 与 `ProjectPackage` `light` manifest。
- `ProjectPackage` 只使用 canonical `exportProfile` 字段，枚举为 `light|portable`；MVP-A 仅接受 `light`，MP4/SRT 单独输出且不回导，并把持久化/manifest `schema_version` 与 HTTP DTO `schemaVersion` 映射为同一个版本值。
- 保存 export 的输入 `TimelineVersion`、AssetVersion 引用、配置、输出引用、审计与失败状态，使重复提交可按 `run_id + logical_operation` 幂等；支持显式 Episode/TimelineVersion 集合的项目级批次，但每集独立渲染和输出。

**Non-Goals:**

- 不实现 `TimelineDraft`、多个 mutable Cut 的创建/选择/切换、MVP-B `portable` payload、Fish Audio、Groq ASR、自动字幕/自动对齐、专业 NLE、回导、发布平台或跨集自动拼接。
- 不接受 `profile`、`export_profile` 或其他 `exportProfile` 别名，不允许 DB、manifest 和 HTTP 各自维护不同的 schema version 来源。真实 renderer 不作为默认路径，但 adapter/probe 是本 change 的必需交付；未配置时必须为 `renderer_unconfigured`。MVP-A 已冻结 9:16/16:9/1:1、1080p、30fps、H.264 `yuv420p`、AAC 48 kHz、UTF-8 SRT、四类音轨、响度目标与 ExportJob 状态。

### 0. 总体计划追溯与实施前置

本设计反向追溯至 `plan-phase-one-drama-mvp-a` 的总体任务 `1.2`、直接实施任务 `4.1`--`4.3` 和共享任务 `5.1`--`5.5`。实施必须以总体任务 `2.1`（scenes/shots）、`2.2`（workflows/runs）、`2.3`（provider/model/skill catalog）完成且已归档 AssetVersion 契约可供取证为前提；这些前置提供来源解析、版本审计和同项目/同集校验。

总体 plan 只表达 OpenSpec 的协调、依赖顺序和验收，不是 runtime module。Timeline/export 代码不得为满足追溯而导入总体 plan 或其他 child change；缺少前置事实、请求 MVP-B `portable`、隐式扩大至多集或请求跨集拼接时必须显式阻塞/拒绝。只有本设计定义的显式 Episode + TimelineVersion 集合可进入多集逐集导出。

## Decisions

### 1. Episode is the timeline ownership boundary

current Cut、`TimelineVersion`、`Clip`、`SoundCue`、字幕和 ExportJob 均有 project/episode 外键。Episode 与 current Cut 是一对一聚合关系；系统不创建可并存或可切换的其他 mutable Cut。发布 Version 是带名称的不可变快照，允许读取和只读比较多个 Version。替代的项目级混合时间线无法阻止跨集素材泄漏，故拒绝。

Timeline owner command contract 固定为 `TrimClip`、`SplitClip`、`ReorderClips`、`DeleteClip`、`ReplaceClipSource`、`SetClipTransform`（仅静态 position/scale/opacity）、`SetSoundCueMix`、`SetDuckingPolicy` 和 `UpsertManualCaption`。每个 command 携带 `expectedRevision`，成功在同一 UoW 立即持久化 current Cut revision；409 `revision_conflict` 必须零部分写入并返回 current revision，调用方回滚乐观状态后 refetch。

`ReplaceClipSource` 只接受一个精确 clipId、expected old AssetVersion id/revision/hash 与 derivative fingerprint，以及同一 project/Episode/Shot 的新 accepted-current eligibility、new AssetVersion id/revision/hash 和 ready derivative fingerprint。替换保留 Clip stable ID、timelineStartFrame、durationFrames、静态 transform、transition 与相邻关系；如果新 source 无法覆盖原 `sourceInFrame + durationFrames`，preflight 失败且不自动裁剪/拉伸。成功只更新 current Cut 的 source reference 并递增 revision，既有 TimelineVersion 永久不变；用户必须另行命名/preflight/publish 新 Version。AssetEdit accept、VideoTake accept、Provider 或 scenes owner 均不得直接执行该 command。

### 2. Frame arithmetic is integer-only

所有 `startFrame`、`durationFrames`、`inFrame`、`outFrame`、排序键和字幕时间转换都使用 30fps 的非负整数帧；裁剪、拆分、排序的 application 操作保留父 Clip 来源和 revision。浮点秒仅用于边界展示，不能进入持久化编辑计算。非整数、负值、零长度、越过素材可用帧、轨道重叠和素材不足均显式失败，绝不插黑。

### 3. Audio and manual captions reuse immutable assets

Timeline 的音频/媒体选择入口必须消费 `implement-project-asset-center` 的 project-scoped selector/query。handoff 只含 project、AssetVersion id/revision/hash、authorization summary 和 derivative fingerprint；Timeline 不复制资产中心的 UploadSession、filter、Asset metadata revision、usage 或 MediaInspection/MediaDerivative 状态，最终 Clip/SoundCue 仍由 Timeline expectedRevision command 创建。

导入的 dialogue/music/ambience/effects 均引用既有同项目/同集可用 `AssetVersion`；`SoundCue.track` 是 PRD `cueType` 的唯一 canonical 分类事实，值仅为这四类，API 不同时接受第二个 `cueType` 别名。每个 cue 记录角色、`startFrame`、`durationFrames`、trigger、0--100 priority、去重有界 `continuityRefs`、静态 gain、mute、solo、线性淡入淡出与授权元数据。trigger 只允许 `manual|scene_start|shot_start|shot_end`：manual 直接使用 startFrame；其他类型必须带同 Episode 的 scene/shot stable ID/revision 和整数 `offsetFrames`，在 command UoW 前解析为冻结 startFrame，目标变化时返回 stale，不静默移动 cue。priority 只决定同轨重叠 cue 的确定性渲染顺序，不自动删除、接受、duck 或改变 gain。continuityRefs 只保存 AssetBible/Scene/Shot/ShotSpec owner ID/revision/hash 引用，不复制内容或触发自动修订。MVP-A 的“音量包络”只等于静态 gain + 线性 fade-in/fade-out，任意 automation points/keyframes 明确拒绝。master limiter 的交付目标为 -14 LUFS-I/-1 dBTP。对白 ducking 作为 Episode timeline-owned mix policy 保存 `enabled`、合并后的 `dialogueIntervals`、正值 `attenuationDb`、非负整数 `attackFrames`/`releaseFrames` 和 `targetTracks`；只允许压低 music/ambience/effects，不压低 dialogue。手工字幕由 cue/clip 帧边界转换为 UTF-8 SRT；不会调用 ASR 或自动对齐。替代的嵌入媒体 bytes 违反 AssetVersion 存储边界，故拒绝。

### 4. Controlled rendering and light package

在既有 renderer 预检之上，显式 probe 必须分别验证实际 binary version、H.264 decoder/encoder、AAC decoder/encoder、`yuv420p` pixel format 和 MP4 mux/demux/container support，并冻结 capability snapshot。缺失任一项返回 `renderer_capability_unsupported`，与 `renderer_unconfigured` 一样在 PreviewArtifact 或 ExportJob 成功副作用前阻断；不得只凭 `ffmpeg`/`ffprobe` 可执行文件存在判断 renderer 可用，也不得改用其他 codec/container。

application 先执行纯预检，并在创建 Job 的同一事务冻结不可变 `ExportExecutionSnapshot`：精确 TimelineVersion、每个输入 AssetVersion 的 id/revision/hash/StorageObjectRef、已解析 StorageProfile id/revision/snapshot hash 与 StorageCapability、renderer capability、完整 generation provenance、实际参数和 usage/cost source。客户端不得提交或覆盖这些 owner 审计事实。每个初始 Job 和显式 retry Job 都必须在同一事务追加独立、可持久恢复的 `export.job.dispatch.requested` outbox；dispatcher 以 `project + batch + job + logicalOperation` 生成稳定 workflow ID，Temporal `episode_export` workflow 在 `media-tasks` 调用实际 export activity。重复 dispatch 只复用同一 workflow，不能创建第二执行。

media activity 只从冻结 snapshot 解析输入，通过 `StoragePort.iter_chunks()` 流式物化到按 Job 隔离的临时目录，再经 `FfmpegRenderPort` 调用真实 `ffmpeg`/`ffprobe` 的结构化白名单参数、输入/输出校验和执行后 loudness measurement；Mock/unconfigured adapter 仅用于测试/未配置诊断，显式 probe 必须验证实际二进制，绝不静默回退。输出固定 9:16/16:9/1:1、1080p、30fps、H.264 `yuv420p`、AAC 48 kHz 与 UTF-8 SRT。`ProjectPackage` 只接受 canonical `exportProfile`，值域严格为 `light|portable`：MVP-A 的请求和 manifest 只能为 `light`，`portable` 留给 MVP-B；`profile`、`export_profile` 和其他别名一律 validation 拒绝。

`light` manifest 必含且不得以空值省略 `schema_version`、manifest version、`exportProfile`、选定 Episode/TimelineVersion、可解析 AssetVersion 引用、音频结构、每项素材/音频的 authorization/license provenance、renderer 实测 loudness、完整去重的 `models[]`（Provider/Profile/Model/CapabilitySnapshot）与 `skillRevisions[]`、实际 parameters、cost/usage value/status/source；`cost=unknown` 也必须记录明确状态与来源。一个 Timeline 可引用多个生成来源，故不得选取“第一个/最新” Model 或 Skill；每个 provider-generated 输入必须从 accepted candidate -> ProviderCall/VideoOperation -> WorkflowRun selection snapshot 逐项追溯，无法追溯、状态不一致或使用未批准 Skill 时 preflight fail-closed。用户上传输入可以没有 generation provenance，但不能为 generated 输入伪造 Mock/Local 审计字段。任一必填字段缺失、未知或与引用 snapshot 不一致时 Schema/preflight 失败；manifest 不内嵌媒体，MP4/SRT 单独输出且不回导。

每个预检失败项使用 `ExportDiagnosticTarget` 指向同项目 owner fact：`targetType=timeline|clip|caption|sound_cue|asset_version|renderer|storage|artifact`，并携带适用的 Episode/TimelineVersion 与精确 owner ID/revision、可选 frame/fieldPath 和 owner 验证的 route token；调用方不得从 message 文本猜测位置。全局 renderer/storage 错误指向对应项目设置 section，Clip/Caption/SoundCue/AssetVersion 错误指向只读定位或可编辑 current Cut，已发布 Version 保持只读。

ExportJob 八态不增加 `uploading`。渲染完成后进入 `packaging`，其 progress subphase 明确为 `uploading|verifying|registering`：每个 MP4/SRT/light 输出通过 StoragePort 以 export operation key 上传到已冻结 profile，随后 stat/checksum/MIME/size 校验，再由 Timeline/export owner 追加独立 `ExportArtifact`。三个 artifact 全部 verified/registered 且 light manifest 引用精确匹配后才可 `succeeded`；unknown 先 reconcile，任一上传/校验/登记失败保持可诊断 `failed` 或 retryable owner state，不伪造 artifact、不重渲染或切换 Local/TOS。

artifact upload 必须先流式计算完整 size/checksum，再按冻结 capability 选择 `[minPartSize,maxPartSize]` 内的 part size 并执行 admission；每个 receipt 的 checksum/ETag/size 必须来自该分片实际 bytes，part 数和大小不得超过 capability。Worker restart 先以相同 operation key、intent 和完整 receipt manifest reconcile/resume；禁止 `Path.read_bytes()`、整文件 bytes 聚合或超限单 part。

项目级多集导出使用 `EpisodeExportBatch`，请求必须提供去重且非空、按用户明确顺序排列的 `{episodeId,timelineVersionId,timelineVersionRevision}` 集合、每集 output base name、统一 export settings、batch expected revision 和幂等键。application 在创建任何 ExportJob 前验证全部成员同项目、TimelineVersion 已发布且 immutable、名称唯一且安全、artifact/renderer/authorization preflight 全部通过；任一失败则零 batch/job/outbox。成功后为每个成员创建独立 ExportJob 和独立 MP4/SRT/light artifacts，文件名包含稳定 episode number/id 与 version identity，不产生合并 RenderPlan 或跨集 artifact。提交后的单集失败不回滚其他已完成集，batch 只汇总逐集 `queued|running|succeeded|failed|cancelled` 和 `succeeded|partially_failed|failed|cancelled`，且 retry 只能针对用户明确选择的失败 member 新 logical operation。

请求还必须显式提供目标 StorageProfile id/revision；application 通过 StorageProfile owner 校验项目归属、enabled/private binding/credential 与实际 revision，并冻结 capability，不接受客户端自报 snapshot。retry 复用原 Job 的冻结 execution snapshot，只替换 Job/artifact/dispatch identity，不重新选择 profile、model、skill 或 TimelineVersion。

导出持久化记录和 manifest 的 `schema_version` 是同一个 canonical 值；HTTP DTO 的 `schemaVersion` 仅为该值的命名映射，不能成为第二来源。创建、读取和导出预检均须验证三处一致；任一不一致、缺失或试图分别赋值时返回 validation，且不写入 ExportJob/manifest。

### 5. Additive API and failure semantics

接口使用 `/v1/projects/{projectId}/episodes/{episodeId}/timeline...` 与 `/exports` 的 additive 资源。ExportJob 状态仅为 `queued`、`preflighting`、`rendering`、`packaging`、`succeeded`、`failed`、`cancel_requested`、`cancelled`。错误保持稳定 envelope：validation、not_found、episode_mismatch、revision_conflict、frame_out_of_bounds、missing_asset、render_preflight_failed、renderer_unconfigured、render_failed；基础/当前版本冲突为 409。响应不含媒体 bytes。

## Data and API Contract

迁移新增每集一对一 current-cut、version/clip、audio track/cue、caption、ducking policy、export job/artifact 表及外键、Episode-current Cut 唯一约束、稳定排序和枚举/check constraints；不得创建 draft 表或支持多个 mutable Cut 的关系/状态。export 持久化记录以 `schema_version` 保存 canonical 版本。JSON Schema 至少覆盖 current Cut、TimelineVersion、Clip、SoundCue、DuckingPolicy、manual subtitle、ExportRequest/Result、ProjectPackage light manifest；DuckingPolicy 的 intervals 必须是合并后的 30fps 整数帧，targetTracks 仅允许 music/ambience/effects，`additionalProperties: false` 拒绝未知字段。ProjectPackage 只允许 `exportProfile` 枚举 `light|portable`，MVP-A Schema/请求只接受 `light`，以 `required` 固定 authorization、loudness、Model/Skill/parameters/cost source 等全部审计字段，并以 `additionalProperties: false` 拒绝 profile 别名和未知字段。HTTP DTO 以 `schemaVersion` 映射同一 `schema_version`。所有输入禁止额外字段并引用 UUID/整数帧。依赖为既有 Project/Episode/AssetVersion/Storage/UoW/Outbox 和新增 FfmpegRenderPort/media worker；不新增 Provider Adapter。

## Risks / Trade-offs

- [素材不足或 render 参数漂移] -> 固定 9:16/16:9/1:1、1080p、30fps、H.264 `yuv420p`、AAC 48 kHz；preflight 拒绝缺素材或不符合固定参数，不插黑或猜测默认值。
- [ducking 区间或滤镜语义漂移] -> 只接受合并后的整数 frame interval、正 attenuation、非负 attack/release 和明确目标轨道；preview/FFmpeg 从同一 RenderPlan 编译，音频回归失败则阻断导出。
- [FFmpeg 与安全] -> command 由 port 从结构化参数构建，不接受 shell 片段；隔离临时目录、校验输入/输出 MIME/hash/时长、限制资源和记录原始 stderr。
- [Scene/Shot 依赖未实现] -> Clip 可先引用已存在 AssetVersion 与可选稳定 source metadata；待依赖就绪后收紧 resolver，不伪造 Scene/Shot。
- [重拍来源时长不足或 revision 竞态] -> ReplaceClipSource 在单一 UoW 前核对 exact old/new fingerprints 与 frame bounds；失败不自动裁剪、不部分替换。
- [多集批次出现部分失败或命名冲突] -> 提交前全集合 preflight，执行后保留逐集独立状态/artifact；不回滚成功集、不自动拼接或重试失败集。
- [light 引用与回导边界] -> `light` 只保存 manifest 与可解析引用，MP4/SRT 单独输出；MVP-A 不回导、不内嵌媒体。

## Migration Plan

1. 先添加 contracts、领域帧算术和 application tests。
2. 添加可逆 Alembic、ORM/Repository、唯一/外键/check constraints；不迁移或改写现有 Timeline 占位数据，必要 mapping 在实施前明确。
3. 添加 HTTP、Outbox、media worker、真实 `ffmpeg`/`ffprobe` `FfmpegRenderPort` adapter、Mock/unconfigured test adapter、BDD 和显式 FFmpeg probe。
4. 回滚只处理新表和未消费任务；已生成输出按 Storage retention 策略保留，具体生产策略待确认。

## 待提供的 probe 输入

- 显式 `ffmpeg`/`ffprobe` 二进制路径、版本与受控 Worker 配置；缺失时返回 `renderer_unconfigured`，不得报告 MP4/SRT 成功。
- additive HTTP path/error envelope 的落地兼容证据；它不得改变已冻结的 ExportJob 状态、媒体格式、light 引用或不回导语义。

## DDD / BDD / SDD / TDD

- **DDD**：Episode 是聚合边界，Version/Export 输入不可变，帧编辑规则在 domain。
- **BDD**：按集编辑唯一 current Cut、命名并只读比较不可变版本、裁剪/拆分/排序、导入音频、手工字幕、MP4/SRT/light 与负例均可观察。
- **SDD**：冻结 API、Schema、DB、port、Outbox、失败和非目标；对未决项目保留问题。
- **TDD**：领域单测先行，应用/adapter/HTTP/worker 分层扩展，MediaInspect/DerivativePort 的 bounded/idempotency/stale tests 先行，真实 FFmpeg 不作为默认 oracle。

## Current / Defined / Todo

- **Current**：仅 schema/ORM 占位。
- **Defined**：MVP-A 功能和边界；portable 等明确排除。
- **Todo**：实现、依赖装配、migration、worker、安全 probe 和回归验证。

## MVP-A boundary closure

保留裁剪、拆分、排序、明确 Clip 删除、四类音轨、静态音量、mute/solo、线性淡入淡出、ducking 和手工字幕文本/时间编辑。静态 `position`/`scale`/`opacity` 只保存单一值，禁止关键帧。故事板插入/复制/批量生成/批量重拍/批量审核、Timeline 独立自动保存、撤销/重做、版本恢复、字幕样式、Run 暂停、审核评论/时间码/提醒、Narration/TTS、循环、调速、轨道锁定和工程包回导均显式属于 MVP-B。

## Assembly, proxy and parity

**DDD**：Timeline owns Clip/SoundCue/reference/ducking/preview derived facts；Assets remains append-only。**BDD**：unaccepted/foreign/duplicate/revision conflict 拒绝，BGM 每步失败零 cue/clip，ducking invalid/overlap/parity 拒绝，Cut change stale+pause preview。**SDD**：canonical RenderPlan/compiler 供 proxy/FFmpeg 共用；绑定 cut revision 或 TimelineVersion、fingerprint/hash，独立于 ExportJob；ducking 参数映射为显式 FFmpeg filter graph。**TDD**：先写 commands/eligibility/ducking normalization/render-plan/golden parity（SSIM >= 0.98、duration/caption/audio <= 1 frame）测试；默认 preview fixture 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），真实 FFmpeg 仅 explicit adapter probe。非目标是 pixel/sample exact、4K/high bitrate browser、专业监看/复杂特效；验收使用既有 strict、worker、probe 命令。

MP4、UTF-8 SRT、light manifest 必须分别 append ExportArtifact（artifactId/type/status/object ref/retention/license）。下载 command 必须从 stable local actor UUID/project policy 逐层验证 Project->Episode->TimelineVersion->ExportJob->Artifact，签发短 TTL read-only grant 且不返回 objectKey/workspace URI。transition 只允许 `cut|crossfade`：30fps 整数帧，duration 有界，Clip adjacency/overlap 不变量由 preview 与 FFmpeg 共用 canonical RenderPlan/compiler；parity failure 不得完成 export。复杂 wipe/mask/auto/keyframe/audio crossfade 非目标。

媒体派生物 readiness 是由 Media Worker 拥有的独立输入事实。`MediaInspection` 保存 canonical observed `mime,size,checksum,duration,timebase,fps,frameCount,width,height,videoCodec,pixelFormat,audioTracks,sampleRate,channels`、tool/version 和 source AssetVersion id/revision/hash。`MediaDerivative` 为 proxy、thumbnail、keyframe index 和 waveform 保存独立的已验证 reference，并包含 derivative schema/version、source fingerprint、状态 `queued|running|ready|failed|stale`、retention/license/hold 与有界 parameters。Timeline 绝不创建这些记录，只消费 source fingerprint 与已接受 current reference 匹配的 `ready` derivative；source/cut revision 变化必须将 preview 标记为 stale。派生 pending/failed/stale 只阻断 Timeline handoff、preview 和 export，不阻断或撤销上游已接受的 scenes current。
