# script-agent-mvp Specification Delta

## ADDED Requirements

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
