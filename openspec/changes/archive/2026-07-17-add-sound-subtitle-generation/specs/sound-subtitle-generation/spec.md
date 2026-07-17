# sound-subtitle-generation Specification

## ADDED Requirements

### Requirement: 声音与字幕生成必须提供独立双标签工作区

系统 SHALL 在 `素材管理 / 声音与字幕生成` 提供 `TTS配音` 和 `字幕` 两个标签，并通过同一个可见声音 Agent 对话协助生成。

#### Scenario: 打开声音与字幕生成

- **GIVEN** 操作者展开素材管理菜单
- **WHEN** 操作者进入 `声音与字幕生成`
- **THEN** 页面 SHALL 只显示 `TTS配音` 和 `字幕` 两个业务标签
- **AND** 页面 SHALL 展示声音 Agent 的消息、建议和工具执行状态
- **AND** 页面 SHALL NOT 显示 AI 音乐、环境音或动作音效生成标签

#### Scenario: 桌面工作区遵循已确认的三栏信息架构

- **GIVEN** 操作者在桌面端打开声音与字幕生成
- **WHEN** 页面完成初始化
- **THEN** 页头 SHALL 同时展示业务路径、页面标题、`TTS配音 / 字幕` 标签和新建任务操作
- **AND** 主工作区 SHALL 从左到右固定展示配音任务列表、生成配置和声音 Agent 三栏
- **AND** 任务筛选、任务卡片及并发状态 SHALL 位于左栏，模型目录、旁白、声音参数、试听及当前任务状态 SHALL 位于中栏
- **AND** 中间桌面宽度在中栏足以容纳双区结构时，试听 SHALL 与模型目录同排，试听及生成操作 SHALL 与声音参数同排并占满剩余可用宽度，不得退回半宽单列形成大片无意义空白
- **AND** 右栏 Agent 头部 SHALL 以“标题和在线状态 / 会话信息和模型选择”两行展示且不得重叠
- **AND** 页面 SHALL NOT 在三栏工作区下方重复展示独立的大任务表

#### Scenario: 桌面工作区使用可读字号层级

- **GIVEN** 操作者在桌面端打开声音与字幕生成
- **WHEN** 页面渲染任务、生成配置、声音 Agent 和失败详情
- **THEN** 常规正文 SHALL 不小于 `12px`，辅助信息 SHALL 不小于 `11px`
- **AND** 任务标题、表单值、主要按钮和失败标题 SHALL 使用 `13px`
- **AND** 旁白编辑正文 SHALL 使用 `14px`
- **AND** 失败供应商消息 SHALL 使用 `12px`，失败诊断字段 SHALL 使用 `11px`
- **AND** 字号提升 SHALL NOT 改变三栏宽度、宽屏中栏尺寸、`140px` 宽屏失败详情或左栏独立滚动
- **AND** 状态徽标、按钮和任务卡 SHALL NOT 发生文字裁切、横向溢出或相互重叠

#### Scenario: Agent 建议不直接执行

- **GIVEN** 声音 Agent 已推荐文本、音色或字幕断句
- **WHEN** 操作者尚未确认生成
- **THEN** 系统 SHALL 允许操作者修改建议内容和参数
- **AND** 系统 SHALL NOT 调用 TTS 或 ASR 服务

### Requirement: 旁白必须支持从当前账号已有脚本导入

系统 SHALL 允许操作者从当前项目“脚本创作”中的 `draft | approved` 脚本选择分镜旁白并替换当前 TTS 文本，同时保存服务端校验后的不可变来源快照。

#### Scenario: 浏览并选择已有脚本

- **GIVEN** 当前项目存在草稿、已通过和已归档脚本
- **WHEN** 操作者打开旁白区的“导入脚本”
- **THEN** 页面 SHALL 只列出当前项目的草稿和已通过脚本
- **AND** 页面 SHALL 支持按标题或来源选题搜索并按最近更新时间展示
- **AND** 已归档脚本 SHALL NOT 进入可选范围
- **AND** 选择脚本后 SHALL 默认选中全部非空分镜并允许取消部分分镜

#### Scenario: 替换当前旁白

