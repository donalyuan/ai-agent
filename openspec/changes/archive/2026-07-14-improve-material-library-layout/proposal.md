## Why

素材库当前默认选中首条素材并常驻展示完整详情表单，右侧面板持续占用画布空间；同时 Konva 节点使用固定三列坐标且标题没有稳定高度，长文件名会与元信息重叠。用户已基于实际页面截图确认 v2 Pencil 原型，需要让详情职责更明确，并彻底修复画布节点的排布与文本边界。

## What Changes

- **BREAKING**：素材库加载完成后不再自动选中首条素材，右侧详情默认隐藏。
- 点击资产列表或画布节点时打开详情抽屉；关闭抽屉时清除当前选择；点击“新增素材”时直接打开新建抽屉。
- 详情抽屉继续承担素材编辑、保存、归档和恢复，并只展示当前素材类型相关的扩展字段。
- 画布节点改为统一尺寸，长文件名固定在两行区域内截断，元信息位置保持稳定。
- 节点列数和位置根据工作区宽度及详情抽屉状态计算，避开资产栏、详情抽屉和底部工具栏。
- 更新前端单元测试和 E2E，覆盖详情开关、类型字段和布局边界。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `material-library-management`：修改素材库画布工作台的详情展示时机、节点布局和表单字段可见性要求。

## Impact

- 前端：`apps/video-agent/app/page.tsx`、素材库页面组件、Konva 画布组件和样式。
- 测试：`apps/video-agent/app/page.test.tsx`、素材布局单元测试和 `apps/video-agent/e2e/workspace.spec.ts`。
- 原型：已在 `docs/prototypes/video-agent/video-agent.pen` 新增素材库 v2 默认状态与详情打开状态。
- API、数据库和 Worker：无变化。
