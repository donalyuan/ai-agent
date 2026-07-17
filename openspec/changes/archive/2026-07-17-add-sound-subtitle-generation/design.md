# 声音与字幕生成 Design

## Context

作品生产需要配音和字幕，但这两类产物也应能在素材阶段独立生成、试听和复用。豆包音色目录会持续更新，若在前端或 migration 中写死，新增音色无法出现、下线音色也无法正确保护历史作品。

## Goals / Non-Goals

**Goals:**

- 建立声音 Agent 驱动但由用户确认的 TTS/字幕生产入口。
- 动态同步并版本化音色、语言、风格和参数能力。
- 以真实 TTS 时间戳或 ASR 输出生成字幕，不伪造对齐。
- 产物可入素材库并被作品生成复用。

**Non-Goals:**

- 不生成 AI 音乐、环境音和动作音效。
- 不处理 Seedance 原声混音或成片字幕烧录。
- 不让 Agent 自动试听、自动切换模型或跳过人工确认。

## DDD

`VoiceCatalog` 是模型版本下的动态能力目录，包含同步批次、`ResourceID`、音色条目、语言/口音、供应商原始情绪字段、参数约束、可用状态和同步时间。原始情绪字段只有在当前协议存在官方请求映射时才可作为生成能力；目录条目下线只影响新生成，历史 `VoiceSnapshot` 永久保留，其中情绪字段保持 nullable 以审计历史数据。

`VoiceCatalogWorker` 是控制面目录同步执行器，与 `SpeechGenerationWorker` 分离。前者只消费 `voice_catalog_syncs`，调用 `ListSpeakers` 并提交完整目录；后者只消费音频检查、TTS、ASR 和 TOS 临时对象清理。开启目录同步不得使任何 TTS/ASR 任务进入运行态。

`SoundSubtitleGenerationTask` 表示独立声音/字幕任务，类型为 `tts_preview`、`tts`、`asr`、`subtitle`。每次执行保存模型、音色、文本、参数、资源用量和请求追踪快照。生成文件继续由 `Material` 管理。

`SoundTaskErrorDetails` 是失败任务的安全诊断值对象。它只允许保存 `http_status`、`provider_error_code` 和已脱敏的 `provider_error_message`；响应追踪继续使用独立的 `upstream_log_id`。原始响应头、原始响应体、请求头、API Key、Authorization、签名和 URL 查询参数不得进入该对象。历史任务未采集到这些字段时保持空对象，不从错误摘要反向猜测。

`SoundScriptSourceSnapshot` 是 TTS 任务可选的不可变来源审计。工作台只提交当前项目脚本 ID、用户打开脚本时看到的 `updated_at` 和所选分镜 ID；应用服务重新读取脚本，校验其仍属于当前项目、状态为 `draft | approved`、版本未变化，并按 `sequence` 保存脚本标题、版本时间及所选分镜的原始旁白。最终 `text_content` 允许在导入后继续编辑，因此执行文本与原始来源快照分别保存。归档、跨项目、已变化、空分镜或不属于该脚本的分镜均不得形成来源快照。

`AudioMaterialInspection` 是已有音频进入 ASR 前的不可计费检查记录。Worker 必须使用 `ffprobe` 从自管素材文件读取真实格式、时长、大小和 SHA-256；确认资源用量与 ASR 执行都引用同一检查快照。源文件摘要变化、检查过期或探测失败时必须重新检查并阻止 ASR，不得信任前端或素材 metadata 中的时长。

`SpeechModel` 继续使用统一 `ai_models` 聚合，不另建旁路模型表。`model_type=speech` 下，`volcengine_tts_v3`、`openai_audio_speech` 与 `volcengine_asr_v3` 是三个独立协议和模型记录；生命周期、乐观锁、凭据脱敏和历史快照沿用现有模型管理规则。现有 `text/image/video` 继续按 `model_type` 各有一个默认模型，`speech` 则按 `api_protocol` 分别维护默认模型，避免不同执行协议互相替换。原生 TTS 的 `X-Api-Key` 与 `ListSpeakers` 所需的 OpenAPI Access Key/Secret Key 是不同凭据；OpenAI Audio Speech 中转使用 Bearer API Key。目录凭据使用独立字段保存且不得进入通用 `settings`、API 明文响应或运行快照。