- **GIVEN** 当前旁白已有文本且操作者选择了一个或多个非空分镜
- **WHEN** 操作者确认导入
- **THEN** 页面 SHALL 按分镜 `sequence` 合并所选旁白并替换当前文本
- **AND** 页面 SHALL NOT 将导入文本追加到旧文本
- **AND** 导入后的文本 SHALL 仍可编辑
- **AND** 导入动作 SHALL NOT 调用 TTS、ASR 或声音 Agent

#### Scenario: 导入文本超过模型上限

- **GIVEN** 所选分镜合并文本超过当前 TTS 模型字符数上限
- **WHEN** 操作者尝试确认导入
- **THEN** 页面 SHALL 阻止导入并展示实际字符数与上限
- **AND** 页面 SHALL NOT 截断旁白或创建声音任务

#### Scenario: 创建任务时锁定来源快照

- **GIVEN** 操作者已从脚本导入旁白并可继续编辑最终文本
- **WHEN** 系统预检试听或 TTS 任务
- **THEN** 客户端 SHALL 提交脚本 ID、读取时的 `updated_at` 和所选分镜 ID
- **AND** 应用服务 SHALL 重新读取脚本并校验项目、`draft | approved` 状态、版本和分镜归属
- **AND** 任务 SHALL 分别保存最终 `text_content`、`source_script_id` 和包含脚本标题、版本、分镜顺序及原始旁白的 `source_script_snapshot`
- **AND** 系统 SHALL NOT 信任客户端提交的原始旁白快照

#### Scenario: 来源脚本在预检前发生变化

- **GIVEN** 操作者导入后来源脚本的 `updated_at` 已变化、被归档或分镜已不存在
- **WHEN** 操作者发起预检
- **THEN** 系统 SHALL 返回稳定的来源失效错误并要求重新导入
- **AND** 系统 SHALL NOT 创建声音任务或静默改用最新脚本内容

#### Scenario: 失败任务重试保留历史来源

- **GIVEN** 带脚本来源快照的 TTS 节点执行失败
- **WHEN** 操作者确认只重试该失败节点
- **THEN** 新任务 SHALL 复制失败任务的 `source_script_id` 与不可变来源快照
- **AND** 系统 SHALL NOT 因来源脚本后续编辑而覆盖或丢失原快照

### Requirement: 工作台菜单路由必须在刷新后恢复当前页面

系统 SHALL 使用数据库菜单配置中的 `route_path` 驱动视频工作台 URL 与一级/二级菜单选择，并通过 Next.js App Router 支持深层路径直达，不得只在组件内保存当前页面。

#### Scenario: 点击声音与字幕菜单后刷新

- **GIVEN** `素材管理 / 声音与字幕生成` 已启用且配置了 `route_path`
- **WHEN** 操作者点击该菜单并刷新浏览器
- **THEN** 地址栏 SHALL 保持该菜单的 `route_path`
- **AND** 工作台 SHALL 恢复素材管理一级菜单和声音与字幕生成二级菜单
- **AND** 页面 SHALL NOT 回到默认内容策略页

#### Scenario: 浏览器前进和后退同步菜单

- **GIVEN** 操作者已依次访问两个已启用工作台菜单
- **WHEN** 操作者使用浏览器后退或前进
- **THEN** 工作台 SHALL 恢复当前 URL 对应的一级和二级菜单
- **AND** 菜单高亮与实际页面 SHALL 保持一致

#### Scenario: 访问未知或不可用菜单路径

- **GIVEN** 当前 URL 不匹配任何可见且已启用的菜单路径
- **WHEN** 工作台完成菜单加载
- **THEN** 系统 SHALL 使用 `replace` 导航到首个可用菜单路径
- **AND** 系统 SHALL NOT 将无效路径保留在浏览器后退栈

### Requirement: TTS 模型和音色能力必须动态加载

系统 SHALL 从统一 `ai_models` 中已启用的 `speech + volcengine_tts_v3 | openai_audio_speech` 模型和版本化能力目录加载音色、语言/口音及可调参数；情绪风格只有在目录返回非空有效值且当前协议存在官方请求字段映射时才允许展示，禁止由前端、代码枚举或 migration 写死声音能力。

#### Scenario: 选择 TTS 模型后加载真实能力

