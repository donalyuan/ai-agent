# episode-audio-export Specification

## Purpose
TBD - created by archiving change implement-episode-timeline-audio-export. Update Purpose after archive.
## Requirements
### Requirement:总体计划追溯和协调边界
本 capability SHALL 反向追溯到 `plan-phase-one-drama-mvp-a` 的总体任务 `1.2`、直接实施任务 `4.1`--`4.3` 和共享任务 `5.1`--`5.8`。实施 MUST 以总体任务 `2.1`、`2.2`、`2.3` 的交付及已归档 AssetVersion 契约为可核验前置；总体 plan 仅协调 change 顺序、范围和验收，MUST NOT 成为运行时代码依赖。完整非目标是 MVP-B `portable` payload、Fish Audio、Groq ASR、自动字幕/自动对齐、专业 NLE、回导、发布平台、跨集自动拼接，以及接受 `profile`/`export_profile` 等 `exportProfile` 别名或让 DB/manifest/HTTP 各自维护版本源。真实 `ffmpeg`/`ffprobe` adapter 与 explicit probe 是本 change 必需交付；Mock/unconfigured 仅用于测试或诊断，绝不报告成功。9:16/16:9/1:1、1080p、30fps、H.264 `yuv420p`、AAC 48 kHz、UTF-8 SRT、四类音轨、light 引用和 ExportJob 状态已冻结；本 capability MUST NOT 承担其外的职责。

#### Scenario:前置或范围不满足时保持边界
- **WHEN** 实施取证发现前置切片尚未可用，或请求把总体 plan/MVP-B 能力作为 audio/export runtime 依赖
- **THEN** 实施显式阻塞或拒绝该请求，记录缺失前置；不得伪造 resolver、导入总体 plan 模块或产生 portable package

#### Scenario:拒绝完整非目标职责泄漏
- **WHEN** audio/export 尝试承担任一列明的非目标、接受 profile 别名、跳过真实 `ffmpeg`/`ffprobe` adapter 的显式 probe，或将 `renderer_unconfigured` 报告为成功
- **THEN** 架构依赖/契约测试失败，且不写 SoundCue、字幕、ExportJob、manifest、审计或 Outbox

### Requirement:导入的 episode audio 与手工字幕
系统 SHALL 允许 timeline 引用同项目、同集可用的不可变 AssetVersion 作为对白、配乐、环境声或音效。`SoundCue.track` MUST 是 PRD `cueType` 的唯一 canonical 分类事实并只允许 `dialogue|music|ambience|effects`；系统 MUST NOT 同时接受第二个 `cueType` 别名。`SoundCue` MUST 记录角色、`startFrame`、正整数 `durationFrames`、`trigger`、0--100 integer `priority`、有界去重 `continuityRefs`、静态 gain、mute/solo、线性 fade-in/fade-out 和授权元数据。trigger MUST 只为 `manual|scene_start|shot_start|shot_end`；非 manual 必须引用同 Episode 的 Scene/Shot ID/revision 并带整数 `offsetFrames`，在写入前确定性解析并冻结 startFrame。priority 只控制同轨重叠渲染顺序；continuityRefs 只引用 AssetBible/Scene/Shot/ShotSpec owner ID/revision/hash。MVP-A 音量包络只由 static gain + linear fades 表达，MUST NOT 接受 automation points/keyframes。字幕 MUST 仅由手工输入的 text/startFrame/endFrame 保存并可由帧转换为 SRT。系统 MUST NOT 调用 Fish Audio、Narration/TTS、Groq ASR 或自动对齐，也不支持循环、调速、轨道锁定或字幕样式。

#### Scenario:添加导入的 audio 与手工字幕
- **WHEN** 用户为 Episode timeline 添加合法 AssetVersion、SoundCue 和手工字幕
- **THEN** 系统保存音频结构和字幕，并保持原 AssetVersion 不变

