## Context

阶段一现有的 Workbench、Review、Asset Center、Timeline、Exports 和 Settings 页面已经消费 project-scoped owner API。产品架构确认的前端基线是源码型 `shadcn/Radix + Tailwind + Lucide`，其中 `apps/web/src/shared/ui` 是唯一共享入口；页面级 CSS 与第二组件库均被项目规则禁止。

这次请求是跨页面视觉重做，正式 UI 必须先经过静态原型确认。原型不得连接真实 API、数据库、上传或任何正式业务行为；其唯一作用是确认阶段一工作台的视觉语言和信息优先级。

## Design Direction

**对象与单一任务**：面向短剧制片人，帮助其在一个项目上下文内判断下一项可安全推进的制作动作。

**Token**：沿用已有 `ui-*` 语义 token，而不是在页面中声明颜色。它的实际调色为 Paper `#f8faf9`、Ink `#1f2933`、Teal `#0f766e`、Mint `#dff1ed`、Rule `#d7e1df`、Signal `#15803d`。正文沿用系统 sans；稳定 ID、版本和时码只使用既有等宽 utility。

**结构**：桌面采用项目栏、生产状态栏和三列工作面；移动端收为可横向浏览的状态栏与单列内容。生产状态栏是唯一的视觉签名，它连续显示文本审核、镜头素材、时间线、导出四个 owner handoff，而不是用装饰性卡片或营销式 hero 替代操作信息。

```text
+---------------- project / episode / readiness ----------------+
| brief -> review -> media -> timeline -> export                 |
+----------+-----------------------------+-----------------------+
| project  | selected episode / next step| factual inspector     |
| context  | image, review, timeline     | versions / warnings   |
+----------+-----------------------------+-----------------------+
```

**约束复核**：常见的深色霓虹工作台、暖色编辑页或杂志式大标题都与本项目的高频审查任务无关，因此不采用。独特性仅用于生产状态栏与真实素材画面；其余使用低密度 token、清晰状态和直接命令，避免视觉噪声。

## Architecture

1. 在 `apps/web/src/prototype/PhaseOneWorkbenchPrototype.tsx` 定义固定类型化演示数据和界面组合，只从 `shared/ui` 导入组件，直接使用 Lucide 图标。
2. 由 `App.tsx` 为 `/prototype` 加入独立路由；该路由不挂载 Query、mutation 或 fetch。正式路由与 owner API 不作行为改动。
3. 原型中的 tab、筛选与选中状态只能由 React 本地状态驱动。任何看似业务命令的按钮都必须是 disabled 或只改变本地展示，且明确标识为原型。
4. 所有布局使用 Tailwind utilities 和既有 token；`styles.css` 只继续承载 Tailwind import、主题映射和全局 base，不增加选择器、页面样式或原型专用规则。
5. 原型确认后，按页面逐项先补测试、再以相同 `shared/ui` 组件替换正式结构，同时保持既有 API contract、scope、CAS、unconfigured 和无副作用约束。
6. 正式迁移的文件边界固定为：`layouts/` 只承载应用壳层与导航；`pages/` 只承载路由级编排；业务状态、owner API 适配和复用展示组件分别留在各自功能域。不得继续向 `App.tsx` 或单一路由页面堆叠跨功能状态和展示细节。
7. 正式页面只能使用 Tailwind utilities、既有语义 token 与 `shared/ui`。迁移到的页面不得保留未定义的历史页面 class、页面级 CSS 选择器或重复的基础控件实现。
8. `/projects` 是项目索引而非项目工作台：它保留全局导航，但不继承项目深链的 `h-dvh` 或滚动裁切约束；内容应按自身高度结束。只有带项目 ID 的工作台深链保持全屏工作区布局。
9. 工作台的滚动画布在桌面端使用项目壳层提供的完整可用宽度；不得以通用阅读宽度上限在左右保留无内容区域。具体面板仍由各自布局约束其内容密度。
10. 当前剧集是工作台标题的上下文信息，应与创作模式和只读状态合并到标题操作区；不得为单一选择器单独创建横向状态栏。窄屏可自然换行，但语义和选择行为保持不变。

## DDD

原型不持有 Project、Episode、Run、AssetVersion、Timeline 或 Export 的领域事实，只显示无法被提交的演示投影。正式迁移仍由各 owner API 作为唯一事实源。

## BDD

制片人打开 `/prototype` 时能看见一条从文本审核到导出的连续生产状态、当前镜头的预览和版本检查信息；切换标签或选择镜头只改变静态显示，不产生网络请求或业务副作用。

## SDD

新增路由、一个原型组件和对应测试，不改变现有 HTTP route、DTO、schema、数据库、Worker 或依赖。图片只能来自已存在的 `public/prototype` 资产；原型不可成为正式导航的默认落点。

## TDD

先断言 `/prototype` 可渲染生产状态和原型标识，并断言页面渲染与本地交互不调用 `fetch`。实现后运行 Web 单元测试、typecheck、lint、format check，并用浏览器检查桌面和移动宽度下的路由、焦点和溢出。

## Risks And Mitigations

- 原型被误当作正式功能：所有业务动作禁用或仅本地展示，测试监控 `fetch` 零调用，页面有清晰原型标识。
- 与现有未提交改动冲突：只新增独立组件、测试、OpenSpec artifacts 和一个路由，读取并适配当前 `App.tsx`，不回退任何已有文件。
- 重做范围漂移到后端：规格明确禁止 API/schema/owner 行为变化；确认后每个正式页面仍使用原有验收测试约束。

## Confirmation Gate

用户已明确确认原型方向。正式迁移按 Workbench、Review、Assets、Timeline、Exports、Settings 顺序执行；每页完成时先保持既有 owner API 行为与回归测试，再替换视觉结构。最后再清理 `App.tsx` 的路由/壳层聚合和已迁移页面的历史样式实现。
