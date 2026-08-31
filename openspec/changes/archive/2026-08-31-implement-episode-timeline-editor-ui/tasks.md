## Stage 1 Timeline Editor Tasks

- [x] 1.1 Owner contracts verified for Timeline/Audio/Export/AssetVersion/Scene/Run; per-Episode scope, 30fps, revisions, ExportJob and diagnostics confirmed.
- [x] 1.2 Query keys isolate current Cut and immutable TimelineVersion; only read-only `versionId` URL state is retained and no media bytes are cached.
- [x] 1.3 Zod fixtures cover 30fps, integer frames, clip transforms, SoundCue tracks, caption, ducking, automation/keyframe rejection and export failure contracts.
- [x] 1.4 `timelineApi` uses explicit Local/Mock profile, schema parsing, expectedRevision commands and authoritative refetch on 409.
- [x] 1.5 TimelineVersion publish uses owner preflight, safe name, current Cut revision and explicit confirmation; renderer remains an export preflight.
- [x] 1.6 ReplaceClipSource and EpisodeExportBatch owner fixtures preserve exact source/revision/derivative facts and per-Episode outputs.
- [x] 2.1 Episode timeline route reads one current Cut, immutable versions and optional read-only compare without cut creation/selection.
- [x] 2.2 Timeline UI exposes integer-frame split/delete commands and owner CAS semantics; unsupported keyframes remain blocked by owner.
- [x] 2.3 Separate video/audio/caption lanes display SoundCue timing/trigger/priority/continuity and ducking facts.
- [x] 2.4 Manual caption text/start/end editing is wired to `UpsertManualCaption`; unsupported automation/NLE features remain absent.
- [x] 2.5 Exports page creates explicit light EpisodeExportBatch and reads per-batch jobs/artifacts while retaining renderer/storage diagnostics.
- [x] 2.5a Export diagnostics remain owner-targeted; UI never infers message/index targets or mutates on diagnostic reads.
- [x] 2.6 Version naming, publish CAS, read-only comparison and failure/no-retry behavior are covered by API/domain tests and UI state.
- [x] 2.7 Replacement remains explicit owner `ReplaceClipSource`; Review handoff never auto-selects or edits Timeline.
- [x] 2.8 Exports route requires explicit Episode/TimelineVersion/output name and never auto-selects current or concatenates episodes.
- [x] 3.1 Stable tracks, toolbar, icon actions, keyboard labels, tooltips and responsive layout use existing Lucide/Tailwind conventions.
- [x] 3.2 Real Playwright navigation verifies Episode selection, Timeline projection and explicit renderer `503 renderer_unconfigured`; no FFmpeg is invoked.
- [x] 3.3 Web tests/typecheck/lint/format, API timeline/catalog/storage tests, strict validation and diff checks pass.
- [x] 4.1 Timeline reuses Asset Center handoff/query ownership; no second upload or metadata/version store exists.
- [x] 4.2 Timeline display uses integer frame positions and deterministic frame-to-time labels; stale preview is owner-blocked.
- [x] 4.3 Contract/API fixtures and browser evidence cover Mock preview and renderer probe boundary.
- [x] 4.4 Export artifacts stay opaque owner grants; UI does not leak object URI and renders only 30fps supported transitions.

## Stage 5 Review Consistency Fixes

- [x] 5.1 Align the strict SoundCue Zod contract with the owner projection, including authorization/license status, structured trigger/continuity references and `scene_start|shot_start|shot_end`, with valid/invalid fixtures.
- [x] 5.2 Let users build a non-empty deduplicated list of multiple Episode + published TimelineVersion + output name selections and submit the whole batch for one owner preflight.
- [x] 5.3 Derive failed member IDs from batch jobs, require an explicit non-empty failed-member selection, and send those Episode IDs to retry without automatic retry or empty requests.
- [x] 5.4 Request and open the existing short-TTL artifact download grant only for registered successful MP4/SRT/light artifacts; keep foreign/expired/held/unauthorized artifacts disabled with owner diagnostics.

## 6. 阶段一组件与媒体领域验收

- [x] 6.1 使用 `react-resizable-panels` 完成桌面固定分区，使用 dnd-kit 完成同父 Clip pointer/keyboard 排序；不得实现 IDE 自由停靠或跨父移动。
- [x] 6.2 使用 TanStack Virtual 渲染长 Clip/日志列表；添加稳定尺寸、键盘/ARIA、虚拟 DOM 有界和排序 CAS/零越权写入测试。
- [x] 6.3 在领域模块接入 PixiJS、WaveSurfer.js、HLS.js，分别验证非空画面、波形 play/pause/seek、同源代理 loading/ready/error；不得将其导出到 `shared/ui`。
- [x] 6.4 保持 React Flow 只读和 graph mutation zero-write，运行桌面 Chrome/Edge Timeline/Exports focused E2E、strict validation 与 `git diff --check`；全部任务保持未勾选直至验收。
