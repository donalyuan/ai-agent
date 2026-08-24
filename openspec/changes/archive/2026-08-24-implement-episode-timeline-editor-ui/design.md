## Context

`implement-episode-timeline-audio-export` 已定义每集唯一 current Cut、命名不可变 TimelineVersion、整数帧、导入音频、手工字幕和 MP4/SRT/light export，但当前只存在基础 Schema/ORM 占位与阶段 0 Web 壳层。本 change 设计桌面编辑器消费层，不拥有 Timeline、AssetVersion、ExportJob 或 FFmpeg 业务事实。

## Goals / Non-Goals

**Goals:**

- 以每集独立路由显示唯一 current Cut，提供命名、preflight、显式发布多个不可变 TimelineVersion、只读版本比较和 30fps 整数帧编辑。
- 分离视频/图片 Clip lanes、独立 caption display 与 SoundCue audio tracks；音频轨只允许 `dialogue|music|ambience|effects`，并提供最小混音、对白 ducking、字幕/导出状态闭环。
- 定义 Query/Zustand/Zod、expected revision、`Mock Provider +` 显式 Local test/offline profile、失败恢复和验证入口。

**Non-Goals:**

- 不实现 `TimelineDraft`、多个 mutable Cut 的创建/选择/切换、Fish Audio、Narration/TTS、Groq、自动字幕/自动对齐、复杂关键帧、循环、调速、轨道锁定、Timeline 独立自动保存、通用撤销/重做、版本恢复、字幕样式、审核评论/时间码/提醒、工程包回导、多机位、专业调色、FFmpeg 或浏览器最终渲染。
- 不保存音视频 bytes、不在客户端自行混音/导出、不接受 float frame 或自动跨集拼接。
- 不实现独立于项目资产中心的素材上传、目录筛选、AssetVersion history、授权或 usage store；Timeline UI 只复用共享 selector/query projection。

## Decisions

### 1. DDD state boundary and route

路由为 `/projects/:projectId/episodes/:episodeId/timeline`，仅可选 `versionId` 进入已发布 TimelineVersion 的只读比较。TanStack Query 以 project/episode/current-Cut revision 或 version identity/revision 分隔 owner Timeline/Export resources；Zustand 仅持有视口缩放、playhead、track mute/solo 临时交互、选择和未提交编辑 command，不能复制 clips/captions、创建或切换 Cut，也不能把 Version 改为可写。未指定 `versionId` 时编辑只作用于唯一 current Cut；指定后只读，不产生编辑 mutation。

发布入口只存在于 current Cut 视图。用户输入非空且通过 owner 规则校验的名称后，UI 先请求 owner timeline preflight，展示成功或逐项诊断，再以 `{name, expectedRevision}` 提交一次显式 publish command。成功响应必须包含新 TimelineVersion 的稳定 ID、名称、`schemaVersion`、revision、`sourceCutRevision` 和 project/episode scope；UI 将它加入版本列表并进入只读查看，但不改写、切换或复制 current Cut。`versionId` 只读视图、页面加载和 preflight 自身不得发布版本。

timeline publish preflight 只校验冻结 Version 所需的 current Cut 合法性、项目/Episode 归属、整数帧、素材/授权/派生引用、音频、字幕和 revision；真实 `ffmpeg`/`ffprobe` renderer capability 属后续 export preflight，不得成为发布 TimelineVersion 的前置，也不得在 publish 时创建 RenderPlan、ExportJob 或 artifact。

时间显示和 command 均把秒数转换为 `frame = integer`，MVP UI 固定 30fps；对 owner 返回非 30fps、浮点、负值、零长度或未关联 episode 的 DTO，Zod adapter 返回诊断，不猜测舍入或重映射。

### 2. SDD: owner API, DTO and optimistic commands

