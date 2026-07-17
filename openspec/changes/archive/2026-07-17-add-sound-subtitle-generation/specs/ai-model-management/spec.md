# ai-model-management Specification Delta

## ADDED Requirements

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
