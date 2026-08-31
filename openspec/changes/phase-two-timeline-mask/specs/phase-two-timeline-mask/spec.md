## ADDED Requirements

### Requirement:版本化 Timeline mask 与 track matte
系统 SHALL 支持带 `expectedRevision` 的 mask/track matte typed command。mask 必须声明 source Clip、时间范围、路径/关键帧、羽化、反转和能力版本；track matte 必须引用同一 Timeline 内已授权的 matte source。所有时间和上限 MUST 通过 schema 校验，历史 TimelineVersion 不得被覆盖。

#### Scenario:提交合法 mask
- **WHEN** editor 对当前 revision 提交在容量和 capability 上限内的 mask
- **THEN** 系统追加新 Timeline revision 和 provenance，旧版本保持可读

#### Scenario:拒绝越界 mask
- **WHEN** mask 包含浮点帧、越界路径点、跨项目 source 或超过能力上限
- **THEN** 返回 validation/forbidden，零 Clip、Outbox、ProviderCall 或 AssetVersion 写入

### Requirement:mask 预览导出和 portable parity
PixiJS 预览、FFmpeg 导出和 portable manifest MUST 消费同一个 mask RenderPlan 和 plan hash；代理降级、renderer 未配置或编译结果不一致时 MUST 阻断成功。

#### Scenario:预览导出 hash 一致
- **WHEN** mask RenderPlan 在 PixiJS 和 FFmpeg 编译器中得到相同 hash
- **THEN** 预览和导出使用相同时间边界、路径和 matte 关系

#### Scenario:编译器不一致
- **WHEN** 两个编译器的 mask plan hash 或关键结果不一致
- **THEN** 返回 renderer diagnostic，不生成 succeeded artifact 或 portable registration
