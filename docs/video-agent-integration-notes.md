# 视频 Agent 候选能力与集成记录

记录日期：2026-08-17
当前状态：集成边界与首版 Provider 已确认；尚未安装或执行任何第三方代码。

项目边界：个人、本地优先，近期不进入生产环境；当前只做剧情短剧与小说改编，不包含商品 TikTok 视频。

系统技术选型和无限画布、剪辑工作台设计见 [视频 Agent 平台技术架构](video-agent-technical-architecture.md)。

## 当前技术方向

- 使用工作流引擎管理长任务、重试、回调、人工审核和状态恢复。
- 使用 AgentScope 2.x 承载创作 Agent、Skill 路由、权限控制和服务接口。
- 使用确定性工具完成视频 API 调用、文件处理、FFmpeg 合成、存储和文件导出。
- 第一阶段采用单 Agent + Skill 路由，不立即引入多 Agent。
- 文本、视频、TTS 和 ASR 都通过 Provider Adapter 接入，业务工作流不绑定供应商 SDK。

## 候选仓库

| 候选能力 | 类型判断 | 建议职责 | 接入方式 | 当前注意事项 |
|---|---|---|---|---|
| [zy-cinematic-realism](https://github.com/popopo-99/zy-cinematic-realism) | 单一 Skill，含参考资料 | 电影化视觉、真实机位、动机光、降低 AI 感、场景负面约束 | 作为分镜完成后的可选“视觉风格增强器” | Skill 内标注 `CC-BY-NC-4.0`，商业使用前必须确认授权；不要让导演风格覆盖故事事实和连续性 |
| [seedance-2.0](https://github.com/Emily2040/seedance-2.0) | Seedance Skill 套件，含校验脚本和 JSON Schema | 把镜头规格编译为 Seedance 提示词，处理首尾帧、连续性、音频、口型、重拍和质检 | 根 Skill 作为 Seedance 路由器，按需加载子 Skill；生成 API 另做受控 Tool/MCP | 只在目标模型为 Seedance 时触发；提示词知识与真实 API 能力必须分离；MIT |
| [drama-skills](https://github.com/worldwonderer/drama-skills) | 短剧全链路 Skill 套件 | 小说分析、短剧开发、剧本、资产、分镜、图片/视频提示词、生产、独立审查 | 作为短剧项目的主流程骨架和顶层路由 | 与其他四个仓库有大量能力重叠；只选定它作为项目状态和阶段所有者；脚本执行前需审计；MIT |
| [novel-writing](https://github.com/wgwtest/novel-writing) | 单一小说写作 Skill，含文本检查脚本 | 原创小说规划、续写、修订，以及改编前的叙事和人物一致性检查 | 仅在“原创小说/长篇文本”入口或深度叙事修订时调用 | 不替代 `drama-skills` 的短剧改编和剧本输出；MIT |
| [storyboard-tiktok-video-skill](https://github.com/meowdoone/storyboard-tiktok-video-skill) | 单一商品 TikTok 分镜 Skill | 未来可能的商品短视频分镜与英文口播 | 当前不接入运行链路，仅保留候选记录 | 暂无商品 TikTok 业务；默认禁用，后续按真实需求重新评估；MIT |
| [semantic-router](https://github.com/aurelio-labs/semantic-router) | 嵌入式语义路由库 | 对已过滤的候选 Skill 做语义排序 | 作为 Agent Worker 内的可选 Python 依赖，不单独部署服务 | 只负责路由评分，不负责 Skill 文件、版本、许可证、MCP 工具或审计；首版可不启用 |

以上仓库均判断为 Agent Skill 或 Skill 套件；`semantic-router` 是路由库，不是 Skill 仓库或 MCP Server。Skill 文件、版本和权限由本项目自建 Registry 管理。

`hyukudan/ai-skills` 不作为首版依赖。若未来确实需要其 MCP/REST 能力，只能通过 `SkillRegistry` 适配器作为可选外部来源接入，不能成为业务事实源或核心路由器。

## Skill 路由设计

路由采用“确定性过滤 + 轻量排序 + 可选语义排序 + 策略裁决”四段式：

1. 根据本地配置、项目类型、当前阶段、目标模型、许可证和启用状态过滤候选 Skill。
2. 先用关键词、能力标签和优先级做本地排序。
3. 候选规模增大后，再使用 `semantic-router` 对候选项做语义排序；按需读取 `SKILL.md` 和参考资料。
4. 由平台策略检查阈值、互斥关系、输入契约和权限；低置信度或相近候选进入人工选择，不让模型任意装载 Skill。

首版由自有 `SkillRegistry` 扫描本地 Git Skill 包、读取 `manifest.yaml`、固定 commit 和许可证，并提供 `list/search/read/resolve` 接口。`SkillRouter` 先执行确定性过滤和关键词/标签匹配；未来可在 Agent Worker 内嵌 `semantic-router`，不可用时回退到本地排序和人工选择。运行记录保存最终选中的 Skill、版本、评分、路由原因和回退路径。

Skill 不直接产生小说或剧本。它负责定义创作步骤、提示上下文、输入输出 Schema、检查规则和工具权限；实际文本由 Codex 中转站或 DeepSeek 通过 `TextModelPort` 生成。每次生成保存模型、Skill 版本、提示参数、输入版本和输出版本，用户确认后的内容才进入下一阶段。

推荐的最小 `manifest.yaml`：

```yaml
name: drama-skills
version: 1.2.0
stages: [script, storyboard, review]
project_types: [short_drama]
capabilities: [episode_planning, scene_writing, shot_design]
allowed_tools: [text_model, asset_query, storyboard_write]
license: MIT
priority: 90
```

`SkillRouter` 的运行链路为：`deterministic_filter -> lexical_rank -> optional semantic_router_rank -> policy_decide`。`semantic-router` 只处理候选排序，不负责安装、更新、许可证、MCP 或审计；因此首版不需要新增 Docker Compose 服务，也不把向量模型作为启动前置条件。

## 建议工作流

### 剧情短剧分支

1. 项目初始化：输入主题、题材、受众、人物设想、时长和风格；也可选填小说、故事梗概或已有剧本作为改编素材
2. 原创故事/小说生成：AgentScope 加载 `novel-writing`，再通过 `TextModelPort` 调用 Codex 中转站或 DeepSeek，生成大纲、人物、世界观、正文或章节草稿
3. 改编分析：存在小说或故事素材时调用 `short-drama-novel-analyze`
4. 短剧开发与剧本生成：AgentScope 加载 `short-drama-develop`、`short-drama-write`，通过同一文本模型生成故事节拍、分集/分场、剧本和对白
5. 人物、场景和道具资产：`short-drama-assets`
6. 镜头与分镜：`short-drama-storyboard`
7. 电影视觉增强：按需调用 `zy-cinematic-realism`
8. 图片提示词：`short-drama-image-prompts`
9. 视频运动和音频提示词：`short-drama-video-prompts`
10. 模型编译：Seedance 目标调用 `seedance-20`；其他视频模型使用独立参数映射
11. 实际视频生成：通过统一 `VideoGenerationPort` 调用已验证的视频适配器
12. 配音与转写：Fish Audio 生成旁白，Groq ASR 生成时间戳与字幕草稿
13. 独立质检：`short-drama-review`，失败镜头进入重拍分支
14. FFmpeg 合成、字幕、音频和 MP4、SRT、工程包导出：确定性工具

### 未来候选：商品 TikTok

当前没有商品 TikTok 视频业务，不建立对应路由分支，也不把 `storyboard-tiktok-video` 纳入首版 Skill 集合。仓库信息仅作为未来候选记录；出现经过验证的商品视频需求后，再单独设计输入事实、分镜、生成和验收契约。

## 路由和优先级

同一任务只允许一个阶段所有者，其他 Skill 只能增强或校验。规则优先级如下：

1. 用户明确要求、版权限制和平台合规规则
2. 小说原文和已确认的项目状态
3. 人物、场景、道具和镜头连续性
4. 当前阶段 Skill 的输出契约
5. 视觉风格和导演参考
6. Seedance 或其他供应商的提示词语法

不得把所有 `SKILL.md` 拼接进全局系统提示词。Agent 先根据任务类型和项目阶段选择 Skill，再按 Skill 的渐进加载规则读取必要参考资料。

## AgentScope 适配设计

### Skill 层

- 将每个 Skill 固定到经过审核的 commit，不直接追踪 `main`。
- 建立统一注册信息：名称、版本、来源、许可证、触发条件、允许工具、输入和输出 Schema。
- `SKILL.md` 作为流程指令，`references/` 按需加载，避免一次性占满上下文。
- Skill 输出先转换成项目的统一结构，再交给下一阶段。

建议的统一数据对象。所有对象都必须保留稳定 ID、版本号、来源运行和确认状态，不能只把模型返回的 Markdown 作为下游输入：

- `ProjectState`：项目目标、平台、比例、时长、预算、当前阶段
- `CreativeBrief`：主题、题材、受众、人物设想、时长、风格和改编要求
- `StorySpec`：文本模型生成并经确认的故事、角色、冲突、节拍和连续性事实
- `ScriptSpec`：文本模型生成并经确认的多集短剧脚本；包含集列表、每集的场次列表、每场的镜头列表，以及动作、对白、时长和镜头前置约束
- `EpisodeSpec`：一集的编号、标题、集级目标/冲突、节拍、连续性覆盖、场次顺序和 `SceneSpec` 引用
- `SceneSpec`：一场的编号、地点、时间、出场角色、道具、场景目标、情绪变化、对白段落和 `ShotSpec` 顺序
- `AssetBible`：人物、场景、道具及不可变特征
- `ShotSpec`：镜头编号、时长、景别/构图、机位/运镜、动作、对白、首尾帧约束、声音提示和连续性引用
- `GenerationSpec`：模型、参考素材、提示词、负面约束和参数
- `TakeReview`：生成结果、缺陷、是否重拍及修订指令

层级关系固定为 `Project -> Episode -> Scene -> Shot`。项目级角色、场景、道具和视觉风格是跨集连续性基线；`EpisodeSpec` 或 `SceneSpec` 可以声明覆盖，但覆盖必须显式记录并传递到相关 `ShotSpec`、Agent 上下文和运行审计。故事板、工作流和剪辑台读取同一份 `EpisodeSpec`/`SceneSpec`/`ShotSpec` 事实，不为展示视图复制镜头数据。

`short-drama-storyboard` 的输出必须按上述层级组织：先生成集/场/镜头索引，再生成每个镜头的画面和生成约束。默认支持一部短剧多集、每集多场、每场多个镜头；允许用户逐集确认、批量生成某一场或某一集，并把指定集的镜头装入该集独立时间线。任何跨场移动、集级连续性变更或批量替换都必须产生可追踪的版本和审计记录。

### Provider Adapter 层

| 能力 | 候选 | 统一接口 | 首版策略 |
|---|---|---|---|
| 文本推理 | Codex 中转站（默认）、DeepSeek | `TextModelPort` | OpenAI 兼容 Provider 同步 `/v1/models`，也允许手工建模；按全局或项目配置切换 |
| 图片生成 | GPT Image 2 中转站（`gpt-image-2`） | `ImageGenerationPort` | 文生图与编辑统一适配；参考图、多图输入、遮罩、尺寸、质量和输出格式按能力声明开放 |
| 视频生成 | Agnes AI（首接）、MiniMax H3、Seedance 2.5 | `VideoGenerationPort` | 首版按 Adapter 发布时冻结的 capability snapshot 验收 Agnes 实际启用能力；其他 Provider 按同一能力契约后续添加 |
| TTS | Fish Audio | `TtsPort` | 生成音频资产，并记录音色、采样率、时长和请求 ID |
| ASR | Groq | `AsrPort` | 生成分段文本、时间戳和字幕草稿 |

Agnes AI 使用 `https://apihub.agnes-ai.com/v1`，通过 `POST /v1/videos` 创建异步视频任务，并通过 `GET /v1/videos/{video_id}` 查询。首版以 capability snapshot 记录 `agnes-video-v2.0` 与 `agnes-video-2.5` 的模型、文档/目录来源、探测时间、账号可用性、参数 Schema 和不支持时的降级行为；预览能力通过 capability probe 和功能开关控制，不假设账户必然可用。

GPT Image 2 中转站通过 `ImageGenerationPort` 接入，首版模型标识为 `gpt-image-2`。Base URL、模型标识、参数 Schema 和默认值由数据库配置；文生图、参考图、多图输入、局部/遮罩编辑、透明背景、尺寸、质量、批量数量和输出格式按实时能力声明启用。中转站返回的 URL 或 base64 图片先在隔离临时目录完成 MIME、尺寸、checksum 和安全校验，再上传火山引擎 TOS 并登记为 `AssetVersion`，同时保存提示词版本、输入图片版本、模型、参数和 request ID，供分镜、关键帧和视频节点追踪引用。

MiniMax H3 与 Seedance 2.5 是后续可配置的视频 Provider。所有 Provider、模型、能力和参数 Schema 均由界面管理，支持新增、编辑、启停、连通性测试和模型同步，不写死在业务流程中。

每次调用统一记录 provider、model、base URL、request ID、参数摘要、状态、错误码、耗时、可获得的费用或估算，以及输入输出资产。API 密钥使用 AES-256-GCM 加密后写入 PostgreSQL；主加密密钥由 Docker Secret 挂载。界面只显示类似 `sk********jjjj` 的掩码，完整密钥不可回显，只允许替换或轮换。密钥不得进入模型上下文、Skill 文本、SSE 事件或普通日志。

### Tool/MCP 层

Skill 负责“怎样思考和生成规格”，Tool/MCP 负责“执行可验证动作”。计划补充：

- 参考素材抓取工具
- 火山引擎 TOS 分片/断点上传、短期签名访问、对象校验与素材版本工具
- 图片生成工具
- 视频生成提交、查询、取消和结果下载工具（Agnes 首接；MiniMax H3、Seedance 2.5 后续接入）
- 媒体元数据、抽帧和 FFmpeg 合成工具
- 字幕、TTS、音乐和响度处理工具
- MP4、SRT、轻量工程包和完整工程包导出工具

对外部稳定服务可以使用 MCP；对项目内部 Python 能力，优先使用 AgentScope `ToolBase`，减少不必要的协议层。

## 安全与许可证门槛

- 当前不执行任何仓库中的 `scripts/`、安装脚本或依赖。
- 接入前逐项检查网络访问、子进程、文件写入、环境变量和密钥读取。
- 第三方脚本放入隔离 Workspace，限制目录、网络和执行时间。
- 写文件、付费生成和删除操作必须经过本地策略检查；高成本生成保留人工确认。
- API 密钥只注入对应工具，不能进入模型上下文或 Skill 文本。
- `zy-cinematic-realism` 当前为非商业许可；个人试用仍需遵守其许可，不对外分发为默认集成。
- 记录每次生成所用的 Skill commit、模型、参数、输入素材和输出资产，便于复现和审计。

## 分阶段实施

1. 定义统一的项目状态和镜头 JSON Schema。
2. 接入 `novel-writing` 与 `drama-skills`，跑通“创意 -> 故事/小说 -> 短剧剧本 -> 分镜 -> 视频提示词 -> 审查”的纯文本链路。
3. 跑通默认 Codex 中转站的 OpenAI 兼容文本适配器，完成 `/v1/models` 同步、手工建模、故事生成、剧本生成和结构化输出验证。
4. 接入 GPT Image 2 中转站，完成角色/场景/分镜图生成、参考图编辑、资产校验后入 TOS 与可追踪性验证。
5. 接入 Agnes AI，按适配器发布时冻结的 capability snapshot 验收账号实际启用的视频模式、异步任务状态和结果下载；预览能力按实时探测结果启用，不把“全部公开能力”作为不变承诺。
6. 接入 Fish Audio、Groq ASR、火山引擎 TOS、StoragePort、本地临时工作区、抽帧、FFmpeg 与人工审核。
7. 建立故事和剧本的版本、人工确认与局部重写流程；商品 TikTok 分支不在当前范围。
8. 建立固定样例、成本指标和质量评测后，再按需接入 MiniMax H3、Seedance 2.5 和多 Agent。
9. 解决许可后，再启用 `zy-cinematic-realism` 对外分发或商业用途。

## 画布素材上下文对话

用户选中画布素材后，右侧 Agent 面板自动绑定当前节点、`AssetVersion`、上下游引用和生成记录。图片支持参考图与遮罩，视频/音频支持时间范围，文本支持选段。AgentScope 必须先输出结构化 `AssetEditPlan` 和费用估算，不能直接修改数据库、覆盖文件或执行自由格式 FFmpeg 命令。

计划执行由 `AssetEditWorkflow` 调用对应 Port：文本生成新结构化版本，图片调用 `ImageGenerationPort.edit`，视频使用编辑/参考能力或重生成候选镜头，Fish Audio 重生成音频片段，时间线生成可撤销领域命令。所有结果先成为候选版本；服务端先生成 `impactAnalysis`，用户接受时选择替换当前镜头、当前场次、当前集或明确勾选的引用集合。已发布版本和历史运行只读，基础版本冲突返回 `409`；文本或连续性修改会把受影响的下游 Spec、AssetVersion、Shot 和 TimelineClip 标记为 `stale`，不会静默替换时间线。

## 已确认约束

- 当前为个人本地项目，以剧情短剧和小说改编为主，不进入生产环境，不开发手机端。
- 小说故事和短剧剧本由文本模型结合 `novel-writing`、`drama-skills` 生成；用户可从创意开始，导入现成小说或剧本仅是可选入口。
- 使用 Docker Compose 启动；数据库为 PostgreSQL，媒体主对象保存在火山引擎 TOS，本地磁盘只保存上传分片、FFmpeg 工作文件和可清理缓存。
- TOS Bucket、Region、Endpoint、认证方式和凭据可配置；默认私有桶与短期签名 URL。PostgreSQL 只保存对象引用、checksum/ETag 和媒体元数据，不保存媒体二进制。
- Codex 中转站是默认文本 Provider，兼容完整 OpenAI API；DeepSeek 为可选 Provider。
- GPT Image 2 中转站是首个图片 Provider，通过可配置的 `ImageGenerationPort` 接入。
- Agnes AI 是首个视频 Provider，并覆盖官方文档公开的全部视频能力；MiniMax H3、Seedance 2.5 通过统一模型管理中心后续接入。
- Fish Audio 用于 TTS，Groq 用于 ASR。
- 所有 Provider 与模型均可从界面新增、编辑、启停、测试和同步；支持全局模型库与项目级默认值、参数覆盖。
- 只导出 MP4、SRT、轻量工程包和完整工程包，不接入内容发布平台。
- API 密钥密文入库，主密钥由 Docker Secret 提供；界面默认掩码且不支持完整密钥回显。
