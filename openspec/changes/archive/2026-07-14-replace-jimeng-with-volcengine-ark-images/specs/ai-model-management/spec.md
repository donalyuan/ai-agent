## MODIFIED Requirements

### Requirement: 系统必须显式记录并校验 API 调用协议

模型记录 SHALL 保存 `api_protocol`、`protocol_version`、`auth_scheme` 和 `request_base_url`，运行时 SHALL 仅根据 `model_type` 与 `api_protocol` 的显式兼容矩阵选择 adapter。

#### Scenario: 文本协议与类型兼容

- **WHEN** 操作者为文本模型选择 `openai_responses` 或 `openai_chat_completions`
- **THEN** 系统 SHALL 接受兼容协议
- **AND** 系统 SHALL 保存协议版本与认证方式

#### Scenario: 图片协议与类型兼容

- **WHEN** 操作者为图片模型选择 `openai_images` 或 `volcengine_ark_images`
- **THEN** 系统 SHALL 接受兼容协议
- **AND** `volcengine_ark_images` SHALL 固定使用 `auth_scheme=bearer`

#### Scenario: 旧 Jimeng 协议不得保存

- **WHEN** 操作者提交 `api_protocol=jimeng_visual`
- **THEN** API 与 PostgreSQL SHALL 拒绝保存
- **AND** API SHALL 返回 `invalid_model_config`

#### Scenario: 其他类型和协议不匹配

- **WHEN** 操作者为图片模型选择 `openai_responses` 或为文本模型选择 `volcengine_ark_images`
- **THEN** 系统 SHALL 拒绝保存
- **AND** 系统 SHALL 返回 `invalid_model_config`

#### Scenario: 运行时不得猜测协议

- **WHEN** 系统解析一个可调用模型
- **THEN** 系统 SHALL 根据 `model_type` 与 `api_protocol` 选择请求结构和响应解析器
- **AND** 系统 SHALL NOT 根据供应商名称、模型名称或 URL 后缀猜测协议

## ADDED Requirements

### Requirement: 火山方舟图片模型配置必须遵循 Ark 协议契约

系统 SHALL 对 `volcengine_ark_images` 使用 Bearer API Key 和规范化请求根地址，Admin SHALL 只暴露该协议需要的字段。

#### Scenario: Admin 选择火山方舟图片协议

- **WHEN** 操作者在图片模型表单选择“火山方舟图片生成”
- **THEN** 表单 SHALL 设置 `api_protocol=volcengine_ark_images` 和 `auth_scheme=bearer`
- **AND** 表单 SHALL 显示 API Key
- **AND** 表单 SHALL NOT 显示或要求 API Secret
- **AND** 图片协议选项 SHALL NOT 包含“即梦 Visual”

#### Scenario: 保存 Ark 根地址

- **WHEN** 操作者提交合法 HTTP(S) Ark 根地址
- **THEN** 系统 SHALL 去除末尾斜线后保存根地址
- **AND** 系统 SHALL NOT 在保存时调用供应商

#### Scenario: 保存 Ark 完整生成地址

- **WHEN** 操作者提交以 `/images/generations` 结尾的完整 Ark 地址
- **THEN** 系统 SHALL 删除该固定后缀并保存请求根地址
- **AND** 系统 SHALL NOT 据此改变已选择的协议

#### Scenario: 拒绝无法规范化的 Ark 地址

- **WHEN** Ark 地址包含 query、fragment、非 HTTP(S) scheme 或无关 endpoint 路径
- **THEN** 系统 SHALL 返回 `invalid_model_config`
- **AND** 系统 SHALL NOT 保存部分规范化结果

#### Scenario: 空图片尺寸保持为空

- **WHEN** 操作者将默认图片尺寸留空后保存图片模型
- **THEN** Admin SHALL 提交 `default_size=null` 和 `supported_sizes=[]`
- **AND** Admin SHALL NOT 提交 `supported_sizes=[""]`

#### Scenario: Ark 单次图片数固定为一

- **WHEN** 操作者保存 `volcengine_ark_images` 模型
- **THEN** 系统 SHALL 保存 `max_images_per_request=1`
- **AND** 每分镜多候选 SHALL 由任务编排为多次独立调用
