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

#### Scenario: Agent 建议不直接执行

- **GIVEN** 声音 Agent 已推荐文本、音色或字幕断句
- **WHEN** 操作者尚未确认生成
- **THEN** 系统 SHALL 允许操作者修改建议内容和参数
- **AND** 系统 SHALL NOT 调用 TTS 或 ASR 服务

### Requirement: TTS 模型和音色能力必须动态加载

系统 SHALL 从已启用 TTS 模型和版本化能力目录加载音色、语言/口音、情绪风格及可调参数，禁止由前端、代码枚举或 migration 写死音色。

#### Scenario: 选择 TTS 模型后加载真实能力

- **GIVEN** Admin 已配置一个启用的 TTS 模型及能力目录
- **WHEN** 操作者选择该模型
- **THEN** 页面 SHALL 展示该模型当前可用的音色、语言/口音、情绪风格和参数
- **AND** 页面 SHALL 展示能力目录更新时间
- **AND** 页面 SHALL NOT 展示目录中不存在的组合

#### Scenario: 切换模型使原选择失效

- **GIVEN** 当前已选择某音色和情绪风格
- **WHEN** 操作者切换到不支持该选择的 TTS 模型
- **THEN** 系统 SHALL 保留并标记原选择失效
- **AND** 系统 SHALL 阻止生成直到操作者重新选择
- **AND** 系统 SHALL NOT 静默替换音色或风格

#### Scenario: Agent 推荐声音风格

- **GIVEN** 当前模型能力目录可用
- **WHEN** 声音 Agent 根据旁白内容推荐音色和风格
- **THEN** Agent SHALL 只从当前目录的可用选项中推荐
- **AND** 操作者 SHALL 能查看、修改、试听并确认推荐
- **AND** Agent SHALL NOT 虚构模型不支持的声音能力

### Requirement: 豆包音色目录必须支持更新后的动态可见性

系统 SHALL 使用 `Action=ListSpeakers&Version=2025-05-20` 按 `ResourceID` 分页全量同步豆包音色目录，并支持 Admin 主动同步、定期同步和工作台检查更新。

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

### Requirement: TTS 配音必须通过已确认的 V3 协议生成

系统 SHALL 使用 `doubao-seed-tts-2.0` 对应资源 `seed-tts-2.0` 和 HTTP Chunked V3 单向流式端点 `/api/v3/tts/unidirectional` 生成首版配音。

#### Scenario: 确认后生成 TTS

- **GIVEN** 操作者已确认文本、模型、音色、语言、风格和参数
- **WHEN** 系统创建 TTS 任务
- **THEN** 请求 SHALL 使用唯一 `X-Api-Request-Id`
- **AND** 请求 SHALL 通过专属 `X-Api-Key` 鉴权
- **AND** 请求 SHALL 启用 `enable_subtitle`
- **AND** 运行审计 SHALL 保存响应 `X-Tt-Logid`

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

### Requirement: 实时试听必须由操作者主动触发

需要调用 TTS 模型的试听 SHALL 在操作者主动触发并确认资源用量后执行，不得因选项切换自动调用。

#### Scenario: 切换音色不自动试听

- **GIVEN** 操作者正在浏览动态音色目录
- **WHEN** 操作者切换音色或情绪风格
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

- **GIVEN** TTS 返回受支持的中文或英文字幕字词时间戳
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
- **WHEN** 操作者确认使用 `doubao-seed-asr-2.0` 生成字幕
- **THEN** 系统 SHALL 使用资源 `volc.seedasr.sauc.duration` 创建 ASR 任务
- **AND** 成功结果 SHALL 创建新的字幕素材并关联来源音频

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
