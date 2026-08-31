## ADDED Requirements

### Requirement:可恢复高级编辑命令
系统 SHALL 支持复制、吸附、Undo/Redo、版本恢复、速度/循环、轨道锁定、关键帧和字幕样式命令；每个命令携带 expectedRevision，恢复只能创建新 current revision，不覆盖历史 TimelineVersion。mask/track matte 不属于本 capability，由独立 `phase-two-timeline-mask` change 负责。

#### Scenario:撤销并恢复为新版本
- **WHEN** 用户在 current Cut 上执行 Undo 或选择历史版本恢复
- **THEN** 系统追加 command history 和新 revision，发布中的导出继续读取原冻结版本

#### Scenario:锁定轨道拒绝写入
- **WHEN** 非持有者尝试修改 locked track
- **THEN** 返回 forbidden/track_locked，零 Clip、Cue、caption 或 Outbox 写入

### Requirement:高级媒体与预览渲染保持 parity
TTS、ASR、速度、关键帧和字幕样式 SHALL 编译到版本化 RenderPlan；PixiJS 预览、FFmpeg 导出和 portable manifest MUST 使用同一 plan hash，并以 golden fixtures 验证时长、字幕边界和音频同步。

#### Scenario:ASR 草稿接受后写入字幕
- **WHEN** ASR 返回带 source hash 的时间戳，用户显式接受并通过 CAS
- **THEN** 生成可编辑 caption revision，保留原手工字幕版本并记录 ProviderCall provenance

#### Scenario:渲染语义不一致时阻断导出
- **WHEN** 浏览器编译器和 FFmpeg 编译器的 plan hash 或关键结果不一致
- **THEN** 导出失败并指向 render diagnostic，不生成 succeeded artifact

### Requirement:Timeline 整体复制与独立自动保存
系统 SHALL 支持以明确的 source `TimelineVersion` 或 current Cut 创建新的 Timeline/Cut，重绑定目标 project、episode 和 scope，并保留来源 hash 与 provenance。TimelineDraft 的自动保存、checkpoint、dirty/clean 和恢复 MUST 独立于 current Cut 的发布命令；自动保存失败或恢复只能追加新 revision，不得覆盖历史版本或已冻结导出。

#### Scenario:复制整条 Timeline
- **WHEN** 用户选择一个可读的 TimelineVersion 复制到目标集并确认引用映射
- **THEN** 系统创建新的 current Cut/草稿，所有 Clip、Cue、caption 引用按目标 scope 校验，源 TimelineVersion 保持不变

#### Scenario:自动保存中断后恢复
- **WHEN** TimelineDraft 自动保存过程中 API 或浏览器重启
- **THEN** 以相同 draft/checkpoint key 恢复已确认的命令，不重复写入、不覆盖 current Cut，并报告 dirty/clean 状态
