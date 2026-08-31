## ADDED Requirements

### Requirement:局部编辑计划和候选闭环
系统 SHALL 支持绑定基础 AssetVersion 的 image/video/audio 局部编辑计划，计划 MUST 冻结 selection/mask/range、base revision/hash、Provider capability、参数、费用和预期输出。execute 只能创建 intent/Outbox，结果先进入候选；用户显式确认后才可通过 owner command 更新引用范围。

#### Scenario:图片局部编辑候选
- **WHEN** 用户对同项目 image AssetVersion 提交合法 selection 并确认执行
- **THEN** 系统创建可追踪 execution intent 和候选版本，保存输入 hash、能力快照、费用和 impact/stale 集合

#### Scenario:视频时间范围编辑
- **WHEN** Provider capability 明确支持视频时间范围且 range 在媒体边界内
- **THEN** 系统只生成该范围的候选，不覆盖原 AssetVersion 或 Timeline 引用

### Requirement:局部编辑安全边界
系统 MUST 拒绝跨项目、版本过期、未知字段、越界 selection/range、未 probe、quota unknown 或成本未确认的局部编辑；不支持的类型或能力 MUST 返回 `unsupported_feature`，不得隐式降级、重复提交或直接写 owner。

#### Scenario:基础版本变化
- **WHEN** execute 或 accept 时 base AssetVersion revision/hash 已变化
- **THEN** 返回 409/stale，零 ProviderCall、Storage mutation、owner mutation 或重复收费

#### Scenario:不支持的局部能力
- **WHEN** 请求包含未登记的 mask、selection 或时间范围能力
- **THEN** 返回 `unsupported_feature`，不创建 intent、Outbox、ProviderCall 或候选
