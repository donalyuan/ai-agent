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

### Requirement: 平台必须统一管理语音模型部署与凭据

平台 SHALL 继续使用 `ai_models` 作为文本、图片、视频和语音模型部署的唯一运行时配置来源，并允许 Admin 对语音模型执行新增、编辑、启停、默认切换和删除。语音模型 SHALL 使用 `model_type=speech`，首版协议 SHALL 为 `volcengine_tts_v3`、`openai_audio_speech` 或 `volcengine_asr_v3`，不得建立绕过统一模型管理的环境变量或独立模型表。

#### Scenario: 新增豆包 TTS 模型

- **GIVEN** 管理员打开 AI 模型管理
- **WHEN** 管理员新增 `speech + volcengine_tts_v3` 模型并提交有效的请求地址、上游模型、`X-Api-Key`、资源 ID 和能力配置
- **THEN** 系统 SHALL 保存新的语音模型记录
- **AND** Admin SHALL 将运行凭据标记为“TTS X-Api-Key”，将目录同步凭据标记为“OpenAPI Access Key（AK）”和“OpenAPI Secret Key（SK）”
- **AND** Admin SHALL 明确说明 OpenAPI AK/SK 仅用于 `ListSpeakers` HMAC 签名且不会进入请求体
- **AND** 管理响应 SHALL 只返回掩码后的运行凭据和目录同步凭据
- **AND** 工作台 SHALL 能从启用模型选项中读取该模型

#### Scenario: 官方同步模式要求目录凭据

- **GIVEN** 管理员正在新增或编辑豆包 TTS 模型
- **WHEN** 管理员选择“官方同步”目录来源
- **THEN** Admin SHALL 展示 OpenAPI Access Key（AK）和 OpenAPI Secret Key（SK）
- **AND** 新增模型时两项凭据 SHALL 必填，编辑时同时留空 SHALL 保留已有凭据
- **AND** 系统 SHALL 将该模型作为自身音色目录与同步任务的拥有者

#### Scenario: 中转模型复用同一上游音色目录

- **GIVEN** 已存在一个启用的官方同步 TTS 模型，其 `api_protocol`、`upstream_model` 和 `resource_id` 与中转模型一致
- **WHEN** 管理员为中转模型选择“复用已有目录”并选择该来源模型
- **THEN** Admin SHALL 隐藏 OpenAPI AK/SK 输入且 SHALL NOT 提交目录凭据
- **AND** 系统 SHALL 保存显式目录来源模型 ID
- **AND** 管理响应 SHALL 返回目录来源模式、来源模型 ID 和显示名
- **AND** 中转模型 SHALL 使用自己的请求地址与 `X-Api-Key` 执行 TTS

#### Scenario: 新增 OpenAI Audio Speech 中转模型

- **GIVEN** 已存在一个启用的官方同步 TTS 模型，其 `upstream_model` 和 `resource_id` 与中转模型一致
- **WHEN** 管理员新增 `speech + openai_audio_speech` 模型并提交 `/v1` 请求地址、Bearer API Key 和该来源模型
- **THEN** 系统 SHALL 保存显式目录来源模型 ID并将完整 `/v1/audio/speech` 归一化为 `/v1`
- **AND** Admin SHALL 隐藏官方同步选项与 OpenAPI AK/SK
- **AND** 模型能力 SHALL 声明 `supports_word_timestamps=false`
- **AND** 系统 SHALL NOT 将该模型按 `volcengine_tts_v3` 请求或响应处理

#### Scenario: 目录来源候选不受模型列表筛选影响

- **GIVEN** 已存在启用的官方同步 TTS 根模型，管理员当前位于其他模型类型标签或设置了状态、供应商、协议、搜索筛选
- **WHEN** 管理员在添加或编辑抽屉中切换为需要共享目录的 TTS 配置
- **THEN** Admin SHALL 独立请求全部启用的语音模型作为目录来源候选
- **AND** 当前模型列表的类型、状态、供应商、协议和搜索词 SHALL NOT 影响候选
- **AND** Admin SHALL 只展示与当前 `upstream_model + resource_id` 匹配的官方 `volcengine_tts_v3` 根模型
- **AND** 候选加载中、加载失败和成功但无匹配模型 SHALL 显示不同状态
- **AND** 加载失败时 SHALL 提供重试且 SHALL NOT 显示为“没有匹配模型”

#### Scenario: 拒绝不匹配或间接共享来源

- **GIVEN** 管理员正在为 TTS 模型选择共享目录来源
- **WHEN** 来源模型与当前模型的 `api_protocol`、`upstream_model` 或 `resource_id` 任一不一致，或来源模型自身也是共享模式
- **THEN** API SHALL 以稳定配置错误拒绝保存
- **AND** 系统 SHALL 拒绝自引用和共享链
- **AND** 系统 SHALL NOT 按显示名或模糊匹配静默选择其他目录