`VoiceCatalogBinding` 定义 TTS 模型的目录来源。`official_sync` 模式只由 `volcengine_tts_v3` 模型持有 OpenAPI AK/SK 并拥有 `voice_catalog_syncs/voice_catalog_entries`；`shared` 模式不保存 AK/SK，通过 `voice_catalog_source_model_id` 指向一个 `official_sync` 根模型。原生共享关系要求 `api_protocol`、`upstream_model` 和 `settings.resource_id` 三项完全一致；`openai_audio_speech` 必须共享一个 `volcengine_tts_v3` 根模型，并要求 `upstream_model + settings.resource_id` 一致。所有共享关系禁止自引用和共享链。工作台目录读取、检查更新、声音 Agent 与 TTS 预检均先解析实际目录来源；任务的 `VoiceSnapshot` 同时保存执行模型与目录来源模型，避免历史任务无法解释音色来源。

目录来源是被共享模型的受保护依赖。仍有共享模型引用时，系统阻止来源模型停用、删除、改为共享模式，或修改 `api_protocol/upstream_model/resource_id`；普通显示信息、运行凭据和目录凭据仍可按乐观锁更新。共享模型自身停用、删除或解除绑定不影响来源目录。该规则将配置错误暴露在 Admin 操作时，而不是等工作台或 Worker 静默读取错误目录。

`SystemTosStagingConfig` 是 Admin“工具与 MCP”边界内的系统工具配置聚合，独立于 `SpeechModel`。系统同一时刻只有一个当前版本，当前版本可启用或停用；每次修改位置、凭据或限制都创建新的不可变配置版本，旧版本继续保留给已创建任务和清理重试。ASR 任务创建时必须锁定 `tos_staging_config_id` 与 `tos_staging_config_version`，Worker 和清理器只允许按该引用读取配置，禁止从当前模型或最新系统配置动态替换。待清理对象属于系统 TOS 工具的运行状态，只约束 TOS 配置变更和停用，不约束任何模型的新增、编辑、默认切换、停用或删除。

`WorkspaceRoute` 是工作台当前一级/二级模块的可分享导航标识。它不建立前端硬编码路由表，而是直接使用 `video_workspace_menus.route_path`；父菜单负责一级归属，子菜单负责具体页面。URL 与选中菜单必须双向同步，项目、表单和任务等业务状态不进入本轮路由契约。

声音 Agent 复用统一 `agent_conversations/agent_messages/agent_runs/agent_steps` Runtime，通过 `sound` adapter 生成文本建议、断句和声音推荐；模型调用和文件生成仍是显式工具步骤。

## BDD

操作者进入 `声音与字幕生成`，在上方选择 TTS 模型和动态音色，查看语言、协议真实支持的参数和目录更新时间。TTS 模型使用与工作台一致的自定义单选弹层，展开层位于触发框正下方并保持同宽，不使用无法稳定控制边界和行高的浏览器原生选项层。Agent 可根据旁白推荐，但选择变化不会自动调用接口；只有点击试听或生成并确认资源用量后才执行。

管理员配置 TTS 模型时明确选择“官方同步”或“复用已有目录”。官方同步显示并要求 OpenAPI AK/SK；复用模式隐藏 AK/SK，要求选择一个与当前协议、上游模型标识和资源 ID 完全一致的官方目录模型。Admin 在列表和编辑表单中展示实际目录来源；共享模型点击“同步音色”或工作台点击“检查更新”时，系统对实际来源模型创建或复用同步任务。

管理员配置 OpenAI Audio Speech 中转时只能复用一个匹配的官方 Seed TTS 根目录，填写 `/v1` 请求根地址与 Bearer API Key。该协议的工作台可以试听和生成配音；当操作者进入 TTS 字词时间戳字幕来源时，页面明确显示当前模型不支持并阻止提交，仍可改用已有音频 ASR，但系统不得静默切换或自动追加 ASR。

Admin 的目录来源候选属于模型关系配置，不属于当前模型列表视图。添加或编辑 TTS 模型时，抽屉独立加载全部启用的语音模型，再按官方 `volcengine_tts_v3` 根目录及 `upstream_model + resource_id` 精确筛选；当前列表页的模型类型、状态、供应商、协议和搜索词不得影响候选。独立请求进行中、失败和成功但无匹配模型必须显示不同状态，失败时提供重试且不得误报为无匹配模型。

操作者可在旁白区打开“导入脚本”，搜索当前账号的草稿和已通过脚本，单选一个脚本后默认选中全部非空分镜，也可取消部分分镜。确认导入时按分镜 `sequence` 合并旁白；当前旁白非空时明确替换而非追加。导入不调用 TTS 且不产生模型资源用量。若所选文本超过当前 TTS 模型字符上限，工作台必须阻止导入，不得截断。

