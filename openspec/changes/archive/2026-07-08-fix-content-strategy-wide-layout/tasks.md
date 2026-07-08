# fix-content-strategy-wide-layout Tasks

## 1. OpenSpec

- [x] 创建 proposal、design、spec 增量和 tasks。
- [x] 运行 `openspec instructions apply --change "fix-content-strategy-wide-layout" --json`，确认 change 可识别。

## 2. 前端布局修复

- [x] 为内容策略当前选题池增加 2552px 宽屏 E2E 回归断言，并先确认红灯失败。
- [x] 调整内容策略工作区最大宽度和三栏比例。
- [x] 确认 1440px 桌面基准布局不回退、不横向溢出。
- [x] 补充选题卡片胶囊和操作按钮不得越界的 E2E 回归并修复卡片高度。

## 3. 验证

- [x] 运行相关 E2E。
- [x] 运行前端 lint。
- [x] 运行 `openspec instructions apply --change "fix-content-strategy-wide-layout" --json` 并确认任务状态与实际一致。
