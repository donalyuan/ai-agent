## Why

阶段二 Timeline 先交付可恢复剪辑、关键帧、速度/循环和音频字幕能力；mask/track matte 会显著扩大渲染语义、代理规格和浏览器性能边界，因此拆成独立 change，避免影响基础 Timeline 和 portable RenderPlan。

## What Changes

- 新增 Timeline mask/track matte 数据模型、轨道关系和版本化命令。
- 支持静态 mask、路径/关键帧 mask、track matte 合成和能力上限。
- 统一 PixiJS 预览、FFmpeg 导出和 portable manifest 的 mask RenderPlan。
- 增加素材 fingerprint、代理降级、容量 admission、权限和 no-side-effect 失败处理。
- 通过真实素材基准冻结 mask 性能、最大点数、并发和导出限制。

## Capabilities

### New Capabilities

- `phase-two-timeline-mask`: Timeline mask/track matte 的 schema、编辑、预览、导出和性能边界。

### Modified Capabilities

- `episode-timeline-editing`: 增加 mask/track matte 的 typed command 和版本化引用约束。

## Impact

- 影响 `services/api` timelines、rendering、exports owner 和共享 Timeline Schema。
- 影响 PixiJS preview compiler、FFmpeg filter graph、Media Worker 和 portable package manifest。
- 需要新增 mask golden fixtures、代理素材基准和 capability probe；真实 FFmpeg/TOS 缺失时保持 `unconfigured`/`blocked`。
