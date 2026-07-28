# work-generation Specification

## Purpose
TBD - created by archiving change add-work-generation. Update Purpose after archive.
## Requirements
### Requirement: 作品生成正式入口必须替换旧视频生成占位菜单

系统 SHALL 在 `作品生产` 下只展示当前作品生产领域入口；旧 `video-generation` 占位菜单 SHALL 保留数据库记录但不得继续返回前端。

#### Scenario: 加载作品生产菜单

- **GIVEN** 数据库同时存在历史 `video-generation` 和正式 `work-generation` 菜单记录
- **WHEN** 工作台读取可见菜单树
- **THEN** `作品生产` SHALL 展示 `作品生成` 和已实现的后续业务入口
- **AND** 工作台 SHALL NOT 展示旧 `视频生成` 占位菜单

### Requirement: 作品生成必须以完整作品为一次用户提交边界

系统 SHALL 汇总全部分镜主图片、镜头描述和旁白，在一次用户确认后创建整部作品的生成运行；页面 SHALL NOT 要求操作者逐图片或逐镜头分别提交视频生成。

#### Scenario: 从完整分镜创建作品草稿

- **GIVEN** 脚本全部分镜均已选择可用主图片
- **WHEN** 操作者进入 `作品生产 / 作品生成`
- **THEN** 系统 SHALL 按分镜顺序创建或恢复作品草稿
- **AND** 草稿 SHALL 包含全部主图片、镜头描述、旁白及来源快照
- **AND** 页面 SHALL 提供一个作品级生成确认入口

#### Scenario: 查看主画面镜头描述

- **GIVEN** 某个主画面的镜头描述超过两行
- **WHEN** 操作者在作品计划中查看主画面清单
- **THEN** 页面 SHALL 完整展示镜头描述
- **AND** 文案 SHALL NOT 被卡片高度或固定行数裁切

#### Scenario: 后台拆分不改变一次提交语义

- **GIVEN** 已确认作品需要多个视频子任务
- **WHEN** 操作者提交作品生成
- **THEN** 系统 SHALL 创建一个作品级运行并在后台创建合法子任务
- **AND** 页面 SHALL 将其展示为一次作品提交及其分步骤详情
- **AND** 页面 SHALL NOT 要求操作者再次逐段启动

#### Scenario: 作品生成工作区使用可读字号

- **GIVEN** 操作者在桌面端打开作品生成工作区
- **WHEN** 页面展示 Agent 对话、作品计划、镜头清单和参数确认
- **THEN** 可见辅助文字 SHALL NOT 小于 `12px`
- **AND** 主要正文、表单值、按钮和标签 SHALL 使用 `13~14px`
- **AND** 区块标题 SHALL 使用不小于 `16px` 的明确层级
- **AND** 字号提升后页面 SHALL NOT 裁切文本或产生横向溢出

#### Scenario: 作品生成工作区占满视口剩余高度

- **GIVEN** 操作者在桌面端打开带完整左侧业务菜单的作品生成工作区
- **WHEN** 菜单内容高度超过或接近当前视口高度
- **THEN** 工作台根节点 SHALL 锁定为当前视口高度且页面根节点 SHALL NOT 产生纵向滚动
- **AND** 左侧菜单 SHALL 在自身区域内滚动
- **AND** 作品生成三栏 SHALL 占满顶部栏和页面标题区下方的剩余高度
- **AND** 三栏底部与视口底部 SHALL 只保留标准页面间距，不得暴露额外页面背景
- **AND** 中间计划与右侧参数 SHALL 在面板内部滚动
- **AND** 参数操作栏 SHALL 固定在参数面板底部

### Requirement: 作品 Agent 对话和最终提示词必须可见可改

系统 SHALL 通过统一 Agent Runtime 提供可见的作品 Agent 对话，展示方案推导、工具步骤、最终提示词和分段提示词，并允许操作者在生成前继续修改。

#### Scenario: Agent 生成全片方案

- **GIVEN** 作品草稿输入完整且模型已选择
- **WHEN** 操作者要求 Agent 规划作品
- **THEN** Agent SHALL 读取全部分镜图片、镜头描述、旁白和输出参数
- **AND** 页面 SHALL 展示 Agent 消息、方案摘要、最终提示词和分段计划
- **AND** 系统 SHALL NOT 在规划阶段调用 Seedance

#### Scenario: 操作者修改最终提示词

- **GIVEN** Agent 已产生待确认方案
- **WHEN** 操作者通过对话或编辑区修改最终提示词
- **THEN** 系统 SHALL 保存新的计划版本
- **AND** 系统 SHALL 重新校验提示词和受影响分段
- **AND** 旧计划 SHALL 失效且不得被提交

#### Scenario: 运行中对话修改

