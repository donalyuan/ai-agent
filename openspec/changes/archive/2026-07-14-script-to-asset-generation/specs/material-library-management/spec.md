# material-library-management Specification Delta

## Purpose

扩展素材库对 AI 生成图片素材的稳定存储、来源标记和候选复用要求。

## ADDED Requirements

### Requirement: AI 生成图片必须作为素材库资产持久化

系统 SHALL 将成功生成的 AI 图片保存为素材库资产，并保留稳定访问 URL 和生成来源信息。

#### Scenario: AI 图片候选入库

- **GIVEN** worker 成功生成并下载 AI 图片候选
- **WHEN** 系统写入素材库
- **THEN** 系统 SHALL 创建 `material_type=image` 的 `materials` 记录
- **AND** `file_url` SHALL 指向自管素材存储的稳定 URL
- **AND** `metadata.source` SHALL 为 `ai_generated`
- **AND** `metadata.generation_task_id` SHALL 记录来源任务
- **AND** `metadata.source_scene_id` SHALL 记录来源分镜

#### Scenario: 未选候选保留为素材

- **GIVEN** 某 AI 图片候选已入库但未被选为分镜主素材
- **WHEN** 操作者查看素材库
- **THEN** 系统 SHALL 仍保留该素材
- **AND** `metadata.candidate_status` SHALL 表示该素材是未选候选

### Requirement: 自管素材存储必须提供稳定访问前缀

系统 SHALL 使用本地持久化卷保存第一版 AI 生成图片，并通过 API 静态访问前缀提供稳定 URL。

#### Scenario: 本地持久化存储

- **GIVEN** worker 处理 AI 图片生成结果
- **WHEN** 图片内容下载成功
- **THEN** worker SHALL 将图片写入本地持久化素材目录
- **AND** API SHALL 通过 `/assets/...` 提供访问
- **AND** `materials.metadata.storage_provider` SHALL 为 `local`

#### Scenario: 不保存供应商临时 URL

- **GIVEN** 供应商返回临时图片 URL
- **WHEN** 系统创建素材库记录
- **THEN** `materials.file_url` SHALL NOT 保存该供应商临时 URL
- **AND** `materials.file_url` SHALL 保存自管素材存储 URL
