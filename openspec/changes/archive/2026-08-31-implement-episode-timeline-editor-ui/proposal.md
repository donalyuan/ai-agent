## Why

每集唯一 current Cut、可命名的不可变 TimelineVersion、整数帧编辑、音频和手工字幕已在后端 timeline change 中定义，但阶段 0 壳层没有让用户检查、编辑和提交这些受版本保护事实的界面。需要以一个轻量、可测试的编辑器 UI 闭环呈现 MVP-A 的每集剪辑与导出状态，而不承诺专业 NLE 能力。

## What Changes

- 新增每集唯一 current Cut 路由、命名不可变 TimelineVersion 的显式预检/发布、读取/只读比较、30fps 整数帧轨道编辑、裁剪、拆分、排序、明确删除、静态 position/scale/opacity 和 revision conflict 恢复交互；发布只追加版本且不切换或覆盖 current Cut，每次成功 command 立即呈现 owner 持久化 revision。
- 分离视频/图片 Clip lanes、caption display 与 SoundCue audio tracks；SoundCue 仅支持 `dialogue|music|ambience|effects`，展示/编辑整数入点/时长、manual/scene/shot trigger、priority、continuity refs、静态音量、mute/solo、线性淡入淡出、对白 ducking 参数、master limiter 和手工字幕；不提供任意音量关键帧曲线。
- 新增 MP4、SRT 与 MVP-A `exportProfile:"light"` manifest/reference-only package 的预检/导出状态展示；ExportJob 固定为 `queued|preflighting|rendering|packaging|succeeded|failed|cancel_requested|cancelled`，`packaging` 内显示 uploading/verifying/registering 进度。预检错误可跳到精确 Clip/Caption/SoundCue/AssetVersion 或 renderer/storage 设置；默认使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），显示可诊断失败而不伪造渲染成功。
- 在重拍视频已接受且派生物 ready 后提供针对既有 Clip 的精确“替换来源”操作，展示 old/new source 与影响并使用 current Cut expectedRevision；成功后提示用户另行发布新 TimelineVersion，不由审核接受自动改 Timeline。
- 提供 `/projects/:projectId/exports` 项目级多集导出选择与批次状态：用户显式勾选 Episode + published TimelineVersion，按集分别命名、预检和显示 MP4/SRT/light artifacts；不自动选择 current、不扩大范围或拼接多集。
- `light` manifest 视图校验并展示 authorization/license、loudness、Model/Skill/parameters/cost source 全部必填审计字段；字段缺失时不得显示可下载成功包。
- 明确不实现 `TimelineDraft`、多个 mutable Cut 的创建/选择/切换、Fish Audio、Narration/TTS、Groq、自动字幕或自动对齐、复杂关键帧、循环、调速、轨道锁定、Timeline 独立自动保存、撤销/重做、版本恢复、字幕样式、审核评论/时间码/提醒、工程包回导、多机位和专业调色。

## Capabilities

### New Capabilities
- `episode-timeline-editor-ui`: 面向单集唯一 current Cut、命名不可变版本、基础混音、手工字幕和导出状态的桌面 UI contract。

### Modified Capabilities
- 无。

## Impact

- 后续实现将修改 `apps/web`，并消费 `implement-episode-timeline-audio-export`、Scene/Shot、AssetVersion 与 WorkflowRun owner contract。
- UI 使用现有 React/Vite/Router 与 Lucide，并在后续实现中引入计划中的 TanStack Query、Zustand、Zod、shadcn/Radix、Tailwind 能力；本 change 不修改依赖或业务代码。

## 素材箱与 Player

本 change 的素材箱复用 `implement-project-asset-center` 的 selector/query/upload projection，并消费 Timeline owner 的 expectedRevision assembly commands 和 proxy preview DTO；它不维护第二套上传、筛选、授权、版本、usage 或派生状态。Player 用 30fps integer-frame playhead 支持 play/pause/seek/frame-step/ended/error。它不创建 AssetVersion、RenderPlan 或 final browser renderer。

## TimelineVersion 发布 UI 合同

用户必须从可编辑 current Cut 显式输入版本名称、运行 owner preflight 并以当前 `expectedRevision` 发布；成功后 UI 读取新追加的不可变 TimelineVersion 并允许只读比较。名称无效、preflight 失败、跨 Episode 或 revision 过期时不得创建 TimelineVersion；409 必须刷新 authoritative current Cut，且不得自动重试发布。这里的“发布”只指冻结 TimelineVersion，不是发布 Workflow，也不是分发到内容平台。

## Artifact/transition UI 合同

**DDD**：UI 只消费 Timeline/ExportArtifact owner facts。**BDD**：三个 artifact 独立状态、held/expired/unauthorized download 与 parity/transition failure 可见。**SDD**：不展示 objectKey/workspace URI，transition selector 仅 `cut|crossfade` 与 owner validation。**TDD**：覆盖 short-grant、30fps adjacency/overlap 与 stale/parity diagnostics。

## Ducking UI contract

UI 只编辑 owner 的 ducking command，字段为 `enabled`、dialogue interval source/preview、`attenuationDb`、`attackFrames`、`releaseFrames` 和 `targetTracks`；UI 不自行生成 FFmpeg filter。非法区间、dialogue target 或非整数参数必须显示 owner diagnostic，并保持当前 TimelineVersion 不变。

## 阶段一组件使用边界

Timeline MUST 复用创作工作台的 `shared/ui` 基线，并使用 `react-resizable-panels` 实现固定素材箱/预览/检查器/时间线分区的有限可调尺寸；不得提供 IDE 自由停靠。Clip 同父排序只允许 dnd-kit pointer/keyboard sensors，TanStack Virtual 负责长 Clip/日志列表。PixiJS 仅负责 Timeline 画面预览，WaveSurfer.js 仅负责音频波形，HLS.js 仅负责同源代理播放；这些领域组件不得进入 `shared/ui`。

Timeline 的 React Flow 入口只能显示固定 published WorkflowVersion 的只读投影，MVP-A 不提供图编辑、连线、保存或发布。验收必须证明共享面板、同父 Clip 排序、虚拟化、Pixi/WaveSurfer/HLS loading/ready/error 与 30fps frame state 均可观察，并保持 owner command、TimelineVersion 和导出边界。
