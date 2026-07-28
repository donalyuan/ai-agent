# script-agent-mvp Specification

## Purpose
TBD - created by archiving change script-agent-mvp. Update Purpose after archive.
## Requirements
### Requirement: 初始数据库 schema 可重放

系统 SHALL 提供 SQLx 初始迁移，用于在空 PostgreSQL 数据库中创建 video-agent MVP 所需的基础业务表、脚本表、分镜表、任务表、发布数据表和 Agent 运行日志表。

#### Scenario: 在空数据库执行初始迁移

- **GIVEN** 一个只包含 PostgreSQL 系统表的空数据库
- **WHEN** 执行 `backend/migrations/20260701000000_initial_schema.sql`
- **THEN** 数据库 SHALL 创建 `projects`、`accounts`、`materials`、`material_embeddings`、`scripts`、`scenes`、`generation_tasks`、`videos`、`publish_tasks`、`metrics`、`revenues`、`agent_runs`、`agent_steps`、`viral_videos`、`content_strategies`
- **AND** `scripts.project_id` SHALL 外键关联 `projects.id`
- **AND** `scenes.script_id` SHALL 外键关联 `scripts.id`

#### Scenario: 迁移保留核心查询索引

- **GIVEN** 初始迁移已经执行成功
- **WHEN** 系统检查数据库索引
- **THEN** 数据库 SHALL 包含 `idx_materials_project`
- **AND** 数据库 SHALL 包含 `idx_scripts_project`
- **AND** 数据库 SHALL 包含 `idx_scenes_script`
- **AND** 数据库 SHALL 包含 `idx_generation_tasks_status`
- **AND** 数据库 SHALL 包含 `idx_publish_tasks_status`
- **AND** 数据库 SHALL 包含 `idx_agent_runs_type`

### Requirement: 脚本与分镜数据约束明确

系统 SHALL 在数据库层约束脚本状态、分镜顺序和分镜时长，避免下游视频生成链路接收到不可识别的脚本结构。

#### Scenario: 脚本状态只允许已知枚举

- **GIVEN** 初始迁移已经执行成功
- **WHEN** 插入或更新 `scripts.status`
- **THEN** 数据库 SHALL 只接受 `draft`、`approved`、`archived`

#### Scenario: 同一脚本内分镜顺序唯一

- **GIVEN** 初始迁移已经执行成功
- **WHEN** 向 `scenes` 插入相同 `script_id` 和 `sequence` 的两条记录
- **THEN** 数据库 SHALL 拒绝第二条重复记录

### Requirement: 脚本 Agent API 生成结构化脚本

系统 SHALL 提供脚本 Agent API，将用户输入或已确认选题转换为包含标题、hook 和有序分镜的结构化脚本。脚本生成请求 MAY 携带 `topic_id`；当携带 `topic_id` 时，系统 SHALL 校验选题归属、选题状态，并在生成成功后保存选题关联和选题快照。

#### Scenario: 用户从已确认选题生成脚本

- **GIVEN** 数据库中存在一个项目
- **AND** 该项目下存在一条 `approved` 选题
- **WHEN** 用户提交 `project_id`、`topic_id`、`style` 和 `scene_count`
- **THEN** 系统 SHALL 返回新建脚本 ID、标题、hook、状态和有序分镜列表
- **AND** 系统 SHALL 将脚本保存到 `scripts`
- **AND** `scripts.topic_id` SHALL 指向该选题
- **AND** `scripts.content.topic_snapshot` SHALL 保存生成时的选题快照
- **AND** 系统 SHALL 将该选题状态更新为 `scripted`

#### Scenario: 非 approved 选题不能生成脚本

- **GIVEN** 数据库中存在一条状态为 `idea` 或 `archived` 的选题
- **WHEN** 用户使用该 `topic_id` 请求生成脚本
- **THEN** 系统 SHALL 拒绝请求
- **AND** 系统 SHALL NOT 创建脚本
- **AND** 系统 SHALL NOT 更新选题状态

#### Scenario: 选题与项目不匹配时拒绝生成

- **GIVEN** 数据库中存在项目 A 和项目 B
- **AND** 项目 A 下存在一条 `approved` 选题
- **WHEN** 用户使用项目 B 的 `project_id` 和项目 A 的 `topic_id` 请求生成脚本
- **THEN** 系统 SHALL 拒绝请求
- **AND** 系统 SHALL NOT 创建脚本

### Requirement: LLM 客户端支持 Responses API

脚本 Agent 的 OpenAI-compatible LLM 客户端 SHALL 支持通过 Responses API 生成结构化脚本，同时保留现有 Chat Completions 兼容能力。

#### Scenario: 使用 Responses endpoint 生成脚本文本

- **GIVEN** `OPENAI_BASE_URL` 指向以 `/responses` 结尾的 endpoint
- **WHEN** 脚本 Agent 请求 LLM 生成脚本
- **THEN** 客户端 SHALL 直接 POST `OPENAI_BASE_URL`
- **AND** 请求体 SHALL 使用 Responses API 的 `input` 消息结构
- **AND** 请求体 SHALL 约束输出为 JSON object
- **AND** 客户端 SHALL 从 `output[].content[].text` 提取非空文本返回给脚本解析器