`timelineApi` 消费 owner 已定义 `/v1/projects/{projectId}/episodes/{episodeId}/timeline...` 和 `/exports` 资源。Zod DTO 分别验证 current Cut `{id, schemaVersion, revision, projectId, episodeId, fps:30, clipLanes, captionDisplay, soundCueTracks, ducking}` 与 TimelineVersion `{id, name, schemaVersion, revision, sourceCutRevision, projectId, episodeId, fps:30, clipLanes, captionDisplay, soundCueTracks, ducking}`；Version DTO 必须只读且不可被 mutation adapter 接受。publish adapter 只提交 owner 定义的名称与 current Cut `expectedRevision`，并验证 preflight response、发布 response 和 current/created version scope；它不得由客户端构造 Version snapshot。Clip 使用整数 `timelineStartFrame/sourceInFrame/durationFrames` 和单一静态 `position/scale/opacity`。SoundCue 验证 canonical `track`（PRD `cueType` 同一事实）、assetVersionId/授权、startFrame/durationFrames、`manual|scene_start|shot_start|shot_end` trigger provenance/offset、0--100 priority、continuityRefs、static gain/mute/solo/linear fades，并拒绝第二 `cueType` 字段和 automation/keyframes；同时验证字幕 cue、master limiter、ducking `{enabled,dialogueIntervals,attenuationDb,attackFrames,releaseFrames,targetTracks}`、ExportDiagnosticTarget、ExportJob 与 `exportProfile:"light"`。MVP-A `light` 只返回 manifest/reference-only package，不内嵌媒体载荷，并必须显示 owner 验证后的 authorization/license、loudness report、Model/Profile/CapabilitySnapshot、SkillRevision、parameters 与 usage/cost value/status/source；任一必填字段缺失时 UI 不显示可下载成功包。ExportJob 只允许 `queued|preflighting|rendering|packaging|succeeded|failed|cancel_requested|cancelled`，packaging 另显示 `uploading|verifying|registering` subphase。所有 `TrimClip`、`SplitClip`、`ReorderClips`、`DeleteClip`、静态 transform、SoundCue、音量、mute/solo、fade、ducking、字幕和 publish command 带 expectedRevision；成功后读取 owner 持久化的新 Cut revision或新增 Version，409 `revision_conflict` 回滚乐观 state 后 refetch，`missing_asset`、`frame_out_of_bounds`、`validation`、`renderer_unconfigured`、`render_preflight_failed`、`render_failed` 显示原始诊断。

### 3. BDD: tracks, audio and export

视觉 Clip lanes 只承载 video/image，caption display 独立承载字幕 cue，SoundCue audio tracks 只承载 dialogue/music/ambience/effects；cue inspector 显示入点/时长、触发目标与 offset、priority、continuityRefs、静态音量、mute/solo 和线性 fade-in/fade-out。UI 只选择同 Episode owner 返回的 Scene/Shot trigger targets，以 stable ID/revision 提交；不自行解析 startFrame、不复制 continuity owner 内容、不暴露任意音量关键帧曲线。Ducking 控件只编辑 owner command：`enabled`、dialogue interval source/preview、`attenuationDb`、`attackFrames`、`releaseFrames`、`targetTracks`；UI 不生成 FFmpeg filter，dialogue 不可作为 target。字幕只由用户编辑 text/startFrame/endFrame 并可预览 SRT；导出面板显示 MP4/SRT/light job 的 preflight/status、packaging upload/verify/register progress 和 artifact reference，不能将未配置 renderer、unknown upload 或未登记 artifact 伪装成完成。

重拍替换入口只在 owner 返回 new video accepted-current eligibility、ready derivative 和一个明确旧 Clip match 时可用。确认页展示 exact Clip、old/new AssetVersion/hash/fingerprint、保留的 frame/transform/transition 和时长不兼容诊断；提交 `ReplaceClipSource(expectedRevision)`，409 回滚/refetch。成功后只更新 current Cut，UI 不自动 publish/export。项目多集导出位于 `/projects/:projectId/exports`，从项目 Episode 列表显式选择每个 published TimelineVersion 和 output base name，先展示全集合 preflight，再提交 EpisodeExportBatch；列表逐集展示 job/artifacts/失败，绝不在浏览器拼接或把 current version 自动加入。

Export diagnostic navigation 只消费 owner `ExportDiagnosticTarget`。Clip/Caption/SoundCue/AssetVersion 错误导航到同项目/同 Episode 的精确只读或 current Cut focus，renderer/storage 错误导航到项目设置；route 到达后重新校验 owner ID/revision。缺少/foreign/stale target 只显示 diagnostic，不从错误 message、列表下标或名称猜测位置，也不修改 published TimelineVersion。

### 4. TDD, UI system and compatibility

