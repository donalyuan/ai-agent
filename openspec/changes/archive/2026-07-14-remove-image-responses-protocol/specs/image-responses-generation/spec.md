## REMOVED Requirements

### Requirement: Responses 图片协议必须按候选独立调用

**Reason**: 产品不再采用 `model_type=image + api_protocol=openai_responses`，该执行模式没有合法入口。

**Migration**: 删除逐候选执行分支；图片任务继续使用 `openai_images` 或 `jimeng_visual` 的既有批量行为。

### Requirement: Responses 图片请求必须使用已确认的非流式格式

**Reason**: 图片 Responses provider 被完整移除，Worker 不再构造 `/responses` 图片请求。

**Migration**: 需要图片生成时配置合法图片协议；不得根据模型名或 URL 自动转换协议。

### Requirement: Responses 图片结果必须严格解析 image_generation_call

**Reason**: Worker 不再接收 Responses 图片结果，因此不保留不可达解析器。

**Migration**: OpenAI Images 与即梦继续使用各自已有响应解析规则。

### Requirement: 候选重试和部分成功必须精确计费与审计

**Reason**: 该要求专属于 Responses 图片逐候选调用，不再适用于合法图片 provider。

**Migration**: 图片任务恢复既有同模型批量重试、候选上限和部分成功规则。

### Requirement: Responses 图片调用必须输出脱敏结构化日志

**Reason**: 不再存在 Responses 图片上游调用，专用请求与响应日志代码应删除。

**Migration**: 保留通用任务错误审计和凭据脱敏要求，不记录不存在的 Responses 调用日志。