- **GIVEN** Admin 已配置一个启用的 TTS 模型及能力目录
- **WHEN** 操作者选择该模型
- **THEN** 页面 SHALL 展示该模型当前可用的音色、语言/口音和参数
- **AND** 页面 SHALL 展示能力目录更新时间
- **AND** 页面 SHALL NOT 展示目录中不存在的组合

#### Scenario: TTS 模型选项与触发框对齐

- **GIVEN** 当前存在一个或多个已启用的 TTS 模型
- **WHEN** 操作者展开 TTS 模型选择
- **THEN** 页面 SHALL 使用工作台自定义单选弹层在触发框正下方展示模型选项
- **AND** 选项层 SHALL 与触发框同宽并使用一致的边框、行高和文本层级
- **AND** 选项层 SHALL NOT 使用浏览器原生 `select` 绘制或与触发框边界错位
- **AND** 选项层 SHALL NOT 与下一行声音配置产生无意义重叠

#### Scenario: 语言单选项在控件下方展开

- **GIVEN** 当前音色只支持一个或多个真实语言代码
- **WHEN** 操作者展开语言选择
- **THEN** 页面 SHALL 使用工作台自定义单选弹层在触发框下方展示中文语言选项
- **AND** 页面 SHALL NOT 使用由浏览器绘制、看似嵌入触发框内部的原生 `select` 选项层

#### Scenario: 目录没有有效情绪能力

- **GIVEN** 当前协议官方请求体没有 `emotion` 字段且音色目录只返回空情绪占位对象
- **WHEN** 页面加载该音色
- **THEN** 页面 SHALL NOT 展示空的情绪风格控件
- **AND** TTS 预检 SHALL NOT 强制要求情绪
- **AND** 前端、后端及 Worker SHALL NOT 伪造 `neutral` 或向上游发送 `emotion`

#### Scenario: 切换模型使原选择失效

- **GIVEN** 当前已选择某音色、语言和声音参数
- **WHEN** 操作者切换到不支持该选择的 TTS 模型
- **THEN** 系统 SHALL 保留并标记原选择失效
- **AND** 系统 SHALL 阻止生成直到操作者重新选择
- **AND** 系统 SHALL NOT 静默替换音色、语言或声音参数

#### Scenario: 宽屏工作区使用可用内容宽度

- **GIVEN** 桌面视口为 1920px
- **WHEN** 声音与字幕工作区渲染
- **THEN** 页头与主工作区 SHALL 为 `1598px`，并在工作台内容区左右各保留约 `24px`
- **AND** 左侧任务栏、生成配置栏和右侧 Agent SHALL 为 `250/1000/276px`，栏间距 SHALL 为 `16px`
- **AND** 1440px 基准 SHALL 继续保持 `1118px` 与 `250/520/276px`
- **AND** 页面 SHALL NOT 将 `1118px` 工作区继续居中而在宽屏两侧留下大块空白

#### Scenario: 宽屏生成配置遵循已确认尺寸

- **GIVEN** 生成配置栏宽为 `1000px`
- **WHEN** TTS 配音配置渲染
- **THEN** 模型与目录状态 SHALL 合计为 `484px`，试听卡片 SHALL 为 `462px`，两者高度 SHALL 为 `54px`
- **AND** 音色与语言 SHALL 为 `650px / 302px`
- **AND** 旁白 SHALL 为 `964px × 180px`
- **AND** 语速、主动试听和生成按钮 SHALL 在下方同一行依次为 `154px / 200px / 586px`
- **AND** 当前任务 SHALL 为 `964px × 70px`
- **AND** 1440px 下字体和控件 SHALL NOT 因宽屏方案被等比例缩放

#### Scenario: Agent 推荐声音配置

- **GIVEN** 当前模型能力目录可用
- **WHEN** 声音 Agent 根据旁白内容推荐音色、语言和声音参数
- **THEN** Agent SHALL 只从当前目录的可用选项中推荐
- **AND** 操作者 SHALL 能查看、修改、试听并确认推荐
- **AND** Agent SHALL NOT 虚构模型不支持的声音能力

#### Scenario: 目录未声明语速默认值

- **GIVEN** 当前声音目录声明了 `speed_ratio` 最小值和最大值但未声明 `default`
- **WHEN** 页面首次初始化声音参数
- **THEN** 页面 SHALL 将语速初始化为合法范围内的 `1.0`
- **AND** 页面 SHALL NOT 使用最小值与最大值的区间中点推导默认语速

