## 0. 总体计划追溯与实施前置

- [x] 0.1 在开始实现前核验 `plan-phase-one-drama-mvp-a` 总体任务 `1.2`、直接实施任务 `4.1`--`4.3` 与共享任务 `5.1`--`5.8`；记录完整非目标为 `TimelineDraft`、多个 mutable Cut 的创建/选择/切换、故事板插入/复制/批量生成/批量重拍/批量审核、Timeline 独立 autosave、undo/redo、version restore、字幕样式、Run 暂停、审核评论/时间码/提醒、Narration/TTS、循环、调速、轨道锁定、工程包回导、portable payload、Fish Audio、Groq ASR、自动字幕/自动对齐、专业 NLE、发布平台和跨集自动拼接，接受 `profile`/`export_profile` 等别名或多个 schema version 来源；同时确认真实 `ffmpeg`/`ffprobe` adapter 与显式 probe 是必需交付，未配置 renderer 仅能返回原始 `renderer_unconfigured`，不得报告成功。固定 9:16/16:9/1:1、1080p、30fps、H.264 `yuv420p`、AAC 48 kHz、UTF-8 SRT、音频、light 引用和 ExportJob 状态，并为职责泄漏编写无写入失败测试。
- [x] 0.2 定向取证总体任务 `2.1`、`2.2`、`2.3` 和已归档 AssetVersion 契约可提供 source resolver/审计/归属事实；验证总体 plan 仅为协调关系，不被导入为运行时代码依赖，缺失前置或范围漂移必须显式阻塞。

## 1. Timeline Contracts and Domain

- [x] 1.1 定义每集唯一 current Cut、可命名的不可变 TimelineVersion、Clip、30fps 整数帧、静态 position/scale/opacity（拒绝 keyframe）、`TrimClip`/`SplitClip`/`ReorderClips`/`DeleteClip`/`ReplaceClipSource`、dialogue/music/ambience/effects SoundCue 的 canonical track/cueType mapping、start/duration、manual/scene/shot trigger、priority、continuityRefs、静态 gain/mute/solo/线性淡入淡出（拒绝 automation keyframe）、ducking policy、master limiter（-14 LUFS-I/-1 dBTP）、UTF-8 手工字幕 text/start/end、EpisodeExportBatch 和 Export/manifest JSON Schema 与正反 fixtures；拒绝 `TimelineDraft` 或多个 mutable Cut lifecycle。固定 9:16/16:9/1:1、1080p、H.264 `yuv420p`、AAC 48 kHz、ExportJob 状态，`ProjectPackage` 只允许 `exportProfile: "light"`，将 authorization/license、loudness、Model/Profile/CapabilitySnapshot、SkillRevision、parameters、usage/cost value/status/source 全部设为 required，拒绝缺失/空值/unknown cost 无来源、`portable` 与所有 profile 别名，并覆盖 `schema_version`/`schemaVersion` 冲突。
- [x] 1.2 实现 Episode timeline 聚合、一个 current cut、排序/裁剪/拆分、不可变发布和 revision 冲突领域测试。
- [x] 1.3 实现 application commands/queries、同项目/同集 resolver、frame preflight、Repository/UoW/Outbox ports 和失败映射测试；每次成功编辑 command 必须立即持久化新 revision，409 返回 authoritative state 且无部分写入。
- [x] 1.4 先写 exact clip/old-new source/eligibility/derivative/frame/revision 正反测试，再实现 `ReplaceClipSource`；成功只换 current Cut source 并保留编辑属性，失败零写入，AssetEdit/VideoTake accept 不得直接调用。

## 2. Persistence and HTTP