#### Scenario: 保留 Chat Completions 兼容模式

- **GIVEN** `OPENAI_BASE_URL` 未以 `/responses` 结尾
- **WHEN** 脚本 Agent 请求 LLM 生成脚本
- **THEN** 客户端 SHALL 继续 POST `{OPENAI_BASE_URL}/chat/completions`
- **AND** 客户端 SHALL 从 `choices[].message.content` 提取非空文本

### Requirement: Responses API 推理参数可配置

脚本 Agent 的 Responses API 客户端 SHALL 允许通过环境变量调整推理强度和最大输出 token，避免需要修改 Rust 代码才能切换模型运行参数。

#### Scenario: 配置推理强度和输出上限

- **GIVEN** `OPENAI_REASONING_EFFORT` 设置为非空且不是 `none`
- **AND** `OPENAI_MAX_OUTPUT_TOKENS` 设置为正整数
- **WHEN** 客户端调用 Responses API
- **THEN** 请求体 SHALL 包含 `reasoning.effort` 且值等于 `OPENAI_REASONING_EFFORT`
- **AND** 请求体 SHALL 包含 `max_output_tokens` 且值等于 `OPENAI_MAX_OUTPUT_TOKENS`

#### Scenario: 关闭 reasoning 字段

- **GIVEN** `OPENAI_REASONING_EFFORT` 设置为 `none`
- **WHEN** 客户端调用 Responses API
- **THEN** 请求体 SHALL 不包含 `reasoning` 字段

### Requirement: xhigh 推理等级使用分步脚本生成

当脚本 Agent 使用 Responses API 且 `OPENAI_REASONING_EFFORT` 为 `xhigh` 时，系统 SHALL 使用分步串行生成模式生成结构化脚本，避免单次完整脚本请求触发供应商上游超时或 `502 upstream_error`。

#### Scenario: xhigh 下生成完整脚本

- **GIVEN** `OPENAI_REASONING_EFFORT` 设置为 `xhigh`
- **AND** 用户已创建内容项目
- **WHEN** 用户提交 `project_id`、`topic`、`style` 和 `scene_count` 到 `POST /api/scripts/generate`
- **THEN** 系统 SHALL 先请求 LLM 生成 `title` 和 `hook`
- **AND** 系统 SHALL 按分镜序号串行请求 LLM 生成单个 `scene`
- **AND** 系统 SHALL 聚合为一个完整脚本响应
- **AND** 响应结构 SHALL 与非分步生成模式保持一致
- **AND** 系统 SHALL 将脚本保存到 `scripts`
- **AND** 系统 SHALL 将所有分镜保存到 `scenes`

#### Scenario: xhigh 下保持分镜顺序和数量

- **GIVEN** `OPENAI_REASONING_EFFORT` 设置为 `xhigh`
- **WHEN** 用户请求生成 `N` 个分镜，其中 `N` 在 3 到 12 范围内
- **THEN** 系统 SHALL 返回严格 `N` 个分镜
- **AND** 分镜 `sequence` SHALL 从 1 到 `N` 连续递增
- **AND** 任一单分镜输出序号不匹配时，系统 SHALL 视为无效 LLM 输出并重试该步骤

#### Scenario: 非 xhigh 配置保留完整生成路径

- **GIVEN** `OPENAI_REASONING_EFFORT` 未设置为 `xhigh`
- **WHEN** 用户请求生成脚本
- **THEN** 系统 SHALL 保留现有完整脚本一次性生成路径
- **AND** 系统 SHALL 不强制拆分为单分镜请求

### Requirement: Full Crew ScriptPackage 必须确定性晋升为正式脚本

Full Crew 的 screenwriter 输出 SHALL 包含正式脚本映射所需的 `title`、`hook`，以及每个 scene 的 `sequence`、`narration`、`visual_description`、`emotion` 和 `duration_sec`；系统 SHALL 对 StoryBible、CharacterBible 和 ScriptDraft 组成的精确 ScriptPackage 做完整 schema 与引用校验，并 SHALL 在包级批准后以零额外模型调用确定性创建正式脚本和分镜。

#### Scenario: 编剧输出满足正式字段契约

- **WHEN** screenwriter 完成 Full Crew role step
- **THEN** ScriptDraft SHALL 包含非空 title、hook 和有序 scenes
- **AND** 每个 scene SHALL 包含正式 Scene 所需全部字段并满足顺序、数量和时长约束
- **AND** StoryBible、CharacterBible 和 ScriptDraft SHALL 关联同一 role attempt、ModelCall 和 package 版本

#### Scenario: 正式字段缺失

- **WHEN** ScriptDraft 缺少任一正式字段、scene 顺序不连续或字段违反领域约束
- **THEN** role step SHALL 失败并记录 schema 错误
- **AND** 系统 SHALL NOT保存部分 ScriptPackage
- **AND** 系统 SHALL NOT创建正式脚本或调用另一个 LLM 修补输出