#### Scenario: 搜索并选择目录音色

- **GIVEN** 当前 TTS 模型已加载包含供应商原名、中文描述、性别、年龄、语言和目录标签的真实音色目录
- **WHEN** 操作者展开音色选择并输入名称、中文描述或中文标签
- **THEN** 页面 SHALL 以可搜索单选列表展示匹配音色
- **AND** 每个选项 SHALL 分层展示供应商原名、中文描述以及中文性别、年龄和语言标签
- **AND** 已选触发器 SHALL 展示供应商原名和截断中文描述
- **AND** 页面 SHALL NOT 在供应商未提供中文音色名时生成或维护伪造译名

#### Scenario: 使用语言和声线 Tag 组合筛选音色

- **GIVEN** 音色弹层已展开且目录包含中文、英文、其他语种及不同声线
- **WHEN** 操作者选择一个语言 Tag 和一个声线 Tag 并输入搜索词
- **THEN** 页面 SHALL 按三项条件的交集展示结果
- **AND** 语言 Tag SHALL 为 `中文 / 英文 / 多语言`，声线 Tag SHALL 为 `男声 / 女声`
- **AND** 再次点击已选 Tag SHALL 清除该维筛选
- **AND** `多语言` SHALL 包含其他单语种、语言未知及真正多语言音色，目录条目 SHALL NOT 因分类口径被隐藏
- **AND** 结果 SHALL 保持扁平单选列表，页面 SHALL NOT 渲染树或嵌套音色组

#### Scenario: 语言代码显示中文名称

- **GIVEN** 目录音色的 `Languages` 同时包含 `Language` 代码和 `Text` 试听文案
- **WHEN** 页面展示音色标签或语言/口音选项
- **THEN** 页面 SHALL 根据 `Language` 代码显示对应中文名称并提交原始代码
- **AND** 页面 SHALL NOT 将 `Text` 试听文案显示为语言名称
- **AND** 未知语言代码 SHALL 原样显示且 SHALL NOT 回退到试听文案

### Requirement: 豆包音色目录必须支持更新后的动态可见性

系统 SHALL 使用 `Action=ListSpeakers&Version=2025-05-20`，按官方 `ResourceIDs`、`Page`、`Limit` 字段分页全量同步豆包音色目录，并支持 Admin 主动同步、定期同步和工作台检查更新。

#### Scenario: 共享模型读取来源目录

- **GIVEN** 当前启用的中转 TTS 模型已绑定同 `api_protocol + upstream_model + resource_id` 的官方目录来源
- **WHEN** 工作台、声音 Agent 或 TTS 预检读取该中转模型的音色目录
- **THEN** 系统 SHALL 返回来源模型的最近同步状态与音色条目
- **AND** 系统 SHALL 返回当前执行模型的 `model_id` 和 `model_settings`
- **AND** 系统 SHALL 明确返回实际 `source_model_id`
- **AND** 系统 SHALL NOT 复制一份目录条目到中转模型

#### Scenario: 共享模型检查目录更新

- **GIVEN** 当前中转 TTS 模型使用共享目录
- **WHEN** Admin 点击“同步音色”或工作台点击“检查更新”
- **THEN** 系统 SHALL 对实际官方目录来源创建或复用同步任务
- **AND** 多个共享模型 SHALL NOT 为同一来源创建重复活动同步任务
- **AND** 目录 Worker SHALL 只使用来源模型的 OpenAPI AK/SK

#### Scenario: 目录同步不启用语音生成

- **GIVEN** 音色目录存在排队任务且 `SPEECH_GENERATION_WORKER_ENABLED=false`
- **WHEN** 操作者只开启 `VOICE_CATALOG_WORKER_ENABLED`
- **THEN** 独立目录 Worker SHALL 消费目录任务并更新健康状态 `voice_catalog_worker=enabled`
- **AND** TTS、ASR、音频检查和 TOS 临时对象清理任务 SHALL NOT 因此被消费
- **AND** 语音生成健康状态 SHALL 继续为 `speech_generation_worker=disabled`

#### Scenario: 完整同步发现新音色

