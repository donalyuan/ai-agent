## ADDED Requirements

### Requirement:Episode-scoped 30fps timeline route
系统 SHALL 在 `/projects/:projectId/episodes/:episodeId/timeline` 按 Episode 读取并编辑唯一 mutable current Cut，并读取、命名和只读比较多个不可变 TimelineVersion。仅可使用可选 `versionId` 进入版本只读视图；MVP-A MUST NOT 创建 `TimelineDraft`、接受 `cutId` 路由，或提供多个 mutable Cut 的创建、选择或切换。编辑 UI MUST 只接受 `fps: 30` 的整数帧 DTO，并以 owner ID/revision/schemaVersion 作为唯一版本事实；不得跨 Episode 读取/编辑或创建并行 Timeline 状态。

#### Scenario:open the current cut of an episode
- **WHEN** 用户进入有 current Cut 的 Episode route
- **THEN** UI 读取该 Episode 的 current Cut、Version 列表和 revision，并将其他 Episode cache 隔离

#### Scenario:receive unsupported frame data
- **WHEN** owner DTO 的 fps 非 30、frame 为浮点/负数或 duration 为零
- **THEN** UI 显示可诊断 contract 错误且不转换、提交或伪造编辑状态

### Requirement:显式发布命名的不可变 TimelineVersion
系统 SHALL 在 current Cut 视图提供命名、preflight 和显式发布 TimelineVersion 的用户闭环。UI MUST 校验名称非空并遵守 owner 的名称规则，先展示 owner preflight 结果，再以 current Cut `expectedRevision` 提交一次 publish command；成功后 MUST 读取新增 TimelineVersion 的稳定 ID、名称、sourceCutRevision、scope、schemaVersion 和 revision，并只读显示/比较该版本。发布 MUST 只追加不可变 TimelineVersion，不得创建第二个 mutable Cut、改写既有 Version、替换 current Cut 或自动启动 ExportJob。这里的 publish MUST NOT 被解释为 Workflow 发布或内容平台分发。

#### Scenario:preflight 后发布命名版本
- **WHEN** 用户在 current Cut 输入合法名称，owner preflight 通过，且提交的 expectedRevision 仍为当前 revision
- **THEN** UI 只提交一次 publish command，显示新增不可变 TimelineVersion，并可进入其只读比较视图；current Cut 与既有 Version 保持不变

#### Scenario:名称或 preflight 无效时不发布
- **WHEN** 名称为空/不符合 owner 规则，或 timeline preflight 返回归属、素材、帧、授权、派生物、音频、字幕或 revision 前置失败
- **THEN** UI 显示逐项 diagnostic，不调用 publish command，也不创建 TimelineVersion、RenderPlan、ExportJob 或 artifact

#### Scenario:发布不依赖 renderer capability
- **WHEN** current Cut 的 timeline preflight 合法但真实 renderer 未配置或不支持目标编码
- **THEN** UI 仍可发布不可变 TimelineVersion；renderer diagnostic 只在后续 export preflight 显示，不在 publish 时创建 ExportJob 或伪造导出成功

#### Scenario:发布时 current Cut 已变化
- **WHEN** preflight 后 current Cut 被其他命令更新，publish 返回 `revision_conflict` 409
- **THEN** UI 丢弃旧 preflight、刷新 authoritative current Cut 并要求用户重新确认，不自动重试、不部分发布也不覆盖任何 Version

### Requirement:Integer-frame clip operations and CAS recovery
系统 SHALL 对视频和图片 Clip 提供整数帧裁剪、拆分、排序、明确删除与静态 position/scale/opacity command，所有写入 MUST 携带 current expectedRevision。静态变换 MUST NOT 接受 keyframe/animation payload。UI 可以乐观呈现命令，但每次成功 command MUST 立即显示 owner 持久化的新 revision；409 revision conflict MUST 回滚并读取 authoritative Cut。

#### Scenario:trim and split a clip
- **WHEN** 用户提交合法的 30fps 整数 in/out/duration 与当前 revision
- **THEN** UI 刷新 owner 返回的 Clip 顺序和递增 revision，已发布 Version 不被改写

#### Scenario:submit a stale clip edit
- **WHEN** Clip command 返回 revision_conflict 或 frame_out_of_bounds
- **THEN** UI 回滚临时预览、显示原始 code/message 并提供刷新，不重复 mutation