桌面端页面按已确认 Pencil 宽屏原型实现：页头承担路径、标题、双标签与新建任务；1440px 基准下页头和主工作区宽 `1118px`，主工作区为 `250px / 520px / 276px` 三栏，栏间距 `16px`。1920px 下页头和主工作区宽 `1598px`，三栏为 `250px / 1000px / 276px`，工作台可用内容区左右各保留约 `24px`，不得继续将旧 `1118px` 工作区居中后留下大块空白。任务列表只能出现在左栏，中栏完整承载目录状态、生成配置、试听和当前任务状态，右栏承载声音 Agent 会话、输入及运行审计；不得再在工作区下方追加一份大任务表。

失败任务按已确认 Pencil `b3HsYi` 展示完整脱敏诊断。中栏失败态展开供应商消息、HTTP 状态、内部/供应商错误码、模型协议、尝试次数、完整 `request_id`、完整 `X-Tt-Logid` 和完成时间；缺失的历史字段明确显示 `-`，不得截断 ID 或伪造上游值。左栏筛选和并发状态固定，只有任务卡列表纵向滚动；每张任务卡按内容固有高度排列且禁止横向滚动，连续失败任务不得压缩标题、错误消息、时间或重试按钮。

桌面工作台按已确认 Pencil `wl2QV` 使用可读字号层级。常规正文不低于 `12px`，时间、状态、标签、会话信息等辅助信息不低于 `11px`；任务标题、表单值、主要按钮和失败标题使用 `13px`，旁白编辑正文使用 `14px`，失败供应商消息使用 `12px`，失败诊断字段使用 `11px`。字号提升不得改变已确认的三栏宽度、宽屏中栏结构、`140px` 宽屏失败详情或左栏独立滚动边界。

操作者从任一菜单进入声音与字幕或其他已启用模块时，地址栏更新为该菜单的 `route_path`。刷新页面或使用浏览器前进、后退后，工作台恢复 URL 对应的一级和二级菜单；未知、隐藏或禁用路径回到首个可用菜单。

TTS 成功时同时获得音频和支持语种的字词时间戳，字幕 Agent 基于真实时间戳断句并生成 SRT。对于已有音频，操作者可明确选择 ASR 生成字幕。任何不支持时间戳的语种不得返回伪成功同步字幕。

## SDD

### 音色目录同步

使用 `Action=ListSpeakers&Version=2025-05-20`，请求体按官方字段提交 `ResourceIDs`、`Page` 和 `Limit`，通过 `speech_saas_prod` OpenAPI HMAC-SHA256 鉴权分页获取完整目录。本地以同步批次暂存，只有全部分页成功才原子切换为最新完整目录。供应商可选集合字段 `Categories`、`NormalLabels`、`SpecialLabels`、`Languages`、`Emotions` 返回 `null` 或非数组时统一归一化为 `[]`，不得把 `null` 写入非空数组列。新条目自动可选；成功完整同步后消失的条目标记 unavailable，不删除。

`ai_models.voice_catalog_source_model_id` 为 `NULL` 表示 `official_sync`，仅允许 `volcengine_tts_v3` 且模型必须同时持有目录 AK/SK；非空表示 `shared`，且当前模型目录 AK/SK 必须为空。`openai_audio_speech` 必须使用 `shared`。API 请求使用 `voice_catalog_mode=official_sync|shared` 和可空 `voice_catalog_source_model_id`，响应额外返回模式、来源模型 ID 与显示名。Repository 在同一事务内锁定来源与引用模型，验证来源是启用且未删除的 `volcengine_tts_v3` 官方目录根模型；原生共享比较 `api_protocol/upstream_model/settings.resource_id`，OpenAI 中转比较 `upstream_model/settings.resource_id`。数据库外键使用 `ON DELETE RESTRICT`，应用层在更新、停用和删除前返回稳定业务错误，不向 Admin 暴露原始 FK/SQL 错误。

目录查询响应中的 `model_id` 保持为操作者选择的执行模型，同时增加 `source_model_id`；`model_settings` 继续来自执行模型，音色条目和最近同步状态来自来源模型。同步请求统一解析到 `source_model_id`，因此共享模型不会创建重复同步批次，定期调度也只扫描拥有 AK/SK 的官方目录模型。TTS 预检必须再次校验执行模型与来源的绑定仍有效，并在 `voice_snapshot` 保存 `catalog_source_model_id`。

