## Context

当前阶段二 Timeline 不包含 mask/track matte。该能力会同时影响 Timeline owner、PixiJS、FFmpeg、Media Worker、portable manifest 和浏览器性能，因此独立于基础剪辑 change。

## Goals / Non-Goals

**Goals:**

- 建立版本化 mask/track matte schema、命令和 RenderPlan。
- 保证预览、导出和 portable 使用同一 mask 语义。
- 以 capability、容量、权限和性能基准控制复杂度。

**Non-Goals:**

- 不实现完整专业合成、3D 跟踪、实时 4K 或任意 FFmpeg 脚本。
- 不修改阶段二基础 Timeline 的既有非 mask 命令。

## Decisions

- mask 作为 Timeline typed command 的不可变 payload，所有时间使用 30fps 整数帧；路径点、羽化、反转和 matte source 均有明确 schema 上限。
- canonical RenderPlan 先生成 mask 中间表示，再分别编译 PixiJS 和 FFmpeg；两者必须共享 plan hash。
- mask 只引用同项目已授权 AssetVersion/Clip，接受 expectedRevision 和 track lock；不复制或覆盖 AssetVersion。
- 先以 Mock/Local/Fake FFmpeg 验收，真实 FFmpeg/TOS 缺失时保持 `unconfigured`/`blocked`。
- 通过真实素材 benchmark 冻结最大点数、关键帧数、代理规格和并发；手工取消和重启恢复使用同一 operation key。

## Risks / Trade-offs

- [Risk] 路径插值和 matte 合成造成预览/导出漂移 → 以 golden parity 阻断不一致的导出。
- [Risk] 大量路径点导致浏览器卡顿 → 分层加载、代理预览、可见元素渲染和硬上限。
- [Risk] mask 引用跨项目或过期 → owner scope、revision、hash 和 license admission 在命令前校验。

## Migration Plan

先新增 schema、表和 feature gate，不改变既有 TimelineVersion；启用时仅对明确选择 mask 的新命令生效。失败时关闭 gate，保留历史版本和诊断，不删除已有 Clip/Cue。

## Open Questions

- 默认 mask 点数、关键帧数量和代理分辨率需由真实素材 benchmark 冻结。