#### Scenario:delete or transform a clip
- **WHEN** 用户使用当前 revision 删除一个 Clip 或提交合法单值 position/scale/opacity
- **THEN** UI 调用 `DeleteClip` 或 `SetClipTransform`，显示立即持久化的新 revision；删除不删除 AssetVersion，已发布 Version 不变

#### Scenario:reject keyframe or stale transform
- **WHEN** transform payload 包含 keyframe/animation 字段或 expectedRevision 已过期
- **THEN** UI 显示 owner validation/409，回滚临时 state、refetch authoritative Cut，且不重复或部分写入

### Requirement:Track, audio and basic mixing controls
系统 SHALL 分离 video/image Clip lanes、caption display 与 SoundCue audio tracks；SoundCue.track MUST 仅为 `dialogue|music|ambience|effects`，并作为 PRD `cueType` 的同一 canonical 分类，不接受第二个并行字段。UI MUST 展示/编辑整数 startFrame/durationFrames、`manual|scene_start|shot_start|shot_end` trigger target/offset、0--100 priority、owner reference-only continuityRefs、静态音量、mute/solo 与线性 fade-in/fade-out；scene/shot target 只来自同 Episode owner projection，最终 startFrame 由 owner 解析。MVP-A volume envelope MUST 仅为 static gain + linear fades，MUST NOT 提供 automation points/keyframes。UI 还 SHALL 提供 owner-controlled dialogue ducking：`enabled`、merged integer `dialogueIntervals`、positive `attenuationDb`、non-negative integer `attackFrames`/`releaseFrames`、`targetTracks=music|ambience|effects`。dialogue MUST NOT be a target；master limiter 仅显示/提交 owner 已支持的静态配置，MUST NOT 提供关键帧、多机位或专业调色控件。

#### Scenario:adjust a music cue
- **WHEN** 用户对同集合法 music cue 提交静态音量和线性淡入淡出
- **THEN** UI 发送带 expectedRevision 的 owner command 并更新该 Cut 的预览

#### Scenario:request unsupported advanced audio or video editing
- **WHEN** 用户请求 Fish/Groq、自动对齐、复杂关键帧、多机位或专业调色
- **THEN** UI 明确显示 unsupported feature，且不创建 Provider、Timeline 或 Export mutation

#### Scenario:reject an unsupported SoundCue track
- **WHEN** owner 或用户 payload 使用 `sfx` 或其他未知 `SoundCue.track`
- **THEN** Zod/contract adapter 返回可诊断 validation，且不创建或修改 SoundCue、Clip lane、caption display 或 ExportJob

#### Scenario:编辑合法 SoundCue 触发和混音字段
- **WHEN** 用户选择同 Episode scene/shot trigger、整数 offset/start/duration、合法 priority/continuityRefs 和 static gain/linear fades
- **THEN** UI 以 owner stable IDs/revisions 和 expectedRevision 提交单一 SoundCue command，显示 owner 解析后的 startFrame/new Cut revision，不自行移动 cue 或修改 continuity owner

#### Scenario:拒绝并行 cueType 或音量关键帧
- **WHEN** payload 同时含 `track`/`cueType`、trigger foreign/stale、priority 越界、refs 无效，或包含 automation/keyframe points
- **THEN** UI 显示 owner/Zod diagnostic、回滚未提交状态，且不创建 SoundCue、TimelineVersion、RenderPlan 或 ExportJob

#### Scenario:edit valid dialogue ducking
- **WHEN** 用户为当前 Cut 提交合法 enabled/interval/attenuation/attack/release/targetTracks 参数
- **THEN** UI 携带 expectedRevision 调用 owner command，显示更新后的 ducking state，并使 preview/RenderPlan cache 按新 revision 失效

#### Scenario:reject invalid ducking controls
- **WHEN** intervals 非整数/空/越界/重叠无法合并，attenuation 非正，attack/release 为负，或 targetTracks 含 dialogue/未知轨道
- **THEN** UI 显示 owner diagnostic，回滚乐观状态且不创建 TimelineVersion、RenderPlan 或 ExportJob