工作台读取缓存目录，不为每次页面加载直接调用供应商。Admin 支持主动同步和定期同步，工作台展示更新时间并可触发检查更新。失败同步保留上次成功目录；已分类的目录错误可保存脱敏中文摘要，未知异常只保存异常类型，不得保存数据库失败行、供应商 URL、签名参数或凭据。自动调度按最近一次成功或失败终态计算间隔，失败后不得在每个轮询周期立即创建新任务；操作者明确点击检查更新仍可在没有活动任务时立即重试。目录任务由独立且默认关闭的 `VOICE_CATALOG_WORKER_ENABLED` 控制，并通过 `/health.voice_catalog_worker` 暴露运行状态；轮询周期使用 `VOICE_CATALOG_POLL_SECONDS`。`SPEECH_GENERATION_WORKER_ENABLED=false` 时目录同步仍可执行，语音生成 Worker 不再顺带消费目录任务。

工作台音色选择使用可搜索单选组合框。触发器展示供应商原名和截断后的中文 `Description`；展开项分层展示供应商原名、完整中文描述，以及中文性别、年龄和语言标签。搜索覆盖原名、描述、性别、年龄、语言、目录普通/特殊标签和分类。供应商未提供官方中文音色名时不得由前端生成或维护伪造译名。

音色弹层使用两排可取消的 Tag 筛选，不使用树或嵌套分组：语言为 `中文 / 英文 / 多语言`，声线为 `男声 / 女声`。两排选择与搜索条件取交集，结果仍是扁平单选列表；再次点击已选 Tag 清除该维筛选。`中文` 只包含语言代码均为中文的音色，`英文` 只包含语言代码均为英文的音色，`多语言` 包含其他单语种、语言未知及真正支持多个语种的音色，保证供应商目录中的其他语种不会因三类展示口径被隐藏。

1920px 下生成配置栏内容宽 `964px`：模型与目录状态合计 `484px`，试听卡片 `462px` 且同高 `54px`；音色和语言分别为 `650px / 302px`；旁白为 `964px × 180px`；下方同一行依次放置 `154px` 语速、`200px` 主动试听和 `586px` 生成按钮；当前任务为 `964px × 70px`。1440px 下继续使用原有纵向排布和 `484px` 内容宽，不对字体或控件做等比例缩放。两基准之间不得在中栏可容纳双区结构时整体退回 `484px` 单列：模型与目录保持 `484px`，试听占满同排剩余宽度；音色/语言、旁白、操作行和当前任务连续利用中栏可用宽度，且不得产生水平溢出。

声音与字幕页面的 CSS 字号契约为：正文、模型信息和 Agent 消息正文至少 `12px`；辅助标签、状态、时间、错误摘要、会话元信息和诊断字段至少 `11px`；任务标题、表单当前值、命令按钮和失败标题为 `13px`；旁白 `textarea` 为 `14px`；失败供应商消息为 `12px`。固定高度徽标和控件必须同步调整行高或内边距，避免文字裁切，且不得使用页面缩放抵消字号提升。

右栏 Agent 头部固定为两行：第一行展示“声音 Agent / 在线”，第二行展示会话信息和文本模型选择；旧版通用 Agent 面板样式不得把两行重新排成两列或造成重叠。声音参数优先使用模型目录显式 `default`；目录未声明 `speed_ratio.default` 时按 TTS 中性语速 `1.0` 初始化，并约束在目录最小值与最大值之间，不得使用区间中点推导语速。

`Languages[].Language` 是语言代码，`Languages[].Text` 是试听文案。工作台语言选项和音色标签只能按 `Language` 代码显示中文名称并提交原始代码，禁止把 `Text` 用作语言名称。首批真实目录代码为 `zh | zh-cn | en | id | pt-br | ja | mx | vi | th | es-mx | fil | fr | ru | de | ko | ms | ar | it`；未知代码原样显示，不得回退到试听文案。

### TTS 协议

- 模型：`doubao-seed-tts-2.0`
- 资源标识：`seed-tts-2.0`
- 端点：`https://openspeech.bytedance.com/api/v3/tts/unidirectional`
- 鉴权：`X-Api-Key`
- 资源 Header：`X-Api-Resource-Id: seed-tts-2.0`
- 请求追踪：唯一 `X-Api-Request-Id`
- 响应追踪：保存 `X-Tt-Logid`
- 字幕：校验当前官方响应 `sentence.words` 的真实字词时间戳
- 语言：工作台选择的目录语言代码映射到官方 `req_params.explicit_language`
- 情绪：当前官方请求体没有结构化 `emotion` 字段；目录空占位不得转成 `neutral`，请求不得发送未定义的 `language` 或 `emotion`

