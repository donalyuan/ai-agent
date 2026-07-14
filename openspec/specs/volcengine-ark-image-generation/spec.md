# volcengine-ark-image-generation Specification

## Purpose
TBD - created by archiving change replace-jimeng-with-volcengine-ark-images. Update Purpose after archive.
## Requirements
### Requirement: Ark 图片请求必须按候选独立调用

Worker SHALL 为每个 Ark 图片候选发起一次独立非流式请求，并 SHALL 关闭顺序组图生成。

#### Scenario: 单候选请求结构

- **GIVEN** 一个 Ark 图片候选准备执行
- **WHEN** Worker 发起供应商请求
- **THEN** 请求 SHALL 使用 `POST <request_base_url>/images/generations`
- **AND** 请求 SHALL 使用 `Authorization: Bearer <API Key>`
- **AND** JSON SHALL 包含数据库中的 `model`、场景 `prompt`、`sequential_image_generation=disabled`、`response_format=b64_json`、`watermark=false` 和 `stream=false`
- **AND** 非空默认尺寸 SHALL 写入 `size`

#### Scenario: 多候选产生多次独立调用

- **GIVEN** 一个分镜请求 `N` 个 Ark 候选
- **WHEN** Worker 执行该分镜
- **THEN** Worker SHALL 发起 `N` 次首次调用
- **AND** 每次调用 SHALL 只请求一个候选
- **AND** Worker SHALL NOT 使用一次 Ark 响应填充多个候选

### Requirement: Ark 参考图必须转换为安全 data URL

Worker SHALL 在调用前读取已授权参考素材，并 SHALL 把有效图片编码为 Ark `image` 字段，而不是把本地素材 URL 直接发送给供应商。

#### Scenario: 本地参考素材

- **GIVEN** 参考素材 URL 使用 `/assets/...` 前缀且位于素材存储根目录内
- **WHEN** Worker 构造 Ark 请求
- **THEN** Worker SHALL 从本地持久化目录读取文件
- **AND** Worker SHALL 按图片实际类型构造 data URL
- **AND** Ark 请求 `image` SHALL 包含该 data URL

#### Scenario: 多张参考素材

- **GIVEN** 任务包含多张合法参考图片
- **WHEN** Worker 构造 Ark 请求
- **THEN** 请求 `image` SHALL 使用 data URL 数组
- **AND** 数组顺序 SHALL 与任务参考素材顺序一致

#### Scenario: 非法参考素材不得调用上游

- **GIVEN** 参考路径越界、读取失败或图片类型无法识别
- **WHEN** Worker 准备 Ark 请求
- **THEN** 当前候选 SHALL 失败
- **AND** Worker SHALL NOT 发起该候选供应商请求

### Requirement: Ark 响应必须严格解析并按真实类型保存

Worker SHALL 只接受符合 Ark 契约的 base64 图片结果，并 SHALL 根据实际图片字节选择文件扩展名后写入自管存储。

#### Scenario: 解析有效 base64 图片

- **WHEN** Ark 返回 `data[].b64_json` 且内容为合法 PNG、JPEG 或 WebP
- **THEN** Worker SHALL 解码图片
- **AND** Worker SHALL 使用与 magic bytes 匹配的扩展名保存
- **AND** Worker SHALL 创建指向 `/assets/...` 的素材记录

#### Scenario: Ark 数据项返回错误

- **WHEN** Ark 返回 `data[].error`
- **THEN** Worker SHALL 将该候选标记为永久失败
- **AND** Worker SHALL NOT 创建素材记录

#### Scenario: Ark 响应契约无效

- **WHEN** 响应缺少 `data`、没有有效 `b64_json`、base64 无法解码或图片类型不受支持
- **THEN** Worker SHALL 将该候选标记为永久失败
- **AND** Worker SHALL NOT 把无效字节写入素材存储

### Requirement: Ark 候选重试和停止规则必须限制费用风险

Worker SHALL 只重试当前候选的临时错误一次，并 SHALL 在永久错误后停止任务剩余 Ark 调用。

#### Scenario: 当前候选临时错误后成功

- **GIVEN** 当前候选首次调用发生网络错误、超时、HTTP `429` 或 HTTP `5xx`
- **WHEN** Worker 处理该错误
- **THEN** Worker SHALL 使用同一模型和相同候选输入重试一次
- **AND** 此前成功候选 SHALL NOT 重复调用
- **AND** 任务 `retry_count` SHALL 增加一

#### Scenario: 当前候选临时错误重试耗尽

- **GIVEN** 当前候选第二次调用仍为临时错误
- **WHEN** Worker 记录该候选失败
- **THEN** Worker SHALL NOT 第三次调用该候选
- **AND** Worker SHALL 继续执行下一个候选

#### Scenario: 永久错误停止剩余调用

- **GIVEN** 当前候选返回鉴权、权限、参数、响应契约或其他永久错误
- **WHEN** Worker 处理该错误
- **THEN** Worker SHALL 停止当前任务剩余 Ark 调用
- **AND** 未执行候选 SHALL 写入失败记录
- **AND** 已成功候选 SHALL 保留且不得重复调用

#### Scenario: 部分成功任务汇总

- **GIVEN** 任务已有成功候选且后续候选失败
- **WHEN** Worker 完成任务汇总
- **THEN** 任务 SHALL 为 `completed`
- **AND** `result.partial` SHALL 为 `true`
- **AND** 生成数量、失败数量和重试次数 SHALL 与实际调用一致

### Requirement: Ark 请求日志必须可审计且不得泄露敏感正文

Worker SHALL 为每个候选 attempt 输出结构化请求与结果日志，并 SHALL 对凭据、参考图和结果图进行不可逆脱敏。

#### Scenario: 输出候选请求日志

- **WHEN** Worker 即将发起 Ark 候选调用
- **THEN** 日志 SHALL 包含任务 ID、模型 ID、协议、候选序号、attempt、method、URL 和脱敏请求摘要
- **AND** 日志中的 API Key 与 `Authorization` 值 SHALL 为 `***`
- **AND** 日志 SHALL NOT 包含参考图 base64 正文

#### Scenario: 输出候选结果日志

- **WHEN** Ark 调用成功或失败
- **THEN** 日志 SHALL 包含 HTTP 状态、耗时、结果数量、图片字节数或错误分类与摘要
- **AND** 日志 SHALL NOT 包含 `b64_json` 正文

#### Scenario: 输出脱敏 curl 等价信息

- **WHEN** Worker 配置输出 Ark curl 等价日志
- **THEN** curl 中的 `Authorization` SHALL 使用 `Bearer ***`
- **AND** `image` data URL SHALL 使用引用数量占位符
- **AND** curl SHALL NOT 包含任何可还原凭据或图片 base64

### Requirement: Ark 自动化和真实验证必须遵守调用上限

系统 SHALL 在自动化阶段使用 fake transport，并 SHALL 把实施后的真实验证限制为用户已确认的最小调用范围。

#### Scenario: 自动化测试不产生费用

- **WHEN** 运行 Rust、Worker、Admin 或 OpenSpec 自动化验证
- **THEN** 系统 SHALL NOT 调用 Ark
- **AND** Worker 测试 SHALL 使用 fake HTTP 响应

#### Scenario: 受控真实验证

- **GIVEN** 用户已确认开始真实验证且数据库没有其他在途图片任务
- **WHEN** 系统执行 Seedream 验证
- **THEN** 系统 SHALL 只创建单分镜、单候选任务
- **AND** 实际 Ark 调用 SHALL 最多为首次一次加临时错误重试一次
- **AND** 系统 SHALL 监控该任务到终态后停止