- **GIVEN** 某作品版本已经开始运行
- **WHEN** 操作者继续通过 Agent 提出修改
- **THEN** 系统 SHALL 将修改保存到下一版草稿
- **AND** 当前运行的锁定快照 SHALL 保持不变

### Requirement: 方案 LLM、视频和 TTS 模型必须独立选择

系统 SHALL 按能力类型展示已启用的方案 LLM、视频和 TTS 模型，首次可预选 Admin 默认模型；Agent 只能推荐，不能自动切换。

#### Scenario: 加载默认模型

- **GIVEN** Admin 分别配置了启用的默认 LLM、视频和 TTS 模型
- **WHEN** 操作者新建作品草稿
- **THEN** 页面 SHALL 分别预选三个默认模型
- **AND** 操作者 SHALL 能在同类型启用模型中修改选择

#### Scenario: 模型切换触发重新规划

- **GIVEN** 作品已有待确认计划
- **WHEN** 操作者切换任一模型
- **THEN** 系统 SHALL 刷新对应模型真实能力
- **AND** 系统 SHALL 重新校验参数、提示词、任务拆分和资源用量
- **AND** 操作者 SHALL 再次确认新计划

#### Scenario: 已选模型停用或删除

- **GIVEN** 草稿引用的模型已被 Admin 停用或删除
- **WHEN** 操作者打开草稿或准备生成
- **THEN** 页面 SHALL 保留原模型选择和快照并标记不可用
- **AND** 系统 SHALL 阻止生成
- **AND** 系统 SHALL NOT 自动替换成默认模型

### Requirement: 作品生成必须使用统一 Select 和动态音色目录选择器

系统 SHALL 对作品生成页的标准单行 Select 使用工作台统一组件与视觉状态；音色 SHALL 使用当前 TTS 模型的动态目录，并提供与声音生成页一致的可搜索筛选选择器。

#### Scenario: 展示统一标准 Select

- **GIVEN** 操作者打开作品生成参数区
- **WHEN** 页面展示方案 LLM、视频模型、TTS 模型、时长、比例、分辨率和声音模式
- **THEN** 标准 Select SHALL 使用一致的高度、字体、边框、圆角和右侧箭头
- **AND** hover、focus、disabled 和 expanded 状态 SHALL 使用同一套工作台样式

#### Scenario: 搜索和筛选音色

- **GIVEN** 当前 TTS 模型已返回可用音色目录
- **WHEN** 操作者展开音色选择器
- **THEN** 页面 SHALL 展示可用数量和可滚动音色列表
- **AND** 操作者 SHALL 能按名称、描述或标签搜索
- **AND** 操作者 SHALL 能按中文、英文、多语言以及男声、女声筛选
- **AND** 每个结果 SHALL 展示名称、描述和语言/声线标签

#### Scenario: 切换模型后原音色失效

- **GIVEN** 草稿已选择某个音色
- **WHEN** 操作者切换到不包含该音色的 TTS 模型
- **THEN** 页面 SHALL 保留原音色并标记为已失效
- **AND** 系统 SHALL 在重新选择有效音色前阻止生成计划
- **AND** 系统 SHALL NOT 静默选择新目录的第一个音色

### Requirement: 操作者必须能选择成片时长、比例和分辨率

系统 SHALL 提供 `15/30/45/60秒` 预设、`4~60秒` 自定义和“跟随配音”时长策略，并按当前视频模型能力目录提供比例和分辨率组合。

#### Scenario: 选择固定时长和输出规格

- **GIVEN** 当前视频模型支持所选比例和分辨率
- **WHEN** 操作者选择固定总时长、比例和分辨率
- **THEN** 系统 SHALL 将其解释为最终成片总时长和输出规格
- **AND** 系统 SHALL 以该组合重新生成作品计划

#### Scenario: 自定义时长越界

- **GIVEN** 操作者选择自定义时长
- **WHEN** 输入小于 4 秒或大于 60 秒
- **THEN** 系统 SHALL 拒绝创建可确认计划
- **AND** 页面 SHALL 明确显示允许范围为 `4~60秒`

#### Scenario: 跟随配音

- **GIVEN** 声音模式包含独立 TTS 且操作者选择“跟随配音”
- **WHEN** TTS 生成并取得实际时长
- **THEN** 系统 SHALL 使用实际配音时长重新计算成片总时长和视频分段
- **AND** 实际时长超过 60 秒时系统 SHALL 阻止继续并要求调整旁白

#### Scenario: 不支持的比例分辨率组合

- **GIVEN** 当前能力目录不支持所选比例与分辨率组合
- **WHEN** 操作者请求规划或确认生成
- **THEN** 系统 SHALL 拒绝该组合
- **AND** 页面 SHALL 只推荐当前模型真实支持的替代组合

### Requirement: Seedance 子任务必须遵守模型真实能力限制

系统 SHALL 在一次作品提交内部按当前模型能力目录自动拆分 Seedance 子任务，并确保每个请求的时长、参考图和提示词合法。