先覆盖 frame arithmetic、store 命令、Zod adapter、Query rollback、组件可访问性与 `Mock Provider +` 显式 Local test/offline profile E2E；后接 owner contract。shadcn/Radix 用于 tabs/menu/dialog/tooltip，Lucide 用于工具按钮，Tailwind 建立固定轨道/工具栏尺寸，避免嵌套卡片和文本按钮替代熟悉图标。AssetVersion 只以 owner ID/version 引用，仍兼容现有 HTTP `schema_version` 的 owner mapping；UI 不创建第二版本源。

## Dependency DAG

```text
AssetVersion + scenes/shots + workflows/runs + provider catalog
                         \        |        /
                  episode timeline/audio/export owner
                                  |
                    episode timeline editor UI
```

## Current / Defined / Todo

- **Current**：基础 Timeline schema/ORM 占位及 Web shell；无编辑器、音频控制、字幕或 export UI。
- **Defined**：每集隔离、30fps、轨道分类、基础混音、对白 ducking、手工字幕、light 导出状态和非目标。
- **Todo**：在 owner endpoints 可用后实现 route/store/adapter/components、`Mock Provider +` 显式 Local test/offline profile fixtures、E2E 和依赖安装。

## Risks / Trade-offs

- [非 30fps 或浮点输入] -> 明确诊断和阻断编辑，不转换或静默舍入。
- [版本冲突覆盖剪辑] -> expectedRevision、乐观回滚、authoritative refetch。
- [mute/solo 误作持久混音] -> 仅在 owner command 支持时写；否则临时 UI state 明确不改变 export。
- [renderer 不可用] -> 展示 `renderer_unconfigured`/原始错误，不显示成功 artifact。
- [ducking 参数与 owner RenderPlan 不一致] -> Zod/owner validation 拒绝非法区间、负/非整数参数和 dialogue target；preview 与 FFmpeg 只消费同一版本化 RenderPlan。
- [发布名称无效、preflight 过期或并发编辑] -> 发布按钮只在 current Cut 和最新成功 preflight 上可用；提交绑定 current `expectedRevision`，409 后丢弃旧 preflight、刷新 authoritative Cut 且不自动重试。

## Migration Plan

先以 `Mock Provider +` 显式 Local test/offline profile Timeline fixture 添加独立路由，保持现有 shell 无业务回归；owner API 实施后 adapter additive 接入。回滚删除新路由/state，不改写 TimelineVersion、ExportJob 或 AssetVersion。

## Acceptance Commands

`openspec validate implement-episode-timeline-editor-ui --strict --json`、`pnpm --filter @video-agent/web test`、`pnpm --filter @video-agent/web typecheck`、`pnpm --filter @video-agent/web lint`、`pnpm --filter @video-agent/web format:check`、`git diff --check -- openspec/changes/implement-episode-timeline-editor-ui`。

## Assembly selector and proxy Player

**DDD**：UI only holds playhead/selection state；owner Timeline owns reference and preview facts。**BDD**：loading/empty/error/duplicate/foreign/unaccepted/revision-conflict 可见；stale Cut 暂停 player。**SDD**：seek writes `currentTime=frame/30`，media clock quantizes/clamps to 30fps integer frame；DTO includes preview binding/fingerprint and does not expose final renderer inference。**TDD**：先写 frame clock/ended/error/stale and selector contract tests；默认 Mock preview，FFmpeg parity is media probe。非目标为 browser final render 和 AssetVersion writes；验收沿用既有 strict/E2E 命令。

导出 UI 逐条展示 MP4/SRT/light ExportArtifact 状态和安全短 TTL grant，不显示 objectKey/workspace URI；foreign/expired/held/unauthorized 仅显示 owner diagnostic。transition 控件只允许 `cut|crossfade`，以 30fps 整数 frame 发送 expectedRevision command，不在浏览器推导 RenderPlan 或绕过 parity gate。

素材箱 Query key、过滤 DTO、上传 reservation/session 状态和 AssetVersion/MediaProjection 均直接复用项目资产中心 owner adapter。Zustand 只保存 Timeline 当前显式 selection；它不得复制 catalog/filter/upload/usage 状态。selector 成功只提交 project/AssetVersion id/revision/hash、authorization summary、derivative fingerprint 与目标 Episode，最终引用仍由 Timeline command 决定。
