# Novex 项目记忆

> 本文件是 Novex AI Agent 基座项目级记忆统一入口，记录长期偏好、稳定规则、历史决策和跨会话背景。`apps/video-agent` 是当前第一个业务应用。

## 记忆文件索引

### 仓库约定
- 详见 `docs/memory/project-memory-structure.md` — 项目记忆结构规则
- 详见 `docs/memory/frontend-design-skill-requirement.md` — 前端设计约束

### 项目背景
- 详见 `docs/memory/project-tech-stack.md` — 技术栈与架构设计
- 详见 `docs/memory/video-agent-workspace-flow.md` — 视频工作台菜单、业务流、Agent 分层和开发阶段规划
- 详见 `docs/requirements/video-agent-mvp.md` — MVP需求边界与验收标准
- 详见 `docs/requirements/video-agent-database-schema.md` — 简化版数据库设计
- 详见 `docs/requirements/video-agent-full-spec.md` — video-agent 完整需求文档

## 核心决策（2026-07-01）

### 技术选型
- **后端**: Rust + Axum + SQLx + PostgreSQL
- **向量库**: Milvus Standalone（20万素材规模）
- **任务队列**: Redis + 简单Job表
- **Worker**: Python（FastAPI）处理视频生成和平台发布
- **前端**: Next.js 14 + TypeScript + shadcn/ui