### Requirement:Manual subtitles and export status
系统 SHALL 允许用户手工编辑字幕 text/startFrame/endFrame，并显示 MP4、SRT 和 MVP-A `exportProfile:"light"` 的 manifest/reference-only preflight、job status 与 artifact reference。light manifest view MUST 校验并显示 authorization/license、loudness report、Model/Profile/CapabilitySnapshot、SkillRevision、parameters 和 usage/cost value/status/source；任一 owner-required 字段缺失/空值时不得显示成功下载。ExportJob.status MUST 仅为 `queued|preflighting|rendering|packaging|succeeded|failed|cancel_requested|cancelled`。UI MUST NOT 调用 ASR/自动对齐、嵌入媒体 bytes、接受 `portable`/profile alias，或在 renderer 未配置/失败时报告完成。

#### Scenario:create manual captions and request a light export
- **WHEN** 用户保存合法手工字幕并显式请求通过 preflight 的 light export
- **THEN** UI 显示 owner ExportJob 的 MP4/SRT/light 状态与引用，不在响应中处理媒体 bytes

#### Scenario:surface export failure
- **WHEN** owner 返回 missing_asset、renderer_unconfigured、render_preflight_failed 或 render_failed
- **THEN** UI 保留原始诊断和可重试/修正入口，且不显示成功 artifact

#### Scenario:导出失败定位到精确问题
- **WHEN** preflight diagnostic 含合法 `ExportDiagnosticTarget` 指向 Clip、Caption、SoundCue、AssetVersion、renderer 或 storage
- **THEN** UI 提供 owner-validated 跳转并在目标页重新校验 ID/revision；foreign/stale/missing target 不跳转、不从 message 猜测位置，也不修改 published Version

#### Scenario:显示 packaging 上传阶段
- **WHEN** ExportJob 为 packaging 且 owner subphase 为 uploading、verifying 或 registering
- **THEN** UI 按 MP4/SRT/light 分别显示进度和可诊断失败，只有三个 ExportArtifact 全部 registered 时显示 succeeded/download；unknown 或缺 artifact 不伪装完成

#### Scenario:reject frozen-contract violations before export mutation
- **WHEN** owner DTO 使用非 30fps、未知 ExportJob.status、`portable`、`profile`/`export_profile` alias，或 light manifest 缺少 authorization/loudness/Model/Skill/parameters/cost source
- **THEN** Zod/contract adapter 显示原始 validation/unsupported_feature，且不创建 ExportJob、manifest 或 artifact

### Requirement:Timeline editor verification boundary
系统 SHALL 以 component/state/frame arithmetic/Zod/Query contract 和 Playwright E2E 覆盖每集隔离、轨道、CAS、字幕、TimelineVersion 命名/preflight/发布、导出和主要失败。默认测试 MUST 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），真实 renderer 不属于默认测试 oracle；页面加载/视图切换不得创建或切换 profile。

#### Scenario:execute timeline UI regression tests
- **WHEN** 维护者运行本 change 的前端验收命令
- **THEN** 失败可定位为 frame/state/contract/component/E2E，且未实现 tasks 保持未勾选

### Requirement:Asset assembly and proxy player
系统 SHALL 复用项目资产中心的素材箱/selector/query/upload projection，提供 image/video Clip、四类 SoundCue 的 add/remove UI，全部携带 expectedRevision 并显示 loading/empty/error/duplicate/foreign/unaccepted/revision-conflict。Timeline UI MUST NOT 建立第二套上传、筛选、Asset metadata/version、authorization、usage 或 derivative owner state。Player SHALL 使用 integer frame playhead：seek 写 `currentTime=frame/30`，播放时将媒体时钟量化并夹取回 30fps frame，支持 play/pause/seek/frame-step/ended/error；stale preview MUST 暂停。

#### Scenario:play a current proxy preview
- **WHEN** current cut 的 matching preview 可用且用户播放或 seek
- **THEN** UI 以整数帧更新 playhead，并只显示 owner preview/RenderPlan facts

#### Scenario:prevent stale preview playback
- **WHEN** Cut revision 改变或 owner 返回 stale preview
- **THEN** Player 暂停并显示 regenerate/error state，不将旧 preview 伪装为 current final render

#### Scenario:复用资产中心 selector 而不复制事实
- **WHEN** 用户从项目资产中心选择同项目 AssetVersion 并交给当前 Episode Timeline
- **THEN** UI 只传 id/revision/hash、authorization summary 和 derivative fingerprint；最终 Clip/SoundCue 由 Timeline owner 创建，筛选、上传、版本和 usage 继续读取资产中心 owner state
### Requirement:安全的 artifact 与 transition 控制
UI MUST 渲染独立 MP4/SRT/light artifact states，且不暴露 objectKey/workspace URI；只可在 30fps 整数帧上提供 `cut|crossfade` transition commands。