#### Scenario:解析事件 trigger 并冻结 SoundCue
- **WHEN** 用户提交同 Episode 的 scene/shot trigger、当前 target revision、整数 offset/start/duration、priority 和合法 continuityRefs
- **THEN** owner 在同一 command 中解析并冻结 startFrame、trigger provenance、priority 和 refs，RenderPlan 以 priority 稳定排序且不自动修改 gain/ducking/审核/current

#### Scenario:拒绝非法 SoundCue 或高级音量包络
- **WHEN** cue 同时提供 `track`/`cueType`、trigger foreign/stale/missing、priority 越界、continuityRefs 重复/foreign/hash 不匹配，或包含 automation/keyframe points
- **THEN** 系统在 UoW/RenderPlan/Outbox 前返回 validation/conflict，零 SoundCue/TimelineVersion/ExportJob/Provider/Storage 写入

#### Scenario:拒绝跨项目或自动 audio 操作
- **WHEN** 音频引用其他项目/剧集、时间越界，或请求 Fish/Groq/自动对齐
- **THEN** 系统返回可诊断 validation、episode_mismatch 或 unsupported_feature，且不创建 cue/字幕

### Requirement:导出 preflight 与受控渲染
系统 SHALL 在生成任何输出前检查 TimelineVersion、AssetVersion 引用、整数帧边界、音频/字幕完整性和渲染参数，并从 owner facts 冻结不可变 `ExportExecutionSnapshot`。每个初始或 retry Job MUST 在同一事务产生持久 `export.job.dispatch.requested` outbox；dispatcher MUST 以稳定 workflow ID 启动 `media-tasks` 的 `episode_export` Temporal workflow/activity，activity MUST 从 snapshot 流式物化输入并实际执行 `EpisodeExportWorker`。渲染 MUST 通过 `FfmpegRenderPort` 的结构化白名单参数执行，并以 Job logical operation 幂等；未配置 renderer 或失败 MUST 显式可诊断。

#### Scenario:渲染有效的 episode 导出
- **WHEN** 已发布 TimelineVersion 通过 preflight，且 renderer 可用
- **THEN** 系统提交一个可审计导出任务，生成关联 MP4、SRT 和 light manifest 输出引用，不在 API 响应中返回媒体 bytes

#### Scenario:拒绝或暴露渲染失败
- **WHEN** 素材缺失、参数不在白名单、FFmpeg 未配置或执行失败
- **THEN** 系统返回/记录 `missing_asset`、`render_preflight_failed`、`renderer_unconfigured` 或 `render_failed` 及原始诊断，不静默 fallback 或报告成功

#### Scenario:提交和 retry 都进入实际 media workflow
- **WHEN** batch 或显式失败成员 retry 在数据库事务中提交成功
- **THEN** 每个新 Job 都有一个可恢复 outbox 和稳定 Temporal workflow identity；重复 dispatch 复用相同 workflow，media worker 注册实际 workflow/activity，Job 不会永久停在 queued

#### Scenario:执行快照来源不足时 fail-closed
- **WHEN** generated AssetVersion 无法追溯 accepted candidate、ProviderCall/VideoOperation、WorkflowRun Skill selection 或 StorageProfile/capability owner snapshot
- **THEN** 全 batch preflight 零写入失败，不使用 mock/local/first/latest 默认值填充审计事实

### Requirement:MVP-A `light` ProjectPackage
系统 SHALL 为 MVP-A 导出版本化 `ProjectPackage`。`exportProfile` 是唯一 canonical profile 字段，值域 MUST 为 `light|portable`；MVP-A 的请求与 manifest MUST 仅接受 `light`，`portable` MUST 只由 MVP-B 实施。manifest MUST 将下列字段全部声明为 required 且不得空值省略：`schema_version`、manifest version、`exportProfile`、Episode/TimelineVersion、可解析 AssetVersion 引用、音频轨道/Cue、每项素材/音频 authorization/license provenance、renderer 实测响度报告、完整 `models[]`（Provider/Profile/Model/CapabilitySnapshot）、完整 `skillRevisions[]`、实际 parameters、usage/cost value/status/source；`cost=unknown` 也必须明确记录状态与来源。多个生成来源 MUST 全量去重记录，不得任选第一个/最新来源；generated source 无完整 provenance 时 fail-closed。`light` MUST NOT 内嵌媒体载荷。持久化 export、manifest 的 `schema_version` 与 HTTP DTO `schemaVersion` MUST 映射同一个值，且不得成为两个版本源。