#### Scenario: 超过单任务时长自动分段

- **GIVEN** 最终作品总时长超过 Seedance 单任务上限
- **WHEN** 系统生成视频分段计划
- **THEN** 系统 SHALL 优先按分镜边界拆分
- **AND** 每段 SHALL 位于 `4~15秒`
- **AND** 尾段不足 4 秒时系统 SHALL 重新分配相邻片段时长

#### Scenario: 参考图超过单任务上限

- **GIVEN** 某候选分段涉及超过 9 张参考图
- **WHEN** 系统校验分段计划
- **THEN** 系统 SHALL 继续拆分或调整分段边界
- **AND** 每个 Seedance 子任务 SHALL 使用不超过 9 张参考图

#### Scenario: 中文提示词过长

- **GIVEN** 某 Seedance 分段中文提示词超过官方建议范围
- **WHEN** 系统完成提示词规划
- **THEN** Agent SHALL 在不丢失核心角色、场景、动作和连续性约束的前提下压缩该提示词
- **AND** 待提交中文提示词 SHALL 控制在 500 字以内
- **AND** 页面 SHALL 展示压缩后的最终文本供确认

#### Scenario: 无法形成合法分段

- **GIVEN** 当前素材、时长和模型能力无法形成全部合法子任务
- **WHEN** 系统执行生成前校验
- **THEN** 系统 SHALL 阻止提交
- **AND** 系统 SHALL 返回具体不合法分段和可调整参数

### Requirement: 作品必须支持三种声音来源模式

系统 SHALL 支持 `独立 TTS`、`Seedance 原声` 和 `Seedance 原声 + TTS` 三种模式；后两种只对真实支持原声的视频模型可见。

#### Scenario: 独立 TTS 默认模式

- **GIVEN** 操作者未主动改变声音模式
- **WHEN** 系统规划作品
- **THEN** 系统 SHALL 使用 `独立 TTS`
- **AND** Seedance 请求 SHALL 使用 `generate_audio=false`
- **AND** 系统 SHALL 生成 TTS 并使用其时间戳生成字幕

#### Scenario: Seedance 原声模式

- **GIVEN** 当前视频模型支持原声且操作者选择 `Seedance 原声`
- **WHEN** 系统规划作品
- **THEN** Seedance 请求 SHALL 使用 `generate_audio=true`
- **AND** 系统 SHALL NOT 生成独立 TTS
- **AND** 开启字幕时系统 SHALL 使用 `doubao-seed-asr-2.0` 对原声生成时间轴

#### Scenario: Seedance 原声加 TTS

- **GIVEN** 当前视频模型支持原声且操作者选择 `Seedance 原声 + TTS`
- **WHEN** 系统规划混音
- **THEN** Seedance 请求 SHALL 使用 `generate_audio=true`
- **AND** 系统 SHALL 生成独立 TTS
- **AND** FFmpeg SHALL 在 TTS 区间自动压低不可分轨的 Seedance 原声
- **AND** 页面 SHALL 在确认前提示双重人声风险和 Agent 无法保证原声无对白

#### Scenario: 视频模型不支持原声

- **GIVEN** 当前视频模型能力目录标记不支持原声
- **WHEN** 页面展示声音模式
- **THEN** 页面 SHALL 只提供 `独立 TTS`
- **AND** 系统 SHALL 拒绝提交 `generate_audio=true`

### Requirement: 已有音频必须可加入作品多轨时间轴

系统 SHALL 允许在作品草稿中选择素材库已有的 BGM、环境音和动作音效，配置时间范围和混音参数；首版 SHALL NOT 调用 AI 生成这些声音。

#### Scenario: 添加已有音频素材

- **GIVEN** 素材库存在可用的 BGM、环境音或动作音效
- **WHEN** 操作者将其加入作品
- **THEN** 系统 SHALL 在多轨时间轴保存素材引用、起止时间、音量和淡入淡出参数
- **AND** 生成计划 SHALL 将其列为本地混音输入而非模型生成任务

#### Scenario: 已有音频超过成片范围

- **GIVEN** 某音频片段的时间范围超出最终成片时长
- **WHEN** 系统执行生成前校验
- **THEN** 系统 SHALL 阻止确认或要求操作者调整裁剪范围
- **AND** 系统 SHALL NOT 静默丢弃超出部分

### Requirement: 字幕必须支持烧录和外挂输出

系统 SHALL 默认烧录字幕并独立生成 `SRT`；操作者可关闭烧录，此时仍须保存外挂字幕文件。

#### Scenario: 默认烧录字幕

- **GIVEN** 作品已生成有效字幕时间轴
- **WHEN** 操作者使用默认字幕设置确认生成
- **THEN** FFmpeg SHALL 将字幕烧录进成片
- **AND** 系统 SHALL 另存独立 `SRT`
- **AND** 字幕样式和烧录开关 SHALL 进入版本快照

