## Why

部分图片模型中转仅通过 OpenAI Responses 兼容协议提供 `gpt-image-2` 图片生成，而当前系统把 `openai_responses` 限定为文本模型，并让图片 Worker 只支持 `openai_images` 与 `jimeng_visual`。这会迫使图片任务请求错误的 `/images/generations` 路径，无法使用已配置的中转模型。

## What Changes

- 新增唯一的跨类型兼容组合：允许 `model_type=image` 搭配 `api_protocol=openai_responses`；其他类型与协议组合保持不变。
- 管理后台图片模型表单新增 `OpenAI Responses` 选项，切换协议时继续同步认证方式和类型化配置。
- 后端与 PostgreSQL 约束接受该组合，同时继续拒绝其他不兼容组合。
- Python 图片 Worker 新增 Responses 图片 provider，按每个候选一次请求调用 `<request_base_url>/responses`，使用 `image_generation` tool，并从 `image_generation_call.result` 提取 base64 图片。
- 每个候选独立记录结果；已成功候选不得因其他候选失败而重复调用。临时错误只允许同一候选重试一次，不得跨模型重试。
- Worker 输出脱敏结构化请求/响应摘要日志，不记录 API Key、API Secret、base64 图片正文或完整 multipart 内容。
- 保持现有每分镜 `1-4` 张、每脚本最多 `48` 张的成本上限；Responses 图片协议下外部调用次数等于候选数，临时错误重试会增加对应失败候选的一次调用。

## Capabilities

### New Capabilities

- `image-responses-generation`: 定义图片 Worker 通过 OpenAI Responses 兼容协议逐候选生成、解析、重试、审计和脱敏日志的行为。

### Modified Capabilities

- `ai-model-management`: 图片模型新增 `openai_responses` 兼容协议组合，其他类型与协议映射不变。
- `model-routed-ai-execution`: 图片 Worker 可根据图片模型的 `openai_responses` 协议构造对应 provider，并保持既有模型解析与费用边界。

## Impact

- 数据库：新增 migration 调整 `ai_models_type_protocol_check`。
- Rust：更新 `novex-model` 协议兼容矩阵、AI 模型仓储校验、素材生成 provider 映射与相关 API/migration 测试。
- Python：更新模型注册表校验、图片 provider、逐候选执行与脱敏日志，补充 fake HTTP 测试。
- Admin：更新模型管理 Pencil 原型、协议选项和表单测试，不改变抽屉布局结构。
- 运行成本：Responses 图片协议会按候选逐次调用；不自动跨模型切换，视频模型仍不发起生成调用。
