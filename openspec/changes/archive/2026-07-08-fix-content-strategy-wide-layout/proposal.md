## Why

内容策略页在超宽桌面下会把中间选题池无限拉宽，导致选题列表变成长条、右侧详情栏相对过窄，整体信息比例失衡。用户已用 2552px 宽屏截图确认该问题不是截图裁切，而是页面布局本身缺少大屏比例约束。

## What Changes

- 为内容策略当前选题池视图增加超宽桌面布局约束。
- 限制内容策略工作区最大宽度，避免选题池随视口无限扩张。
- 允许右侧选题详情栏在大屏下适度放宽，提升详情阅读比例。
- 增加 E2E 布局回归，覆盖 2552px 宽屏比例。

## Capabilities

### New Capabilities

### Modified Capabilities

- `content-topic-management`: 内容策略页当前选题池在超宽桌面下必须保持可读的三栏比例。

## Impact

- 影响 `apps/video-agent/app/styles.css` 的内容策略布局样式。
- 影响 `apps/video-agent/e2e/workspace.spec.ts` 的视觉布局回归断言。
- 不影响后端 API、数据库、选题状态流转或移动端适配。