#### Scenario: 被共享的目录来源受生命周期保护

- **GIVEN** 一个官方同步 TTS 模型仍被一个或多个共享模型引用
- **WHEN** 管理员尝试停用、删除、改为共享模式，或修改其 `api_protocol`、`upstream_model`、`resource_id`
- **THEN** API SHALL 返回稳定的目录来源被引用错误
- **AND** 来源模型与所有共享模型 SHALL 保持原配置
- **AND** 管理员 SHALL 先解除或迁移全部共享绑定后再执行该操作

#### Scenario: 管理员选择 TTS 时间戳语言

- **GIVEN** 管理员正在新增或编辑豆包 TTS 模型
- **WHEN** 管理员配置时间戳语言
- **THEN** Admin SHALL 使用可搜索多选下拉展示“简体中文”和“美式英语”
- **AND** 下拉触发器 SHALL 以中文显示当前已选项，搜索 SHALL 只过滤中文标签且不得改变选择
- **AND** 点击下拉外部或按 `Escape` SHALL 关闭下拉并保留当前选择
- **AND** Admin SHALL 分别提交标准代码 `zh-cn` 和 `en-us`
- **AND** Admin SHALL 保证至少选择一项，不接受自由文本或目录外语言
- **AND** ASR 模型 SHALL 将 `*` 显示为只读“自动识别（全部语言）”，不得要求管理员编辑该内部值

#### Scenario: 新增豆包 ASR 模型

- **GIVEN** 管理员打开 AI 模型管理
- **WHEN** 管理员新增 `speech + volcengine_asr_v3` 模型并提交有效配置
- **THEN** 系统 SHALL 保存独立的 ASR 模型记录
- **AND** 模型请求和响应 SHALL NOT 包含 TOS 地址、凭据或暂存限制
- **AND** Admin SHALL 只读展示系统 TOS 工具状态并提供“工具与 MCP”跳转入口
- **AND** 系统 SHALL NOT 将该记录用于 TTS 或音色目录同步

#### Scenario: TTS 与 ASR 分别维护默认模型

- **GIVEN** 已存在启用的 TTS 默认模型和 ASR 默认模型
- **WHEN** 管理员替换其中一种语音协议的默认模型
- **THEN** 系统 SHALL 只替换同一语音协议的默认模型
- **AND** 另一种语音协议的默认模型 SHALL 保持不变
- **AND** `text/image/video` 按类型维护默认模型的既有行为 SHALL 保持不变

#### Scenario: 编辑语音模型并保留空凭据

- **GIVEN** 语音模型已经配置运行凭据或目录同步凭据
- **WHEN** 管理员编辑非敏感字段并将凭据输入留空
- **THEN** 系统 SHALL 保留已有凭据
- **AND** 系统 SHALL 增加乐观锁版本
- **AND** 历史任务的模型快照 SHALL NOT 被回写

#### Scenario: 长语音模型表单始终提供保存操作

- **GIVEN** 管理员在桌面视口编辑字段高度超过抽屉可视区域的 `openai_audio_speech` 模型
- **WHEN** 管理员在抽屉首屏修改 Bearer API Key
- **THEN** 抽屉标题栏和底部操作栏 SHALL 始终保持可见
- **AND** 只有中间字段区 SHALL 独立纵向滚动
- **AND** 管理员无需滚动到全部字段末尾即可提交更新
- **AND** Admin SHALL 将新 Bearer API Key 连同当前版本和目录来源提交到模型更新 API
- **AND** 保存失败时抽屉 SHALL 保持打开并保留管理员输入

#### Scenario: TOS 待清理对象不阻止模型生命周期操作

- **GIVEN** 系统 TOS 工具存在待清理临时对象
- **WHEN** 管理员新增、编辑、默认切换、停用或删除 TTS/ASR 模型
- **THEN** 系统 SHALL 按既有模型规则执行操作
- **AND** 系统 SHALL NOT 因 TOS 待清理对象拒绝模型操作

#### Scenario: 语音协议与配置不匹配

- **GIVEN** 管理员正在新增或编辑语音模型
- **WHEN** TTS 协议缺少 TTS 资源/能力配置，或 ASR 协议提交了不兼容配置
- **THEN** API SHALL 拒绝保存并返回稳定的配置错误
- **AND** 系统 SHALL NOT 静默改成其他协议或模型类型

#### Scenario: 旧模型和客户端保持兼容

- **GIVEN** 数据库中已有 `text`、`image` 或 `video` 模型，旧客户端仍按原类型查询
- **WHEN** 语音模型 migration 和 API 上线
- **THEN** 旧记录 SHALL 无需重写且继续通过原协议校验
- **AND** 原有模型管理 CRUD、默认模型和模型选项响应 SHALL 保持既有行为
- **AND** 未请求 `speech` 的客户端 SHALL NOT 被迫处理语音专属字段
