## ADDED Requirements

### Requirement: 内容策略页必须在超宽桌面保持可读比例

`apps/video-agent` 内容策略当前选题池视图 SHALL 在超宽桌面视口下限制工作区和选题池的横向扩张，使选题 Agent、选题池和选题详情保持可读比例。

#### Scenario: 超宽桌面下选题池不无限拉伸

- **GIVEN** 操作者在 2552px 宽桌面视口打开内容策略当前选题池
- **WHEN** 页面完成加载并展示选题池、选题 Agent 和选题详情
- **THEN** 内容策略工作区 SHALL NOT 铺满整个剩余视口宽度
- **AND** 选题池宽度 SHALL NOT 超过 1040px
- **AND** 选题详情栏宽度 SHALL 至少为 420px
- **AND** 选题卡片内状态胶囊、质量胶囊、评分和移除按钮 SHALL NOT 超出卡片边框
- **AND** 页面 SHALL NOT 改变选题筛选、选题列表滚动和选题详情操作能力
