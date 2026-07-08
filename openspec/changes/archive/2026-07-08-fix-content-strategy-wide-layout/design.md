## Context

当前内容策略页使用 `.contentStrategyGrid { grid-template-columns: 360px minmax(360px, 1fr) 360px; }`。在 2552px 宽屏下，主工作区宽度为 2280px，中间选题池被拉伸到 1496px，右侧详情栏仍为 360px，形成明显比例失衡。

本次已通过 Pencil MCP `snapshot_layout` 读取 `docs/prototypes/video-agent/video-agent.pen`，返回无布局问题；但当前会话暴露的 Pencil MCP 工具不支持节点编辑，`open_document` 返回 `No handler found for method 'open-document'`，因此本轮无法通过 Pencil MCP 写入原型节点变更。

## Goals / Non-Goals

**Goals:**

- 超宽桌面下限制内容策略工作区最大宽度。
- 让当前选题池、选题 Agent、选题详情保持稳定可读比例。
- 用 E2E 覆盖 2552px 宽屏，防止选题池再次无限扩张。

**Non-Goals:**

- 不新增移动端布局。
- 不调整历史生成页信息架构。
- 不修改选题数据、Agent 生成逻辑或 API。

## Decisions

- 内容策略工作区使用左对齐 `max-width`，而不是居中。这样保持工作区仍贴近左侧业务菜单，不在菜单与内容之间制造大块空白。
- 当前选题池三栏改为 `360px + 1fr + minmax(360px, 420px)`，并配合工作区最大宽度，使选题池不会在超宽屏无限拉长，同时详情栏在大屏有更合理宽度。
- 保留 `body min-width: 1440px`。当前产品只覆盖桌面端运营后台，不在本次引入窄屏响应式重排。

## Risks / Trade-offs

- 超宽屏右侧会留下空白区域 -> 通过左对齐和最大宽度换取列表可读性，避免主要内容被过度拉伸。
- 详情栏放宽后 1440px 基准宽度仍需保持不溢出 -> 使用 `minmax(360px, 420px)`，小桌面保持 360px。