#### Scenario: 关闭字幕烧录

- **GIVEN** 操作者关闭字幕烧录
- **WHEN** 系统执行最终合成
- **THEN** 成片 SHALL 不包含烧录字幕
- **AND** 系统 SHALL 继续输出并保存 `SRT`

#### Scenario: 仅修改字幕

- **GIVEN** 已完成版本的视频片段和音频未变化
- **WHEN** 操作者派生新草稿并只修改字幕文本或样式
- **THEN** 影响分析 SHALL 只安排字幕重建和最终合成
- **AND** 系统 SHALL NOT 再次调用 Seedance

### Requirement: 最终成片必须由 FFmpeg 确定性合成

系统 SHALL 使用 FFmpeg 执行视频片段拼接、多轨混音、原声 ducking、字幕处理和封装，并输出 `MP4(H.264) + AAC`。

#### Scenario: 合成标准成片

- **GIVEN** 全部必需视频、音频和字幕输入已成功
- **WHEN** 合成步骤执行
- **THEN** 输出容器 SHALL 为 MP4
- **AND** 视频编码 SHALL 为 H.264
- **AND** 音频编码 SHALL 为 AAC
- **AND** 输出时长 SHALL 与确认的最终成片时长一致

#### Scenario: 必需输入缺失

- **GIVEN** 任一必需视频片段、主音轨或字幕输入缺失或校验失败
- **WHEN** 合成步骤准备执行
- **THEN** 系统 SHALL 将合成节点标记失败并说明缺失输入
- **AND** 系统 SHALL NOT 输出或入库伪装成功的成片

### Requirement: 生成确认必须展示非金额资源用量并保证幂等

系统 SHALL 在作品生成前展示模型和参数快照、视频子任务数、视频总秒数、TTS 字符数、ASR 音频时长及输出物，并使用幂等键防止重复创建运行；系统 SHALL NOT 计算金额。

#### Scenario: 确认有效计划

- **GIVEN** 当前计划通过全部能力和输入校验
- **WHEN** 页面显示最终确认
- **THEN** 页面 SHALL 展示模型、音色、时长、比例、分辨率、声音模式、字幕配置、子任务数和资源用量
- **AND** 页面 SHALL NOT 展示价格、币种、预计费用、实际费用或金额上限
- **AND** 只有操作者确认后系统才 SHALL 创建运行

#### Scenario: 相同幂等键重复提交

- **GIVEN** 某计划和 `Idempotency-Key` 已创建作品运行
- **WHEN** 客户端使用相同 key 重试确认请求
- **THEN** API SHALL 返回原作品运行
- **AND** 系统 SHALL NOT 创建第二组 TTS、Seedance、ASR 或合成任务

#### Scenario: 输入变化后提交旧计划

- **GIVEN** 模型、主图片、提示词或输出参数在计划生成后发生变化
- **WHEN** 客户端提交旧计划版本
- **THEN** 系统 SHALL 拒绝执行
- **AND** 系统 SHALL 要求重新规划和再次确认

### Requirement: 作品 Worker 必须按锁定配置调用真实生成服务

系统 SHALL 在显式启用真实模式后，按作品运行锁定的模型 ID、模型版本、协议和能力快照分别执行 Seedance 与 TTS；密钥 SHALL 只在 Worker 内存中使用，不得进入运行快照、审计、日志或 API 响应。

#### Scenario: 真实模式执行合法作品

- **GIVEN** 作品运行锁定了启用且版本一致的 `volcengine_ark_video` 视频模型和 `volcengine_tts_v3` TTS 模型
- **AND** 真实模式已显式启用且 fake 模式已关闭
- **WHEN** Worker 领取该运行的可执行步骤
- **THEN** Worker SHALL 按步骤类型调用对应真实 provider
- **AND** Worker SHALL NOT 使用 fake 产物完成任何真实步骤

#### Scenario: 模型配置失效

- **GIVEN** 运行锁定的模型不存在、已停用、协议不匹配或版本发生变化
- **WHEN** Worker 执行外部调用预检
- **THEN** 系统 SHALL 阻止外部调用并将步骤转入可诊断的人工处理状态
- **AND** 系统 SHALL NOT 自动改用当前默认模型

### Requirement: Seedance 参考图必须经受控 TOS 暂存

系统 SHALL 在创建 Seedance 任务前校验全部本地参考图，并通过运行锁定的系统 TOS 配置上传为确定性临时对象，生成上游可访问的短期 HTTPS URL；长期凭据和签名查询参数不得持久化。

#### Scenario: 暂存全部参考图后提交

- **GIVEN** 视频步骤引用 1 至 9 张位于自管存储中的合法图片
- **WHEN** Worker 准备创建 Seedance 任务
- **THEN** Worker SHALL 校验图片路径、类型、大小与摘要
- **AND** Worker SHALL 上传或复用确定性 TOS 对象并验证签名 URL 可读取
- **AND** 只有全部图片预检成功后才可提交 Seedance 请求