### OpenAI Audio Speech 中转协议

- 协议：`openai_audio_speech`
- 端点：请求根地址追加 `/audio/speech`，管理端同时接受完整 `/v1/audio/speech` 并归一化为 `/v1`
- 鉴权：`Authorization: Bearer <API Key>`
- 请求：只发送 `model`、`input`、`voice`、`response_format` 和可选 `speed`
- 目录：必须复用同 `upstream_model + resource_id` 的官方 `volcengine_tts_v3` 根目录
- 字幕：`supports_word_timestamps=false`，不得生成 TTS 同步字幕或自动追加 ASR
- 响应：按 `Content-Type` 与音频签名校验二进制媒体后落盘，不按 V3 NDJSON 解析

### 脚本旁白导入

工作台使用既有脚本列表与详情 API，不新增旁路脚本数据源。列表按最近更新时间展示当前项目脚本，只保留 `draft | approved`；选择脚本后读取详情并按 `scenes[].sequence` 展示非空旁白。确认导入后前端保留 `script_id`、详情 `updated_at` 和所选 `scene_id`，后续试听或生成预检一并提交。

应用服务必须重新读取脚本并锁定来源快照，不能接受前端提交的旁白快照。任务新增 `source_script_id` 与 `source_script_snapshot`；快照至少包含脚本标题、脚本 `updated_at`、所选分镜 ID、顺序和原始旁白。预检确认摘要必须覆盖来源快照，使脚本版本或分镜选择变化后旧确认令牌失效。人工重试继续复制失败任务的来源快照，不依赖脚本当前版本，也不得用最新脚本覆盖历史来源。

密钥不得进入 API 响应、日志、Agent 消息、任务快照或素材 metadata。流式响应必须完成协议和媒体完整性校验后再落成功状态。

### 失败诊断

供应商 HTTP 非 2xx 响应必须在抛出任务错误前解析受限大小的 JSON。Worker 只从根对象、`header` 或 `error` 对象提取错误码和消息，并从协议允许的响应头读取 `X-Tt-Logid`、`X-Request-Id` 或 New API 实际使用的 `X-OneAPI-Request-Id`；提取结果经过换行归一化、长度限制和敏感键值掩码后写入 `error_details` 与 `upstream_log_id`。非 JSON、超出白名单或无法识别的内容只保留稳定的 HTTP 摘要，不保存原文。

`sound_subtitle_tasks.error_details` 使用非空 JSONB 对象并由数据库约束其类型。成功任务清空该对象；失败任务 API 返回该对象以及已有的 `error_code`、`error_summary` 和 `upstream_log_id`。旧任务迁移默认 `{}`，不回填不可恢复的供应商信息。

### 字幕与 ASR

TTS 主链路优先使用返回的中文/英文字词时间戳。字幕 Agent 只负责断句、文本与样式，不自行推测时间。已有音频使用 `doubao-seed-asr-2.0`，API 资源 `volc.seedasr.auc`；ASR 不替代新生成 TTS 的时间戳主链路。

已有音频在展示确认信息前先创建幂等的 `AudioMaterialInspection`。检查只允许读取 `/assets` 下的自管文件，使用 `ffprobe` 解析真实媒体流并计算文件 SHA-256；成功快照包含时长、字节数、容器格式和音频 codec，不包含外部 URL 或凭据。ASR 创建时必须校验检查属于同一项目与素材、处于成功状态且未被后续检查替代，资源确认中的时长由服务端从该快照生成。

### 统一语音模型管理

`ai_models.model_type` 增加 `speech`，`api_protocol` 支持 `volcengine_tts_v3 | openai_audio_speech | volcengine_asr_v3`；原生语音使用 `api_key`，OpenAI Audio Speech 使用 `bearer`。数据库 migration 只扩展 CHECK 约束，不重写现有记录；旧的 `text/image/video` 协议配对、按类型默认模型和查询响应保持不变，语音默认模型由 `(speech, api_protocol)` 唯一索引和同范围事务锁维护。

Admin 新增“语音模型”分类，允许新增和编辑显示名、协议、请求根地址、上游模型、API Key、超时、排序及版本化 `settings`。TTS 模型明确选择官方同步或复用已有目录：仅官方同步模式维护目录 Access Key/Secret Key，编辑时空凭据表示保留旧值；复用模式选择已有官方目录来源且不显示、不提交 AK/SK。TTS `settings` 至少保存资源 ID、输出格式/采样率、可调参数约束及时间戳支持声明；ASR `settings` 至少保存资源 ID、支持格式、最大音频时长及时间戳支持声明。协议切换必须重新校验并清除不适用的非敏感配置，不能把 TTS 记录当作 ASR 记录执行。