### 当前架构决策
- 当前仓库定位已从根级 video-agent MVP 调整为 **Novex AI Agent Foundation monorepo**
- `video-agent` 保留为 `apps/video-agent` 下的首个业务应用
- OpenSpec change `align-novex-foundation-architecture` 已于 2026-07-01 归档
- `script-agent-mvp` 已在 Novex 基座结构下完成并归档，脚本生成、读取、列表、状态更新 API 已实现
- `apps/video-agent/` 是 `VEDIO-AGENT / 视频工作台` 的正式视频生产工作台边界；当前已迁入脚本 Agent 前端闭环，打通“生成脚本 -> 查看分镜 -> 更新状态”。素材匹配和视频生成编排作为后续 OpenSpec change 推进
- `admin/` 已收敛为 Novex 平台管理后台入口，承载用户、权限、模型、工具、MCP、任务、日志、运行状态、成本、限额和健康检查等控制面能力，不再承载日常视频内容生产流程
- `apps/video-agent/` 前端工作台的对外可见产品品牌名为 `VEDIO-AGENT`，展示名为“视频工作台”；原型、UI 和当前工作台设计文档不得使用 `Novex Admin` 作为展示品牌
- 视频工作台 Pencil 原型源文件统一保存在 `docs/prototypes/video-agent/video-agent.pen`；后续有关视频工作台的原型修改都以该文件为准，不再使用 `docs/prototypes/script-agent-workspace/` 截图目录
- 视频工作台 `.pen` 原型修改必须通过 Pencil MCP 写入并用 `batch_get` 或编辑器顶层状态验证；不得直接手改 JSON 后视为原型已更新
- OpenSpec change 达到 `all_done` 后只报告可归档；未经用户明确命令，不得执行 `openspec archive` 或以其他方式自动归档。
- 用户已确认视频工作台业务流程走向：内容策略 -> 脚本创作 -> 素材管理 -> 作品生产 -> 发布运营 -> 数据分析 -> 工作流任务；前端一级菜单和开发阶段规划详见 `docs/memory/video-agent-workspace-flow.md`
- 视频工作台导航应以数据库持久化菜单配置作为单一来源，一级菜单固定围绕业务流程组织；`apps/video-agent` 不得继续用 6 个 Agent 硬编码数组作为一级导航，Agent 状态只能作为二级菜单、模块状态或执行状态展示
- 视频工作台不是单一脚本 Agent 页面；选题、脚本、素材、视频、发布、优化六类 Agent 能力应映射到业务菜单下的二级入口、模块状态或执行状态，不再作为前端一级导航；当前 `script-agent-workspace` 只实现脚本创作下的脚本生成模块闭环
- `projects` 是内容项目/账号方向/内容生产边界，不是具体选题；当前脚本生成必须绑定真实 `project_id`，但选题尚无独立管理模型，只作为 `topic` 文本输入和脚本上下文保存。没有选题池前，不显示“当前选题”或选题管理入口；后续应在“内容策略/选题池”中确认选题，再进入脚本创作并让脚本引用 `topic_id` 或保存选题快照
- 用户已确认内容策略与选题池第一版走“选题池优先”：内容策略页展示项目策略摘要和选题池闭环，支持人工创建选题和选题 Agent 批量生成候选；选题状态为 `idea -> approved -> scripted -> archived`，Agent 生成候选自动入库为 `idea` 并记录批次。选题 Agent 第一版接入通用 Agent Runtime 的 `topic` adapter，输入只依赖项目定位和用户补充要求；脚本生成关联 `topic_id` 并保存 `scripts.content.topic_snapshot`，成功后选题状态更新为 `scripted`
- 用户已确认历史生成批次补充选题采用“同主题上下文 + 补充批次”语义：补充生成必须创建新的 `topic_generation_batches`，新批次 `supplement_of_batch_id` 指向原始批次，新选题 `content_topics.batch_id` 指向补充批次本身；对补充批次再次补充时归一到最初原始批次。前端查看选题时按主题组聚合展示原始批次和补充批次的选题，批次只作为生成来源和审计记录。
- 用户已确认账号/项目管理此前暂列为后续功能；2026-07-08 已确认第一版账号/策略资料沉淀进入当前内容策略范围。第一版采用 `projects` 作为内容账号/内容生产边界并扩展结构化 `strategy_profile`，前端展示语义统一为“账号”；本轮只做内容账号策略资料，不做抖音、小红书等平台账号凭据、发布账号管理或 `accounts` 表改造。用户进一步确认本版加入“AI 生成策略草稿，人工确认后保存”：AI 草稿只预填编辑表单，不得自动写入 `strategy_profile`，草稿生成只能手动触发并需要成本控制。用户最新纠正：账号策略资料只在内容策略下独立二级页面“账号策略”维护，当前选题池不得展示账号策略区块、策略资料状态/摘要或编辑入口；后续完整账号管理再单独拆出。
- 用户已确认内容策略下一轮优化聚焦“选题太多不好筛”，采用“主题组评审快照 + 双页面同步分层展示 + 第一版手动触发、自动预留”的设计：AI 评审按原始批次主题组聚合原始/补充批次选题，输出优先/备选/淘汰、疑似重复、风险点和推荐理由；评审结果在“历史生成”和“当前选题池”同一主题组上下文同步展示；AI 评审只作为决策辅助，不得自动修改 `ContentTopic.status`
- 用户已确认主题组质量排名的第一版目标不是泛化质量排序，而是“选出最值得立刻生成脚本的主题组”；后续设计应按“脚本产出优先级排名”推进，默认基于已有选题、补充批次归组和最新成功主题组评审快照做确定性排序，不在历史列表加载时额外调用 AI。未评审或评审过期的主题组必须标记为待评审/需重新评审，不得排为高优先级；排名只辅助决策，不自动改变 `ContentTopic.status`
- 用户已确认内容策略下一轮优化聚焦“Agent 生成选题质量不稳定”，采用“生成前策略约束 + 生成后质量闸门”的方向推进：第一版在 `topic` Agent 生成候选后、写入 `content_topics` 前执行质量闸门，覆盖账号匹配度、具体度、差异化、脚本化可行性、风险与评分可信度；首轮低质比例过高时最多自动重写一次；只把通过项写入选题池，淘汰候选只保留在质量报告中。该能力不得新增 `quality_rejected` 选题状态，不得改变 `idea -> approved -> scripted -> archived` 生命周期，也不得自动确认、归档、删除选题或生成脚本
- 已建立第一版通用对话 Agent Runtime 后端基座：`agent_conversations` / `agent_messages` 承载连续对话，单轮消息继续写入 `agent_runs` / `agent_steps`，脚本 Agent 已接入对话式分镜修改能力；后续选题、素材、视频、发布、优化 Agent 应接入同一 Runtime/adapter 接口，不得各自实现孤立聊天逻辑。当前未实现前端聊天面板，`apps/video-agent` UI 接入仍需先走 Pencil 原型确认
- 用户已确认新的脚本创作产品约定：脚本生成也应走脚本 Agent 对话入口；后续 `apps/video-agent` 不应在右侧并列保留独立“生成脚本”大表单和“脚本 Agent 对话”输入框，而应使用单一脚本 Agent 对话承载无脚本时生成脚本、有脚本时修改脚本。该改造通过 OpenSpec change `conversational-script-generation` 推进，前端实现前仍需更新 `docs/prototypes/video-agent/video-agent.pen` 并获得确认
- 脚本智能体详情展示已选定“时间轴对照视图”：左侧表达分镜顺序和节奏节点，右侧并排展示旁白与画面指令；后续实现不要回退成纯卡片流或纯表格
- 用户已确认素材 Agent 下一步规划采用“旧素材复用优先 + AI 图片自动生成 + AI 视频人工二次确认”边界：AI 图片默认每分镜 3 张候选、可调整为 1-4 张，未选候选也进入素材库并标记来源；旧人物/IP 素材可作为参考图；生成图片必须落到自管素材存储后再写入 `materials.file_url`；第一版图片供应商为 `gpt-image-2` 和即梦，用户可选择供应商，失败不得自动跨供应商重试。后续“大模型管理”归 `admin/` 平台控制面，不放在视频工作台；视频工作台只消费已启用供应商、默认模型和限额配置。
- 2026-07-11 已确认并实现统一 AI 模型管理，此条覆盖此前“素材生成直接选择供应商”的旧约定：PostgreSQL `ai_models` 统一纳管文本、图片、视频模型及供应商、显式 API 调用协议、请求地址、上游模型、推理配置、超时和凭据；API Key/API Secret 原文入库，但 API、日志和运行快照必须掩码或排除。`admin/` 支持添加、编辑、删除、启停和默认替换；`apps/video-agent` 的账号策略草稿、选题生成/补充、主题组评审、脚本确认、脚本对话和素材生成均由用户选择启用模型并提交 `model_id`，不得再按供应商字符串或环境变量路由。临时错误只允许同模型重试，不得自动跨模型切换；视频工作台仍不新增视频生成模型调用入口。
- 2026-07-10 `gpt-image-2` 真实调用仍不可用：使用独立 `OPENAI_IMAGE_KEY` 和 `OPENAI_IMAGE_BASE_URL=https://api.zeekai.cc` 对 `/v1/images/generations` 执行一次 `n=1`、无重试的 SSE 请求，服务端返回 `HTTP 403 permission_error: Image generation is not enabled for this group`，未生成图片；后续不得继续盲目重试或开启 Worker，必须先在 ZeekAI 确认该 Key 所属分组已启用图片生成，再做一次受控验证。不得在记忆、日志或回复中记录真实 Key。
- 2026-07-13 用户确认回退 `model_type=image + api_protocol=openai_responses`，并进一步确认以正式 `volcengine_ark_images` 完整替换内部旧协议 `jimeng_visual`：图片最终只允许 `openai_images | volcengine_ark_images`，Ark 固定使用 Bearer API Key；每个候选独立调用一次，单候选临时错误最多重试一次，永久错误停止剩余调用；不得保留旧协议、旧 `jimeng` 审计值、VisualService SDK 或 `JIMENG_*` 兼容路径。Admin“设为默认”使用 `POST` 与 Worker `/assets/...` 本地参考图安全读取属于独立修复，继续保留。
- 2026-07-14 `see-dream` 火山方舟图片协议已完成一次受控真实验证：单分镜、单候选、无参考图，首次调用成功且未重试，生成 2048x2048 JPEG 并落入自管素材存储。验证后 `ASSET_GENERATION_WORKER_ENABLED` 保持关闭；Ark 结构化请求/curl 日志已接入 Uvicorn INFO handler，后续请求将输出脱敏日志。
- 2026-07-14 用户在受控真实验证成功后明确确认启用图片任务自动执行，并接受后续任务按候选计费。当前本地运行环境使用 `ASSET_GENERATION_WORKER_ENABLED=true`；其他环境的 `.env.example` 默认值仍保持 `false`，需操作者显式启用。
- 2026-07-14 用户确认 AI 新生成图片的实际文件名统一为 `{脚本名称}-镜头{两位序号}-第{两位候选序号}张.{实际扩展名}`：保留中文并执行 NFC、非法字符清理和 255 UTF-8 字节安全截断，空标题回退“未命名脚本”；使用 Worker 领取任务时的脚本标题快照，batch 与 `per_candidate` 都按原始 1-based 候选槽位编号，部分失败不得重排。文件继续位于生成任务 UUID 目录，`materials.file_name` 与物理 basename 一致，素材/候选 metadata 记录 `script_title_snapshot`、`scene_sequence`、`candidate_index`；只影响新文件，不重命名历史素材。用户随后确认已通过自然任务验证。
- 2026-07-14 用户确认后续 Ark 受控调用与自然任务已经补齐 `script-to-asset-generation` 12.5 的成功链路验收；结合此前 `gpt-image-2` 失败终态、错误展示、永久错误熔断和费用上限验证，该 OpenSpec change 已达到 `64/64`、`all_done`。
- 2026-07-14 已归档当前全部 active OpenSpec change：`script-to-asset-generation`、`support-image-responses-protocol`、`fix-ai-model-default-request-method`、`remove-image-responses-protocol`、`replace-jimeng-with-volcengine-ark-images`、`friendly-generated-image-filenames`。最终主规格删除整份 `image-responses-generation` capability，图片协议只保留 `openai_images | volcengine_ark_images`，并保留友好图片文件名规则；active change 列表为空。
- 用户已确认素材生成应作为 `素材管理 / 素材生成` 独立二级入口，不放在 `脚本创作 / 脚本生成` 页面；页面文案中 `Agent` 仍作为产品名称保留，只去掉 `Topic Source`、独立 `Agent`、`素材 Agent` 这类说明性小标题。
- 2026-07-14 用户确认首版作品生成采用作品级一次提交：汇总全部分镜已确认图片和镜头描述，由 LLM 通过可见的作品生成 Agent 对话生成全片提示词，用户查看并确认后再提交 Seedance；产品交互不得要求按图片逐张发起。Seedance 输出再与已有音频和 TTS 配音合成为最终成片。上游编排必须遵守 Seedance 正式素材数量、输出时长和提示词限制，前端一次操作不得被虚假描述为任意长度作品永远只有一次上游调用。
- 2026-07-14 用户确认首版作品的 TTS 配音和字幕由作品 Agent 生成与编排，不以人工预先上传完整配音、字幕作为主流程；这些能力必须作为作品生产前置组成部分与 Seedance 视频生成和最终合成一并设计。生成结果进入素材库并保留模型、提示词、来源、时间轴和任务审计；LLM 负责旁白规划与字幕分段，实际声音由 TTS 模型生成，字幕时间轴基于 TTS 返回时间戳或对齐结果产生。
- 2026-07-14 声音设计的长期目标仍包括 TTS、BGM、环境音、动作音效和 AI 音乐；用户最新确认首版只实现 TTS 配音与字幕生成/对齐，AI 音乐/BGM 生成、环境音生成和动作音效生成暂缓，等待选定正式 API 或开源项目后另行设计。已有 BGM、环境音和动作音效仍可上传素材库并在作品多轨时间轴中选择、混音和复用，暂缓范围只是不做这三类 AI 生成。
- 2026-07-14 用户确认 TTS 生成前需要选择声音风格，页面上方应根据当前语音模型的真实能力目录展示音色、语言/口音、情绪风格和可调参数；目录来源只能是供应商接口或 Admin 按官方资料维护的版本化配置，不得由 LLM 猜测或前端硬编码。模型切换后动态刷新且不得静默替换失效选择，Agent 可推荐但用户必须查看、试听并确认；需要实时调用模型的试听必须由用户主动触发。
- 2026-07-14 用户确认作品允许超过 Seedance 单任务 15 秒，但用户侧仍一次确认提交，后台按模型限制自动拆分连续任务并合成。生成前由用户选择成片总时长、画面比例和分辨率；选项必须来自当前视频模型的真实能力目录并校验组合。总时长需结合 TTS 实际时长和分镜数预检，任一输出参数变化都要重新计算提示词、任务拆分和资源用量并再次确认。
- 2026-07-14 用户确认作品时长支持 `15/30/45/60秒` 预设、`4~60秒` 自定义和“跟随配音”；固定时长以最终成片总时长为准，“跟随配音”先生成 TTS 后按实际配音时长确定。Seedance 后台拆分优先使用分镜边界并确保每段满足 `4~15s`，尾段不足最短时长时重分配相邻片段。
- 2026-07-14 用户确认作品生成失败时保留全部成功结果，只重试失败节点；重试前重新展示将再次调用的模型任务和资源用量并人工确认，不自动跨模型或供应商。未获得上游任务 ID 的临时错误最多同模型自动重试一次，获得上游任务 ID 后只查询/恢复原任务，不得重复提交。整体重生成创建新作品版本，运行中 Agent 修改只进入下一版草稿。
- 2026-07-14 用户确认首版作品生成涉及的方案 LLM、视频和 TTS 模型按能力类型独立选择；首次可预选 Admin 默认模型，用户可修改，Agent 只推荐不得自动切换。生成前统一确认模型快照、任务数量和资源用量；切换模型需刷新真实能力并重校验，停用/删除模型保留原选择但阻止生成，运行开始后锁定模型和参数快照。AI 音乐和音效模型选择待对应生成能力恢复时再增加。
- 2026-07-14 用户确认成片局部修改采用不可变版本：从已有版本复制新草稿，Agent 先展示修改差异、影响范围和需要再次调用的任务/资源用量，确认后只重生成受影响素材并执行必要合成，未变化素材复用。新结果进入素材库，旧版本与旧素材不覆盖；比例/分辨率等全局变化需明确提示全部视频片段重生。
- 2026-07-14 用户确认作品库默认缩略图网格并支持列表切换，详情管理预览、多轨时间轴、版本记录、调用审计、版本对比和继续修改。生成过的作品只允许归档/恢复，未发生外部调用的空白草稿可删除。完成版本可下载 MP4、字幕、混音/分轨声音和制作包，并携带选定版本进入发布运营但不自动发布；发布状态不进入作品生命周期。
- 2026-07-14 用户确认作品生产“生成任务”使用高密度列表与右侧分步骤详情，展示作品版本、阶段、进度、子任务结果和资源用量，以及首版 TTS、字幕、Seedance、混音、合成各步骤审计。排队任务可取消，运行中仅按供应商能力取消；失败节点重试需确认再次调用的任务，失败任务可隐藏但任务、错误和调用审计永久保留。作品任务与一级“工作流任务”的跨 Agent/队列监控保持边界。
- 2026-07-14 用户确认素材管理保留“声音与字幕生成”二级菜单；首版页面只展示并实现 `TTS配音 / 字幕` 两个标签，共用可见声音 Agent 和任务能力。此前规划的 `AI音乐 / 环境与音效` 标签在对应接口或开源方案确定前不展示空入口。TTS 和字幕结果自动进入素材库，重新生成产生新素材且不覆盖旧文件。
- 2026-07-14 用户确认最终二级菜单为：`素材管理 / 素材库、画面生成、声音与字幕生成`，`作品生产 / 作品生成、生成任务、作品库`。现有“素材生成”改名“画面生成”且只负责图片候选和主画面选择，不再新建逐分镜视频任务；Seedance 全部进入作品级一次提交。历史 `video_draft/video_generation` 只读保留审计，新任务采用作品生产领域模型，菜单仍由数据库驱动。
- 2026-07-14 用户最终确认作品生产不建设任何金额费用能力：不维护价格、币种或金额快照，不展示预计/实际/增量费用，也不设置金额授权上限；此条覆盖此前本轮作品生产讨论中的所有金额费用表述。系统仍必须保留非金额安全控制，包括生成/试听/重试前的主动确认、任务数/视频时长/TTS 字符数等资源用量、作品最长 `60秒`、幂等、防重复提交、并发限制、不自动跨模型、上游任务恢复和人工重试。此变更只作用于新作品生产范围，不改变已归档图片素材生成能力的既有数量上限和历史审计事实。
- 2026-07-14 用户确认 TTS 音色必须从动态目录选择，禁止前端、代码枚举或 migration 写死。豆包语音通过官方 `ListSpeakers 2025-05-20` 按 `ResourceID` 分页同步并缓存，Admin 支持主动/定期同步，工作台显示更新时间并可检查更新，新音色同步后自动出现。完整同步后消失的音色仅标记不可用于新生成，不删除；草稿保留失效选择并阻止生成，不静默替换，历史作品保留完整音色与参数快照。
- 2026-07-14 用户确认首版 TTS 使用 `doubao-seed-tts-2.0`（`seed-tts-2.0`）与 HTTP Chunked V3 单向流式接口 `/api/v3/tts/unidirectional`，专属 `X-Api-Key` 鉴权并记录 `X-Api-Request-Id`、`X-Tt-Logid`；音色使用 `ListSpeakers 2025-05-20` 动态目录，生成时启用 `enable_subtitle` 获取中文/英文字幕字词时间戳。字幕 Agent 负责断句/样式、供应商时间戳负责对齐，不支持时间戳的语种/方言不得伪造；ASR 仅用于已有音频转字幕，不进入新生成 TTS 主链路。
- 2026-07-14 用户确认最终成片使用 FFmpeg 合成并输出 `MP4(H.264) + AAC`；字幕默认烧录，同时独立保存 `SRT`，用户可关闭烧录仅保留外挂字幕。烧录开关、样式和字幕文件进入作品版本快照，字幕修改只重建字幕并重新合成，不重新调用 Seedance。
- 2026-07-14 用户确认作品生成必须允许选择是否使用 Seedance 原声，不固定 `generate_audio`；仅支持原声的视频模型展示该选项，页面说明原声可能同时包含不可分离的人声、BGM 和音效。选择进入作品版本快照，切换后重算提示词、字幕来源、任务计划和资源用量；原声与独立 TTS 的具体组合方式仍待继续确认。
- 2026-07-14 用户确认声音来源最终为三种模式：默认 `独立 TTS`（Seedance 无声，使用豆包 TTS/字幕时间戳）、`Seedance 原声`（不生成 TTS，字幕使用 `doubao-seed-asr-2.0` 对原声生成时间轴）、`Seedance 原声 + TTS`（原声作为不可分轨单声道背景，TTS 区间由 FFmpeg 自动压低原声）。混合模式提示双重人声风险，Agent 的“无对白/旁白”提示不保证完全生效；不支持原声的视频模型隐藏后两种模式。ASR 使用 `volc.seedasr.sauc.duration`，只处理已有或 Seedance 原声音频字幕。
- 2026-07-15 用户确认上述 6 个二级模块不得合并为一个聚合 OpenSpec change，必须分别维护为 6 个可独立评审和实施的 change：`extend-material-library-for-work-production`（素材库）、`redefine-scene-visual-generation`（画面生成）、`add-sound-subtitle-generation`（声音与字幕生成）、`add-work-generation`（作品生成）、`add-work-generation-task-management`（生成任务）、`add-work-library-management`（作品库）。每个 change 都必须独立包含 proposal、DDD/BDD/SDD/TDD design、详细规格和未执行 tasks。
- 2026-07-15 `redefine-scene-visual-generation` 已完成实施并达到 `14/14 all_done`，保持 active 等待用户明确命令后再归档：`素材管理` 数据库菜单按 `素材库 / 画面生成 / 声音与字幕生成` 排序，其中声音与字幕入口保持 planned/disabled 等待独立 change；画面生成新写路径只允许图片候选，历史 `video_draft/video_generation` 与 `video/video_task` 只读保留；作品生成只能消费按分镜排序、带 SHA-256 `input_version` 的 `SceneVisualManifest`，缺图、失败、归档、文件缺失或输入过期必须阻断，不得双写旧视频任务。
- Video Agent 前端工作台当前仅覆盖桌面端运营后台，不涉及移动端原型、移动端适配或移动端验收；后续如需要移动端，应单独提出 OpenSpec change