#### Scenario: 参考图暂存失败

- **GIVEN** 任一参考图缺失、越界、损坏或 TOS 签名 URL 不可读取
- **WHEN** Worker 执行提交前预检
- **THEN** 系统 SHALL 阻止 Seedance 创建请求并记录稳定错误分类
- **AND** 系统 SHALL NOT 用缺图请求或内网 URL 继续提交

#### Scenario: Seedance 1.5 首尾帧输入

- **GIVEN** 作品包含多于 2 个分镜且运行锁定 `doubao-seedance-1-5-pro-251215`
- **WHEN** 系统规划单个受控视频任务
- **THEN** 系统 SHALL 保留全部分镜语义并只选择首、尾两张参考图
- **AND** Worker SHALL 分别发送 `role=first_frame` 与 `role=last_frame`
- **AND** 单任务时长 SHALL 为 `4~12s`

#### Scenario: 图片协议与模型不匹配

- **GIVEN** 图片数量、role 或时长不符合锁定的 Seedance 模型家族
- **WHEN** Worker 执行提交前预检
- **THEN** 系统 SHALL 在 TOS 上传和 Seedance POST 前拒绝执行
- **AND** 系统 SHALL NOT 在 1.5 首尾帧与 2.0 多参考图契约之间自动切换

### Requirement: Seedance 异步任务必须可恢复且不得重复计费提交

系统 SHALL 使用官方创建、查询和取消协议，并把上游 task ID 作为恢复边界；创建请求不得自动重试，Worker 恢复时已有 task ID 只能查询或取消原任务。

#### Scenario: 创建并持久化上游任务

- **GIVEN** 真实视频步骤通过全部预检且尚无 upstream task ID
- **WHEN** Worker 调用 `POST /api/v3/contents/generations/tasks`
- **THEN** 请求 SHALL 使用 `content[]`、`duration`、`ratio`、`resolution` 与显式 `generate_audio`
- **AND** Worker SHALL 在首次查询前持久化响应中的 task ID
- **AND** 系统 SHALL NOT 自动重复该 POST

#### Scenario: Worker 重启后恢复轮询

- **GIVEN** 运行中 attempt 已持久化 upstream task ID
- **WHEN** Worker 重启并重新领取该步骤
- **THEN** Worker SHALL 使用 `GET /api/v3/contents/generations/tasks/{id}` 查询原任务
- **AND** Worker SHALL NOT 创建新的 Seedance 任务

#### Scenario: 创建结果不确定

- **GIVEN** Seedance POST 可能已送达但 Worker 未取得可信 task ID
- **WHEN** 网络中断、响应超时或响应无法确认
- **THEN** 步骤 SHALL 进入 `waiting_manual` 并标记 `unknown_submission`
- **AND** Worker SHALL NOT 自动重试或创建新 attempt

#### Scenario: 创建被上游明确拒绝

- **GIVEN** Seedance POST 返回明确的非 2xx 响应且没有可信 task ID
- **WHEN** Worker 处理 provider 错误
- **THEN** 系统 SHALL 仅持久化脱敏的 provider code、message 与 request ID
- **AND** 系统 SHALL 清理本次已暂存的参考图
- **AND** 系统 SHALL NOT 保存原始响应体、签名 URL 或凭据

#### Scenario: 取消真实视频任务

- **GIVEN** 运行中 attempt 已持久化 task ID 且收到取消请求
- **WHEN** Worker 执行取消
- **THEN** Worker SHALL 对原 task ID 调用官方 DELETE 并持久化脱敏响应
- **AND** 本地 cancelled 状态 SHALL 与上游取消结果一致

### Requirement: 真实生成产物必须校验并进入自管素材库

系统 SHALL 在 Seedance 成功后下载真实视频并校验媒体，在 TTS 成功后保存真实音频，最终由 FFmpeg 消费这些已登记输入生成标准成片；最终作品必须登记为素材并关联运行。

#### Scenario: 下载真实 Seedance 视频

- **GIVEN** Seedance 查询返回 succeeded 和 `content.video_url`
- **WHEN** Worker 获取视频结果
- **THEN** Worker SHALL 限制下载大小并校验视频流、容器和时长
- **AND** Worker SHALL 将文件原子写入自管存储并登记中间视频素材
- **AND** 素材长期 URL SHALL NOT 直接使用短期 provider URL

#### Scenario: 合成并登记最终作品

- **GIVEN** 视频、独立 TTS 和字幕步骤均有合法自管素材
- **WHEN** compose 步骤执行
- **THEN** FFmpeg SHALL 生成 `MP4(H.264) + AAC` 成片
- **AND** 系统 SHALL 幂等登记唯一 `final_video` 素材并写入运行 `result_material_ids`
- **AND** 素材库 SHALL 能加载缩略图并播放该成片

