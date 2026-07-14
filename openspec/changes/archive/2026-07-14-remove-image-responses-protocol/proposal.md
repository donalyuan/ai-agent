## Why

图片模型的 OpenAI Responses 兼容扩展不再采用，系统需要恢复原有严格协议矩阵，并完整移除该组合的配置、校验和运行时执行能力。由于放开约束的 migration 已在运行库执行，本次必须使用追加迁移恢复最终约束，而不能删除迁移历史。

## What Changes

- **BREAKING**：图片模型不再允许 `api_protocol=openai_responses`，只允许 `openai_images` 或 `jimeng_visual`。
- Admin 图片模型表单移除 `OpenAI Responses` 选项，Pencil 原型删除对应添加状态。
- Rust API、领域兼容矩阵和素材任务映射恢复拒绝 `image + openai_responses`。
- Python Worker 删除 Responses 图片 provider、逐候选请求模式和专用请求日志；异常图片 Responses 配置在调用前失败。
- 新增 append-only migration 恢复 PostgreSQL 最终约束，不删除已应用 migration，不自动改写模型数据。
- 保留 Admin “设为默认”使用 `POST` 的修复，以及 Worker `/assets/...` 本地参考图安全读取能力。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ai-model-management`: 恢复图片模型只允许 `openai_images` 与 `jimeng_visual`，所有管理入口拒绝图片 Responses。
- `model-routed-ai-execution`: 图片 Worker 只构造 OpenAI Images 或即梦 provider，不执行 Responses 图片调用。
- `image-responses-generation`: 移除此前 change 引入的 Responses 图片逐候选生成、解析、重试和日志要求。

## Impact

- 数据库：追加 migration 重建 `ai_models_type_protocol_check`；部署前不得存在图片 Responses 模型记录。
- Rust：更新 `novex-model`、素材生成应用映射及 API/migration 测试。
- Python：更新模型注册表，删除 Responses 图片 provider、逐候选执行与相关测试，保留本地参考图安全读取。
- Admin：更新协议选项和页面测试，保留默认模型 `POST` 修复。
- 原型与文档：通过 Pencil MCP 删除对应状态，更新 memory，并保留旧 change 和 migration 作为历史。
- 外部调用：本变更不发起任何真实供应商请求，不产生图片生成费用。
