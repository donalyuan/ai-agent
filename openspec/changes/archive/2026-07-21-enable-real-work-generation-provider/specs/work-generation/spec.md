## ADDED Requirements

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
