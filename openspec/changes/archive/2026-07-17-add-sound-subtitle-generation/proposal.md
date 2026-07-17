# 声音与字幕生成 Proposal

## Why

首版作品需要由 Agent 生成 TTS 配音和可对齐字幕，但当前没有独立的声音生产入口，也没有可更新的音色目录。需要建设可单独使用、又能被作品编排复用的声音与字幕生成能力。

## What Changes

- 新增 `素材管理 / 声音与字幕生成`，首版只提供 `TTS配音 / 字幕` 两个标签和可见声音 Agent 对话。
- 工作台导航使用数据库菜单的 `route_path` 作为 URL，刷新、浏览器前进和后退时恢复当前一级/二级页面，不再回到默认页。
- 在统一 `ai_models` 中新增 `speech` 模型类型，Admin 支持新增、编辑、启停、默认切换和删除语音模型；现有 `text/image/video` 数据、API 和管理流程保持兼容。
- 首版语音协议包括 `volcengine_tts_v3`、`openai_audio_speech` 与 `volcengine_asr_v3`；原生 TTS、中转 TTS 和 ASR 使用独立模型记录、协议、资源标识和不可变运行快照。
- 首版 TTS 使用 `doubao-seed-tts-2.0`、资源 `seed-tts-2.0` 和 HTTP Chunked V3 单向流式接口。
- 通过 `ListSpeakers 2025-05-20` 按 `ResourceID` 分页同步动态音色目录，禁止前端或代码写死。
- 原生 TTS 模型支持“官方同步”和“复用已有目录”两种目录来源；OpenAI Audio Speech 中转模型必须在 `upstream_model` 与 `resource_id` 均与官方 `volcengine_tts_v3` 目录模型一致时复用该目录，无需重复配置 OpenAPI AK/SK。
- OpenAI Audio Speech 中转使用 Bearer API Key 和 `/v1/audio/speech`，只生成音频；其响应没有可信字词时间戳时必须阻止同步字幕，不得伪造时间轴或自动追加 ASR 调用。
- 音色目录同步使用独立的 `VOICE_CATALOG_WORKER_ENABLED`，不得依赖或隐式开启可能执行 TTS/ASR 的语音生成 Worker。
- 展示模型真实支持的音色、语言/口音、情绪风格和可调参数；Agent 可推荐，用户必须确认。
- 旁白支持从当前账号“脚本创作”中的草稿或已通过脚本导入，按分镜选择并保留不可变来源快照；归档脚本不进入可选范围。
- TTS 使用当前官方响应中的 `sentence.words` 字词时间戳，字幕 Agent 负责断句和样式，供应商时间戳负责对齐。
- 已有音频可通过 `doubao-seed-asr-2.0` 和资源 `volc.seedasr.auc` 生成字幕；不支持时间戳时不得伪造。
- 本地音频调用 ASR 前通过系统公用的私有 TOS 工具中转，只向 ASR 提交短期签名 URL；TOS 在 Admin“工具与 MCP”中独立配置并供全部 ASR 模型共用，任务终态后清理对象，TOS 不替代素材库自管存储。
- 试听、生成、重试前展示字符数/音频时长和任务数量，不计算金额。
- TTS 和字幕结果自动进入素材库，重新生成不覆盖旧素材。
- 失败任务保存并展示完整的脱敏结构化诊断，包括 HTTP 状态、供应商错误码/消息、完整请求追踪 ID、模型协议和尝试次数；不得保存或返回原始响应头/响应体。
- 左栏任务数量超过可见空间时只滚动任务列表，任务卡不得被压缩变形，筛选栏与项目并发状态保持固定。
- 声音与字幕桌面工作台采用可读字号层级：正文不低于 `12px`、辅助信息不低于 `11px`，任务标题与失败标题为 `13px`，旁白编辑正文为 `14px`，不得为维持高密度布局回退到 `7~10px` 文本。

## Capabilities

### New Capabilities

- `sound-subtitle-generation`: 定义声音 Agent、动态音色目录、TTS、试听、字幕对齐、ASR 和素材入库。

### Modified Capabilities

- `ai-model-management`: 扩展统一模型注册表、管理 API 和 Admin 表单以支持语音模型及豆包 TTS/ASR 协议，不改变现有模型类型行为。

## Impact

- 数据：新增语音模型约束、显式音色目录来源关系、版本化系统 TOS 工具配置、音色目录同步、声音任务、字幕时间轴、脚本来源快照和模型/音色/工具配置快照引用。
- Admin：新增语音模型管理及独立的“工具与 MCP / 私有 TOS”配置页，并提供音色主动/定期同步和状态可观测。
- 后端/Worker：接入 TTS V3、ASR、流式落盘和字幕生成。
- 前端：新增双标签声音工作区和动态声音参数选择，并补齐数据库菜单驱动的工作台路由持久化。
- 依赖：生成结果通过 `extend-material-library-for-work-production` 的素材契约入库。
- 依赖：TOS 临时中转使用火山引擎官方 Python SDK，真实上传、ASR 和清理验证仍需单独许可。

## Non-Goals

- 不生成 AI 音乐/BGM、环境音或动作音效，也不展示空入口。
- 不负责 Seedance、作品时间轴混音或最终成片合成。
- 不由 LLM 猜测音色或模型支持的声音风格。
- 不为 `openai_audio_speech` 音频伪造 TTS 字词时间戳，也不自动发起二次 ASR。
- 不维护价格、币种、金额或费用上限。
- 未经单独许可，不执行真实 TTS、ASR 或 TOS 外部调用验证。
