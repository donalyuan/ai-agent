## 1. 规格与原型

- [x] 1.1 补齐 proposal、design、三项 capability delta spec 和任务清单，并通过 OpenSpec strict validate。
- [x] 1.2 取得用户对书面规格的明确确认。
- [x] 1.3 通过 Pencil MCP 删除“状态 - 添加图片模型（OpenAI Responses）”，并验证其余原型状态和布局未变化。

## 2. TDD 锁定回退行为

- [x] 2.1 修改 Rust 兼容矩阵、AI 模型 API 和 migration 测试，要求拒绝 `image + openai_responses`，运行并确认 RED。
- [x] 2.2 修改 Worker 注册表和 provider factory 测试，要求在外部调用前拒绝图片 Responses，运行并确认 RED。
- [x] 2.3 修改 Admin 页面测试，要求图片协议下拉不包含 `OpenAI Responses`，并确认默认模型操作仍使用 `POST`，运行并确认 RED。
- [x] 2.4 为 `/assets/...` 本地参考图读取和路径越界校验建立独立于 Responses provider 的回归测试。

## 3. 数据库与 Rust 回退

- [x] 3.1 新增 `20260713020000_remove_image_responses_protocol.sql`，检查冲突记录并恢复图片协议最终约束。
- [x] 3.2 恢复 `ApiProtocol::supports` 只允许文本 Responses，并删除素材任务的 Responses 图片 provider 映射。
- [x] 3.3 运行相关 Rust 测试并确认新迁移最终拒绝图片 Responses，其他合法组合通过。

## 4. Worker 与 Admin 回退

- [x] 4.1 Worker 注册表恢复只接受 `openai_images | jimeng_visual`。
- [x] 4.2 删除 Responses 图片 provider、逐候选模式、专用字段和专用日志，恢复单一批量图片处理路径。
- [x] 4.3 保留并验证 `/assets/...` 本地参考图读取、路径越界校验和既有 OpenAI Images/即梦行为。
- [x] 4.4 Admin 删除图片协议中的 `OpenAI Responses`，保留“设为默认”使用 `POST` 的修复。

## 5. 文档与综合验证

- [x] 5.1 更新 `MEMORY.md` 和 `docs/memory/video-agent-workspace-flow.md`，记录该协议组合已回退并保留迁移历史。
- [x] 5.2 运行 Rust workspace、Worker 和 Admin 全量测试，以及 Admin lint/build。
- [x] 5.3 运行 OpenSpec strict validate、`openspec instructions apply` 和 `git diff --check`。
- [x] 5.4 在数据库无冲突记录前提下重建 API、Worker、Admin，确认服务健康和 migration 生效。
- [x] 5.5 验证 Admin 不展示该协议，API 与数据库拒绝 `image + openai_responses`，且不发起真实计费调用。