#### Scenario:导出 `light` package
- **WHEN** 用户导出通过 preflight 的单集 TimelineVersion 并以 `exportProfile: "light"` 请求
- **THEN** manifest 包含公共审计和引用字段、canonical `schema_version`，HTTP DTO 返回对应的 `schemaVersion`，且可由 Schema 解析

#### Scenario:拒绝缺少必需审计字段的 `light` manifest
- **WHEN** manifest 缺少或置空 authorization/license、loudness report、任一 Model/Profile/CapabilitySnapshot、任一 SkillRevision、parameters、usage/cost status/value/source 中任一字段，或 `cost=unknown` 没有来源
- **THEN** JSON Schema 与 export preflight 均失败，不生成成功 ExportJob/package，且诊断指出精确缺失字段

#### Scenario:以 renderer 实测结果写入 loudness
- **WHEN** renderer 完成 MP4 并返回 loudness measurement
- **THEN** manifest 写入实际 integrated LUFS/true peak、measurement tool/version；缺少或伪造固定目标值时 packaging 失败

#### Scenario:拒绝 `portable`、profile alias 或跨 episode package
- **WHEN** MVP-A 请求 `exportProfile: "portable"`、`profile`/`export_profile`/其他别名、自动拼接多集，或在一个 package 中引用其他项目/剧集素材
- **THEN** 系统返回 unsupported_feature 或 episode_mismatch，不生成包

#### Scenario:拒绝冲突的 schema 版本映射
- **WHEN** 请求或存储映射使 export 记录、manifest `schema_version` 与 HTTP DTO `schemaVersion` 缺失或不一致
- **THEN** 系统返回 validation，不生成 ExportJob、manifest、MP4 或 SRT，且诊断指出冲突字段

### Requirement:增量兼容与持久化边界
系统 SHALL 以 additive API、Schema 和数据库表实现音频/导出，不改变现有 Project/Episode/AssetVersion HTTP 或 objectKey 契约。外部 FFmpeg MUST 在 UoW commit 后执行；领域写入与 Outbox MUST 同事务。

#### Scenario:outbox 失败可见
- **WHEN** 导出领域记录已提交但后续 worker 无法取得任务或 renderer
- **THEN** 系统保留可查询的任务/失败状态和诊断，不覆盖 TimelineVersion 或伪造输出

### Requirement:按冻结 Storage capability 流式上传 artifact
系统 SHALL 对 MP4、SRT、light 先流式计算完整 size/checksum，再按提交时冻结的 StorageCapability admission 选择 part size 并生成多个真实 receipt。每个 part MUST 不超过 `maxPartSizeBytes`，part count/object size MUST 在 capability 内；complete manifest MUST 精确匹配已上传 receipt。Worker MUST 以相同 operation key/intent/manifest reconcile 或 resume，MUST NOT 用 `read_bytes()`、整文件内存聚合或超限单 part。

#### Scenario:上传大于单 part 上限的 MP4
- **WHEN** MP4 大于 64 MiB 且 capability 的 max part 为 64 MiB
- **THEN** Worker 以多个不超限分片流式上传、校验并登记，内存峰值有界且 complete manifest 与真实 receipts 一致

#### Scenario:重启后恢复 multipart
- **WHEN** Worker 在部分 part 或 complete 响应未知后重启
- **THEN** 相同 Job/operation key 先 reconcile/resume 并复用相同分片 receipt；不重渲染、不产生第二对象或伪造 manifest

