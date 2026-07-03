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

系统 SHALL 提供脚本 Agent API，将用户输入的选题转换为包含标题、hook 和有序分镜的结构化脚本。

#### Scenario: 用户从选题生成脚本

- **GIVEN** 用户已创建内容项目
- **WHEN** 用户提交 `project_id`、`topic`、`style` 和 `scene_count`
- **THEN** 系统 SHALL 返回新建脚本 ID、标题、hook、状态和有序分镜列表
- **AND** 系统 SHALL 将脚本保存到 `scripts`
- **AND** 系统 SHALL 将分镜保存到 `scenes`

#### Scenario: 用户生成 A/B 版本

- **GIVEN** 已存在一个脚本
- **WHEN** 用户提交相同选题并传入 `parent_id`
- **THEN** 系统 SHALL 创建一个新脚本
- **AND** 新脚本的 `parent_id` SHALL 指向原脚本

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

