## ADDED Requirements

### Requirement:阶段二 Provider 按能力准入
Fish Audio、Groq、音乐生成和新增视频模式 SHALL 先完成 adapter 安装、approved catalog、显式 feature gate、credential、capability probe、参数 schema、配额和费用映射；未满足前置 MUST 保持 candidate/unconfigured 且零外部调用。

#### Scenario:首次 probe 后才可运行
- **WHEN** 用户对阶段二 operation 提供 explicit live opt-in 并完成 probe
- **THEN** 系统冻结 capability snapshot；只有后续显式 enable/default/run 才可调用

#### Scenario:Provider 失败不隐式切换
- **WHEN** TTS/ASR/音乐 Provider 返回 unknown、429 或 credential error
- **THEN** 保留原生状态和 correlation，按 policy 重试或阻断，不切换其他 Provider、不重复收费

### Requirement:媒体语音结果拥有版本和来源
TTS/ASR/音乐结果 MUST 经过 MIME、时长、采样率、checksum、license 和 source hash 校验后登记为 immutable AssetVersion 或 caption revision；ProviderCall、usage 和用户确认必须可追溯。

#### Scenario:接受 ASR 对齐结果
- **WHEN** ASR 结果与输入音频 AssetVersion hash 匹配且用户接受
- **THEN** 系统追加字幕 revision 和 provenance，旧字幕不被覆盖