- [x] 2.1 新增可逆 Alembic 与 SQLAlchemy current-cut/version/clip/audio/caption/export 表、外键、Episode-current Cut 一对一唯一约束和 check constraints；不创建 draft 表或多 mutable Cut 关系。以 DB/manifest `schema_version` 为 canonical 值映射 HTTP DTO `schemaVersion`，不建立第二版本源。
- [x] 2.2 实现 Repository adapter、稳定排序、版本快照和 transaction/Outbox 集成测试，不改写既有 AssetVersion 或 Timeline 占位数据。
- [x] 2.3 添加 additive timeline/audio/export HTTP API、contracts 和 409/validation/missing-asset/error-envelope tests，覆盖 `schema_version`/`schemaVersion` 冲突、缺失和 profile alias 拒绝。
- [x] 2.4 添加 project-scoped EpisodeExportBatch preflight/submit/read/retry APIs，覆盖显式完整成员集合、稳定逐集命名、全集合预检零部分提交、幂等与逐集状态。

## 3. Media Worker and Export

- [x] 3.1 定义并实现受控真实 `ffmpeg`/`ffprobe` `FfmpegRenderPort` adapter、仅测试的 mock/unconfigured adapter、结构化固定参数白名单、临时目录与输入/输出校验；显式 probe 必须逐项记录 binary version、H.264 decoder/encoder、AAC decoder/encoder、`yuv420p`、MP4 mux/demux/container，缺失返回 `renderer_unconfigured|renderer_capability_unsupported` 并在 preview/export 前阻断。
- [x] 3.2 实现 media worker 幂等导出、独立 MP4/UTF-8 SRT、`exportProfile: "light"` 仅 manifest/可解析引用且不回导的 ProjectPackage、ExportJob 状态与原始 stderr 诊断；不得接受 portable 或 profile 别名。
- [x] 3.2a 实现 EpisodeExportBatch fan-out：每个显式 Episode/TimelineVersion 独立 RenderPlan/ExportJob/MP4/SRT/light artifacts，禁止合并 RenderPlan/跨集拼接；单集失败汇总为 partially_failed，重试仅对显式失败 member 使用新 logical operation。
- [x] 3.3 编写 BDD：每集隔离、30fps 整数帧裁剪/拆分/排序/删除、静态 position/scale/opacity 与 keyframe 拒绝、成功立即持久化/409 authoritative recovery、四类导入音频、gain/mute/solo/fade/limiter、ducking interval/attenuation/attack/release/target tracks、手工字幕文本/时间、preflight、缺素材不插黑、越界、跨集、light manifest 全部 required 审计字段的正反 fixtures、MVP-A portable/profile alias 以及 schema version 映射冲突拒绝。

## 4. Verification

- [x] 4.1 执行 domain/application/adapter/HTTP/worker/BDD 与 manifest Schema 正反定向测试，覆盖全部 required authorization/loudness/Model/Skill/parameters/cost source 字段、`cost=unknown` 来源、`exportProfile` 值域、MVP-A `light` 限制、alias 拒绝、ducking normalization/zero-write 和 `schema_version`/`schemaVersion` 的单一来源映射。
- [x] 4.2 以显式配置执行真实 `ffmpeg`/`ffprobe` adapter probe 或记录 `renderer_unconfigured` 及原始错误；不得把未配置路径报告为成功。
- [x] 4.3 依次运行 `openspec status --change "implement-episode-timeline-audio-export" --json`、`openspec instructions apply --change "implement-episode-timeline-audio-export" --json`、`openspec validate implement-episode-timeline-audio-export --strict --json`、`openspec validate --changes --strict --json`、`pnpm run check` 和 `git diff --check`；记录仅剩 renderer probe 输入/API envelope 证据，全部实现 task 完成前不得勾选本验收项。

## 5. Eligibility, Assembly and Preview

