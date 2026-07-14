# ai-model-management Specification

## Purpose
TBD - created by archiving change manage-ai-models-and-workspace-selection. Update Purpose after archive.
## Requirements
### Requirement: 系统必须统一管理文本、图片和视频模型部署

系统 SHALL 使用统一 AI 模型记录管理 `text`、`image`、`video` 三类模型部署，并 SHALL 将供应商、API 调用协议和上游模型标识作为不同字段保存。

#### Scenario: 创建文本模型部署

- **WHEN** 操作者提交显示名称、`model_type=text`、供应商、兼容的文本 API 调用协议、请求根地址、上游模型标识、认证凭据、超时、推理等级和最大输出 Token
- **THEN** 系统 SHALL 创建文本模型记录
- **AND** 系统 SHALL 返回不含原文凭据的模型详情

#### Scenario: 创建图片模型部署

- **WHEN** 操作者提交 `model_type=image`、兼容的图片 API 调用协议和图片类型专属配置
- **THEN** 系统 SHALL 保存支持尺寸、默认尺寸和单次最大图片数
- **AND** 系统 SHALL NOT 要求图片模型填写文本推理等级

#### Scenario: 创建视频模型部署但不执行生成

- **WHEN** 操作者提交 `model_type=video`、`runway_api` 或 `kling_api` 以及视频能力配置
- **THEN** 系统 SHALL 保存视频模型及其分辨率、宽高比和时长范围
- **AND** 系统 SHALL NOT 因创建或保存配置发起视频生成请求

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

### Requirement: 模型凭据必须原文持久化但不得通过 API 泄露

按操作者明确确认，系统 SHALL 将 API Key 和可选 API Secret 原文保存到 PostgreSQL；任何模型 API 响应、调用快照、日志和错误消息 SHALL NOT 包含原文凭据。

#### Scenario: 新增凭据后返回掩码

- **WHEN** 操作者创建包含 API Key 或 API Secret 的模型
- **THEN** 数据库 SHALL 保存提交的原文凭据
- **AND** API SHALL 只返回掩码和已配置标记
- **AND** API SHALL NOT 返回可还原的完整凭据

#### Scenario: 编辑时留空保持原凭据

- **GIVEN** 模型已经保存凭据
- **WHEN** 操作者编辑其他字段并将凭据字段留空
- **THEN** 系统 SHALL 保持原凭据不变
- **AND** 更新响应 SHALL 继续只返回掩码

#### Scenario: 错误信息不得包含凭据

- **WHEN** 供应商返回包含请求上下文的错误或模型配置校验失败
- **THEN** 系统 SHALL 清理错误摘要中的 API Key 和 API Secret
- **AND** 系统 SHALL NOT 将原文凭据写入日志

### Requirement: 管理后台必须提供完整模型生命周期操作

`admin/` SHALL 通过模型管理 API 提供列表、筛选、创建、编辑、设为默认、启用、停用和删除操作，并 SHALL 使用版本号避免并发编辑互相覆盖。

#### Scenario: 设为默认使用公开 API 契约

- **GIVEN** 操作者读取了一个已启用非默认模型的当前 `version`
- **WHEN** 操作者点击“设为默认”
- **THEN** Admin SHALL `POST /api/admin/models/:model_id/default`
- **AND** 请求体 SHALL 包含当前 `version`
- **AND** Admin SHALL NOT 使用后端未注册的 `PUT` 方法

#### Scenario: 设为默认成功后刷新列表

- **WHEN** 默认模型切换 API 返回成功
- **THEN** Admin SHALL 重新加载模型列表
- **AND** 新默认模型 SHALL 显示默认标记
- **AND** 页面 SHALL NOT 显示请求失败错误

### Requirement: 每类模型必须维护明确的默认路由

系统 SHALL 保证每个模型类型最多一个未删除默认模型，并 SHALL 只允许启用模型成为默认模型。

#### Scenario: 第一条启用模型成为默认

- **GIVEN** 某模型类型没有启用模型
- **WHEN** 操作者创建或启用该类型第一条模型
- **THEN** 系统 SHALL 将该模型设为默认

#### Scenario: 原子替换默认模型

- **GIVEN** 某类型已有默认模型和另一个启用模型
- **WHEN** 操作者将另一个模型设为默认
- **THEN** 系统 SHALL 在同一事务取消旧默认并设置新默认
- **AND** 系统 SHALL NOT 产生两个默认模型

#### Scenario: 停用默认模型时指定替代

- **GIVEN** 默认模型之外仍存在启用模型
- **WHEN** 操作者停用默认模型
- **THEN** 请求 SHALL 提供同类型 `replacement_model_id`
- **AND** 系统 SHALL 原子替换默认后停用旧模型

#### Scenario: 无替代模型时关闭类型能力

- **GIVEN** 默认模型是该类型唯一启用模型
- **WHEN** 操作者明确确认停用该模型
- **THEN** 系统 SHALL 允许该类型没有默认模型
- **AND** 该类型的新调用 SHALL 返回 `no_default_model` 或模型不可用错误

### Requirement: 模型删除必须保留历史引用

系统 SHALL 根据模型是否被运行记录或生成任务引用决定物理删除或逻辑删除。

#### Scenario: 物理删除未引用模型

- **GIVEN** 模型未被任何运行记录或生成任务引用
- **WHEN** 操作者确认删除
- **THEN** 系统 SHALL 物理删除模型记录
- **AND** 工作台 SHALL 不再返回该模型

#### Scenario: 逻辑删除已引用模型

- **GIVEN** 模型已被运行记录或生成任务引用
- **WHEN** 操作者确认删除
- **THEN** 系统 SHALL 设置 `status=deleted` 和 `deleted_at`
- **AND** 系统 SHALL 保留历史引用和调用快照
- **AND** 系统 SHALL 拒绝使用该模型发起新调用

### Requirement: 工作台模型选项接口必须与管理详情隔离

系统 SHALL 提供按模型类型查询的只读工作台选项接口，该接口 SHALL 只返回已启用且未删除模型的非敏感字段。

#### Scenario: 查询文本模型选项

- **WHEN** 工作台请求 `GET /api/model-options?type=text`
- **THEN** 系统 SHALL 返回文本模型的 ID、显示名称、供应商、协议、上游模型标识和默认状态
- **AND** 默认模型 SHALL 排在可预测位置
- **AND** 响应 SHALL NOT 包含请求地址、API Key、API Secret 或完整运行配置

#### Scenario: 没有可用模型

- **WHEN** 某类型不存在启用且未删除模型
- **THEN** 工作台选项接口 SHALL 返回空选项
- **AND** 系统 SHALL NOT 注入环境变量或硬编码模型作为兜底

### Requirement: AI 模型管理页必须先通过 Pencil 原型确认

正式实现 `admin/` AI 模型管理页前 SHALL 创建并验证 Pencil 原型，页面 SHALL 采用紧凑筛选、表格、抽屉表单和明确的破坏性操作确认。

#### Scenario: 原型覆盖核心管理状态

- **WHEN** 开发者提交后台模型管理原型供确认
- **THEN** 原型 SHALL 覆盖三类模型筛选、模型列表、添加或编辑抽屉、停用默认模型替代和删除确认
- **AND** 原型 SHALL 遵循根级 `DESIGN.md`
- **AND** 用户明确确认后 SHALL 进入正式前端编码

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