Admin 模型抽屉落实已确认 Pencil 的三段式结构：标题栏、可独立纵向滚动的字段区和固定底部操作栏。语音能力字段增加导致表单高于桌面视口时，不得让保存操作随字段内容滚出可视区域；保存进行中和目录来源无效仍按现有规则禁用主操作。编辑 `openai_audio_speech` 时，新 Bearer API Key 必须与当前乐观锁版本和有效目录来源一并提交；请求失败保留抽屉及输入，成功后才关闭并刷新列表。

模型配置校验失败不得只留下无法定位的统一文案。API 仍返回不含凭据的稳定错误响应，同时服务端按安全白名单记录具体校验分支；日志不得包含请求体、API Key、目录 AK/SK、Authorization 或原始 URL 查询参数。`openai_audio_speech` 的路由测试必须覆盖已成为该协议默认模型后的 Bearer API Key 更新，确认更新不会被默认模型规则或共享目录校验误拒绝。

目录来源选择使用独立的 Admin 模型查询 `type=speech&status=enabled`，不得复用模型表格的已筛选结果。客户端只从独立结果中保留未删除、启用、`voice_catalog_mode=official_sync` 的 `volcengine_tts_v3`，并继续按当前表单的 `upstream_model + settings.resource_id` 精确匹配。抽屉首次进入需要共享目录的 TTS 配置、重新打开或操作者主动重试时刷新该查询；查询失败时保留稳定错误状态，不得将失败结果归一化为空候选。

Admin 的 TTS 时间戳语言使用受控的可搜索多选下拉，不接受自由文本。触发器只显示已选中文标签，下拉内只显示“简体中文”“美式英语”，分别保存 `zh-cn`、`en-us`；搜索只按中文标签过滤且不改变选择，点击外部或按 `Escape` 关闭并保留当前选择，始终保证至少选择一项。ASR 的 `*` 不作为可编辑语言值暴露，界面显示只读“自动识别（全部语言）”。

Admin 在“工具与 MCP / 私有 TOS”独立维护系统公用配置，包含 endpoint、region、bucket、object prefix、签名有效期、文件/音频限制及独立 Access Key/Secret Key。管理响应只返回凭据掩码，编辑时两项凭据同时留空表示保留当前值；保存时使用乐观锁创建新版本。首次配置和任何连接参数变化都先保存为未启用版本，经真实连接检查成功后才允许启用；仅启用开关变化时可沿用同一组连接参数的成功检查结果。TOS 字段和凭据不得进入 `ai_models`、`ai_models.settings`、模型管理响应或模型运行快照。ASR 模型表单只读展示系统 TOS 是否已配置并提供跳转入口。

系统 TOS 管理 API 为 `GET /api/tools/tos-staging`、`PUT /api/tools/tos-staging` 和 `POST /api/tools/tos-staging/check`。读取未配置状态返回稳定的 `configured=false`；保存必须提交期望版本，首次配置使用空版本，版本冲突返回稳定错误。检查 API 只将当前不可变版本置为 `queued`，独立的系统 TOS 工具 Worker 领取后置为 `running`，通过官方 Python `tos` SDK 执行 `HeadBucket`，并在配置前缀下以固定探针完成 `PutObject -> 签名 GET 实读 -> DeleteObject`；只有整条能力检查和清理成功才能写入 `succeeded`，本地字段校验不得冒充成功。探针失败必须尽力删除同一固定对象键，使后续重试可覆盖并完成清理。该 Worker 使用独立的 `TOS_TOOL_WORKER_ENABLED` 开关且默认关闭，启用连接检查不得同时开启可能计费的 TTS/ASR 队列。检查失败只记录脱敏错误类型。系统未配置、未启用或未通过连接检查时，ASR 检查仍可执行，但创建 ASR 任务必须失败且不得入队。

工作台只从 `/api/model-options?type=speech` 获取启用模型，并按任务要求过滤 TTS 或 ASR 协议；旧客户端继续使用原有类型和值，不要求迁移。

### 工作台路由

Next.js App Router 增加 catch-all 页面承接菜单深层 URL，页面实现继续复用同一个工作台组件。菜单加载完成后，前端按规范化后的当前 `pathname` 匹配已启用菜单及其已启用子菜单；命中子菜单时同时恢复父菜单，命中父菜单时进入该父菜单的默认子页面。菜单点击通过客户端路由导航到接口返回的 `route_path`，不得根据 `menu_key` 拼接另一套路径。

