# material-library-management Delta Specification

## ADDED Requirements

### Requirement: 素材库必须统一管理作品生产生成物

系统 SHALL 将作品生产产生的 TTS 音频、ASR/TTS 字幕、混音音频和最终可复用媒体登记为素材，并与已有图片、视频、音频和字幕使用同一素材生命周期管理。

#### Scenario: TTS 和字幕生成成功后自动入库

- **GIVEN** 作品运行中的 TTS 和字幕步骤成功
- **WHEN** 系统持久化步骤输出
- **THEN** 系统 SHALL 分别创建 `audio` 和 `subtitle` 素材
- **AND** 素材 SHALL 使用自管存储稳定 URL
- **AND** 素材 SHALL 关联来源作品、版本、运行和步骤

#### Scenario: 重新生成不覆盖旧素材

- **GIVEN** 某作品版本已经生成 TTS 或字幕素材
- **WHEN** 操作者确认重新生成相关节点
- **THEN** 系统 SHALL 创建新的素材记录和文件
- **AND** 系统 SHALL NOT 覆盖、改写或删除旧素材

#### Scenario: 文件落盘失败不登记素材

- **GIVEN** 作品步骤已返回媒体结果
- **WHEN** 自管存储写入或完整性校验失败
- **THEN** 系统 SHALL 将步骤结果标记失败
- **AND** 系统 SHALL NOT 创建伪成功素材记录

### Requirement: 生成素材必须保留可审计快照

系统 SHALL 为作品生产生成的素材保存足以回溯结果的来源、模型、提示词、时间轴和参数快照，且 SHALL NOT 保存明文密钥。

#### Scenario: 查看 TTS 素材来源

- **GIVEN** 素材由 TTS 步骤生成
- **WHEN** 操作者查看素材详情
- **THEN** 系统 SHALL 展示来源作品和版本、模型快照、音色快照、声音参数、文本摘要、语言、时长和来源任务
- **AND** 系统 SHALL 保留供应商请求追踪 ID
- **AND** 系统 SHALL NOT 展示或保存 `X-Api-Key`

#### Scenario: 查看字幕素材来源

- **GIVEN** 素材由 TTS 时间戳或 ASR 生成
- **WHEN** 操作者查看字幕素材详情
- **THEN** 系统 SHALL 展示字幕语言、格式、对齐来源、时间轴版本和来源音频
- **AND** 系统 SHALL 能区分 `tts_timestamp` 与 `asr` 来源

### Requirement: 已有声音素材必须可用于作品混音

系统 SHALL 允许操作者从 `active` 音频素材中选择已有 BGM、环境音和动作音效用于作品时间轴，但首版 SHALL NOT 提供这些类型的 AI 生成入口。

#### Scenario: 选择已有音频进入作品

- **GIVEN** 素材库存在标记为 BGM、环境音或动作音效的 `active` 音频
- **WHEN** 操作者在作品生成中选择已有音频
- **THEN** 系统 SHALL 将素材引用和混音参数加入作品草稿
- **AND** 系统 SHALL NOT 复制或覆盖原素材

#### Scenario: 归档音频不可用于新作品

- **GIVEN** 某音频素材状态为 `archived`
- **WHEN** 操作者为新作品选择声音素材
- **THEN** 系统 SHALL NOT 将该素材列为可选项
- **AND** 已完成历史版本 SHALL 继续保留该素材快照和引用

#### Scenario: 不展示未落地 AI 声音生成入口

- **GIVEN** AI 音乐、环境音和动作音效生成尚未配置正式能力
- **WHEN** 操作者打开素材管理
- **THEN** 页面 SHALL NOT 展示 AI 音乐、环境音生成或动作音效生成标签及可执行按钮
- **AND** 素材库 SHALL 继续允许上传和管理已有相关音频

### Requirement: 素材筛选必须覆盖作品生产声音类型

素材库 SHALL 支持按音频用途、生成来源、来源作品和来源版本筛选作品生产素材。

#### Scenario: 按音频用途筛选

- **GIVEN** 素材库同时存在 TTS、BGM、环境音和动作音效
- **WHEN** 操作者选择某一音频用途筛选
- **THEN** 系统 SHALL 只返回匹配用途的音频素材
- **AND** 未标注用途的历史音频 SHALL 保持可见且显示为未分类

#### Scenario: 从作品版本定位生成素材

- **GIVEN** 某作品版本生成了视频、音频和字幕产物
- **WHEN** 操作者按该作品版本筛选素材库
- **THEN** 系统 SHALL 返回与该版本关联的全部可复用素材
- **AND** 结果 SHALL 保留各自产物类型和生成步骤信息
