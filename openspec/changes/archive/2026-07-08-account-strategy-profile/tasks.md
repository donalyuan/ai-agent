# account-strategy-profile Tasks

## 1. OpenSpec 与设计

- [x] 创建 proposal、design、spec 增量和 tasks。
- [x] 运行 `openspec instructions apply --change "account-strategy-profile" --json`，确认 change 可识别。
- [x] 写入 `docs/superpowers/specs/2026-07-08-account-strategy-profile-design.md`。

## 2. 记忆与原型

- [x] 更新项目记忆，记录账号策略资料第一版边界。
- [x] 更新项目记忆，记录 AI 草稿必须人工确认后保存和成本控制边界。
- [x] 根据用户确认修订设计：账号策略资料作为“内容策略 > 账号策略”独立二级页面，不放在当前选题池里完整编辑。
- [x] 更新 `docs/prototypes/video-agent/video-agent.pen`，覆盖账号策略资料查看、AI 草稿生成和编辑。
- [x] 更新 `docs/prototypes/video-agent/video-agent.pen`，将账号策略资料原型调整为独立二级页面，并从当前选题池移除账号策略区块。
- [x] 获得用户明确原型确认后再进入前端编码。

## 3. 数据库与仓储

- [x] 新增 migration：`projects.strategy_profile JSONB NOT NULL DEFAULT '{}'::jsonb`。
- [x] 新增递增 migration，把“账号策略”作为内容策略二级菜单持久化，并保持“历史生成 / 当前选题池”顺序后移。
- [x] 实现账号策略资料 domain model 和 DTO。
- [x] 扩展 Project repository 创建、读取和更新策略资料。
- [x] 实现 repository 与 route tests。

## 4. 后端 API 与 Agent 上下文

- [x] 扩展 `GET /api/projects` 和 `POST /api/projects` 的策略资料字段。
- [x] 实现 `PUT /api/projects/:project_id/strategy-profile`。
- [x] 实现 `POST /api/projects/:project_id/strategy-profile/draft`。
- [x] 实现统一账号策略上下文格式化函数。
- [x] 实现账号策略草稿 LLM prompt、JSON Schema、输出解析和成本控制。
- [x] 将账号策略上下文注入选题生成 prompt。
- [x] 将账号策略上下文注入质量闸门 prompt。
- [x] 将账号策略上下文注入主题组评审 prompt。
- [x] 覆盖参数校验、项目不存在和存储失败场景。
- [x] 更新后端菜单迁移测试和运行态菜单同步，覆盖 `account-strategy`。

## 5. 前端实现

- [x] 扩展 `apps/video-agent/app/lib/api.ts` 类型和 API 方法。
- [x] 顶部选择器文案改为“当前账号”。
- [x] 内容策略左侧二级菜单新增“账号策略”，并保持“历史生成”“当前选题池”可切换。
- [x] 实现“内容策略 > 账号策略”独立页面，展示账号策略资料卡片和缺失提示。
- [x] 当前选题池不展示账号策略区块、策略资料状态/摘要或编辑入口。
- [x] 在账号策略页实现账号策略资料编辑表单。
- [x] 实现 AI 生成策略草稿入口、草稿摘要和预填表单。
- [x] 保存成功后同步项目列表、当前账号和页面回显。
- [x] 保存失败时展示错误且不覆盖旧资料。

## 6. 验证

- [x] 运行后端相关 Rust 测试。
- [x] 运行前端相关 Vitest。
- [x] 运行视频工作台 E2E。
- [x] 运行 `openspec instructions apply --change "account-strategy-profile" --json` 并确认任务状态与实际一致。

## 7. 原型反馈修正

- [x] 将“账号策略”独立页面实现重对齐 Pencil Frame `桌面 - 账号策略独立页面 v1 待确认`，覆盖标题工具栏、资料主体卡、基础资料、结构化策略、应用说明、AI 草稿区和右下操作按钮。
- [x] 更新前端/E2E 测试，锁定账号策略独立页原型结构，并保持当前选题池不展示账号策略区块。
- [x] 重新运行前端相关 Vitest、视频工作台 E2E、OpenSpec 校验和 diff 空白校验。

## 8. 取消交互与布局反馈修正

- [x] 取消按钮仅在表单或 AI 草稿存在未保存变更时可点击。
- [x] 点击取消恢复当前账号正式资料，并清空 AI 草稿补充方向与草稿摘要。
- [x] 右侧“结构化策略”区域与“基础资料”顶边对齐，并延展到 AI 草稿区域所在行，避免右下空白。
- [x] 重新运行前端相关 Vitest、视频工作台 E2E、OpenSpec 校验和 diff 空白校验。

## 9. 结构化策略字段反馈修正

- [x] 将“目标受众”从单行输入框改为多行文本域。
- [x] 补充前端测试锁定“目标受众”为文本域。
- [x] 重新运行前端相关 Vitest、OpenSpec 校验和 diff 空白校验。