### 架构原则
1. `backend/` 承担控制面 API 和业务编排入口
2. 可复用 AI 能力沉淀到 `crates/*`
3. Python 只做 `services/*` sidecar/runtime
4. 业务应用放入 `apps/*`
5. video-agent 业务范围仍参考 `docs/requirements/video-agent-mvp.md`

### 开发环境
- 环境初始化必须从 `/server/docker-compose.yml` 进入，并 include `/server/ai-agent/docker-compose.yml`
- 已复用现有 PostgreSQL 服务 `biga-postgres`，本项目使用独立数据库 `video_agent`
- 已复用现有 Redis 服务 `bs-redis`，本项目使用 Redis DB index `/2`
- 当前服务端口：API `18180->8080`，Video Worker `18181->8081`，Admin `18182->3000`，Video Agent 工作台 `18183->3000`
- Compose 服务名：`ai-agent-api`、`ai-agent-video-worker`、`ai-agent-admin`、`ai-agent-video-agent`
- 本项目服务容器内工作目录统一为 `/app`
- `apps/video-agent` 本地开发默认不再通过 Compose 注入 `NEXT_PUBLIC_API_BASE_URL=http://localhost:18180`；浏览器端未显式配置 `NEXT_PUBLIC_API_BASE_URL` 时，按当前页面 `hostname` 派生同机 API 地址 `<protocol>//<hostname>:18180`，以支持 `http://<本机内网IP>:18183` 访问工作台并请求同机 API。
- 本项目禁止调用 GitNexus，包括其 skill、MCP 工具及 `gitnexus` CLI；代码探查、影响分析和调试统一以仓库文件、Git 记录和实际运行结果为依据。

### 六大Agent
1. **选题Agent**: 热点分析 + 爆款选题生成
2. **脚本Agent**: 结构化脚本 + 分镜生成
3. **素材Agent**: 语义检索 + 智能匹配
4. **视频Agent**: 多平台视频生成编排
5. **发布Agent**: 多平台自动发布
6. **优化Agent**: 数据回流 + 策略优化（Month 4）

## 记忆文件约定

1. 本文件是统一入口，具体主题记忆位于 `docs/memory/`，产品与需求文档位于 `docs/requirements/`
2. 每次新会话开始前、上下文压缩后恢复时，必须先读取本文件
3. 只记录已确认且后续会复用的信息，禁止写入临时探索、一次性报错、敏感信息
4. 重大决策变更时，同步更新本文件和对应的详细记忆文件
5. `docs/memory/` 与 `docs/requirements/` 跟随项目，可跨机器同步