- **GIVEN** 供应商目录新增音色且分页接口可用
- **WHEN** 系统完成指定 `ResourceID` 的全量同步
- **THEN** 新音色 SHALL 自动进入该模型的可选目录
- **AND** 系统 SHALL 更新成功同步时间和目录版本

#### Scenario: 完整同步后音色消失

- **GIVEN** 本地目录存在某音色但本次完整同步结果不再包含它
- **WHEN** 同步成功提交
- **THEN** 系统 SHALL 将该音色标记为不可用于新生成
- **AND** 系统 SHALL NOT 删除音色记录或历史快照
- **AND** 引用该音色的草稿 SHALL 保留选择并阻止生成

#### Scenario: 分页同步中途失败

- **GIVEN** 音色目录包含多个分页
- **WHEN** 任一分页获取失败
- **THEN** 系统 SHALL 保留上一次完整成功目录
- **AND** 系统 SHALL NOT 将本次缺失条目批量标记下线
- **AND** 系统 SHALL 记录同步失败状态供重试

#### Scenario: 供应商可选数组返回空值

- **GIVEN** `ListSpeakers` 的标签、分类、语言或情绪字段返回 `null` 或非数组
- **WHEN** 系统提交完整目录
- **THEN** 系统 SHALL 将对应字段归一化为 `[]`
- **AND** 系统 SHALL NOT 因非空数据库数组约束导致整批目录失败

#### Scenario: 定期目录同步失败

- **GIVEN** 最近一次自动或手动目录同步已经失败
- **WHEN** 独立目录 Worker 继续轮询
- **THEN** 系统 SHALL 至少等待模型配置的同步间隔后才创建下一次自动任务
- **AND** 系统 SHALL NOT 在每个轮询周期重复调用供应商
- **AND** 操作者在没有活动任务时 SHALL 仍可主动发起一次检查更新

#### Scenario: 未知目录同步异常

- **GIVEN** 目录提交遇到未分类的数据库或运行时异常
- **WHEN** 系统记录失败终态
- **THEN** 错误摘要 SHALL 只保存稳定前缀和异常类型
- **AND** 错误摘要 SHALL NOT 保存数据库失败行、供应商 URL、签名参数或凭据

### Requirement: TTS 配音必须通过已确认的 V3 协议生成

系统 SHALL 使用 `doubao-seed-tts-2.0` 对应资源 `seed-tts-2.0` 和 HTTP Chunked V3 单向流式端点 `/api/v3/tts/unidirectional` 生成首版配音。

#### Scenario: 确认后生成 TTS

- **GIVEN** 操作者已确认文本、模型、音色、语言、风格和参数
- **WHEN** 系统创建 TTS 任务
- **THEN** 请求 SHALL 使用唯一 `X-Api-Request-Id`
- **AND** 请求 SHALL 通过专属 `X-Api-Key` 鉴权
- **AND** 请求 SHALL 使用 `X-Api-Resource-Id: seed-tts-2.0`
- **AND** 所选语言 SHALL 按官方字段 `explicit_language` 发送
- **AND** 请求 SHALL NOT 发送官方当前协议未定义的 `language` 或 `emotion`
- **AND** 运行审计 SHALL 保存响应 `X-Tt-Logid`
- **AND** 音色快照 SHALL 保存实际目录来源模型 ID，且 SHALL NOT 保存来源模型 AK/SK

#### Scenario: TTS 生成成功

- **GIVEN** 供应商流式响应完整且音频校验通过
- **WHEN** Worker 完成音频落盘
- **THEN** 系统 SHALL 创建新的 TTS 音频素材
- **AND** 系统 SHALL 保存文本、模型、音色、参数、时长和请求追踪快照
- **AND** 系统 SHALL NOT 在日志或素材 metadata 中保存明文 `X-Api-Key`

#### Scenario: 临时错误自动重试受限

- **GIVEN** TTS 请求未取得可恢复的上游结果且遇到临时错误
- **WHEN** Worker 处理错误
- **THEN** 系统 SHALL 只在同一模型自动重试最多 1 次
- **AND** 系统 SHALL NOT 自动切换模型、音色或供应商

### Requirement: OpenAI Audio Speech 中转必须使用独立协议

系统 SHALL 将 New API/OneAPI 类 `/v1/audio/speech` 中转建模为 `openai_audio_speech`，不得作为 `volcengine_tts_v3` 保存或执行。