#### Scenario: 批准 ScriptPackage 后晋升

- **GIVEN** 当前 ScriptPackage digest 已通过人工 Gate 且来源选题仍可晋升
- **WHEN** 系统执行 ScriptPackagePromotion
- **THEN** 系统 SHALL 创建状态为 `approved` 的正式 Script 和全部 Scene
- **AND** Script SHALL 保存 project_id、topic_id、topic_snapshot、production ID、package digest 和来源产物引用
- **AND** 系统 SHALL NOT要求操作者再次批准同一脚本
- **AND** 晋升过程 SHALL NOT调用模型

#### Scenario: 晋升操作重复提交

- **GIVEN** 相同 ProductionRun、ScriptPackage digest 和晋升幂等键已经成功创建正式脚本
- **WHEN** 客户端重试晋升命令
- **THEN** 系统 SHALL 返回原 Script 和 Scene 集合
- **AND** 系统 SHALL NOT创建第二个脚本、分镜或新版本

#### Scenario: 旧 ScriptPackage 不得晋升

- **GIVEN** ScriptPackage 获批后任一组成产物产生新版本
- **WHEN** 客户端请求晋升旧 package digest
- **THEN** 系统 SHALL 返回 `stale_package`
- **AND** 系统 SHALL NOT创建或修改正式 Script/Scene

### Requirement: Full Crew 后续产物不得静默修改已批准脚本

正式脚本晋升后，Director 和其他制作角色 SHALL 使用真实 Script/Scene ID 作为输入和引用；需要改变旁白、分镜语义或 Scene 结构时，系统 SHALL 通过现有脚本版本关系创建新的 Script，而 SHALL NOT直接覆盖已批准 Script/Scene。

#### Scenario: ShotContract 引用正式 Scene

- **GIVEN** ScriptPackage 已晋升为正式 Script/Scene
- **WHEN** Director 生成 ShotContract
- **THEN** 每个 ShotContract SHALL 引用存在且属于该 Script 的真实 `scenes.id`
- **AND** 系统 SHALL 拒绝自由字符串、跨脚本 Scene 或无法解析的 scene reference

#### Scenario: 导演修改已批准脚本语义

- **WHEN** Director 建议改变已批准旁白、Scene 顺序或核心叙事内容
- **THEN** 系统 SHALL 要求创建带 `parent_id` 的新 Script 版本并重新经过相应 Gate
- **AND** 原 Script、Scene、来源快照和下游审计 SHALL 保持不变
- **AND** 依赖旧 Script 的 ProductionPackage 和 WorkPlan SHALL 失效

#### Scenario: 脚本语义修订必须重新形成 ScriptPackage

- **GIVEN** 正式 Script 已由 Full Crew 晋升
- **WHEN** 操作者接受旁白、Scene 顺序、Scene 结构或核心叙事修改
- **THEN** 系统 SHALL 创建新的 script revision epoch 并由 screenwriter 生成 StoryBible、CharacterBible 和 ScriptDraft 一致版本集合
- **AND** 新 ScriptPackage SHALL 重新通过包级审批后才能确定性晋升
- **AND** 新 Script SHALL 以 `parent_id` 引用原 Script
- **AND** Director 或其他下游角色 SHALL NOT直接写入 Script/Scene

#### Scenario: 新脚本晋升使旧下游失效

- **GIVEN** 新 ScriptPackage 已批准并成功晋升为子 Script
- **WHEN** 系统提交晋升事务
- **THEN** 旧 Script SHALL 保持 approved 历史事实且不得被覆盖
- **AND** 当前 ProductionRun SHALL 将正式脚本关联切换到新 Script
- **AND** 依赖旧 Script 的 ProductionPackage、SceneVisualManifest 关联、WorkVersion 草稿和未确认 WorkPlan SHALL 失效
- **AND** 已确认或已运行的 WorkVersion SHALL 保持不可变并进入显式重新制作决策

### Requirement: Full Crew ScriptPackage reject 必须保持来源和事务边界

ScriptPackage 被拒绝时，系统 SHALL 保留原 package、GateDecision、role attempt 和 ModelCall，创建新的 screenwriter revision step，并 SHALL NOT修改 Topic、创建正式 Script/Scene 或复用旧 approval。达到固定修订上限后 SHALL 停止模型调用并要求取消或新建制作意图。

#### Scenario: 拒绝后生成新 ScriptPackage

- **WHEN** 操作者以非空理由拒绝当前 ScriptPackage
- **THEN** 原 package SHALL 保持不可变
- **AND** 新 screenwriter attempt SHALL 使用新的 revision epoch 和独立资源预占
- **AND** 只有新 package digest 的批准才能触发晋升

#### Scenario: 重放旧 ScriptPackage approval

- **GIVEN** ScriptPackage 已被 reject 且存在更新 revision epoch
- **WHEN** 客户端重放旧 digest 的 approve 或 promotion 命令
- **THEN** 系统 SHALL 返回 `stale_package`
- **AND** 系统 SHALL NOT修改当前 revision 或 Topic 状态