根路径、未知路径、隐藏路径或禁用路径在菜单加载完成后使用 `replace` 规范化到首个可用菜单路径，避免浏览器后退栈产生无效项。URL 查询参数和锚点不参与菜单匹配；本轮不持久化账号、标签页、筛选器、草稿或弹窗状态。

### 可靠性

试听和生成请求使用幂等键及并发限制。未取得可恢复结果的临时错误只允许同模型自动重试 1 次，不自动跨模型/音色/供应商。人工重试前再次展示字符数或音频时长。

### ASR TOS 临时中转

`volc.seedasr.auc` 只接受供应商可访问的 `audio.url`。Worker 从自管素材存储读取音频，以 `{project_id}/{task_id}/{source_sha256}.{ext}` 作为确定性对象键上传私有 TOS Bucket，并通过官方 `tos` SDK 生成短期 GET 签名 URL。相同任务重试必须复用对象键和 ASR `X-Api-Request-Id`，不得重复创建不可追踪对象或重复提交已获得上游任务 ID 的请求。

Worker 镜像必须包含 `ffprobe`。ASR 上传前按任务锁定的系统 TOS 配置版本再次计算源文件 SHA-256，并验证实际时长未超过模型和工具配置上限；与确认时检查摘要不一致时以稳定错误终止，要求操作者重新检查和确认，禁止继续上传。配置版本不存在、版本不匹配或凭据不完整时必须稳定失败，禁止回退到当前配置或模型字段。

签名 URL 只存在于进程内请求上下文，不写入数据库、日志、Agent 消息或素材 metadata。ASR 成功、失败或取消后都进入清理步骤；删除失败不得把 ASR 结果改为失败，但必须记录不含 URL/凭据的 `cleanup_pending` 状态并由定期清理任务重试。暂存配置限制单文件大小、音频时长、签名有效期和对象保留上限，不建设金额字段。

## TDD

- 目录测试：多页完整同步、新增、下线、分页失败原目录不变、草稿失效选择。
- Worker 解耦：目录开关与语音生成开关独立、健康状态可见、只开启目录 Worker 时不得消费 TTS/ASR。
- 模型管理：`speech` 类型、协议配对、类型化 settings、官方凭据标签与用途提示、语音凭据脱敏及旧模型回归。
- Admin 模型抽屉：长语音表单字段区独立滚动、标题与操作栏保持可见、中转 Bearer API Key 更新提交及失败输入保留。
- 目录共享：官方/共享模式、来源键一致性、禁止自引用和共享链、来源生命周期保护、目录查询/同步解析与任务快照来源审计。
- Admin 时间戳语言：中文标签、可搜索多选下拉、外部点击与 `Escape` 关闭、至少一项约束、标准代码提交、ASR 只读自动识别。
- TOS 暂存：系统工具 CRUD/乐观锁/脱敏、真实 `HeadBucket` 检查队列与启用闸门、模型 CRUD 解耦、任务配置版本锁定、`ffprobe` 真实时长、源摘要变化阻断、官方 SDK contract、确定性对象键、短期签名、幂等上传、终态清理和失败补偿。
- Provider contract：V3 headers、流式协议、追踪 ID、`sentence.words`、脱敏和错误分类。
- OpenAI Audio Speech contract：Bearer Header、`/v1/audio/speech`、标准请求字段、二进制音频校验、永久鉴权错误不重试及无时间戳字幕阻断。
- 失败诊断：火山根对象/`header` 与 OpenAI `error` JSON 解析、完整追踪 ID、非 JSON 回退、敏感消息掩码、JSONB 持久化和 API 白名单返回。
- 领域/API：Agent 只推荐、主动试听、确认幂等、同模型重试、模型停用和素材入库。
- 字幕：TTS 时间戳断句、ASR 已有音频、不支持时间戳明确失败、SRT 格式。
- 前端/E2E：仅双标签、动态目录、失效选择不静默替换、无金额字段。
- 前端原型一致性：1440px 下 `1118px` 与 `250/520/276px`，1920px 下 `1598px` 与 `250/1000/276px`，约 `24px` 内容边距；宽屏中栏内部 `484/462px` 顶部同高、`650/302px` 音色语言、`964×180px` 旁白、下方 `154/200/586px` 控制行和 `964×70px` 当前任务；中间桌面宽度在能容纳双区结构时不得退回 `484px` 单列或留下大片无意义空白；左栏任务列表、右栏 Agent 运行审计，以及不存在底部重复任务表。
- 真实声音能力：语言自定义单选弹层始终位于触发框下方；空 `Emotions` 不渲染、不阻断预检；Provider 只发送 `explicit_language`，绝不发送未定义的 `language/emotion`。
- 工作台音色选择：原名与中文描述分层展示、中文标签搜索、语言与声线 Tag 交集筛选及再次点击关闭、扁平单选列表、键盘/外部点击关闭、失效选择保留，以及全部真实语言代码中文映射且绝不展示试听文案。
- 工作台路由：菜单点击写入数据库 `route_path`、深层 URL 直达、刷新恢复、前进后退同步、无效或禁用路径回退。
- 失败态与任务列表：完整脱敏诊断字段、长 ID 不截断、历史字段缺失显示、8 个连续失败任务的固有卡片高度、列表独立纵向滚动、无横向滚动及并发区固定。
- 可读字号：组件保留任务、表单、Agent 和失败详情的语义层级；桌面 E2E 读取 computed style 验证正文/辅助信息下限及 `13px` 任务标题、`14px` 旁白、`13/12/11px` 失败详情，同时复验三栏尺寸、`140px` 宽屏失败详情和无裁切/溢出。

