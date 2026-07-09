# Material Library Management Design

## Goal

为视频工作台建设素材管理第一版“素材库”页面，先完成素材登记和管理闭环。第一版采用“画布优先”的素材画布工作台，只管理当前账号下已有素材 URL 和可选缩略图 URL 的业务元数据，不做真实文件上传、自动抽帧、自动元数据抓取、画布连线编排、Embedding、语义检索、素材检索 Agent、分镜素材候选或作品生产读取素材清单。

## DDD

`Material` 是素材库聚合根，归属一个真实 `projects.id`。前端继续以“当前账号”呈现该内容生产边界，后端沿用 `project_id` 外键。

画布第一版只是素材库的前端呈现方式，不新增 `CanvasNode`、`Edge` 或工作流编排聚合。素材节点由 `Material` 列表派生，节点位置第一版不持久化，刷新后按素材类型和更新时间自动排布。

素材类型为：

- `video`：视频素材，前端显示“视频”。
- `image`：图片素材，前端显示“图片”。
- `audio`：音频素材，前端显示“音频”。
- `subtitle`：字幕素材，前端显示“字幕”。

字幕素材仍按 URL 素材登记，`file_url` 指向 `.srt`、`.vtt`、`.ass`、`.txt` 等字幕文件，语言、格式等信息保存在 `metadata`。

缩略图第一版不作为独立文件资源管理，只作为素材 metadata 中的可选 `thumbnail_url`：

- 图片素材默认可用 `file_url` 作为资产栏和画布节点缩略图。
- 视频素材可手动填写封面图 URL。
- 音频素材可手动填写封面图 URL；未填写时显示音频类型占位。
- 字幕素材默认显示字幕类型占位；如填写 `thumbnail_url` 则用于资产栏和画布节点预览。

系统不得在第一版自动下载远程素材、截取视频帧、生成音频波形或自动抓取封面。

素材生命周期第一版只区分：

- `active`：可用素材，默认查询和画布视图展示。
- `archived`：已归档素材，不参与默认视图，但可筛选查看并恢复。

`usage_count` 第一版只读展示，不由用户手动编辑。后续脚本分镜、素材清单或作品生产引用素材时，再由对应业务链路维护使用次数。

## BDD

运营人员进入“素材管理 > 素材库”后，默认看到画布优先工作台：主区域是一整块素材节点画布，资产栏和详情栏只是画布上的辅助浮层或窄面板，不把页面切成三个等价栏目。页面可以按类型、状态、标签和关键词筛选素材。

运营人员可以新增素材，填写文件名、素材类型、素材 URL、可选缩略图 URL、标签、来源/授权备注和类型相关元数据。新增成功后，新素材出现在资产栏顶部和画布中，并被选中。

运营人员可以在画布节点中看到缩略图或类型占位，用于快速区分素材。运营人员可以选择一个素材节点查看详情并编辑基础信息。保存成功后，资产栏、画布节点和详情同步展示最新信息。

运营人员可以归档不再使用的素材。归档后素材从默认 `active` 视图移除；切换到 `archived` 筛选后可以查看并恢复。

空状态下页面显示空画布，并提供“新增素材”入口。

## Design System References

本页面沿用项目根 `DESIGN.md` 的 VEDIO-AGENT 工作台设计系统。正式原型参考：

- `Ant Design`：后台工作台分栏组织、筛选控件密度和操作位置。
- `IBM Carbon`：表单分组、可访问标签、错误提示、加载状态和空状态。
- `GitHub Primer`：低干扰边框、中性表面、状态标签和资产摘要扫描体验。

正式原型明确拒绝营销 hero、装饰渐变、大插画、多层卡片嵌套和上传/语义检索等非一期入口。画布背景可以使用低对比度网格，但不得使用深色娱乐化界面作为正式后台主色；参考截图只借鉴“画布为主、资产和属性面板辅助、底部工具栏”的交互结构。

## SDD

### 数据库

沿用现有 `materials` 表，新增递增 migration，不修改已应用旧 migration。

新增字段：

```sql
ALTER TABLE materials
    ADD COLUMN status VARCHAR(20) NOT NULL DEFAULT 'active';
```

新增或替换约束：

- `materials_status_check`：`status IN ('active', 'archived')`
- `materials_type_check`：`material_type IN ('video', 'image', 'audio', 'subtitle')`

新增索引：

```sql
CREATE INDEX idx_materials_project_status_updated
    ON materials(project_id, status, updated_at DESC);
```

### 字段口径

- `file_name`：展示名，必填。
- `file_url`：已有素材 URL，必填，后端做基础 URL 格式校验。
- `material_type`：`video | image | audio | subtitle`。
- `tags`：字符串数组，前端以标签输入维护。
- `metadata`：JSONB，存缩略图 URL、格式、尺寸、时长、语言、来源授权备注等扩展信息。
- `usage_count`：只读展示，默认 `0`。
- `status`：`active | archived`。

第一版 metadata 由结构化小字段写入，不让用户直接编辑大段 JSON：

- 通用：`thumbnail_url`、`source_note`、`license_note`。
- 视频/音频：`duration_sec`、`format`。
- 图片：`width`、`height`、`format`。
- 字幕：`language`、`subtitle_format`。

`thumbnail_url` 为可选字段，非空时后端做基础 HTTP/HTTPS URL 校验。图片素材未填写 `thumbnail_url` 时，前端可使用 `file_url` 作为缩略图预览；其他类型未填写时展示类型占位，不进行自动生成。

