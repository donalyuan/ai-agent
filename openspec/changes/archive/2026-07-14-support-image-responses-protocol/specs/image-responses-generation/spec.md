## ADDED Requirements

### Requirement: Responses 图片协议必须按候选独立调用

图片 Worker SHALL 对 `model_type=image` 且 `api_protocol=openai_responses` 的任务按每个候选一次外部请求执行生成，并 SHALL NOT 将候选数作为 `n` 参数发送。

#### Scenario: 单分镜生成三个候选

- **GIVEN** 图片任务包含一个分镜且要求三个候选
- **WHEN** Worker 使用 Responses 图片 provider 执行任务
- **THEN** Worker SHALL 发起三次独立 Responses 请求
- **AND** 每次请求 SHALL 只对应一个候选

#### Scenario: 成功候选不得重复调用

- **GIVEN** 前两个候选已经生成成功
- **AND** 第三个候选发生临时错误
- **WHEN** Worker 重试第三个候选
- **THEN** Worker SHALL 只重试第三个候选
- **AND** Worker SHALL NOT 再次调用前两个成功候选

### Requirement: Responses 图片请求必须使用已确认的非流式格式

Worker SHALL POST `<request_base_url>/responses`，使用数据库中的 `upstream_model` 作为 `model`，使用分镜提示词作为 `input`，并 SHALL 配置和强制调用 `image_generation` tool。

#### Scenario: 无参考图的候选请求

- **WHEN** Worker 为没有参考图的分镜生成一个候选
- **THEN** 请求 SHALL 包含 `model`、含 `input_text` 的用户输入、`tools=[{"type":"image_generation"}]` 和对应 `tool_choice`
- **AND** 请求 SHALL NOT 包含 `n`
- **AND** 请求 SHALL NOT 启用 streaming

#### Scenario: 使用默认图片尺寸

- **GIVEN** 图片模型 `settings.default_size` 为非空合法值
- **WHEN** Worker 构造 Responses 图片工具
- **THEN** `image_generation` tool SHALL 包含该 `size`

#### Scenario: 使用参考图

- **GIVEN** 图片任务包含可读取的参考图
- **WHEN** Worker 构造候选请求
- **THEN** Worker SHALL 下载参考图并在用户输入中追加 base64 data URL 形式的 `input_image`
- **AND** Worker SHALL NOT 将参考图 base64 写入日志

### Requirement: Responses 图片结果必须严格解析 image_generation_call

Worker SHALL 只从完整 JSON 响应的 `output` 数组中读取 `type=image_generation_call` 且包含非空 `result` 的项目，并 SHALL 将 `result` 作为 base64 图片解码。

#### Scenario: 成功返回图片

- **WHEN** 响应包含合法 `image_generation_call.result`
- **THEN** Worker SHALL 解码图片并写入自管素材存储
- **AND** Worker SHALL 创建对应素材和分镜候选记录

#### Scenario: 响应缺少图片调用结果

- **WHEN** 响应不包含合法 `image_generation_call.result`
- **THEN** Worker SHALL 将该响应视为永久格式错误
- **AND** Worker SHALL NOT 尝试从其他字段猜测图片内容

#### Scenario: 返回非法 base64

- **WHEN** `image_generation_call.result` 不是合法 base64
- **THEN** Worker SHALL 将当前候选标记失败
- **AND** Worker SHALL 停止当前任务后续外部调用

### Requirement: 候选重试和部分成功必须精确计费与审计

Worker SHALL 对每个候选的临时错误最多重试一次， SHALL 累加实际重试次数，并 SHALL 保留已经成功的候选结果。

#### Scenario: 单个候选临时错误后成功

- **WHEN** 某候选首次调用返回 `429` 或 `5xx` 且重试成功
- **THEN** Worker SHALL 保存该候选
- **AND** 任务 `retry_count` SHALL 增加一

#### Scenario: 单个候选重试后仍失败

- **WHEN** 某候选首次调用和唯一一次重试均返回临时错误
- **THEN** Worker SHALL 记录该候选失败
- **AND** Worker SHALL 继续处理同任务的下一个候选

#### Scenario: 永久错误停止剩余调用

- **WHEN** 某候选返回认证、非法请求、配置或响应结构永久错误
- **THEN** Worker SHALL 记录当前候选失败
- **AND** Worker SHALL 不再调用当前分镜剩余候选和后续分镜

#### Scenario: 任务部分成功

- **GIVEN** 任务至少生成一个候选且至少一个候选失败
- **WHEN** Worker 完成任务
- **THEN** 任务状态 SHALL 为 `completed`
- **AND** 任务结果 SHALL 记录 `partial=true`、真实成功数、失败数和重试数

### Requirement: Responses 图片调用必须输出脱敏结构化日志

Worker SHALL 为每次 Responses 图片调用输出单行结构化请求和结果摘要，并 SHALL 排除凭据和图片正文。

#### Scenario: 打印请求摘要

- **WHEN** Worker 准备调用一个候选
- **THEN** 日志 SHALL 包含任务、分镜、候选、attempt、URL、model、timeout、prompt、参考图数量和工具配置
- **AND** 日志 SHALL NOT 包含 Authorization、API Key、API Secret 或参考图 base64

#### Scenario: 打印成功摘要

- **WHEN** Responses 图片调用成功
- **THEN** 日志 SHALL 包含 HTTP 成功状态、响应输出类型和图片数量
- **AND** 日志 SHALL NOT 包含返回图片 base64

#### Scenario: 打印失败摘要

- **WHEN** Responses 图片调用失败
- **THEN** 日志 SHALL 包含 HTTP 状态或异常类型及受限长度错误摘要
- **AND** 日志 SHALL NOT 包含请求凭据或完整二进制响应
