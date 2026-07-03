# Tasks

## 1. 数据库菜单模型

- [x] 1.1 新增迁移 `backend/migrations/20260703010000_video_workspace_menus.sql`，创建 `video_workspace_menus` 表、约束、索引和 `updated_at` 触发器。
- [x] 1.2 在迁移中写入 7 个一级菜单种子数据：内容策略、脚本创作、素材管理、作品生产、发布运营、数据分析、工作流任务。
- [x] 1.3 为 `脚本创作` 写入至少一个可用二级菜单，关联当前脚本生成闭环；其他业务菜单可写入计划态二级菜单。
- [x] 1.4 更新数据库迁移测试，断言菜单表存在、`menu_key` 唯一、隐藏/禁用字段存在、7 个一级菜单排序正确。

## 2. 后端菜单读取接口

- [x] 2.1 新增菜单领域模型和响应 DTO，字段使用 `menu_id`、`menu_key`、`label`、`route_path`、`icon`、`menu_type`、`module_key`、`agent_key`、`sort_order`、`is_enabled`、`status`、`metadata`、`children`。
- [x] 2.2 新增菜单 repository，从 `video_workspace_menus` 读取 `is_visible = true` 的菜单并按父子层级组装树。
- [x] 2.3 在 `backend/src/lib.rs` 增加 `GET /api/video-workspace/menus` 路由。
- [x] 2.4 增加后端路由测试：返回 7 个一级业务菜单、默认脚本创作为 enabled、planned 菜单 disabled、隐藏菜单不返回、同级菜单按 `sort_order` 排序。

## 3. 前端菜单数据接入

- [x] 3.1 在 `apps/video-agent/app/lib/api.ts` 增加 `WorkspaceMenuNode` 类型和 `listWorkspaceMenus` 方法。
- [x] 3.2 更新 `apps/video-agent/app/lib/api.test.ts`，覆盖菜单接口成功、错误响应和字段映射。
- [x] 3.3 在 `apps/video-agent/app/page.tsx` 移除硬编码 `agents` 数组，改为加载菜单 API 并渲染业务菜单树。
- [x] 3.4 默认选中 `script-creation`，并将现有脚本生成、脚本列表、时间轴对照详情保留在脚本创作视图中。
- [x] 3.5 为菜单加载中、加载失败、禁用菜单和隐藏菜单行为补齐页面状态，不恢复旧 Agent 硬编码菜单。

## 4. 前端测试与原型

- [x] 4.1 更新 `apps/video-agent/app/page.test.tsx`，断言页面展示 7 个一级业务菜单，且不把旧 6 个 Agent 作为一级导航。
- [x] 4.2 更新 `apps/video-agent/e2e/workspace.spec.ts`，mock 菜单 API 并验证默认进入脚本创作、禁用菜单不可点击、脚本闭环仍可用。
- [x] 4.3 更新 `docs/prototypes/video-agent/video-agent.pen`，让原型左侧导航展示业务一级菜单和脚本创作下的模块层级。

## 5. 文档与验证

- [x] 5.1 更新 `apps/video-agent/README.md`，说明视频工作台菜单由数据库和 `/api/video-workspace/menus` 控制。
- [x] 5.2 更新 `MEMORY.md` 和 `docs/memory/video-agent-workspace-flow.md`，记录菜单持久化和前端单一来源规则。
- [x] 5.3 运行 `openspec validate persist-video-workspace-menu-hierarchy --json`。
- [x] 5.4 运行后端迁移/路由相关测试、`apps/video-agent` 单测、lint、build 和 E2E。