#### Scenario: 中转模型生成配音

- **GIVEN** 启用的 `openai_audio_speech` 模型已绑定匹配的官方 Seed TTS 音色目录
- **WHEN** 操作者确认试听或生成配音
- **THEN** Worker SHALL 使用 Bearer API Key 调用 `{request_base_url}/audio/speech`
- **AND** 请求 SHALL 使用标准 `model/input/voice/response_format/speed` 字段
- **AND** Worker SHALL 校验二进制音频响应后创建新音频素材
- **AND** Worker SHALL NOT 发送 V3 `X-Api-Key` Headers 或解析 V3 NDJSON

#### Scenario: 中转模型请求同步字幕

- **GIVEN** 当前模型为 `openai_audio_speech` 且能力声明不支持字词时间戳
- **WHEN** 操作者选择 TTS 字词时间戳生成同步字幕
- **THEN** 工作台与 API SHALL 明确阻止提交
- **AND** 系统 SHALL NOT 伪造字幕时间轴
- **AND** 系统 SHALL NOT 自动创建二次 ASR 任务
- **AND** 操作者 SHALL 仍可显式切换到已有音频 ASR 流程

### Requirement: 实时试听必须由操作者主动触发

需要调用 TTS 模型的试听 SHALL 在操作者主动触发并确认资源用量后执行，不得因选项切换自动调用。

#### Scenario: 切换声音配置不自动试听

- **GIVEN** 操作者正在浏览动态音色目录
- **WHEN** 操作者切换音色、语言或声音参数
- **THEN** 页面 SHALL 更新当前选择
- **AND** 系统 SHALL NOT 自动调用 TTS 接口

#### Scenario: 主动试听

- **GIVEN** 当前文本片段、模型和声音参数有效
- **WHEN** 操作者点击试听并确认 TTS 字符数
- **THEN** 系统 SHALL 创建一次受幂等和并发限制的试听请求
- **AND** 页面 SHALL 播放成功返回的试听音频
- **AND** 页面 SHALL NOT 展示或计算金额费用

### Requirement: 字幕必须使用真实时间对齐来源

字幕 Agent SHALL 负责文本断句和样式，系统 SHALL 使用 TTS 返回时间戳或 ASR 结果形成时间轴，不得对不支持时间戳的语种或方言伪造对齐结果。

#### Scenario: 从 TTS 时间戳生成字幕

- **GIVEN** TTS 返回受支持的中文或英文 `sentence.words` 字词时间戳
- **WHEN** 字幕 Agent 完成断句
- **THEN** 系统 SHALL 依据供应商时间戳生成字幕时间轴
- **AND** 系统 SHALL 输出新的 `SRT` 字幕素材

#### Scenario: TTS 语种不支持时间戳

- **GIVEN** 当前 TTS 语种或方言不返回可信时间戳
- **WHEN** 操作者请求同步字幕
- **THEN** 系统 SHALL 明确标记无法自动对齐
- **AND** 系统 SHALL NOT 生成伪造时间戳的成功字幕

#### Scenario: 已有音频通过 ASR 生成字幕

- **GIVEN** 操作者选择已有或上传的音频素材
- **WHEN** 系统使用 `ffprobe` 完成该自管音频的真实时长、格式和 SHA-256 检查，且操作者确认使用 `doubao-seed-asr-2.0` 生成字幕
- **THEN** 系统 SHALL 使用 API 资源 `volc.seedasr.auc` 创建 ASR 任务
- **AND** 系统 SHALL 锁定当前启用的系统 TOS 工具配置 ID 与版本
- **AND** 系统 SHALL 将本地音频幂等上传至该锁定版本的私有 TOS 暂存空间
- **AND** 系统 SHALL 只向 ASR 提交短期签名 GET URL
- **AND** 成功结果 SHALL 创建新的字幕素材并关联来源音频

#### Scenario: 音频检查与执行时文件不一致

- **GIVEN** 操作者已依据成功的音频检查快照确认真实时长
- **WHEN** ASR Worker 执行前发现自管文件 SHA-256 与检查快照不一致
- **THEN** 系统 SHALL 在上传 TOS 前以稳定错误终止任务
- **AND** 系统 SHALL 要求重新检查并再次确认资源用量
- **AND** 系统 SHALL NOT 信任前端或素材 metadata 中的时长继续执行