### API

- `GET /api/projects/:project_id/materials?type=&status=&q=&tag=`
- `POST /api/projects/:project_id/materials`
- `GET /api/materials/:material_id`
- `PUT /api/materials/:material_id`
- `PUT /api/materials/:material_id/status`

列表默认 `status=active`。显式传 `archived` 时返回归档素材；传 `all` 时返回全部状态。

错误处理：

- 空文件名、非法 URL、非法类型、非法状态返回 `400`。
- 素材不存在返回 `404`。
- 更新素材时校验素材归属的 `project_id`，不得跨账号更新。

### 前端

新增页面级文件：

- `apps/video-agent/app/pages/material-library/MaterialLibraryPage.tsx`
- `apps/video-agent/app/pages/material-library/materialModel.ts`

`app/page.tsx` 只负责状态编排和路由分发，不继续堆叠完整页面 UI。

页面采用画布优先的素材画布工作台：

- 主画布：占据素材库主工作区，使用低对比网格承载素材节点，节点展示缩略图或类型占位、文件名、类型、状态和核心标签。
- 资产浮层：位于画布左侧的窄工具面板，承载关键词、类型、状态、标签筛选和素材摘要；后续可收起，不作为等宽主栏。
- 详情浮层：位于画布右侧的属性面板，仅承载选中节点详情、新增素材、编辑素材、归档或恢复；空状态下可显示新增表单。
- 底部画布工具栏：悬浮在画布底部，提供新增素材、缩放、居中、网格开关和画布视图提示。

第一版画布交互边界：

- 支持选择节点、筛选节点、缩放显示、居中视图和新增素材入口。
- 不保存节点位置，不支持用户拖拽后的布局持久化。
- 不表达节点连线含义，不做生成任务编排或素材到作品生产链路。
- 如展示连线，只能作为未来“素材到作品生产”预留的弱视觉提示，不能产生业务行为。

导航：

- 启用一级菜单“素材管理”。
- 新增并启用二级菜单“素材库”。
- “素材检索”“候选确认”“素材清单确认”等能力暂不启用。

## TDD

后端测试：

- migration 增加 `status` 默认值并允许 `subtitle` 类型。
- 创建素材时校验必填字段、URL、类型和标签。
- 创建或编辑素材时校验可选 `thumbnail_url` 的 URL 格式。
- `GET /api/projects/:project_id/materials` 默认只返回 `active`。
- 按 `type`、`status`、`q`、`tag` 过滤素材。
- 更新素材时不能跨账号修改。
- `PUT /api/materials/:material_id/status` 支持归档和恢复。
- 非法状态和非法类型返回 `400`，不存在素材返回 `404`。

前端测试：

- 菜单进入“素材管理 > 素材库”。
- 空画布状态展示“新增素材”入口。
- 新增素材成功后插入资产栏顶部、出现在画布中并选中。
- 编辑素材成功后资产栏、画布节点和详情同步更新。
- 资产栏和画布节点展示缩略图；缺少缩略图时展示对应素材类型占位。
- 默认视图不显示已归档素材。
- 切换到归档筛选后可以看到并恢复素材。
- `subtitle` 类型显示为“字幕”。

E2E：

- 覆盖从左侧菜单进入“素材管理 > 素材库”，看到主画布、资产浮层、详情浮层、底部画布工具栏和新增素材入口。

常规验证：

- `docker exec ai-agent-api cargo fmt -- --check`
- `docker exec ai-agent-api cargo test`
- `docker exec ai-agent-api cargo clippy --all-targets --all-features -- -D warnings`
- `docker exec ai-agent-video-agent npm run lint`
- `docker exec ai-agent-video-agent npm run test`
- `docker exec ai-agent-video-agent npm run build`
- `docker exec ai-agent-video-agent npm run test:e2e`
- `openspec validate --all`

## Prototype Gate

进入前端实现前，必须通过 Pencil MCP 更新 `docs/prototypes/video-agent/video-agent.pen`，并获得用户明确确认。

正式原型必须覆盖：

- 素材管理一级菜单启用。
- 二级菜单“素材库”。
- 画布优先的素材画布工作台布局。
- 空状态。
- 新增素材表单。
- 编辑素材表单。
- 画布节点缩略图或类型占位。
- 归档和恢复操作。

## OpenSpec Plan

确认设计文档后，新建 OpenSpec change：`material-library-management`。

Artifacts：

- `openspec/changes/material-library-management/proposal.md`
- `openspec/changes/material-library-management/design.md`
- `openspec/changes/material-library-management/specs/material-library-management/spec.md`
- `openspec/changes/material-library-management/tasks.md`

实现过程中，代码改动必须与 `tasks.md` 同步；完成后执行 `openspec instructions apply --change "material-library-management" --json` 并确认状态。

## Scope Boundary

本次不做：

- 真实文件上传。
- 对象存储或本地文件存储策略。
- 自动抽取视频帧。
- 自动生成音频波形或封面图。
- 自动抓取远程元数据。
- 画布节点位置持久化。
- 画布连线语义、任务编排或 DAG。
- Embedding 或 Milvus 写入。
- 语义检索。
- 素材检索 Agent。
- 分镜素材候选。
- 素材清单确认。
- 作品生产读取素材清单。
- 发布平台素材同步。
- 移动端适配。