#### Scenario: 真实输入缺失或损坏

- **GIVEN** 任一必需真实输入缺失、损坏或媒体校验不通过
- **WHEN** compose 步骤准备执行
- **THEN** 步骤 SHALL 失败并指出具体输入
- **AND** 系统 SHALL NOT 回退生成 fake 幻灯片或登记伪成品

### Requirement: 真实作品生成必须执行硬性成本闸门

系统 SHALL 在 Worker 启动和每次外部提交前执行真实模式、运行 allowlist、任务数量、时长、TTS 字符数、ASR 数量、并发与自动重试限制；任一限制不满足不得调用收费 provider。

#### Scenario: 执行获批的单作品验证

- **GIVEN** allowlist 只包含获批运行
- **AND** 该运行只有一个不超过 15 秒的视频任务、TTS 不超过 398 字符且没有 ASR 步骤
- **AND** Worker 并发为 1 且外部提交自动重试为 0
- **WHEN** Worker 执行成本预检
- **THEN** 系统 SHALL 允许该运行进入真实 provider 流程
- **AND** 系统 SHALL 记录不含金额和密钥的资源用量审计

#### Scenario: 运行超出批准边界

- **GIVEN** 运行不在 allowlist 或任一任务数、时长、字符、ASR、并发、重试限制超界
- **WHEN** Worker 准备执行外部调用
- **THEN** 系统 SHALL 在调用前阻止执行并记录明确限制项
- **AND** 系统 SHALL NOT 部分提交该运行

#### Scenario: fake 与 real 同时启用

- **GIVEN** fake 模式和 real 模式同时配置为启用
- **WHEN** 作品 Worker 启动
- **THEN** 系统 SHALL 将配置视为错误并拒绝启动作品生成循环
- **AND** 系统 SHALL NOT 猜测应使用哪一种 provider

### Requirement: 受控真实运行必须支持不可变旁白覆盖

系统 SHALL 允许作品计划为派生版本提供可选精简旁白覆盖；覆盖文本必须进入 WorkVersion 和 Run 锁定快照，并同时驱动 TTS 字符计数、TTS 请求与字幕，且不得修改来源脚本或原分镜旁白。

#### Scenario: 创建精简旁白派生版本

- **GIVEN** 来源脚本旁白不适合已确认的 15 秒真实成片
- **WHEN** 操作者明确确认精简旁白和兼容音色并创建新计划
- **THEN** 系统 SHALL 将 trim 后的覆盖文本写入新 WorkVersion 输入快照
- **AND** 资源用量 SHALL 按覆盖文本计算 TTS 字符数
- **AND** 来源脚本及旧 WorkVersion SHALL 保持不变

#### Scenario: 执行覆盖旁白

- **GIVEN** 真实运行锁定了旁白覆盖与兼容中文音色
- **WHEN** Worker 执行 TTS 和字幕步骤
- **THEN** TTS 请求和 SRT SHALL 使用完全相同的覆盖文本
- **AND** Worker SHALL NOT 回退拼接原分镜旁白

#### Scenario: 覆盖旁白无效

- **GIVEN** 计划请求携带空白或超过当前 TTS 模型上限的旁白覆盖
- **WHEN** 系统创建计划
- **THEN** 系统 SHALL 在创建运行前拒绝请求
- **AND** 系统 SHALL NOT 修改脚本或调用任何 provider

### Requirement: 作品必须支持可审计的静音视频派生模式

系统 SHALL 支持 `silent` 声音模式；该模式不得调用 TTS 或 ASR，不得生成字幕，Seedance 必须关闭原声，最终成品必须包含静音 AAC 以保持标准播放兼容性。系统不得通过修改既有失败运行来伪造跳过节点。

#### Scenario: 创建静音派生计划

- **GIVEN** 操作者明确确认跳过 TTS 并生成静音视频
- **WHEN** 系统创建 `silent` 作品计划和运行
- **THEN** 计划 SHALL NOT 要求 TTS 模型、音色或旁白覆盖
- **AND** 资源用量 SHALL 为 `tts_characters=0` 和 `asr_seconds=0`
- **AND** 新运行 SHALL 与既有 TTS 失败运行相互独立

#### Scenario: 构建静音 DAG

- **GIVEN** 已确认 `silent` 运行
- **WHEN** 系统创建运行步骤
- **THEN** TTS、ASR 和字幕步骤 SHALL 为非必需 `blocked`
- **AND** mix SHALL 只依赖全部视频步骤
- **AND** compose SHALL 依赖 mix

#### Scenario: 调用静音 Seedance

- **GIVEN** `silent` 视频步骤通过成本与参考图预检
- **WHEN** Worker 创建 Seedance 任务
- **THEN** 请求 SHALL 显式发送 `generate_audio=false`
- **AND** 系统 SHALL NOT 调用 TTS 或 ASR provider

