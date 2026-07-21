## ADDED Requirements

### Requirement: 声音 Agent 必须使用完整当前编辑上下文和可用音色目录

系统 SHALL 在每轮声音建议中使用操作者发送时的当前旁白、TTS 模型、已选音色、语言、声音参数和字幕断句快照，并 SHALL 在当前 TTS 模型的全部可用音色中生成建议，不得依赖“旁边文本”等未传输的界面引用或按固定数量静默截断目录。

#### Scenario: 根据当前编辑区理解用户引用

- **GIVEN** 操作者已在中栏编辑旁白和声音配置
- **WHEN** 操作者向声音 Agent 发送包含“旁边的文本”或同类界面引用的要求
- **THEN** 请求 SHALL 携带发送时的完整旁白和当前声音配置快照
- **AND** Agent Prompt SHALL 明确区分用户要求与当前编辑上下文
- **AND** Agent SHALL NOT 仅根据自由文本猜测未传输的旁白

#### Scenario: 从完整可用目录推荐音色

- **GIVEN** 当前 TTS 模型存在超过 80 个可用音色
- **WHEN** 声音 Agent 生成声音建议
- **THEN** Agent 的候选输入 SHALL 覆盖全部可用音色
- **AND** 系统 SHALL NOT 使用固定数量截断导致目录后部音色不可推荐
- **AND** 候选输入 SHALL 排除试听文案、头像和其他与推荐无关的目录字段

#### Scenario: 拒绝与会话不一致的声音上下文

- **GIVEN** 声音会话已绑定一个 TTS 模型
- **WHEN** 消息上下文缺失或携带不同的 TTS 模型 ID
- **THEN** 系统 SHALL 返回稳定校验错误
- **AND** 系统 SHALL NOT 调用文本模型或读取不同模型的音色目录

#### Scenario: 建议仍需人工确认

- **GIVEN** 声音 Agent 已基于当前上下文返回建议
- **WHEN** 操作者尚未应用、试听或确认生成
- **THEN** 系统 SHALL NOT 调用 TTS 或 ASR
- **AND** 操作者 SHALL 能在应用后继续修改文本和参数

#### Scenario: 使用供应商支持的结构化 JSON 模式

- **GIVEN** 当前文本供应商的 Responses 接口支持 `json_object` 但对严格 `json_schema` 返回网关错误
- **WHEN** 声音 Agent 请求声音建议
- **THEN** 系统 SHALL 使用 `json_object` 输出模式并在 Prompt 中提供完整字段契约
- **AND** 系统 SHALL 使用 Rust 类型和目录能力执行严格输出校验
- **AND** 系统 SHALL NOT 在 `json_schema` 失败后自动降级、重试或产生第二次计费调用