### Requirement:冻结的 MVP-A render 与 audio 合同
系统 SHALL 仅支持 9:16、16:9、1:1 的 1080p、30fps 整数帧输出，视频 MUST 为 H.264 `yuv420p`，音频 MUST 为 AAC 48 kHz，字幕 MUST 为 UTF-8 SRT。音轨 MUST 仅分类为 dialogue、music、ambience、effects，并支持静态 gain、mute、solo、线性淡入淡出、master limiter 和对白 ducking，目标为 -14 LUFS-I/-1 dBTP。素材不足 MUST 失败，MUST NOT 插入黑帧。

#### Scenario:拒绝不符合合同的 render 或不足的媒体
- **WHEN** 请求使用不受支持的 aspect ratio、非 1080p/30fps/H.264 `yuv420p`/AAC 48 kHz 参数，或 timeline 素材不足
- **THEN** preflight 返回可诊断失败，不写成功 ExportJob、不生成黑帧或替代媒体

### Requirement:Dialogue ducking 与 RenderPlan/FFmpeg 映射
系统 SHALL 在 Episode TimelineVersion 保存 `ducking`：`enabled`、由对白 SoundCue 或显式输入解析并合并的整数帧 `dialogueIntervals`、正值 `attenuationDb`、非负整数 `attackFrames`/`releaseFrames` 和 `targetTracks`（仅 `music|ambience|effects`）。dialogue 轨道 MUST NOT 被 duck。canonical RenderPlan MUST 将 attenuation 编译为 target track 的负增益，FFmpeg filter graph 与 proxy preview MUST 消费相同冻结参数；重叠区间 MUST 只产生一次衰减。

#### Scenario:一致地渲染已配置 ducking
- **WHEN** 当前 TimelineVersion 配置合法 ducking 并通过 preflight
- **THEN** preview、RenderPlan 和 FFmpeg 输入包含相同 intervals/attenuation/attack/release/targetTracks，dialogue 保持原增益，目标轨道按预期衰减并通过音频回归

#### Scenario:导出前拒绝无效 ducking
- **WHEN** intervals 非整数/空/越界/无法合并，attenuation 非正，attack/release 为负，或 targetTracks 含 dialogue/未知轨道
- **THEN** timeline command/preflight 返回可诊断 validation，不写 TimelineVersion/RenderPlan/ExportJob，不生成 MP4/SRT/light artifact

### Requirement:ExportJob 状态与实际 renderer probe
系统 SHALL 将 ExportJob 状态限制为 `queued`、`preflighting`、`rendering`、`packaging`、`succeeded`、`failed`、`cancel_requested`、`cancelled`。本 change MUST 实现真实 `ffmpeg`/`ffprobe` adapter；Mock/unconfigured adapter 仅用于测试或配置缺失诊断。显式 probe MUST 分别验证 binary version、H.264 decoder/encoder、AAC decoder/encoder、`yuv420p` 和 MP4 mux/demux/container support，并冻结 capability snapshot；未配置返回 `renderer_unconfigured`，任一能力缺失返回 `renderer_capability_unsupported`，两者都 MUST 在 preview/ExportJob 成功副作用前阻断。

#### Scenario:renderer 不可用时不得伪成功
- **WHEN** Worker 未配置实际 `ffmpeg`/`ffprobe` 或 probe 失败
- **THEN** ExportJob 保持可查询失败/未配置诊断，且不产生 MP4、SRT 或 package success

#### Scenario:codec 或 container 能力缺失时阻断
- **WHEN** 二进制存在但缺少 H.264/AAC decoder/encoder、`yuv420p` 或 MP4 mux/demux/container 中任一必需能力
- **THEN** owner 返回 `renderer_capability_unsupported` 和逐项 probe evidence，不创建 PreviewArtifact、rendering ExportJob、MP4/SRT/light success 或替代编码输出
