# 回退图片模型 OpenAI Responses 协议

## 背景

此前 change `support-image-responses-protocol` 为图片模型新增了 `openai_responses` 协议，并同步修改 PostgreSQL 约束、Rust 兼容矩阵、素材任务路由、Python Worker、Admin 协议选项和 Pencil 原型。该数据库 migration 已在运行库执行，因此不能通过删除 migration 文件恢复旧状态。

运行库当前不存在任何 `model_type=image + api_protocol=openai_responses` 记录，包括逻辑删除记录，可以在不迁移业务数据的前提下重新收紧约束。

## 方案

采用 append-only 彻底回退：保留已执行的 `20260713010000_image_responses_protocol.sql` 作为迁移历史，新增更晚 migration，将图片协议重新限制为 `openai_images | jimeng_visual`。同时删除 Rust、Worker、Admin 和 Pencil 原型中的 Responses 图片支持，使所有层的最终兼容矩阵一致。

新 migration 在重建约束前显式检查是否仍存在图片 Responses 模型；若存在则抛出明确错误并停止迁移，不自动修改、删除或猜测替代协议。

## DDD

- `openai_responses` 恢复为文本模型协议，只属于 `model_type=text`。
- 图片模型只允许 `openai_images` 与 `jimeng_visual`。
- 模型类型与协议的兼容规则由 PostgreSQL 约束、Rust 领域类型和 Worker 注册表共同执行，Admin 仅展示合法组合。
- 历史任务快照保持原样，不回写已完成或失败任务的审计数据。

## BDD

- 操作者创建或编辑图片模型时，协议下拉不再出现 `OpenAI Responses`。
- API 收到 `image + openai_responses` 时返回 `invalid_model_config`，且不保存模型。
- 数据库直接插入该组合时违反最终约束。
- Worker 遇到历史或异常配置的图片 Responses 模型时返回 `invalid_model_config`，不得发起上游请求。
- 既有 `openai_images`、`jimeng_visual` 和文本 `openai_responses` 调用继续按原行为工作。

## SDD

- 新增 `remove-image-responses-protocol` OpenSpec change，明确 supersede 旧支持 change，但不删除旧规格和迁移历史。
- 新增 `20260713020000_remove_image_responses_protocol.sql`，先检查冲突记录，再重建 `ai_models_type_protocol_check`。
- Rust 删除图片 Responses 兼容和素材 provider 映射。
- Worker 删除 `OpenAIResponsesImageProvider`、逐候选执行模式、Responses 专用日志与辅助字段；保留 `/assets/...` 到 `ASSET_STORAGE_ROOT` 的本地参考图读取和路径越界校验。
- Admin 删除图片协议中的 `OpenAI Responses`；保留“设为默认”调用 `POST` 的独立修复。
- 通过 Pencil MCP 删除“状态 - 添加图片模型（OpenAI Responses）”原型状态，不直接编辑 `.pen` JSON。
- 更新项目 memory，移除已被本回退覆盖的稳定协议决策，保留真实调用历史作为已废止背景仅在必要文档中说明。

## TDD

1. 先修改兼容矩阵测试，使 Rust、API、migration、Worker 注册表、provider factory 和 Admin 均要求拒绝图片 Responses，并运行得到 RED。
2. 实现追加迁移和各层代码删除，使聚焦测试转为 GREEN。
3. 删除仅用于 Responses 图片 provider、逐候选计费和专用日志的测试；把 `/assets` 本地参考图测试改为独立或 `openai_images` 路径测试，防止误删现有能力。
4. 运行 Rust workspace、Worker、Admin 全量测试，以及 Admin lint/build、OpenSpec strict validate 和 `git diff --check`。
5. 重建服务后验证数据库最终约束、API 拒绝行为、Worker 健康状态和 Admin 运行页面。

## 明确保留范围

- `admin/app/lib/api.ts` 中“设为默认”使用 `POST` 的修复及其回归测试。
- Worker 对 `/assets/...` 本地参考图的安全读取逻辑及其回归测试。
- 已执行 migration、旧 OpenSpec change 和既有任务模型快照，不伪造历史。

## 非目标

- 不新增其他图片协议或协议自动推断。
- 不修改文本 Responses 客户端。
- 不自动转换现有模型协议；迁移前若出现冲突记录则中止并要求显式处理。
- 不发起任何真实图片、视频或其他可能计费的上游调用。
