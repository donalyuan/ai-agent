# ADR-0012：分离 Timeline、Media 与导出产物 owner

- 状态：已接受
- 日期：2026-08-23

## 决策

Episode 是 Timeline 聚合边界：每集只有一个 mutable current Cut，所有 typed command 使用
`expectedRevision` 并立即持久化；发布只追加命名、不可变的 `TimelineVersion`。Clip、四类
`SoundCue`、手工字幕、ducking 与静态 transform 使用 30fps 整数帧，素材只引用同项目/同集的
accepted-current `AssetVersion` 与 exact ready derivative；Timeline 不复制 Assets/Storage/Media owner
状态，也不因媒体 accept 自动替换 Clip。

Media Worker 独占 `MediaInspection`、`MediaDerivative` 与 `PreviewArtifact`，以 AssetVersion
id/revision/hash 形成 source fingerprint，并保存 canonical metadata、tool/schema/version、bounded output、
retention/license/hold。preview 与最终导出共用 canonical `RenderPlan`；MVP-A 转场仅 `cut|crossfade`，
parity gate 固定 SSIM >= 0.98 且时长、字幕、音频误差不超过 1 frame。

项目多集导出必须显式提交 Episode + immutable TimelineVersion 全集合；application 在任何写入前完成
renderer 与 canonical RenderPlan 全集预检，然后每集创建独立 ExportJob。Worker 使用稳定包含
`outputBaseName`、Episode ID 和 TimelineVersion ID 的文件名，生成并分别上传、校验、登记 MP4、UTF-8
SRT 与 `exportProfile=light` manifest。`packaging` 只用 `uploading|verifying|registering` 子阶段；未知
Storage 响应先按 operation key reconcile，不重渲染、不切换 profile、不伪造成功。

`ExportArtifact` 下载必须校验 Project -> Episode -> TimelineVersion -> ExportJob -> Artifact 完整归属链、
verified/retention/license/hold，并只签发最长 300 秒的 opaque read-only grant；公共响应不得包含
objectKey、workspace URI 或持久 URL。生产 FFmpeg 只接受显式 `FFMPEG_PATH`/`FFPROBE_PATH` 并逐项 probe
H.264/AAC decoder/encoder、`yuv420p` 与 MP4 mux/demux；缺失时保持
`renderer_unconfigured|renderer_capability_unsupported`，Mock renderer 只用于显式测试。

## 结果

- Alembic `0020_timeline_export_owner` 持久化 normalized current Cut/Clip/cue/caption、immutable Version、
  Media facts、Export batch/job/artifact/diagnostic，并保持可逆且不创建 TimelineDraft/第二 mutable Cut。
- `ExportDiagnosticTarget` 解析 Clip/Caption/SoundCue/AssetVersion/Artifact exact owner，renderer/storage 只
  定位项目设置；published TimelineVersion 始终只读。
- MVP-A 不实现 portable payload、跨集拼接、专业 NLE、automation/keyframes、TTS/ASR、自动字幕或回导。
