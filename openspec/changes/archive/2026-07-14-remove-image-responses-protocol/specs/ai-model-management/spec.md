## MODIFIED Requirements

### Requirement: 系统必须显式记录并校验 API 调用协议

模型记录 SHALL 保存 `api_protocol`、`protocol_version`、`auth_scheme` 和 `request_base_url`，运行时 SHALL 仅根据 `model_type` 与 `api_protocol` 的显式兼容矩阵选择 adapter。

#### Scenario: 文本协议与类型兼容

- **WHEN** 操作者为文本模型选择 `openai_responses` 或 `openai_chat_completions`
- **THEN** 系统 SHALL 接受兼容协议
- **AND** 系统 SHALL 保存协议版本与认证方式

#### Scenario: 图片协议与类型兼容

- **WHEN** 操作者为图片模型选择 `openai_images` 或 `jimeng_visual`
- **THEN** 系统 SHALL 接受兼容协议
- **AND** 系统 SHALL 保存对应认证方式和图片配置

#### Scenario: 图片模型不得使用 OpenAI Responses

- **WHEN** 操作者为图片模型选择 `openai_responses`
- **THEN** Admin SHALL 不提供该协议选项
- **AND** API 与 PostgreSQL SHALL 拒绝保存该组合
- **AND** API SHALL 返回 `invalid_model_config`

#### Scenario: 其他类型和协议不匹配

- **WHEN** 操作者为图片模型选择 `openai_chat_completions` 或为文本模型选择 `jimeng_visual`
- **THEN** 系统 SHALL 拒绝保存
- **AND** 系统 SHALL 返回 `invalid_model_config`

#### Scenario: 运行时不得猜测协议

- **WHEN** 系统解析一个可调用模型
- **THEN** 系统 SHALL 根据 `model_type` 与 `api_protocol` 选择请求结构和响应解析器
- **AND** 系统 SHALL NOT 根据供应商名称、模型名称或 URL 后缀猜测协议
