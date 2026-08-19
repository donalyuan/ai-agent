## ADDED Requirements

### Requirement: 项目 HTTP API

API SHALL 提供 `POST /v1/projects`、`GET /v1/projects` 和 `GET /v1/projects/{project_id}`。请求与响应 SHALL 使用 camelCase JSON，并返回可验证的 HTTP Project DTO 契约字段。

共享 `packages/contracts` Schema 是领域/持久化边界，字段 `schema_version` 保留 snake_case；HTTP DTO 的 `schemaVersion` 是显式传输层映射，不要求 HTTP JSON 直接作为共享 Schema 实例校验。

#### Scenario: 创建并读取项目

- **WHEN** 客户端 POST 合法 `{ "name": "Demo" }` 后 GET 返回的 ID
- **THEN** POST 返回 201，GET 返回 200，并包含相同 name、draft status、schemaVersion 和 revision

#### Scenario: 无效项目请求

- **WHEN** 客户端 POST 缺少 name 或传入空白 name
- **THEN** API 返回 422，错误体包含稳定的 validation 错误类型

### Requirement: 剧集 HTTP API

API SHALL 提供 `POST /v1/projects/{project_id}/episodes`、`GET /v1/projects/{project_id}/episodes` 和 `GET /v1/episodes/{episode_id}`。Episode JSON SHALL 使用 `projectId`、`number`、`title`、`schemaVersion` 和 `revision`。

#### Scenario: 创建并列出剧集

- **WHEN** 客户端为存在的项目 POST 合法 `{ "number": 1, "title": "Opening" }`
- **THEN** API 返回 201，项目剧集列表返回该 Episode 且不包含其他项目的记录

#### Scenario: 剧集父级不存在

- **WHEN** 客户端向不存在的 project ID POST Episode
- **THEN** API 返回 404，错误体 type 为 `project_not_found`

### Requirement: If-Match 并发更新

API SHALL 提供 `PATCH /v1/projects/{project_id}` 和 `PATCH /v1/episodes/{episode_id}`，并要求 `If-Match` header 携带十进制 revision。服务端 SHALL 将冲突映射为 409，且不得静默覆盖。

#### Scenario: 版本匹配更新

- **WHEN** PATCH 携带当前 `If-Match` revision 和合法字段
- **THEN** API 返回 200，revision 增加 1，并返回更新后的对象

#### Scenario: 版本冲突

- **WHEN** PATCH 携带过期或格式非法的 `If-Match`
- **THEN** API 返回 409，错误体 type 为 `revision_conflict`，且对象保持未变

### Requirement: 数据库不可用边界

未配置或无法连接业务数据库时，API SHALL 保持 health 端点可启动，但项目/剧集业务端点 SHALL 返回 `503 database_unavailable`，不得静默使用内存数据或真实外部服务。

#### Scenario: 无数据库配置启动

- **WHEN** API 在没有 `DATABASE_URL` 的本地环境启动并请求项目端点
- **THEN** health/live 可返回 200，项目端点返回 503 和 database_unavailable
