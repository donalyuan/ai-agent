## Purpose

定义阶段一制片工作台的静态原型、零业务副作用确认门，以及确认后使用共享组件库迁移正式页面的约束。该规格确保原型与正式页面共享可访问的组件基础，同时不改变既有 owner API、项目范围、revision CAS 或显式业务命令语义。

## Requirements

### Requirement:静态阶段一工作台原型

系统 SHALL 在 `/prototype` 提供一个仅使用固定演示数据的阶段一制片工作台原型。原型 SHALL 呈现项目上下文、文本审核、镜头素材、时间线和导出之间的连续生产状态，以及可扫描的当前镜头与版本检查信息。它 MUST 使用 `shared/ui`、Radix 无障碍原语、Tailwind 语义 token 和 Lucide 图标，且 MUST NOT 新增页面级 CSS、第二套组件库或自定义基础控件。

#### Scenario:查看静态生产状态

- **WHEN** 用户打开 `/prototype`
- **THEN** 页面显示固定的项目、Episode、生产 handoff、镜头素材和版本检查信息，且所有状态来自静态演示数据

#### Scenario:在原型中改变本地展示

- **WHEN** 用户切换原型标签或选择另一条镜头展示项
- **THEN** 页面只更新本地展示状态，不创建 Project、Run、UploadSession、AssetVersion、TimelineVersion 或 ExportJob

### Requirement:原型零业务副作用

原型页面加载、标签切换和任意可用控件 MUST NOT 调用 owner API、`fetch`、Query mutation、上传、Provider、存储或导出操作。原型中的业务命令 MUST 保持禁用或只承载无副作用的本地展示。

#### Scenario:反复访问原型

- **WHEN** 用户刷新 `/prototype` 并重复切换其可用控件
- **THEN** 不发生网络请求或业务状态变化，正式 `/projects` 路由与其 API 行为不受影响

### Requirement:正式迁移须经确认

在用户明确确认原型的视觉与信息架构前，系统 MUST NOT 用原型替换任一正式阶段一页面、接入真实 owner API、修改 owner command 或移除既有正式业务页面。确认后正式迁移仍 MUST 保留现有 project scope、CAS、`unconfigured`、owner 事实和 MVP-A 非目标。

#### Scenario:原型尚未确认

- **WHEN** 原型完成本地技术验证但用户尚未确认
- **THEN** `/prototype` 可单独访问，正式 `/projects`、Workbench、Review、Assets、Timeline、Exports 和 Settings 继续保持当前业务行为

### Requirement:确认后按组件库迁移正式页面

用户确认原型后，系统 SHALL 按 Workbench、Review、Assets、Timeline、Exports、Settings 的顺序，将对应正式页面迁移到 `shared/ui`、Radix 无障碍原语、Tailwind 语义 token 与 Lucide 图标。迁移 MUST 保留既有 owner API、project scope、revision CAS、显式业务命令与 `unconfigured` 语义，且 MUST NOT 新增页面级手写 CSS。

#### Scenario:进入已迁移的正式页面

- **WHEN** 用户从项目导航进入一个已迁移的阶段一页面
- **THEN** 页面使用共享工作台壳层呈现项目上下文和对应功能，保留原有路由、读取范围和显式命令行为

### Requirement:正式前端按功能拆分

系统 SHALL 将应用壳层、路由级页面、功能状态/API 适配和可复用展示组件分置于独立文件与功能域。`App.tsx` MUST 只负责应用 Provider 与路由组合；迁移后的路由页面 MUST NOT 承载跨功能展示组件或未定义的历史页面 class。

#### Scenario:维护一个阶段一页面

- **WHEN** 维护者修改一个正式阶段一功能
- **THEN** 相关状态与 API 适配位于该功能域，共享壳层与基础控件不需要被复制或在单一页面文件中重写