## Decisions

### 动态目录而非硬编码

供应商目录是新音色和下线状态的事实来源；Admin 版本化配置补充官方支持但接口未直接返回的参数能力。Agent 只能消费目录，不能创造目录。

### 同一上游模型复用目录

中转服务可能只提供 TTS 运行凭据，不提供 `ListSpeakers` OpenAPI AK/SK。目录能力属于上游模型与资源版本，不属于中转请求地址；因此使用显式来源关系复用同一 `api_protocol + upstream_model + resource_id` 的官方目录，并通过根来源和生命周期约束避免隐式同名匹配或共享链。

### TTS 时间戳优先

新生成 TTS 已能返回字幕时间戳，使用原生时间轴比二次 ASR 更准确且少一次调用。ASR 只处理已有/上传音频。

### OpenAI Audio Speech 只生成音频

New API 中转公开路由为 `/v1/audio/speech`，请求与响应不等同于豆包 HTTP Chunked V3。独立协议避免把“可以保存”误当作“可以按 V3 执行”。该响应没有可信 `sentence.words` 时只允许生成音频；同步字幕必须改由操作者显式选择已有音频 ASR，系统不自动增加供应商调用。

### 试听显式触发

试听会产生真实模型调用，模型或音色切换不应自动执行。用户确认资源用量后才调用。

## Risks / Trade-offs

- [供应商目录字段不足] -> 用 Admin 按官方资料维护的版本化能力补充，禁止 LLM 推测。
- [分页同步部分成功造成误下线] -> 全部分页成功后原子提交，失败保留旧目录。
- [共享来源被修改导致音色错配] -> 建立显式外键，保存时校验三项目录身份，并在仍被引用时阻止来源停用、删除或改变身份。
- [语种不支持时间戳] -> 阻止伪同步字幕，明确提示改用已有音频 ASR 或人工处理。
- [流式中断产生损坏文件] -> 完整性校验通过后才入库。
- [完整上游返回泄露凭据] -> 只解析和持久化诊断白名单，原始响应头/响应体始终丢弃。
- [任务数量增加压缩卡片] -> 左栏固定头尾区域，列表使用独立纵向滚动和固有内容行高。

## Migration Plan

1. 建立动态目录和 fake provider contract，不执行真实调用。
2. 扩展统一模型管理，建立独立版本化系统 TOS 工具配置，并接入 TTS/ASR 任务与素材登记。
3. 为 TTS 模型增加目录来源关系；现有已配置 AK/SK 的模型迁移为 `official_sync`，中转模型由管理员显式选择可复用来源。
4. 原型确认后启用双标签工作区。
5. 真实验证必须单独获得许可，并限制字符数、任务数、并发和重试。
6. 回滚时停用任务消费和菜单，保留目录、素材与审计。

既有 ASR 模型上的 TOS 字段迁移时按默认、启用状态、排序和创建时间确定当前系统版本，同时保留每个旧配置为历史版本并记录迁移来源；既有 ASR 任务按原 `model_id` 关联对应历史配置。迁移完成后删除 `ai_models` 的全部 staging 字段与约束，不保留双写或读取兼容路径。

## Open Questions

无阻塞问题。AI 音乐和音效接口尚未选定，明确排除。
