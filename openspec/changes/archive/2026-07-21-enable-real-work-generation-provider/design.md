## Context

作品生成领域、运行 DAG、任务管理和 fake 成品登记已经存在，但 Worker 目前统一使用 `FakeWorkProvider`，不会读取运行锁定的视频/TTS 模型，也不会调用外部生成服务。真实执行涉及数据库配置解析、本地参考图向公网可读 TOS 的暂存、异步 Seedance 任务恢复、TTS、媒体下载与 FFmpeg 合成，且视频生成会产生外部费用。

原 fake 运行 `9fd81164-d187-4690-a199-454265b66e0a`、失败的 TTS 派生运行 `d2eea1d4-f418-45e2-8e05-7ab02dc11eaa`、误用 Seedance 1.5 多参考图契约的失败静音运行 `3cc84910-74ed-4549-b64e-599256f75f42` 和未经单独模型切换确认的 Seedance 2.0 失败运行 `153e171a-2a6d-42a3-8ebe-92df2bf5454e` 均作为历史证据保留，不覆盖状态。最终受控验证继续使用用户配置的 `doubao-seedance-1-5-pro-251215`，从同一作品创建新的 `silent` 派生版本和运行：一个 `12s`、`16:9`、`1080p` 任务，以第 1 张为 `first_frame`、第 6 张为 `last_frame`，不调用 TTS/ASR，不生成字幕，并发 1，自动提交重试 0。

## Goals / Non-Goals

**Goals:**

- 按作品运行锁定的模型与能力快照执行真实 Seedance 和 TTS。
- 让本地参考图通过当前系统 TOS 工具形成外部 provider 可读取的短期 HTTPS URL。
- 在 Worker 重启、轮询超时和取消场景下保持上游任务唯一且可恢复。
- 下载、校验、自管存储并登记真实最终作品，不依赖短期 provider URL 播放。
- 以运行级硬限制和显式开关控制真实调用范围与费用。

**Non-Goals:**

- 本次不执行 ASR，不扩展 Seedance 参考视频/参考音频输入。
- 不实现批量真实运行、并行视频生成或自动重新提交视频任务。
- 不改动作品生成页面与已确认原型。
- 不替换声音与字幕模块既有独立任务 API。

## Decisions

### 1. 使用步骤类型路由，而不是一个通用真实 provider

Worker 根据 `step_type` 路由到 `video_segment`、`tts`、`subtitle`、`compose` 执行器。`video_segment` 使用 Seedance provider；`tts` 复用现有语音模型解析与 Volcengine TTS provider；字幕由 TTS 时间信息或本地确定性时间轴生成；`compose` 只消费已成功步骤的自管素材。

这样可以保持每种外部协议的请求、状态和错误语义独立。备选的“让一个 provider 模拟所有步骤”无法正确表达异步视频任务与同步 TTS 的差异，予以拒绝。

### 2. 模型和 TOS 配置按运行锁定 ID/版本解析

Worker 从步骤所属运行读取 `model_snapshot`、`parameter_snapshot`、`prompt_snapshot`、`timeline_snapshot` 与版本输入快照，再通过 `PostgresModelRegistry` 解析对应模型。真实执行要求模型仍存在、启用、协议匹配、版本与锁定快照一致；凭据只存在于内存运行配置，日志和数据库审计只记录脱敏字段。

TOS 使用任务锁定配置；若旧运行未锁定 TOS 配置，则在首次外部提交前原子锁定当前 enabled/current 配置 ID 与版本，后续恢复不得切换。

### 3. 参考图采用确定性 TOS 对象并在提交前完整预检

本地 `/assets/...` URL 必须解析到 `ASSET_STORAGE_ROOT` 下的常规文件，校验 MIME、大小、摘要后，以 `project/run/step/digest` 组成确定性对象键上传。全部图片上传并生成 HTTPS 短期 URL 后，Worker 对 URL 做读取预检，再允许创建 Seedance 任务。

重复执行 staging 可复用相同对象键；运行达到终态后进入可重试清理队列。备选的直接把内网 URL 传给 Seedance 无法由上游稳定访问，予以拒绝。

### 4. Seedance 创建与查询分离，task ID 是恢复边界

创建请求使用 `POST /api/v3/contents/generations/tasks`，正文使用 `content[]`，文本项为 `type=text`，参数为 `duration`、`ratio`、`resolution` 和显式 `generate_audio`。Worker 必须按模型家族构造图片项：Seedance 1.5 单图使用 `first_frame`，双图依次使用 `first_frame/last_frame`，时长限制为 `4~12s`；Seedance 2.0 的 `1~9` 张多参考图使用 `reference_image`，时长限制为 `4~15s`。模型、图片数量、role 或时长不匹配时必须在 TOS 上传和 POST 前拒绝。HTTP 客户端只允许连接模型配置的 HTTPS endpoint，设置有限超时，不自动重试 POST。

