## Overview

本 change 将视频工作台导航从“前端硬编码 Agent 入口”调整为“数据库驱动的业务菜单树”。菜单的一级结构与用户确认的业务流程一致，二级结构承载具体模块和 Agent 能力状态。`apps/video-agent` 只负责读取和呈现菜单；菜单控制的写入能力后续归属 `admin/` 或后端控制面。

## Data Model

新增表建议命名为 `video_workspace_menus`。该表只保存视频工作台菜单，不扩展为全平台通用菜单表，避免在第一个业务应用阶段引入不必要抽象。

建议字段：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | `UUID` | 主键，默认 `gen_random_uuid()` |
| `parent_id` | `UUID NULL` | 父菜单，引用 `video_workspace_menus(id)`，为空表示一级菜单 |
| `menu_key` | `VARCHAR(80)` | 稳定业务键，全表唯一，如 `script-creation` |
| `label` | `VARCHAR(60)` | 前端显示文本，如 `脚本创作` |
| `description` | `TEXT` | 菜单业务说明 |
| `route_path` | `VARCHAR(160)` | 前端路由或视图路径，如 `/scripts` |
| `icon` | `VARCHAR(40)` | 前端图标键，前端映射到 lucide 或现有图标库 |
| `menu_type` | `VARCHAR(20)` | `section`、`page` 或 `group` |
| `module_key` | `VARCHAR(80) NULL` | 对应业务模块键 |
| `agent_key` | `VARCHAR(80) NULL` | 关联底层 Agent，如 `script-generation-agent` |
| `sort_order` | `INT` | 同级排序 |
| `is_enabled` | `BOOLEAN` | 是否可点击进入 |
| `is_visible` | `BOOLEAN` | 是否返回给视频工作台前端 |
| `status` | `VARCHAR(20)` | `active`、`planned` 或 `disabled` |
| `metadata` | `JSONB` | 阶段、徽标、提示文案等扩展信息 |
| `created_at` | `TIMESTAMPTZ` | 创建时间 |
| `updated_at` | `TIMESTAMPTZ` | 更新时间 |

约束要求：

1. `menu_key` 必须唯一，后端和前端不得依赖数据库 UUID 判断业务菜单。
2. `parent_id` 必须引用同表记录，并禁止自引用。
3. `sort_order` 必须非负。
4. `menu_type`、`status` 必须有数据库 CHECK 约束。
5. 菜单查询必须按父级和 `sort_order` 稳定排序。

## Seed Menus

初始种子数据必须包含 7 个一级菜单，顺序如下：

1. `content-strategy`：内容策略，默认 `planned`、可见但不可进入。
2. `script-creation`：脚本创作，默认 `active`、可见且可进入，路由 `/scripts`。
3. `material-management`：素材管理，默认 `planned`、可见但不可进入。
4. `production`：作品生产，默认 `planned`、可见但不可进入。
5. `publishing`：发布运营，默认 `planned`、可见但不可进入。
6. `analytics`：数据分析，默认 `planned`、可见但不可进入。
7. `workflow-tasks`：工作流任务，默认 `planned`、可见但不可进入。

二级菜单用于承载当前和后续模块。例如 `script-creation` 下至少包含 `script-generator`，关联 `script-generation-agent`，并指向当前已实现的脚本生成、脚本列表和时间轴详情闭环。其他一级菜单可先写入计划态二级菜单，用于展示模块边界和后续开发位置。

## API Contract

新增后端接口：

```http
GET /api/video-workspace/menus
```

响应形态：

```json
{
  "menus": [
    {
      "menu_id": "uuid",
      "menu_key": "script-creation",
      "label": "脚本创作",
      "description": "脚本生成、分镜确认和状态流转",
      "route_path": "/scripts",
      "icon": "file-pen-line",
      "menu_type": "section",
      "module_key": "script",
      "agent_key": null,
      "sort_order": 20,
      "is_enabled": true,
      "status": "active",
      "metadata": { "phase": 1 },
      "children": []
    }
  ]
}
```

接口规则：

1. 只返回 `is_visible = true` 的菜单。
2. `is_enabled = false` 的菜单仍返回给前端，但前端必须以禁用态展示。
3. 返回结构必须是树形，不要求前端自行拼父子关系。
4. 若数据库没有菜单种子数据，接口必须返回结构化错误，不能在前端恢复旧硬编码 Agent 菜单。
5. 本 change 不要求新增写入接口；后续菜单编辑写入接口应归属 `admin/` 或后端控制面。

## Frontend Behavior

`apps/video-agent` SHALL 以菜单 API 作为导航单一来源：

1. 首屏加载时先获取 `/api/video-workspace/menus`，渲染左侧一级菜单和必要的二级菜单。
2. 默认选中 `script-creation`，当前脚本闭环保留在该菜单下。
3. 不再以 `agents` 常量渲染 6 个智能体作为一级导航。
4. Agent 名称可以在二级菜单、模块状态或执行状态中出现，但不能替代业务一级菜单。
5. 隐藏菜单不得渲染；禁用菜单可见但不可点击，并展示计划态或禁用态。
6. 菜单加载失败时展示导航错误状态，不回退到前端硬编码菜单。

## Boundary

视频工作台内只展示生产流程相关菜单。平台配置、模型供应商、模型路由、API Key、MCP、Worker、队列、系统日志、权限、审计、成本和限额继续归 `admin/` 或后端控制面。

## Test Strategy

1. OpenSpec：运行 `openspec validate persist-video-workspace-menu-hierarchy --json`。
2. 数据库：迁移测试确认 `video_workspace_menus` 表、约束、索引和 7 个一级菜单种子数据存在。
3. 后端：路由测试确认 `GET /api/video-workspace/menus` 返回排序后的树形菜单，并正确过滤隐藏菜单。
4. 前端单测：确认 API client 调用菜单接口，页面渲染 7 个一级业务菜单，禁用/隐藏行为正确。
5. E2E：确认桌面端左侧导航不再出现旧 6 个 Agent 作为一级菜单，默认进入脚本创作，脚本生成和时间轴详情仍可用。
