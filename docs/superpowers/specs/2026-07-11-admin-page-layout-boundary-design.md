# Admin 页面布局边界修复设计

## 问题

`AdminShell` 的 `.adminWorkbench` 同时承担公共壳层和首页两行布局，固定声明 `grid-template-rows: auto 1fr`。模型管理页把 Header、Tab、筛选栏、错误提示和表格作为多个直接子节点传入后，这些节点被分配到显式及隐式 Grid 轨道，最终在高视口中被拉散。

## 设计

`AdminShell` 只负责左侧导航和右侧内容区域，不规定业务页面内部轨道。首页使用 `adminOverviewPage` 管理 Topbar 与能力列表；模型页使用 `modelManagementLayout` 以纵向 Flex 排列 Header、Tab、筛选栏、提示和表格。模型表格区域占剩余空间，但内容不足时仍紧跟筛选栏，不制造大块空白。

## 验收

- `1920x948` 下 Header、Tab、筛选栏、表格连续排列。
- Tab 顶部距 Header 底部不超过 2px。
- 筛选栏顶部距 Tab 底部不超过 2px。
- 表格顶部距筛选栏底部不超过 2px。
- 首页 Topbar 与管理能力列表保持正常排列。
- 添加、编辑抽屉与确认弹窗不受布局容器影响。
