## Why

当前 Timeline 只有 Schema/ORM 占位，无法为每一集维护可审计的剪辑、导入音频、手工字幕或可重放导出。MVP-A 需要在不承诺专业 NLE 或真实自动化音频能力的前提下，形成按集 MP4/SRT/light 工程包闭环。

## What Changes

- 新增每集唯一 mutable current Cut、可命名的不可变 `TimelineVersion` 与整数帧 `Clip`；MVP-A 不创建 `TimelineDraft`，也不提供多个 mutable Cut 的创建、选择或切换生命周期。
- 新增导入 dialogue/music/ambience/effects，`SoundCue` 的整数入点/时长、触发来源、优先级、连续性引用、静态 gain、mute、solo、线性淡入淡出、对白自动 ducking、master limiter（-14 LUFS-I/-1 dBTP）和手工字幕契约；MVP-A 音量包络仅为静态 gain + 线性 fade，不包含任意关键帧曲线。
- 新增导出前预检、真实 `ffmpeg`/`ffprobe` adapter 的受控显式 probe 边界及独立 MP4、UTF-8 SRT、`ProjectPackage` `exportProfile: "light"` manifest 契约；固定 9:16/16:9/1:1、1080p、30fps、H.264 `yuv420p`、AAC 48 kHz。
- 导出预检 diagnostic 携带 owner-validated Episode/TimelineVersion/Clip/Caption/SoundCue/AssetVersion/renderer/storage 定位引用；`packaging` 内显式经历 artifact upload -> stat/checksum/MIME verify -> ExportArtifact register，用户可见“上传”进度，全部独立 artifact 登记成功后才可 `succeeded`。
- renderer capability probe 必须分别验证 H.264 decoder/encoder、AAC decoder/encoder、`yuv420p` 和 MP4/container；缺失任一能力都在 preview/export 前返回稳定 unsupported diagnostic，不得只凭二进制存在判断可用。
- `light` manifest 将 authorization/license、响度测量、Model/Profile/CapabilitySnapshot、SkillRevision、实际 parameters 和 usage/cost value/status/source 全部设为必填，并以正反 Schema fixtures 拒绝缺失、空值或 unknown cost 无来源。
- 一个 Episode 同时只能有一个 current cut；剪辑支持排序、裁剪和拆分，且拒绝跨集混用、越界和非整数帧。
- MVP-A 还明确支持静态 `position`/`scale`/`opacity`（禁止关键帧）和 `DeleteClip`；每个成功编辑 command 立即持久化，过期 `expectedRevision` 返回 409 且 UI/调用方恢复 authoritative state。
- 新增显式 `ReplaceClipSource`：单镜头重拍结果经 video accept、scenes current CAS 和 MediaInspect ready 后，用户指定既有 Clip 及其旧 source fingerprint，以 current Cut `expectedRevision` 精确替换来源；失败零写入，成功后仍需显式发布新的 TimelineVersion，AssetEdit/video accept 不得直接改 Timeline。
- 新增项目级多集导出批次：用户显式提交 Episode + immutable TimelineVersion 的完整集合，预检全部通过后按集创建独立 ExportJob/MP4/SRT/light artifacts，并按集稳定命名；不自动选择 current、不跨集合扩展，也不拼接多集成一个视频。
- Timeline 素材选择读取 `implement-project-asset-center` 的 project-scoped selector/query 与精确 AssetVersion handoff；Timeline 不维护第二套上传、筛选、授权、版本或派生状态事实。

## Capabilities

### New Capabilities

- `episode-timeline-editing`: 每集唯一 current Cut、命名不可变版本、立即持久化 revision 和整数帧编辑。
- `episode-audio-export`: MVP-A 导入音频、手工字幕、受控渲染和 MP4/SRT/light 导出。

### Modified Capabilities

- 无。现有 Project/Episode、AssetVersion 与基础 Timeline Schema/ORM 占位不改变既有已实现行为。

## Impact

预期影响 `services/api` 的 timeline/audio/export domain/application/repository/interface、`workers/media`、`packages/contracts`、Alembic、FFmpeg execution port、Outbox 和定向测试。完整非目标是 MVP-B `portable` payload、Fish Audio、Groq ASR、自动字幕/自动对齐、专业 NLE、回导、发布平台和跨集自动拼接，以及接受 `profile`/`export_profile` 等 `exportProfile` 别名或让 DB/manifest/HTTP 各自维护版本源。真实 renderer 不是默认开发路径，但本 change 必须实现 adapter 并以显式配置 probe；不得把未配置 renderer 报告为成功。fps、分辨率、编码/容器/响度、音频轨道、light 引用格式和 ExportJob 状态均已冻结。

## 总体计划追溯与边界