#### Scenario: 合成静音标准成片

- **GIVEN** `silent` 运行的全部视频步骤成功并已进入自管存储
- **WHEN** compose 步骤执行
- **THEN** FFmpeg SHALL 为成片补充等长静音 AAC
- **AND** 输出 SHALL 为可播放的 `MP4(H.264)+AAC`
- **AND** 系统 SHALL NOT 生成或烧录字幕

### Requirement: 已批准 ProductionPackage 必须复用现有作品计划链路

系统 SHALL 将当前已批准 ProductionPackage 通过类型化 Application Service 转换为现有画面生成和 WorkPlan 输入，并 SHALL 复用既有 Work、WorkVersion、WorkPlan、WorkGenerationRun 和 Worker DAG；ProductionOrchestrator SHALL NOT直接插入作品生成步骤、调用媒体 provider 或建立第二套视频任务。

#### Scenario: 主画面不完整时等待

- **GIVEN** ProductionPackage 已批准但正式 Script 的任一 Scene 缺少有效主画面
- **WHEN** Full Crew 请求创建作品计划
- **THEN** 现有 SceneVisualManifest 校验 SHALL 返回具体 blocker
- **AND** 系统 SHALL NOT创建可确认 WorkPlan、WorkGenerationRun 或 provider 任务

#### Scenario: 从 ProductionPackage 创建 WorkPlan

- **GIVEN** SceneVisualManifest 完整且 input version 有效
- **WHEN** Full Crew 提交当前 ProductionPackage 的 typed plan input
- **THEN** WorkGenerationService SHALL 创建或更新同一 Script 的既有 Work 草稿和 WorkPlan
- **AND** WorkPlan SHALL 保存 ProductionRun、ProductionPackage digest、Script/Scene、主画面和相关产物来源引用
- **AND** 导演、表演和声音方案 SHALL 进入可见 Prompt、时间线或声音建议快照
- **AND** 系统 SHALL NOT自动确认计划

#### Scenario: ProductionPackage 变化使旧计划失效

- **GIVEN** WorkPlan 引用了某个已批准 ProductionPackage digest
- **WHEN** 当前 ProductionPackage、Script、SceneVisualManifest、Prompt、主画面、模型、音色、声音模式、字幕、时间线或输出参数发生变化
- **THEN** 旧 WorkPlan SHALL 失效且不得确认
- **AND** 系统 SHALL 基于新输入创建或更新合法计划修订

#### Scenario: 操作者修改 Full Crew 下游方案

- **GIVEN** WorkPlan 已从已批准 ProductionPackage 创建
- **WHEN** 操作者修改 Prompt、模型、音色、声音模式、字幕、时间线或输出参数
- **THEN** 系统 SHALL 保存相对 ProductionPackage 的显式 override diff
- **AND** 全部修改 SHALL 进入 WorkVersion 快照和 WorkPlan input fingerprint
- **AND** 系统 SHALL NOT回写 ProductionPackage 或把人工修改伪装成原 Gate 已批准内容
- **AND** 旧计划 SHALL 失效并要求重新规划、展示资源和确认

### Requirement: Full Crew 作品运行必须继续人工确认非金额资源

Full Crew 创建的 WorkPlan SHALL 继续展示模型、音色、时长、比例、分辨率、声音模式、字幕配置、视频任务数、视频总秒数、TTS 字符数和 ASR 时长；只有操作者通过现有幂等确认接口后，系统才 SHALL 创建 WorkGenerationRun，并 SHALL NOT计算或展示金额。

#### Scenario: 确认 Full Crew 作品计划

- **GIVEN** WorkPlan 当前有效且全部能力和输入校验通过
- **WHEN** 操作者查看并确认模型、参数和非金额资源用量
- **THEN** 现有确认接口 SHALL 幂等创建一个 WorkGenerationRun
- **AND** ProductionRun SHALL 保存正式 run ID 并进入外部等待状态
- **AND** ProductionOrchestrator SHALL NOT绕过该确认

#### Scenario: 相同确认重复提交

- **GIVEN** 相同 WorkPlan 和 Idempotency-Key 已创建 WorkGenerationRun
- **WHEN** Full Crew resume 或客户端重试确认
- **THEN** 系统 SHALL 返回原 WorkGenerationRun
- **AND** 系统 SHALL NOT创建第二组视频、TTS、ASR 或合成任务

#### Scenario: 资源限制不满足

- **WHEN** WorkPlan 超出视频任务数、总时长、TTS 字符、ASR 数量、并发或重试限制
- **THEN** 系统 SHALL 在创建 WorkGenerationRun 前阻断
- **AND** 系统 SHALL 返回具体非金额限制项
- **AND** 系统 SHALL NOT通过缩短输入、切换模型或部分提交继续

### Requirement: Full Crew QC 返工必须遵守作品版本治理