#### Scenario: ASR 终态清理 TOS 临时对象

- **GIVEN** ASR 任务已经成功、失败或取消且存在 TOS 临时对象
- **WHEN** Worker 执行终态清理
- **THEN** 系统 SHALL 删除该任务的临时对象
- **AND** 签名 URL、查询参数和 TOS 凭据 SHALL NOT 写入数据库、日志、Agent 消息或素材 metadata
- **AND** 删除失败 SHALL 记录不含敏感信息的待清理状态并允许定期重试
- **AND** 删除失败 SHALL NOT 覆盖已经成功的 ASR 与字幕结果

### Requirement: 私有 TOS 暂存必须作为系统公用工具独立管理

系统 SHALL 在 Admin“工具与 MCP”中维护一个供全部 ASR 模型共用的版本化私有 TOS 工具配置，且 SHALL NOT 将 TOS 字段保存在 `ai_models` 或模型快照中。

#### Scenario: 首次配置并启用系统 TOS 工具

- **GIVEN** 系统尚未配置私有 TOS 工具
- **WHEN** 管理员提交有效的 Endpoint、Region、Bucket、Object Prefix、AK/SK、签名有效期和文件/音频限制
- **THEN** 系统 SHALL 创建首个未启用的当前配置版本
- **AND** 管理响应 SHALL 只返回 AK/SK 掩码和已配置状态
- **AND** 未通过该版本的真实连接检查前，系统 SHALL 拒绝启用并返回稳定错误
- **WHEN** 管理员发起检查且系统 TOS 工具 Worker 使用官方 `tos` SDK 对该版本完成 Bucket 访问、探针写入、签名读取和删除
- **THEN** 系统 SHALL 标记连接成功并允许管理员创建仅启用状态变化的新版本
- **AND** 全部 ASR 模型 SHALL 共享该工具而无需各自配置

#### Scenario: 连接检查异步执行且不得伪造成功

- **GIVEN** 当前系统 TOS 工具版本已保存
- **WHEN** 管理员使用期望版本调用连接检查 API
- **THEN** API SHALL 将检查状态置为 `queued`，独立的系统 TOS 工具 Worker 领取后置为 `running`
- **AND** 系统 TOS 工具 Worker SHALL 使用独立且默认关闭的执行开关，不得因连接检查而启用 TTS/ASR 任务消费
- **AND** 只有官方 `tos` SDK 的 `HeadBucket` 与固定探针的 `PutObject`、签名 GET 实读、`DeleteObject` 全部成功后 SHALL 写入 `succeeded`
- **AND** 探针失败 SHALL 尽力删除同一固定对象键，后续重试 SHALL 能覆盖并再次清理该键
- **AND** 本地字段校验 SHALL NOT 写入连接成功状态
- **AND** 失败状态 SHALL 只保存脱敏错误类型，不得保存 AK/SK、签名 URL 或上游响应敏感内容

#### Scenario: 编辑系统 TOS 工具并保留凭据

- **GIVEN** 当前系统 TOS 工具已配置且不存在待清理对象
- **WHEN** 管理员使用当前版本编辑非敏感字段并将 AK/SK 同时留空
- **THEN** 系统 SHALL 创建新的当前配置版本并保留旧凭据
- **AND** 旧版本 SHALL 保留供既有任务和清理重试读取
- **AND** 版本冲突 SHALL 返回稳定错误且不得覆盖较新配置

#### Scenario: 待清理对象阻止工具变更但不影响模型

- **GIVEN** 系统存在 `cleanup_pending` 的 TOS 临时对象
- **WHEN** 管理员尝试修改或停用系统 TOS 工具
- **THEN** 系统 SHALL 拒绝工具变更并返回待清理对象数量
- **AND** ASR 模型生命周期操作 SHALL 继续按模型规则执行

#### Scenario: ASR 任务锁定系统 TOS 配置版本

- **GIVEN** 当前系统 TOS 工具已配置、已通过连接检查并启用
- **WHEN** 系统创建 ASR 任务
- **THEN** 任务 SHALL 保存 `tos_staging_config_id` 和 `tos_staging_config_version`
- **AND** Worker 与清理重试 SHALL 只读取该锁定版本
- **AND** 系统 SHALL NOT 回退到最新配置或 ASR 模型字段