- 本 change 反向追溯到 `plan-phase-one-drama-mvp-a`：总体任务 `1.2` 要求保留该追溯，直接实施任务为 `4.1`--`4.3`；共享版本、UoW/Outbox、`Mock Provider +` 显式 Local test/offline profile 与验收规则还须满足总体任务 `5.1`--`5.5`。
- 实施前置为总体任务 `2.1` 的 scenes/shots、`2.2` 的 workflows/runs、`2.3` 的 provider/model/skill catalog，以及已归档的 AssetVersion 契约；它们提供版本化来源、审计和同项目/同集 resolver。`plan-phase-one-drama-mvp-a` 只协调 change 顺序和验收，不是 Timeline/export 的运行时代码依赖。
- 完整非目标是 MVP-B `portable` payload、Fish Audio、Groq ASR、自动字幕/自动对齐、专业 NLE、回导、发布平台和跨集自动拼接，以及接受 `profile`/`export_profile` 等 `exportProfile` 别名或让 DB/manifest/HTTP 各自维护版本源。真实 renderer 不是默认开发路径，但本 change 必须实现 adapter 并以显式配置 probe；不得把未配置 renderer 报告为成功。fps、分辨率、编码/容器/响度、音频轨道、light 引用格式和 ExportJob 状态均已冻结。

## DDD / BDD / SDD / TDD

- **DDD**：Timeline 由 Episode 拥有；唯一 current Cut、不可变 Version、Clip 与 SoundCue/字幕是可审计编辑事实。
- **BDD**：用户可编辑并导出一集，也可显式选择多个已发布单集版本逐集导出；不可观察地混用跨集素材、越界裁剪、自动扩展范围或自动拼接均失败。
- **SDD**：定义 REST、Schema、DB、FFmpeg port、manifest、兼容性和失败状态，不定义未证实的布局或编码策略。
- **TDD**：以帧算术、current cut、预检和 mock renderer 的失败测试驱动实现，真实 FFmpeg 仅显式探测。
- **BDD 补充**：正反测试必须覆盖静态变换、Clip 删除、每次成功编辑立即持久化、409 零副作用恢复，以及 trim/split/reorder、四类音轨、静态音量、mute/solo、淡入淡出、ducking 和手工字幕文本/时间编辑。
- **SDD 补充**：故事板插入/复制/批量生成/批量重拍/批量审核、Timeline 独立自动保存、撤销/重做、版本恢复、字幕样式、Run 暂停、审核评论/时间码/提醒、Narration/TTS、循环、调速、轨道锁定和工程包回导均为 MVP-B 非目标；静态变换不得演变为关键帧。

## Current / Defined / Todo

- **Current**：只有基础 Schema/ORM 占位，尚无应用、音频、FFmpeg、media worker 或导出。
- **Defined**：MVP-A 的按集编辑与 light 导出契约。
- **Todo**：实现迁移、接口、受控执行、测试和兼容验证。

## 装配与预览闭合

本 change 拥有 expectedRevision add/remove Clip 与四类 SoundCue owner commands、asset eligibility、BGM import assembly、source-bound proxy preview 与 shared RenderPlan parity；remove 只删 Timeline reference。它不覆盖 AssetVersion、不复用 ExportJob 状态，MVP-A 不承诺专业监看或像素级相同。

## Dialogue ducking contract

MVP-A 的 TimelineVersion MUST 冻结 `ducking.enabled`、由对白 SoundCue 或显式输入解析并合并的整数帧 `dialogueIntervals`、正值 `attenuationDb`、非负整数 `attackFrames`/`releaseFrames` 和 `targetTracks`（仅 `music|ambience|effects`）。RenderPlan 将 `attenuationDb` 编译为目标轨道负增益；dialogue 轨道不被压低，重叠区间不得重复叠加。preview 与 FFmpeg 使用同一 filter 参数，音频回归验证区间、衰减和 attack/release 一致。

## ExportArtifact 与转场合同

**DDD**：MP4、UTF-8 SRT、light manifest 是独立 ExportArtifact；Timeline 只支持 `cut|crossfade`。**BDD**：artifact foreign/expired/held/unauthorized、transition adjacency/overlap/parity failure 均拒绝。**SDD**：artifact 含 id/type/status/object ref/retention/license，下载逐层归属授权短 TTL read-only grant；30fps integer frames、bounded transition duration、canonical RenderPlan/compiler。**TDD**：三 artifact 独立、授权零泄漏和 preview/FFmpeg parity tests。

## Asset center selector 与 renderer capability

**DDD**：资产中心只交付 selector/query projection，Timeline owner 仍唯一创建 Clip/SoundCue；Media Worker/renderer probe 只声明可执行能力。**BDD**：foreign/stale/unauthorized selector、usage/derivative unavailable 与 codec/container 缺失均在 Timeline mutation/preview/export 前可见。**SDD**：handoff 只含 project/AssetVersion id/revision/hash/authorization/derivative fingerprint，probe snapshot 逐项含 decoder/encoder/pixel-format/container。**TDD**：禁止第二套 library 状态、二进制存在即成功和 capability 缺失时的部分导出。