Full Crew QualityGate 产生的局部或全局返工 SHALL 通过现有 Work Library 版本治理从被评审 WorkVersion 派生 `edit` 或 `full_regeneration` 草稿、差异计划和新的人工确认；系统 SHALL 保留原 WorkVersion、WorkGenerationRun、成功媒体和 QC 证据，不得原地覆盖或自动再次调用 provider。

#### Scenario: 局部返工派生 edit 版本

- **GIVEN** QC 只拒绝部分可独立重生成的 take
- **WHEN** 操作者接受局部返工建议
- **THEN** 系统 SHALL 创建或复用来源 WorkVersion 对应的 `edit` 草稿
- **AND** 差异计划 SHALL 标明受影响任务、可复用素材和非金额资源用量
- **AND** 只有再次人工确认后系统才 SHALL 创建新运行

#### Scenario: 全局返工派生 full regeneration 版本

- **GIVEN** QC 问题影响全局视觉、比例、分辨率、完整叙事或全部媒体
- **WHEN** 操作者接受整体返工建议
- **THEN** 系统 SHALL 创建或复用 `full_regeneration` 草稿和完整差异计划
- **AND** 原完成版本及其媒体、运行和审计 SHALL 保持不变

#### Scenario: QC 不通过不得伪装作品生成失败或成功批准

- **GIVEN** WorkGenerationRun 已技术成功并登记 final media，但 Full Crew QC 未通过
- **WHEN** 系统展示作品和 ProductionRun 状态
- **THEN** WorkGenerationRun SHALL 保持真实技术终态
- **AND** ProductionRun SHALL 显示质量未批准或等待返工
- **AND** 系统 SHALL NOT把技术成功等同于 Full Crew 质量批准

### Requirement: WorkGenerationRun 技术终态必须真实传播到 Full Crew

Full Crew SHALL 只通过既有 WorkGeneration Application Service 查询、重试或取消作品运行，并 SHALL 保留其真实 `queued/running/succeeded/failed/waiting_manual/cancelling/cancelled` 技术状态。ProductionRun 只有在作品运行 succeeded、final media 已登记且 required take inventory 完整后才能进入 Editor/QC；其他终态 SHALL 映射为明确等待、阻断、注意或取消状态。

#### Scenario: 作品运行失败

- **WHEN** WorkGenerationRun 进入 `failed`
- **THEN** ProductionRun SHALL 保存原 run ID、失败分类和可重试性并停止推进
- **AND** ProductionOrchestrator SHALL NOT自动重试、创建第二个运行或执行 Editor/QC

#### Scenario: 作品运行需要人工处理

- **WHEN** WorkGenerationRun 因上游提交结果不确定进入 `waiting_manual`
- **THEN** ProductionRun SHALL 进入 `attention_required`
- **AND** resume SHALL NOT重复提交 provider 请求

#### Scenario: 作品运行成功但成片证据不完整

- **GIVEN** WorkGenerationRun 状态为 `succeeded`
- **WHEN** final media、compose 消费关系或 take inventory 任一缺失
- **THEN** ProductionRun SHALL 保持 evidence blocker
- **AND** 系统 SHALL NOT把技术成功等同于可进行质量评审

#### Scenario: Full Crew 请求取消作品运行

- **GIVEN** ProductionRun 已保存 cancellation intent 且 WorkGenerationRun 仍在执行
- **WHEN** Orchestrator 调用既有取消端口
- **THEN** WorkGenerationRun SHALL 按原有 provider 取消协议进入真实 `cancelling/cancelled/waiting_manual` 状态
- **AND** ProductionRun SHALL 在结果确定前保持 `cancelling` 或 `attention_required`
- **AND** 系统 SHALL NOT直接更新作品运行为 cancelled

### Requirement: Full Crew 作品幂等必须校验请求内容

Full Crew 使用现有作品确认、重试和取消接口时，幂等记录 SHALL 同时绑定命令作用域、WorkPlan/Run ID 和 canonical request digest；同 key 同 digest SHALL 返回原结果，同 key 不同 digest SHALL 返回冲突，防止把旧作品运行误绑定到变化后的计划。

#### Scenario: 相同 key 确认不同计划修订

- **GIVEN** Idempotency-Key 已为某 WorkPlan revision 创建 WorkGenerationRun
- **WHEN** 客户端以相同 key 确认新的 plan ID、plan version 或 input fingerprint
- **THEN** 系统 SHALL 返回 `idempotency_conflict`
- **AND** 系统 SHALL NOT返回旧运行作为新计划的执行结果

#### Scenario: 相同 key 和相同计划重放

- **WHEN** 客户端以相同 key、plan ID、plan version 和 request digest 重放确认
- **THEN** 系统 SHALL 返回原 WorkGenerationRun
- **AND** 系统 SHALL NOT创建第二组外部任务