- [x] 5.1 定义 expectedRevision add/remove Clip 与四类 SoundCue commands；校验 accepted/current same-project Episode AssetVersion、explicit audio selection、remove 仅删 reference。
- [x] 5.1a 实现 SoundCue trigger resolution、priority stable ordering、continuityRefs owner validation 和 static-gain/linear-fade-only envelope；覆盖 foreign/stale target、offset/frame 越界、duplicate refs、priority 边界、track/cueType 双字段和 automation/keyframe 零写入。
- [x] 5.1b 定义 `ExportDiagnosticTarget` Schema/owner resolver 与 tests，覆盖 Clip/Caption/SoundCue/AssetVersion 精确定位、renderer/storage 设置定位、published Version 只读和 message/数组位置不可作为定位依据。
- [x] 5.1c 实现 ExportJob packaging 的 `uploading|verifying|registering` subphase、三个 artifact 的 StoragePort operation/reconcile/stat/checksum/MIME/size verification 与单次 ExportArtifact append；覆盖 unknown/duplicate/partial failure/no-rerender/no-fallback。
- [x] 5.2 添加 BGM upload/import 前置失败、foreign/unaccepted/duplicate/revision conflict 的零 cue/clip fixtures。
- [x] 5.2a 接入 `implement-project-asset-center` 的 project-scoped selector/query handoff，只消费 AssetVersion id/revision/hash、authorization summary 与 derivative fingerprint；覆盖 stale/foreign/unauthorized/partial-unavailable、Timeline 不复制上传/filter/metadata/usage/derivative owner state和失败不重新上传/生成派生物。
- [x] 5.3 定义 source-bound ProxyRendition/PreviewManifest/PreviewArtifact、cut revision/timelineFingerprint/renderPlanHash stale semantics 与独立于 ExportJob 的状态。
- [x] 5.4 实现 canonical RenderPlan/compiler parity fixtures；golden sample 明确 SSIM >= 0.98、duration/caption/audio <= 1 frame，并验证 ducking intervals/attenuation/attack/release 在 preview 与 FFmpeg filter graph 相同，真实 FFmpeg 只走 explicit media probe。
- [x] 5.4a 定义并测试 `MediaInspectPort`/`MediaDerivativePort` 与独立 `MediaInspection`/`MediaDerivative` contracts：canonical metadata、proxy、thumbnail、keyframe index、waveform、source AssetVersion id/revision/hash、tool/derivative schema/version、retention/license/hold、bounded output 与 idempotent retry；Media Worker 是唯一生成 owner，Provider/Timeline 不得越界写入。
- [x] 5.4b 覆盖 exact candidate/source/ShotSpec video accept/reject/retake、unaccepted/foreign/stale/derivative-not-ready、source/cut fingerprint mismatch、thumbnail/keyframe/waveform bounds、ffprobe claimed-vs-observed mismatch 与 worker restart recovery；证明 derivative pending/failed/stale 不改变 accepted current，Timeline/Export 只消费 accepted current + ready derivatives。

## DDD / BDD / SDD / TDD

- **DDD**：1.x 固化 Episode timeline 和不可变版本。
- **BDD**：3.3 覆盖可观察编辑与导出行为；失败路径不可静默降级。
- **SDD**：1.1、2.x、3.x 覆盖 API、Schema、DB、依赖、安全执行、兼容性和非目标。
- **TDD**：每项实现先添加分层失败测试再实现最小行为。

## Current / Defined / Todo

- **Current**：任务均未实施，FFmpeg/audio/export 不存在。
- **Defined**：MVP-A 只支持 `light`、每集独立 Timeline/导出、显式多集逐集导出批次、导入音频和手工字幕，不支持跨集拼接。
- **Todo**：完成所有未勾选任务，并在依赖与未决参数明确后进行实现验收。
- [x] 8.1 定义/测试三个独立 ExportArtifact 和逐层归属下载授权：cross-project/episode、expired/held/unauthorized 拒绝且不泄露 objectKey/workspace URI；grant 短 TTL read-only。
- [x] 8.2 定义/测试 `cut|crossfade` 30fps integer-frame、bounded duration、Clip adjacency/overlap 与 canonical RenderPlan/compiler preview/FFmpeg parity；parity failure 不报告成功，复杂转场/auto/keyframe/audio crossfade 为非目标。

## 9. 审查一致性修复

- [x] 9.1 为 `export.batch.created` 建立实际 media worker 消费入口并补 Temporal/dispatcher 注册与执行测试；提交后的 queued ExportJob 必须可进入逐集执行链，而不是只注册健康 activity。
- [x] 9.2 按冻结 Storage capability 对 MP4/SRT/light 进行有界流式 checksum 与 multipart upload，part size 不得超过 adapter 上限，禁止 `read_bytes()` 或单 part 承载超限产物，并覆盖重启 reconcile/幂等 complete。