#### Scenario: 系统 TOS 未配置时阻止 ASR 创建

- **GIVEN** 系统 TOS 工具未配置、未通过连接检查或未启用
- **WHEN** 操作者确认创建 ASR 任务
- **THEN** API SHALL 返回稳定配置错误且不得创建任务
- **AND** 音频检查 SHALL 仍可独立执行

### Requirement: 声音与字幕任务必须展示非金额资源用量

系统 SHALL 在试听、生成和失败重试前展示模型、TTS 字符数或音频时长及任务数量，并要求主动确认，但 SHALL NOT 建设金额费用能力。

#### Scenario: TTS 生成前确认

- **GIVEN** TTS 文本和参数校验通过
- **WHEN** 操作者准备生成
- **THEN** 页面 SHALL 展示模型、音色、字符数、预计输出数量和字幕选项
- **AND** 只有操作者确认后系统才 SHALL 创建任务
- **AND** 页面 SHALL NOT 展示价格、币种、预计费用或金额上限

#### Scenario: 失败节点重试前确认

- **GIVEN** 某 TTS 或 ASR 节点失败
- **WHEN** 操作者请求重试
- **THEN** 页面 SHALL 展示将再次调用的模型任务和资源用量
- **AND** 系统 SHALL 只重试失败节点
- **AND** 已成功素材 SHALL 继续复用

### Requirement: 失败任务必须提供完整脱敏诊断且列表布局稳定

系统 SHALL 为新产生的声音与字幕失败任务保存并返回安全白名单内的结构化诊断，并在工作台展示完整追踪信息；系统 SHALL NOT 保存或返回原始供应商响应头、响应体或任何凭据。左栏任务数量超过可见空间时 SHALL 只滚动任务列表，任务卡 SHALL 保持内容固有高度。

#### Scenario: 火山语音 HTTP 错误保存结构化诊断

- **GIVEN** 火山 TTS 或 ASR 返回非 2xx JSON，且响应包含供应商错误码、消息和 `X-Tt-Logid`
- **WHEN** Worker 将任务置为失败
- **THEN** 任务 SHALL 保存 HTTP 状态、供应商错误码、脱敏供应商消息和完整 `X-Tt-Logid`
- **AND** API SHALL 同时返回内部错误码、稳定错误摘要、完整 `request_id`、模型快照、尝试次数和完成时间
- **AND** 系统 SHALL NOT 保存原始响应头或响应体

#### Scenario: 中转或非 JSON 错误安全回退

- **GIVEN** 中转 TTS 返回 OpenAI `error` 对象、无法识别的 JSON 或非 JSON 响应
- **WHEN** Worker 解析失败响应
- **THEN** 系统 SHALL 只提取白名单内的错误码、消息和允许的响应追踪头
- **AND** New API 中转返回的 `X-OneAPI-Request-Id` SHALL 作为响应追踪 ID 保存
- **AND** 消息中的 API Key、Authorization、Bearer、Access Key 或 Secret Key 值 SHALL 被掩码
- **AND** 无法识别的响应 SHALL 只保存稳定 HTTP 摘要，不得保存原文

#### Scenario: 历史失败任务没有结构化诊断

- **GIVEN** 失败任务创建于结构化诊断上线前且只保存了错误摘要
- **WHEN** 工作台读取该任务
- **THEN** API SHALL 返回空 `error_details`
- **AND** 工作台 SHALL 对缺失的供应商字段显示 `-`
- **AND** 系统 SHALL NOT 从错误摘要猜测供应商错误码、消息或追踪 ID

#### Scenario: 多个失败任务不压缩左栏卡片

- **GIVEN** 当前筛选结果包含 8 个连续失败任务
- **WHEN** 左栏可见高度不足以展示全部任务
- **THEN** 筛选栏与项目并发状态 SHALL 保持固定
- **AND** 任务列表 SHALL 只产生纵向滚动且不得产生横向滚动
- **AND** 每张任务卡的标题、模型、状态、时间、错误摘要和重试按钮 SHALL 完整显示且不得因任务数量增加被压缩
