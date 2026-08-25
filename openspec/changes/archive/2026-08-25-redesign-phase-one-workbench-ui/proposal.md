## Why

阶段一已具备五个 owner UI 闭环，但当前交付仍混有大型手写页面结构和历史样式迁移痕迹，难以稳定复用已确认的组件基线。需要先用静态、可运行原型确认制片工作台的信息架构和视觉方向，再将已存在的业务页面逐步重写为共享 `shadcn/Radix + Tailwind + Lucide` 组件组合。

## What Changes

- 新增只读 `/prototype` 路由，使用固定演示数据呈现项目工作台、文本审核、素材、时间线和导出之间的生产状态；它不读取或写入 owner API，不创建项目、Run、上传或导出。
- 将阶段一正式前端的后续重写限定为既有 `shared/ui` 组件、其语义 token、Radix 无障碍原语和 Lucide 图标；禁止页面级手写 CSS、第二套组件库和重复的基础组件变体。
- 保留所有已定义的项目范围、owner command、scope/CAS、外部能力 `unconfigured` 语义和 MVP-A 非目标；本 change 不修改 API、领域规则或数据模型。
- **BREAKING（内部实现）**：原有正式页面中遗留的自定义页面样式和重复基础控件将在原型获确认后移除或迁移，视觉结构可改变，但已经规格化的交互语义与路由 contract 保持不变。

## Capabilities

### New Capabilities

- `phase-one-component-library-prototype`: 阶段一工作台的静态、可运行组件库原型及其确认门。

### Modified Capabilities

- 无。

## Impact

- 影响 `apps/web/src` 的路由组合、共享 UI 消费方式和后续页面实现；先新增原型组件及其测试，正式 owner API 页面在用户确认前不接入或改写。
- 复用当前 `apps/web/src/shared/ui`、Radix、Tailwind、Lucide 和仓库已有 `/public/prototype` 图片，不新增 UI 依赖、不修改后端、schema 或 Compose 环境。