作品计划的引用模式进入能力快照。`first_last_frames` 模式按完整分镜生成一个语义连续的提示词，但每段只选择首尾两张参考图，不能因“最多 2 图”把 6 个分镜错误拆成 3 个收费任务；`multi_reference` 模式才按模型参考图上限分组。

成功响应中的 task ID 在任何查询前持久化到 attempt。恢复时存在 task ID 则只执行 GET；不存在 task ID 且上次提交结果不确定则标记 `waiting_manual/unknown_submission`。只有明确的提交前失败允许维持 queued，真实模式仍不自动创建第二次 attempt。

### 5. provider 输出必须进入自管素材

Seedance 成功后从 `content.video_url` 下载到临时文件，限制响应大小并用 `ffprobe` 校验视频流、时长与容器，再原子移动到 `ASSET_STORAGE_ROOT/generated/artifacts/<project_id>/`，登记 `video` 素材并把 ID 写入步骤 `result_material_ids`。短期上游 URL只记录脱敏元数据，不作为素材长期 URL。

TTS 输出同样登记为中间音频素材。`compose` 从依赖步骤素材读取真实文件，通过 FFmpeg 生成 `MP4(H.264)+AAC`，再登记唯一 `final_video`；缺失、损坏或时长异常必须失败，禁止回退 fake 幻灯片。

### 6. 成本控制在配置和运行数据两层强制

真实模式必须同时满足 `WORK_GENERATION_REAL_PROVIDER_ENABLED=true` 与 `WORK_GENERATION_FAKE_PROVIDER_ENABLED=false`。Worker 启动和每次提交前都校验 allowlist 运行 ID、视频任务数 `<=1`、单段时长 `<=15`、TTS 字符数 `<=398`、ASR 步骤数 `0`、并发 `1`、自动提交重试 `0`。任一条件不满足立即进入人工处理，不调用外部 provider。

fake 与 real 开关同时开启属于配置错误，Worker 拒绝启动作品生成循环。轮询 GET 可按固定间隔持续执行；POST、TTS 合成和真实视频下载不自动重试。

### 7. 精简旁白作为派生版本覆盖

计划请求可携带可选 `narration_override`。服务端对文本做 trim、非空和字符上限校验，将其保存到 WorkVersion 输入快照；资源用量、TTS 请求和 SRT 均使用该锁定文本。覆盖不写回 `scripts` 或原分镜旁白，不影响素材画面、视频提示词和参考图。

本次覆盖文本与中文音色均由用户明确确认。备选的直接修改原脚本会污染上游创作数据，予以拒绝；在 Worker 环境变量中注入旁白会破坏运行可审计性，同样拒绝。

### 8. 静音视频使用独立派生运行

`AudioMode::Silent` 不选择 TTS 模型或音色，资源用量固定为 `tts_characters=0/asr_seconds=0`。DAG 保留 TTS、ASR、字幕记录用于统一任务展示，但三者均为非必需 `blocked`；mix 只依赖全部视频段，compose 依赖 mix。

Seedance 请求显式使用 `generate_audio=false`。真实 compose 只消费视频段，通过 FFmpeg 加入与成片等长的静音 AAC 并输出 `MP4(H.264)+AAC`，不生成或烧录字幕。失败的 TTS 运行保持 `waiting_manual`，禁止通过改写原运行跳过失败节点。

## Risks / Trade-offs

- [Seedance 创建响应丢失但任务已创建] → 不自动重提，标记 `unknown_submission` 并保留脱敏请求摘要供人工核查。
- [TOS 签名 URL 在长队列中到期] → 只在即将提交时签名，若 task ID 已存在则无需重新提交参考图。
- [上游 URL 过期或返回非视频] → 成功查询后立即下载，做大小、类型和 `ffprobe` 校验；失败保留上游 task ID 供人工恢复下载。
- [旧运行缺少真实执行所需快照] → 外部提交前做完整预检并明确失败，不推断或静默替换模型/素材。
- [TTS 同步请求结果不确定] → 不自动重复调用；保存请求摘要并进入人工处理。
- [现有 fake 测试受影响] → fake 模式保留并显式注入，真实 provider 使用 HTTP/TOS mock 覆盖。

## Migration Plan

1. 先增加数据字段与 Worker 单元/集成测试，保持真实开关关闭。
2. 部署 Worker，使用 fake 模式完成既有回归。
3. 对目标运行执行只读成本预检、TOS 上传/签名读取预检和模型版本预检。
4. 关闭 fake、启用 real，并通过 allowlist 只放行目标运行；单 Worker 并发 1。
5. 启动唯一一次真实运行，持续轮询同一 task ID，下载并合成登记成品。
6. 验证素材库可播放后关闭目标 allowlist 或真实执行开关。

回滚时关闭 `WORK_GENERATION_REAL_PROVIDER_ENABLED` 并保留运行、attempt 与 upstream task ID；不得删除上游状态或以 fake 结果覆盖真实运行。若任务已提交，恢复工具只能继续查询/取消该 task ID。

## Open Questions

无。真实调用范围、成本边界和目标运行已由用户确认。