#### Scenario:Ineligible export or parity failure is visible
- **WHEN** grant 为 foreign/expired/held/unauthorized，或 canonical parity 失败
- **THEN** UI 显示 diagnostic，且不提供 download/success state。

### Requirement:重拍后的精确 Clip source replacement UI
系统 SHALL 只在新视频已 accepted-current、同 project/Episode/Shot 且 derivative ready 时，为用户选定的既有 Clip 提供 Replace source。UI MUST 展示 clipId、old/new AssetVersion id/revision/hash/fingerprint、保留的 frame/transform/transition 和 owner preflight，并以 current Cut expectedRevision 提交 `ReplaceClipSource`。成功只刷新 current Cut 并提示发布新 TimelineVersion；MUST NOT 在 AssetEdit/VideoTake accept 时自动替换、自动裁剪/拉伸、发布或导出。

#### Scenario:确认后替换并保留历史版本
- **WHEN** 用户检查 old/new source 后明确替换且 owner command 成功
- **THEN** UI 显示同一 Clip 的新 source 和新 Cut revision，既有 TimelineVersion 仍显示旧 source，并提供独立 publish 入口

#### Scenario:替换冲突时回滚
- **WHEN** owner 返回 stale/foreign/unaccepted/derivative-not-ready/frame_out_of_bounds/revision_conflict
- **THEN** UI 回滚乐观状态、显示原始 diagnostic 并 refetch，不自动重试、不改 Version/ExportJob

### Requirement:项目多集逐集导出 UI
系统 SHALL 在 `/projects/:projectId/exports` 提供项目级 EpisodeExportBatch 视图，让用户显式选择非空且去重的 Episode + published TimelineVersion、设置每集安全唯一 output base name，并在提交前展示全集合 preflight。提交后 SHALL 按集显示独立 ExportJob 和 MP4/SRT/light artifacts，以及 batch `succeeded|partially_failed|failed|cancelled` 汇总。UI MUST NOT 自动选择 current Version、隐式扩大集合、拼接多集或自动重试失败集。

#### Scenario:选择多集并查看逐集输出
- **WHEN** 用户显式选择多个 published TimelineVersion 且全集合 preflight 通过
- **THEN** UI 提交一次 batch，按 Episode 展示独立 jobs/artifacts 和稳定文件名，不显示合并视频

#### Scenario:任一成员预检失败时不提交
- **WHEN** 任一选择重复、foreign、stale、未发布、命名冲突或 owner preflight 失败
- **THEN** UI 展示逐项 diagnostic 且不调用 batch submit，不静默删除失败选择或替换为 current

#### Scenario:显式选择失败成员重试
- **WHEN** batch 含一个或多个 failed Job，用户勾选非空失败 Episode 集合并确认重试
- **THEN** UI 只发送所选 `episodeIds` 和新 logical operation；空集合、成功/运行中成员均禁用且不发送请求

#### Scenario:下载已验证的成功 artifact
- **WHEN** succeeded Job 的 MP4/SRT/light artifact 状态为 verified 且 owner download-grant 校验通过
- **THEN** UI 请求 short-TTL grant 并打开 opaque access path；其他状态、hold、expired、foreign 或 unauthorized artifact 保持禁用并显示 owner diagnostic，不泄露 objectKey/workspace URI

### Requirement:时间线组件与领域引擎边界
Timeline UI SHALL 使用共享 `shared/ui` 与 `react-resizable-panels` 的固定分区、dnd-kit 的同父 Clip 排序和 TanStack Virtual 的长列表/日志；PixiJS、WaveSurfer.js、HLS.js SHALL 只在时间线领域模块中分别提供画面、波形和同源代理播放。上述领域组件 MUST NOT 进入 `shared/ui`。React Flow 只能作为固定 published WorkflowVersion 的只读投影，MVP-A MUST NOT 提供图编辑。

#### Scenario:操作时间线与媒体预览
- **WHEN** 用户调整有限分区、用 pointer/keyboard 重排同父 Clip、滚动长列表、拖动播放头或查看代理状态
- **THEN** 布局尺寸稳定、排序范围可验证、虚拟 DOM 有界，Pixi 画面非空，WaveSurfer/HLS 状态可见，且不产生 graph mutation 或隐式 owner 写入
