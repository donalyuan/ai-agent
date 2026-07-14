## Context

`support-image-responses-protocol` 已把 `image + openai_responses` 写入 PostgreSQL、Rust、Python Worker、Admin 和 Pencil 原型，且 `20260713010000_image_responses_protocol.sql` 已在运行数据库成功执行。运行库当前不存在任何该组合记录，包括逻辑删除记录，因此可以不迁移业务数据，直接通过更晚 migration 恢复最终约束。

本次回退与两个同期但独立的修复重叠在相同文件中：Admin 默认模型请求方法已从错误的 `PUT` 修复为后端公开的 `POST`；Worker 新增了 `/assets/...` 本地参考图安全读取。两者均被现有非 Responses 流程使用，必须保留。

## Goals / Non-Goals

**Goals:**

- 让数据库、Rust、Worker、Admin 和原型全部恢复拒绝图片 Responses。
- 保留完整 migration、OpenSpec 和任务快照历史。
- 删除只为 Responses 图片存在的 provider、逐候选执行和专用日志代码。
- 证明文本 Responses、OpenAI Images、即梦和两个独立修复没有回归。

**Non-Goals:**

- 不新增替代协议，不根据模型名、供应商或 URL 猜测协议。
- 不自动把图片 Responses 模型改成 `openai_images`。
- 不修改文本 Responses 客户端或已有任务历史快照。
- 不执行真实上游生成调用。

## Decisions

### 1. 使用追加迁移恢复最终约束

保留 `20260713010000_image_responses_protocol.sql`，新增 `20260713020000_remove_image_responses_protocol.sql`。新 migration 在重建约束前检查所有未物理删除和已逻辑删除的模型记录；只要存在 `image + openai_responses` 就抛出明确异常并中止，不隐式改写数据。

直接删除旧 migration 会让已运行环境的 `_sqlx_migrations` 与仓库不一致；只改应用层则数据库仍接受非法组合，均不采用。

### 2. 删除全链路执行能力

Rust `ApiProtocol::supports` 恢复仅允许文本 Responses，素材任务映射不再把 Responses 当作图片供应商。Python 注册表只接受 `openai_images | jimeng_visual`，provider factory 不再构造 Responses provider。

Worker 删除 `OpenAIResponsesImageProvider`、`per_candidate` 分支、候选 attempt 字段、专用结构化日志和只服务该协议的解析辅助。批量图片处理恢复单一路径，避免保留不可达兼容代码。

### 3. 保留独立功能并重建针对性测试

Admin 的 `setDefaultAiModel()` 继续发送 `POST`。Worker 的 `default_binary_get()` 继续把 `/assets/...` 映射到 `ASSET_STORAGE_ROOT` 并校验路径不能越界；原先挂在 Responses provider 下的本地参考图测试改为独立或 OpenAI Images 编辑路径测试。

### 4. 原型和历史规格分别处理

通过 Pencil MCP 删除顶层“状态 - 添加图片模型（OpenAI Responses）”，不直接编辑 `.pen`。旧支持 change 保留为已实施历史，新 change 通过相反的 capability delta 明确 supersede；不篡改旧任务勾选状态。

## Risks / Trade-offs

- [部署前又创建了图片 Responses 模型] -> migration 明确失败并停止部署，不自动转换；先由操作者显式删除或改回合法协议后重试。
- [按整文件回退误删同期修复] -> 采用逐块修改，并为默认 `POST` 与本地参考图读取保留独立回归测试。
- [数据库、Rust、Worker、Admin 矩阵再次漂移] -> 四层分别测试同一非法组合，并在运行库执行最终约束检查。
- [删除逐候选分支影响批量 provider] -> 先锁定现有 OpenAI Images/即梦批量测试，再运行 Worker 全量测试。
- [旧 change 与新 change 同时处于活动列表] -> 在新 proposal/design 中明确覆盖关系，完成回退后分别保留真实状态，不修改历史内容冒充从未实施。

## Migration Plan

1. 通过 Pencil MCP 删除对应原型状态并验证布局。
2. 在测试中先把预期改为拒绝图片 Responses，得到 RED。
3. 停止可写 API/Worker 的部署窗口，确认数据库不存在冲突记录。
4. 部署追加 migration、Rust API、Worker 和 Admin；migration 先恢复数据库约束。
5. 运行全量验证并确认服务健康、Admin 下拉无该选项、API 与数据库均拒绝该组合。
6. 若 migration 因冲突记录失败，保持旧版本服务停止，显式处理冲突模型后重新执行；不得跳过约束或自动改协议。

## Open Questions

无。回退边界、历史保留方式和独立修复保留范围均已确认。
