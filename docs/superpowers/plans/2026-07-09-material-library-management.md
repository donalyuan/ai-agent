# Material Library Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在视频工作台实现“素材管理 > 素材库”第一版画布优先工作台，支持 URL 素材登记、可选缩略图 URL、筛选、节点查看、编辑、软归档和恢复。

**Architecture:** 后端沿用 `materials` 表作为素材库聚合根，新增 `material_repository` 封装查询和状态流转，`lib.rs` 暴露 project-scoped 列表/创建 API 和 material-scoped 详情/更新/状态 API。画布节点由前端根据素材列表派生，不新增画布节点或连线持久化模型。前端新增独立页面级 `MaterialLibraryPage`，`app/page.tsx` 只做状态编排和菜单分发；正式实现前必须先完成 OpenSpec change 与 Pencil 原型确认。

**Tech Stack:** Rust + Axum + SQLx + PostgreSQL JSONB；Next.js 14 + TypeScript + Vitest + Playwright；OpenSpec spec-driven workflow；Pencil MCP prototype gate。

---

## Execution Rules

- 不执行 `git add`、`git commit`、`git push`，除非用户在执行阶段明确确认。
- 不修改任何已应用旧 migration，只新增递增 migration。
- 前端代码实现前必须更新 `docs/prototypes/video-agent/video-agent.pen` 并等待用户明确确认原型。
- OpenSpec tasks 每完成一组就同步勾选。

## File Map

- Create: `openspec/changes/material-library-management/proposal.md`，素材库管理 change 提案。
- Create: `openspec/changes/material-library-management/design.md`，复制并收敛已确认设计。
- Create: `openspec/changes/material-library-management/specs/material-library-management/spec.md`，素材库管理规格。
- Create: `openspec/changes/material-library-management/tasks.md`，实现任务清单。
- Modify: `docs/prototypes/video-agent/video-agent.pen`，新增“素材管理 > 素材库”正式原型。
- Create: `backend/migrations/20260709010000_material_library_management.sql`，扩展素材类型、状态和索引，启用素材库菜单。
- Create: `backend/src/repositories/material_repository.rs`，素材聚合、过滤、创建、更新和状态流转。
- Modify: `backend/src/repositories/mod.rs`，导出素材仓储类型。
- Modify: `backend/src/agents/models/request.rs`，新增素材请求/响应 DTO、枚举和校验。
- Modify: `backend/src/agents/models/mod.rs`，导出素材 DTO。
- Modify: `backend/src/lib.rs`，新增素材 repository accessor、路由、handler、错误映射。
- Modify: `backend/tests/database_migrations.rs`，覆盖素材 schema 和菜单迁移。
- Create: `backend/tests/material_repository_contract.rs`，覆盖素材仓储契约。
- Create: `backend/tests/material_routes.rs`，覆盖素材 API。
- Modify: `apps/video-agent/app/lib/api.ts`，新增素材类型和 API wrapper。
- Modify: `apps/video-agent/app/lib/api.test.ts`，覆盖素材 API wrapper。
- Create: `apps/video-agent/app/pages/material-library/materialModel.ts`，前端素材标签、表单、payload helper。
- Create: `apps/video-agent/app/pages/material-library/MaterialLibraryPage.tsx`，画布优先的素材画布工作台页面。
- Modify: `apps/video-agent/app/page.tsx`，接入素材状态、加载、保存、归档/恢复和菜单分发。
- Modify: `apps/video-agent/app/page.test.tsx`，覆盖素材库页面交互。
- Modify: `apps/video-agent/app/styles.css`，新增画布优先的素材画布工作台样式。
- Modify: `apps/video-agent/e2e/workspace.spec.ts`，覆盖进入素材库和画布骨架。

---

### Task 1: OpenSpec Artifacts And Pencil Prototype Gate

**Files:**
- Create: `openspec/changes/material-library-management/proposal.md`
- Create: `openspec/changes/material-library-management/design.md`
- Create: `openspec/changes/material-library-management/specs/material-library-management/spec.md`
- Create: `openspec/changes/material-library-management/tasks.md`
- Modify: `docs/prototypes/video-agent/video-agent.pen`

- [ ] **Step 1: Create OpenSpec proposal**

Create `openspec/changes/material-library-management/proposal.md`:

```markdown
# material-library-management

## Summary

实现视频工作台 Phase 3 第一版“素材管理 > 素材库”：当前账号下的 URL 素材登记、筛选、编辑、软归档和恢复。

## Motivation

脚本创作之后需要稳定的素材资产入口。当前数据库已有 `materials` 和 `material_embeddings` 表，但缺少素材管理页面、API 和仓储逻辑；在进入素材检索 Agent、Embedding、分镜候选前，应先建立素材库基础管理闭环。

## Scope

- 启用“素材管理”一级菜单并新增二级“素材库”。
- 扩展 `materials.material_type` 支持 `subtitle`。
- 新增 `materials.status` 支持 `active` 和 `archived`。
- 提供素材列表、详情、创建、更新、归档和恢复 API。
- 支持可选手动缩略图 URL；图片素材未配置时可使用 `file_url` 预览，其他类型显示类型占位。
- 提供画布优先的素材画布工作台页面：主画布占据素材库主工作区，资产栏和详情编辑作为画布上的辅助浮层或窄面板，底部提供轻量画布工具栏。

## Out Of Scope

- 文件上传、对象存储或本地文件存储。
- 自动抓取远程素材元数据。
- 自动抽取视频帧、生成音频波形或抓取封面图。
- 画布节点位置持久化、节点连线语义、任务编排或 DAG。
- Embedding、Milvus、语义检索和素材检索 Agent。
- 分镜素材候选、素材清单确认和作品生产读取素材清单。
- 发布平台素材同步和移动端适配。
```

- [ ] **Step 2: Create OpenSpec design**

Copy the approved design from `docs/superpowers/specs/2026-07-09-material-library-management-design.md` into `openspec/changes/material-library-management/design.md`.

- [ ] **Step 3: Create OpenSpec spec**

Create `openspec/changes/material-library-management/specs/material-library-management/spec.md`:

```markdown
## ADDED Requirements

### Requirement: 素材库必须支持 URL 素材登记

系统 SHALL 允许操作者在当前账号下登记已有素材 URL，并保存文件名、类型、标签、元数据、使用次数、状态和可选缩略图 URL。

#### Scenario: 创建视频素材

- **GIVEN** 当前账号存在
- **WHEN** 操作者提交文件名、`material_type=video`、合法 `file_url`、可选 `thumbnail_url` 和标签
- **THEN** 系统 SHALL 创建 `active` 素材
- **AND** 响应 SHALL 返回素材 ID、账号 ID、文件名、类型、URL、标签、metadata、`thumbnail_url`、`usage_count=0`、状态和时间字段

#### Scenario: 创建字幕素材

- **GIVEN** 当前账号存在
- **WHEN** 操作者提交 `material_type=subtitle` 且 `file_url` 指向字幕文件 URL
- **THEN** 系统 SHALL 创建字幕素材
- **AND** metadata SHALL 可保存字幕语言和字幕格式

#### Scenario: 缩略图展示

- **GIVEN** 当前账号下存在素材
- **WHEN** 素材配置了合法 `thumbnail_url`
- **THEN** 页面 SHALL 在资产栏、画布节点和详情中展示该缩略图
- **AND** 图片素材未配置 `thumbnail_url` 时，页面 SHALL 可使用 `file_url` 作为缩略图
- **AND** 音频或字幕素材未配置 `thumbnail_url` 时，页面 SHALL 显示类型占位

### Requirement: 素材库必须支持筛选和默认可用列表

系统 SHALL 提供当前账号下的素材列表查询，默认只返回 `active` 素材，并支持按类型、状态、关键词和标签筛选。

#### Scenario: 默认列表只展示可用素材

- **GIVEN** 当前账号下存在 `active` 和 `archived` 素材
- **WHEN** 操作者打开素材库且未显式选择状态
- **THEN** 页面和 API SHALL 只返回 `active` 素材

#### Scenario: 查看归档素材

- **GIVEN** 当前账号下存在 `archived` 素材
- **WHEN** 操作者选择状态筛选“已归档”
- **THEN** 页面和 API SHALL 展示归档素材

### Requirement: 素材库必须支持编辑、归档和恢复

系统 SHALL 允许操作者编辑素材基础信息，并将素材状态在 `active` 和 `archived` 之间切换。

#### Scenario: 编辑素材基础信息

- **GIVEN** 当前账号下存在一条素材
- **WHEN** 操作者修改文件名、URL、缩略图 URL、标签或 metadata 并保存
- **THEN** 系统 SHALL 更新素材
- **AND** 资产栏、画布节点和详情 SHALL 展示最新内容

#### Scenario: 归档后默认列表移除

- **GIVEN** 当前素材状态为 `active`
- **WHEN** 操作者归档素材
- **THEN** 系统 SHALL 将状态更新为 `archived`
- **AND** 默认素材视图 SHALL 不再展示该素材

#### Scenario: 恢复归档素材

- **GIVEN** 当前素材状态为 `archived`
- **WHEN** 操作者恢复素材
- **THEN** 系统 SHALL 将状态更新为 `active`
- **AND** 默认素材视图 SHALL 可再次展示该素材

### Requirement: 素材库页面必须采用画布工作台

`apps/video-agent` SHALL 在“素材管理 > 素材库”提供画布优先工作台：主区域是一整块素材节点画布，资产栏和详情编辑以画布上的辅助浮层或窄面板呈现，底部提供轻量画布工具栏。

#### Scenario: 空状态

- **GIVEN** 当前账号没有可用素材
- **WHEN** 操作者打开素材库
- **THEN** 页面 SHALL 展示空画布状态
- **AND** 页面 SHALL 提供“新增素材”入口

#### Scenario: 素材库画布骨架

- **GIVEN** 当前账号存在素材
- **WHEN** 操作者打开素材库
- **THEN** 页面 SHALL 展示主画布、资产浮层、详情浮层和底部画布工具栏
- **AND** 素材节点 SHALL 展示缩略图或类型占位
- **AND** 资产浮层和详情浮层 SHALL 不把画布切分成三个等价栏目
- **AND** 页面 SHALL 不展示上传、语义检索、分镜候选或素材清单确认入口
```

- [ ] **Step 4: Create OpenSpec tasks**

Create `openspec/changes/material-library-management/tasks.md`:

```markdown
# Tasks

- [ ] 1. OpenSpec 和 Pencil 原型
  - [ ] 1.1 补齐 proposal/design/spec/tasks
  - [ ] 1.2 更新 `docs/prototypes/video-agent/video-agent.pen`
  - [ ] 1.3 导出或截图验证原型
  - [ ] 1.4 等待用户明确确认原型
- [ ] 2. 后端 schema 和素材仓储
  - [ ] 2.1 新增素材 migration
  - [ ] 2.2 新增素材 repository 和契约测试
  - [ ] 2.3 新增素材 DTO 和校验
- [ ] 3. 后端素材 API
  - [ ] 3.1 新增列表、创建、详情、更新、状态 API
  - [ ] 3.2 补齐路由测试和错误映射
- [ ] 4. 前端 API、页面和状态编排
  - [ ] 4.1 新增素材 API wrapper 和测试
  - [ ] 4.2 新增素材库页面级组件和 model helper
  - [ ] 4.3 接入菜单分发、加载、保存、归档和恢复
- [ ] 5. E2E 与收尾验证
  - [ ] 5.1 补齐素材库 E2E
  - [ ] 5.2 运行后端、前端、OpenSpec 和 diff 验证
```

- [ ] **Step 5: Validate OpenSpec artifacts**

Run: `openspec validate --all`

Expected: all specs pass; no structural errors in the new change.

- [ ] **Step 6: Update Pencil prototype**

Use Pencil MCP against `docs/prototypes/video-agent/video-agent.pen`:

1. Read the top-level frames with `mcp__pencil.batch_get`.
2. Locate the latest desktop video workspace frame that already contains the left rail and content strategy styling.
3. Copy that frame to a new frame named `桌面 - 素材库管理 v1 评审版`.
4. Update the left rail so “素材管理” is active and enabled.
5. Add second-level “素材库” as active.
6. Replace the main content with a material canvas workbench:
   - left asset rail: keyword, type, status, tags and material summaries.
   - center canvas: material nodes for video, image, audio, subtitle with thumbnails or type placeholders.
   - right detail/edit form: file name, type, URL, thumbnail URL, tags, source/license notes, metadata, save, archive/restore.
   - bottom toolbar: add, zoom, center, grid and view mode controls.
7. Add empty-state representation or a separate frame `桌面 - 素材库画布空状态 v1 评审版`.
8. Use `mcp__pencil.snapshot_layout` with `problemsOnly=true` and `mcp__pencil.get_screenshot` on the new frame to verify no overlap/clipping.

- [ ] **Step 7: Stop at prototype gate**

Ask the user to review the Pencil prototype and wait for an explicit confirmation phrase such as “确认开发” or “按这个原型开发”. Do not continue to Task 2 before that confirmation.

---

### Task 2: Backend Schema And Material Repository

**Files:**
- Create: `backend/migrations/20260709010000_material_library_management.sql`
- Modify: `backend/tests/database_migrations.rs`
- Create: `backend/src/repositories/material_repository.rs`
- Modify: `backend/src/repositories/mod.rs`
- Create: `backend/tests/material_repository_contract.rs`

- [ ] **Step 1: Write failing migration assertions**

In `backend/tests/database_migrations.rs`, extend `migrations_create_video_agent_core_schema` after existing `materials` assertions:

```rust
assert!(
    column_exists(&test_pool, "materials", "status").await,
    "materials.status should exist"
);
assert!(
    constraint_exists(&test_pool, "materials", "materials_status_check").await,
    "materials.status should be constrained"
);
assert!(
    index_exists(&test_pool, "idx_materials_project_status_updated").await,
    "materials project/status index should exist"
);
sqlx::query(
    r#"
    INSERT INTO projects (id, name, positioning, description)
    VALUES ('11111111-1111-4111-8111-111111111111', '测试账号', '', '')
    "#,
)
.execute(&test_pool)
.await
.expect("project fixture should be inserted");
sqlx::query(
    r#"
    INSERT INTO materials (project_id, material_type, file_url, file_name)
    VALUES ($1, 'subtitle', 'https://cdn.example.com/subtitles/demo.vtt', 'demo.vtt')
    "#,
)
.bind(Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap())
.execute(&test_pool)
.await
.expect("subtitle material should be accepted");
```

Run: `docker exec ai-agent-api cargo test --test database_migrations migrations_create_video_agent_core_schema -- --nocapture`

Expected: fail because `materials.status`, the new constraint, the new index, and `subtitle` type support do not exist.

- [ ] **Step 2: Create migration**

Create `backend/migrations/20260709010000_material_library_management.sql`:

```sql
-- Enable the first material library management slice without changing old migrations.

ALTER TABLE materials
    ADD COLUMN status VARCHAR(20) NOT NULL DEFAULT 'active';

ALTER TABLE materials
    DROP CONSTRAINT materials_type_check;

ALTER TABLE materials
    ADD CONSTRAINT materials_type_check
        CHECK (material_type IN ('video', 'image', 'audio', 'subtitle'));

ALTER TABLE materials
    ADD CONSTRAINT materials_status_check
        CHECK (status IN ('active', 'archived'));

COMMENT ON COLUMN materials.status IS '素材库状态：active 可用，archived 已归档但保留历史引用。';

CREATE INDEX idx_materials_project_status_updated
    ON materials(project_id, status, updated_at DESC);

UPDATE video_workspace_menus
SET
    is_enabled = true,
    status = 'active',
    metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{phase}', '3'::jsonb, true),
    updated_at = NOW()
WHERE menu_key = 'material-management';

INSERT INTO video_workspace_menus (
    id,
    parent_id,
    menu_key,
    label,
    description,
    route_path,
    icon,
    menu_type,
    module_key,
    agent_key,
    sort_order,
    is_enabled,
    is_visible,
    status,
    metadata
)
SELECT
    '30000000-0000-4000-8000-000000000001',
    parent.id,
    'material-library',
    '素材库',
    '登记和管理当前账号下的视频、图片、音频和字幕素材。',
    '/materials/library',
    'folder-open',
    'page',
    'materials.library',
    NULL,
    10,
    true,
    true,
    'active',
    '{"phase":3}'::jsonb
FROM video_workspace_menus parent
WHERE parent.menu_key = 'material-management'
ON CONFLICT (menu_key) DO UPDATE
SET
    parent_id = EXCLUDED.parent_id,
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    route_path = EXCLUDED.route_path,
    icon = EXCLUDED.icon,
    menu_type = EXCLUDED.menu_type,
    module_key = EXCLUDED.module_key,
    agent_key = EXCLUDED.agent_key,
    sort_order = EXCLUDED.sort_order,
    is_enabled = EXCLUDED.is_enabled,
    is_visible = EXCLUDED.is_visible,
    status = EXCLUDED.status,
    metadata = EXCLUDED.metadata,
    updated_at = NOW();
```

- [ ] **Step 3: Write failing repository contract tests**

Create `backend/tests/material_repository_contract.rs` with tests covering:

```rust
#[tokio::test]
async fn material_repository_creates_filters_archives_and_restores_materials() {
    // Arrange a migrated temporary database and a project fixture.
    // Create video and subtitle materials.
    // Assert default list returns active only.
    // Archive one material and assert it disappears from default list.
    // List archived and restore it.
    // Assert keyword/tag/type filters return the expected material.
}

#[tokio::test]
async fn material_repository_rejects_cross_project_update() {
    // Arrange two projects and one material under project A.
    // Attempt update with project B as expected_project_id.
    // Assert MaterialRepositoryError::MaterialNotFound.
}
```

Use concrete fixtures:

```rust
let project_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
let subtitle_url = "https://cdn.example.com/subtitles/demo.vtt".to_string();
let tags = vec!["字幕".to_string(), "中英双语".to_string()];
```

Run: `docker exec ai-agent-api cargo test --test material_repository_contract -- --nocapture`

Expected: fail because `material_repository` does not exist.

- [ ] **Step 4: Implement material repository**

Create `backend/src/repositories/material_repository.rs` with:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialType {
    Video,
    Image,
    Audio,
    Subtitle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialStatus {
    Active,
    Archived,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaterialStatusFilter {
    Active,
    Archived,
    All,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub id: Uuid,
    pub project_id: Uuid,
    pub material_type: MaterialType,
    pub file_url: String,
    pub file_name: String,
    pub thumbnail_url: Option<String>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    pub usage_count: i32,
    pub status: MaterialStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MaterialListFilter {
    pub material_type: Option<MaterialType>,
    pub status: MaterialStatusFilter,
    pub q: Option<String>,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CreateMaterialInput {
    pub project_id: Uuid,
    pub material_type: MaterialType,
    pub file_url: String,
    pub file_name: String,
    pub thumbnail_url: Option<String>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct UpdateMaterialInput {
    pub project_id: Uuid,
    pub material_type: MaterialType,
    pub file_url: String,
    pub file_name: String,
    pub thumbnail_url: Option<String>,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
}

#[async_trait]
pub trait MaterialRepository: Send + Sync {
    async fn create_material(&self, input: CreateMaterialInput) -> Result<Material, MaterialRepositoryError>;
    async fn get_material(&self, material_id: Uuid) -> Result<Material, MaterialRepositoryError>;
    async fn list_materials(&self, project_id: Uuid, filter: MaterialListFilter) -> Result<Vec<Material>, MaterialRepositoryError>;
    async fn update_material(&self, material_id: Uuid, input: UpdateMaterialInput) -> Result<Material, MaterialRepositoryError>;
    async fn update_material_status(&self, material_id: Uuid, status: MaterialStatus) -> Result<Material, MaterialRepositoryError>;
}
```

Implement SQL using `sqlx::query`, not ad hoc string concatenation. Use predicates:

```sql
WHERE project_id = $1
  AND ($2::text IS NULL OR material_type = $2)
  AND ($3::text IS NULL OR status = $3)
  AND ($4::text IS NULL OR file_name ILIKE '%' || $4 || '%' OR file_url ILIKE '%' || $4 || '%')
  AND ($5::text IS NULL OR tags @> ARRAY[$5]::text[])
ORDER BY updated_at DESC, id DESC
```

- [ ] **Step 5: Export repository module**

Modify `backend/src/repositories/mod.rs`:

```rust
pub mod material_repository;

pub use material_repository::{
    CreateMaterialInput, Material, MaterialListFilter, MaterialRepository,
    MaterialRepositoryError, MaterialStatus, MaterialStatusFilter, MaterialType,
    PostgresMaterialRepository, UpdateMaterialInput,
};
```

- [ ] **Step 6: Verify schema and repository tests**

Run:

```bash
docker exec ai-agent-api cargo test --test database_migrations migrations_create_video_agent_core_schema -- --nocapture
docker exec ai-agent-api cargo test --test material_repository_contract -- --nocapture
```

Expected: both pass.

---

### Task 3: Backend Material API

**Files:**
- Modify: `backend/src/agents/models/request.rs`
- Modify: `backend/src/agents/models/mod.rs`
- Modify: `backend/src/lib.rs`
- Create: `backend/tests/material_routes.rs`

- [ ] **Step 1: Write failing route tests**

Create `backend/tests/material_routes.rs` using the `project_routes.rs` temporary database pattern. Add tests:

```rust
#[tokio::test]
async fn material_routes_create_list_update_archive_and_restore() {
    // POST /api/projects/:project_id/materials with material_type=subtitle.
    // GET /api/projects/:project_id/materials returns active material.
    // PUT /api/materials/:material_id changes file_name, tags, metadata.
    // PUT /api/materials/:material_id/status archived removes it from default list.
    // GET /api/projects/:project_id/materials?status=archived returns it.
    // PUT /api/materials/:material_id/status active restores it.
}

#[tokio::test]
async fn material_routes_reject_invalid_payloads() {
    // Empty file_name => 400.
    // Invalid file_url => 400.
    // Unknown material_type => 400.
    // Unknown status => 400.
    // Unknown material_id => 404.
}
```

Use payload:

```json
{
  "material_type": "subtitle",
  "file_url": "https://cdn.example.com/subtitles/demo.vtt",
  "thumbnail_url": "https://cdn.example.com/covers/demo.jpg",
  "file_name": "demo.vtt",
  "tags": ["字幕", "中英双语"],
  "metadata": {
    "language": "zh-CN",
    "subtitle_format": "vtt",
    "source_note": "人工整理",
    "license_note": "内部可用"
  }
}
```

Run: `docker exec ai-agent-api cargo test --test material_routes -- --nocapture`

Expected: fail because routes and DTOs do not exist.

- [ ] **Step 2: Add DTOs and validation**

In `backend/src/agents/models/request.rs`, add:

```rust
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MaterialPayloadRequest {
    pub material_type: String,
    pub file_url: String,
    pub thumbnail_url: Option<String>,
    pub file_name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct MaterialStatusRequest {
    pub status: String,
}

#[derive(Clone, Debug, Deserialize, Default, PartialEq)]
pub struct MaterialListQuery {
    #[serde(rename = "type")]
    pub material_type: Option<String>,
    pub status: Option<String>,
    pub q: Option<String>,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MaterialResponse {
    pub material_id: Uuid,
    pub project_id: Uuid,
    pub material_type: String,
    pub file_url: String,
    pub thumbnail_url: Option<String>,
    pub file_name: String,
    pub tags: Vec<String>,
    pub metadata: Value,
    pub usage_count: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct MaterialListResponse {
    pub materials: Vec<MaterialResponse>,
}
```

Validation rules:

- `file_name.trim()` must be non-empty and no more than 255 chars.
- `file_url.trim()` must parse as `http` or `https` URL.
- `thumbnail_url`, when present and non-empty, must parse as `http` or `https` URL.
- tags are trimmed, empty tags removed, duplicates removed, max 30 tags, each max 40 chars.
- metadata must be a JSON object.
- `material_type` must parse to `MaterialType`.
- status must parse to `MaterialStatus` or `MaterialStatusFilter`.

- [ ] **Step 3: Export DTOs**

Modify `backend/src/agents/models/mod.rs` to export:

```rust
MaterialListQuery, MaterialListResponse, MaterialPayloadRequest,
MaterialResponse, MaterialStatusRequest,
```

- [ ] **Step 4: Add routes and handlers**

Modify `backend/src/lib.rs` imports and state:

```rust
use repositories::{
    CreateMaterialInput, MaterialRepository, MaterialRepositoryError, PostgresMaterialRepository,
    UpdateMaterialInput,
};

fn material_repository(&self) -> Result<PostgresMaterialRepository, ScriptApiError> {
    let pool = self
        .pg_pool
        .clone()
        .ok_or_else(|| ScriptApiError::State("database pool is not configured".to_string()))?;
    Ok(PostgresMaterialRepository::new(pool))
}
```

Add routes before `.layer(cors)`:

```rust
.route(
    "/api/projects/:project_id/materials",
    get(list_materials).post(create_material),
)
.route(
    "/api/materials/:material_id",
    get(get_material).put(update_material),
)
.route("/api/materials/:material_id/status", put(update_material_status))
```

Handlers:

```rust
async fn list_materials(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<MaterialListQuery>,
) -> Result<Json<MaterialListResponse>, ScriptApiError> {
    ensure_project_exists(&state, project_id).await?;
    let materials = state
        .material_repository()?
        .list_materials(project_id, query.try_into()?)
        .await?;
    Ok(Json(MaterialListResponse {
        materials: materials.into_iter().map(MaterialResponse::from).collect(),
    }))
}
```

Implement create/update/status/get in the same style as topic handlers. For update, pass `project_id` from loaded material into `UpdateMaterialInput` so the repository preserves project ownership.

- [ ] **Step 5: Add error mapping**

Extend `ScriptApiError`:

```rust
MaterialRepository(MaterialRepositoryError),
MaterialValidation(String),
```

Map:

- `MaterialRepositoryError::MaterialNotFound(id)` => `404` with `{"error":"素材不存在","material_id":id}`
- `MaterialRepositoryError::ProjectNotFound(id)` => `404` with `{"error":"项目不存在","project_id":id}`
- `MaterialRepositoryError::Storage(message)` => `500` with `{"error":"素材存储失败","details":message}`
- `MaterialValidation(message)` => `400`

- [ ] **Step 6: Verify route tests**

Run:

```bash
docker exec ai-agent-api cargo test --test material_routes -- --nocapture
docker exec ai-agent-api cargo test --test database_migrations --test material_repository_contract --test material_routes
```

Expected: pass.

---

### Task 4: Frontend API Types And Material Page

**Files:**
- Modify: `apps/video-agent/app/lib/api.ts`
- Modify: `apps/video-agent/app/lib/api.test.ts`
- Create: `apps/video-agent/app/pages/material-library/materialModel.ts`
- Create: `apps/video-agent/app/pages/material-library/MaterialLibraryPage.tsx`
- Modify: `apps/video-agent/app/styles.css`

- [ ] **Step 1: Write failing API wrapper tests**

In `apps/video-agent/app/lib/api.test.ts`, add tests that assert paths and query strings:

```ts
it("lists materials with filters", async () => {
  const fetcher = vi.fn().mockResolvedValue(
    new Response(JSON.stringify({ materials: [] }), { status: 200 }),
  );
  const client = createApiClient("http://api.test", fetcher);

  await listMaterials(client, project.project_id, {
    material_type: "subtitle",
    status: "archived",
    q: "demo",
    tag: "字幕",
  });

  expect(fetcher).toHaveBeenCalledWith(
    "http://api.test/api/projects/11111111-1111-4111-8111-111111111111/materials?type=subtitle&status=archived&q=demo&tag=%E5%AD%97%E5%B9%95",
    { headers: { accept: "application/json" } },
  );
});
```

Run: `docker exec ai-agent-video-agent npm run test -- app/lib/api.test.ts`

Expected: fail because material API functions do not exist.

- [ ] **Step 2: Add frontend API types and functions**

In `apps/video-agent/app/lib/api.ts`, add:

```ts
export type MaterialType = "video" | "image" | "audio" | "subtitle";
export type MaterialStatus = "active" | "archived";
export type MaterialStatusFilter = MaterialStatus | "all";

export type Material = {
  material_id: string;
  project_id: string;
  material_type: MaterialType;
  file_url: string;
  thumbnail_url: string | null;
  file_name: string;
  tags: string[];
  metadata: Record<string, unknown>;
  usage_count: number;
  status: MaterialStatus;
  created_at: string;
  updated_at: string;
};

export type MaterialPayload = {
  material_type: MaterialType;
  file_url: string;
  thumbnail_url?: string | null;
  file_name: string;
  tags: string[];
  metadata: Record<string, unknown>;
};

export type MaterialListResponse = {
  materials: Material[];
};

export type MaterialFilters = {
  material_type?: MaterialType | "all";
  status?: MaterialStatusFilter;
  q?: string;
  tag?: string;
};
```

Add wrappers:

```ts
export function listMaterials(client: ApiClient, projectId: string, filters: MaterialFilters = {}) {
  const searchParams = new URLSearchParams();
  if (filters.material_type && filters.material_type !== "all") {
    searchParams.set("type", filters.material_type);
  }
  if (filters.status && filters.status !== "all") {
    searchParams.set("status", filters.status);
  }
  if (filters.q?.trim()) {
    searchParams.set("q", filters.q.trim());
  }
  if (filters.tag?.trim()) {
    searchParams.set("tag", filters.tag.trim());
  }
  const query = searchParams.toString();
  return request<MaterialListResponse>(
    client,
    `/api/projects/${projectId}/materials${query ? `?${query}` : ""}`,
  );
}
```

Also add `createMaterial`, `getMaterial`, `updateMaterial`, `updateMaterialStatus`.

- [ ] **Step 3: Create material model helper**

Create `apps/video-agent/app/pages/material-library/materialModel.ts`:

```ts
import type { Material, MaterialPayload, MaterialStatus, MaterialStatusFilter, MaterialType } from "../../lib/api";

export type MaterialFormState = {
  file_name: string;
  material_type: MaterialType;
  file_url: string;
  thumbnail_url: string;
  tags_text: string;
  source_note: string;
  license_note: string;
  duration_sec: string;
  format: string;
  width: string;
  height: string;
  language: string;
  subtitle_format: string;
};

export const defaultMaterialForm: MaterialFormState = {
  file_name: "",
  material_type: "video",
  file_url: "",
  thumbnail_url: "",
  tags_text: "",
  source_note: "",
  license_note: "",
  duration_sec: "",
  format: "",
  width: "",
  height: "",
  language: "",
  subtitle_format: "",
};
```

Add label maps and helpers:

```ts
export const materialTypeLabels: Record<MaterialType, string> = {
  video: "视频",
  image: "图片",
  audio: "音频",
  subtitle: "字幕",
};

export const materialStatusLabels: Record<MaterialStatus, string> = {
  active: "可用",
  archived: "已归档",
};

export const materialStatusFilterOptions: Array<{ value: MaterialStatusFilter; label: string }> = [
  { value: "active", label: "可用" },
  { value: "archived", label: "已归档" },
  { value: "all", label: "全部" },
];
```

Implement `materialToForm(material)`, `materialPayloadFromForm(form)`, `getMaterialPreview(material)`, and `formatMaterialDate(value)`.

`getMaterialPreview(material)` returns:

- `material.thumbnail_url` when present.
- `material.file_url` for `image` materials without `thumbnail_url`.
- `null` plus the material type label for audio/subtitle/video placeholders without a thumbnail URL.

- [ ] **Step 4: Create MaterialLibraryPage component**

Create `apps/video-agent/app/pages/material-library/MaterialLibraryPage.tsx` with props:

```ts
type MaterialLibraryPageProps = {
  materials: Material[];
  selectedMaterial: Material | null;
  loading: boolean;
  error: string;
  actionError: string;
  saving: boolean;
  filters: {
    material_type: MaterialType | "all";
    status: MaterialStatusFilter;
    q: string;
    tag: string;
  };
  form: MaterialFormState;
  editing: boolean;
  onFilterChange: (filters: MaterialLibraryPageProps["filters"]) => void;
  onSelectMaterial: (materialId: string) => void;
  onNewMaterial: () => void;
  onEditMaterial: () => void;
  onCancelEdit: () => void;
  onFormChange: (form: MaterialFormState) => void;
  onSaveMaterial: () => void;
  onUpdateStatus: (status: MaterialStatus) => void;
};
```

Render:

- `.materialCanvasWorkspace`
- `.materialAssetRail`
- `.materialCanvas`
- `.materialCanvasNode`
- `.materialCanvasToolbar`
- `.materialDetailPanel`

Use buttons for type/status filters, a text input for keyword/tag, canvas nodes for materials, and a form with stable labels. Do not render upload, semantic search, scene matching, material checklist controls, persisted node position controls, or task orchestration links.

- [ ] **Step 5: Add CSS**

In `apps/video-agent/app/styles.css`, add scoped classes:

```css
.materialCanvasWorkspace {
  display: grid;
  grid-template-columns: 280px minmax(520px, 1fr) 360px;
  gap: 16px;
  align-items: stretch;
  max-width: 1800px;
}

.materialAssetRail,
.materialCanvas,
.materialDetailPanel {
  min-width: 0;
}

.materialCanvas {
  position: relative;
  min-height: 620px;
  overflow: hidden;
}

.materialAssetList {
  display: grid;
  gap: 10px;
  max-height: calc(100vh - 230px);
  overflow: auto;
}
```

Keep the visual language consistent with current workbench panels and avoid nested cards.

- [ ] **Step 6: Verify API tests**

Run: `docker exec ai-agent-video-agent npm run test -- app/lib/api.test.ts`

Expected: pass.

---

### Task 5: Frontend State Wiring And Page Tests

**Files:**
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/page.test.tsx`

- [ ] **Step 1: Write failing page tests**

In `apps/video-agent/app/page.test.tsx`, extend the API mock:

```ts
listMaterials: vi.fn(),
createMaterial: vi.fn(),
getMaterial: vi.fn(),
updateMaterial: vi.fn(),
updateMaterialStatus: vi.fn(),
```

Add a material-enabled menu fixture:

```ts
const materialWorkspaceMenus: WorkspaceMenuListResponse = {
  menus: [
    ...contentStrategyWorkspaceMenus.menus.slice(0, 2),
    {
      ...menuNode("material-management", "素材管理", true, "active", 30),
      children: [
        {
          ...menuNode("material-library", "素材库", true, "active", 10),
          menu_type: "page",
          module_key: "materials.library",
        },
      ],
    },
    menuNode("production", "作品生产", false, "planned", 40),
    menuNode("publishing", "发布运营", false, "planned", 50),
    menuNode("analytics", "数据分析", false, "planned", 60),
    menuNode("workflow-tasks", "工作流任务", false, "planned", 70),
  ],
};
```

Add tests:

```ts
it("opens material library from the workspace menu", async () => {
  vi.mocked(api.listWorkspaceMenus).mockResolvedValue(materialWorkspaceMenus);
  vi.mocked(api.listProjects).mockResolvedValue({ projects: [project] });
  vi.mocked(api.listMaterials).mockResolvedValue({ materials: [] });

  render(createElement(Home));
  fireEvent.click(await screen.findByRole("button", { name: /素材管理/ }));

  expect(await screen.findByRole("heading", { name: "素材库" })).toBeInTheDocument();
  expect(screen.getByText("还没有素材")).toBeInTheDocument();
  expect(api.listMaterials).toHaveBeenCalledWith(expect.anything(), project.project_id, {
    material_type: "all",
    status: "active",
    q: "",
    tag: "",
  });
});
```

Also test:

- create material inserts and selects it.
- archive removes material from active list.
- archived filter shows archived material and restore sets status active.
- subtitle type renders “字幕”.

Run: `docker exec ai-agent-video-agent npm run test -- app/page.test.tsx`

Expected: fail because page state is not wired.

- [ ] **Step 2: Wire page state**

In `apps/video-agent/app/page.tsx`:

1. Add constants:

```ts
const materialManagementMenuKey = "material-management";
const materialLibraryMenuKey = "material-library";
```

2. Add state:

```ts
const [materials, setMaterials] = useState<Material[]>([]);
const [selectedMaterialId, setSelectedMaterialId] = useState<string | null>(null);
const [loadingMaterials, setLoadingMaterials] = useState(false);
const [materialError, setMaterialError] = useState("");
const [materialActionError, setMaterialActionError] = useState("");
const [savingMaterial, setSavingMaterial] = useState(false);
const [materialFilters, setMaterialFilters] = useState({
  material_type: "all" as MaterialType | "all",
  status: "active" as MaterialStatusFilter,
  q: "",
  tag: "",
});
const [materialForm, setMaterialForm] = useState<MaterialFormState>(defaultMaterialForm);
const [editingMaterial, setEditingMaterial] = useState(false);
```

3. Add `loadMaterials`, `handleNewMaterial`, `handleSaveMaterial`, `handleUpdateMaterialStatus` using API wrappers.
4. In menu selection, route `material-management` and `material-library` to the material page.
5. Render `MaterialLibraryPage` when `selectedMenuKey === materialManagementMenuKey`.

- [ ] **Step 3: Verify page tests**

Run:

```bash
docker exec ai-agent-video-agent npm run test -- app/page.test.tsx app/lib/api.test.ts
```

Expected: pass.

---

### Task 6: E2E, OpenSpec Tasks, And Final Verification

**Files:**
- Modify: `apps/video-agent/e2e/workspace.spec.ts`
- Modify: `openspec/changes/material-library-management/tasks.md`

- [ ] **Step 1: Add E2E route mocks**

In `apps/video-agent/e2e/workspace.spec.ts`, add `materialWorkspaceMenus` and `materials` fixtures:

```ts
const material = {
  material_id: "abababab-abab-4aba-8aba-abababababab",
  project_id: projectId,
  material_type: "subtitle",
  file_url: "https://cdn.example.com/subtitles/demo.vtt",
  file_name: "demo.vtt",
  tags: ["字幕", "中英双语"],
  metadata: { language: "zh-CN", subtitle_format: "vtt" },
  usage_count: 0,
  status: "active",
  created_at: "2026-07-09T00:00:00Z",
  updated_at: "2026-07-09T00:00:00Z",
};
```

Mock:

```ts
await page.route("**/api/projects/*/materials**", async (route) => {
  await route.fulfill({ json: { materials: [material] } });
});
```

- [ ] **Step 2: Add E2E scenario**

Add:

```ts
test("material library shows the first management slice", async ({ page }) => {
  await mockApi(page, { workspaceMenus: materialWorkspaceMenus });
  await page.goto("/");
  await page.getByRole("button", { name: /素材管理/ }).click();

  await expect(page.getByRole("heading", { name: "素材库" })).toBeVisible();
  await expect(page.getByText("demo.vtt")).toBeVisible();
  await expect(page.getByText("字幕")).toBeVisible();
  await expect(page.getByLabel("素材 URL")).toBeVisible();
});
```

- [ ] **Step 3: Check OpenSpec tasks**

Update `openspec/changes/material-library-management/tasks.md` as tasks complete. Before final verification, all implementation tasks should be checked except archive/commit tasks, because this plan does not include git operations.

- [ ] **Step 4: Run final verification**

Run:

```bash
docker exec ai-agent-api cargo fmt -- --check
docker exec ai-agent-api cargo test
docker exec ai-agent-api cargo clippy --all-targets --all-features -- -D warnings
docker exec ai-agent-video-agent npm run lint
docker exec ai-agent-video-agent npm run test
docker exec ai-agent-video-agent npm run build
docker exec ai-agent-video-agent npm run test:e2e
openspec instructions apply --change "material-library-management" --json
openspec validate --all
git diff --check
git status --short
```

Expected:

- Rust format, tests, clippy pass.
- Frontend lint, tests, build, E2E pass.
- `openspec instructions apply` reports progress aligned with checked tasks.
- `openspec validate --all` passes.
- `git diff --check` has no whitespace errors; existing LF/CRLF warnings are reported if present.
- `git status --short` lists only intentional files.

If a command fails because Docker services are unavailable, report the exact command and failure. Do not claim completion without verification evidence.

---

## Self-Review

- Spec coverage: URL registration, `subtitle`, `active/archived`, default active listing, archived filter, edit, archive/restore, three-column page, empty state, OpenSpec and Pencil gates are covered.
- Scope guard: upload, remote metadata crawling, automatic video frame extraction, automatic waveform/cover generation, Embedding, Milvus, semantic search, material Agent, scene matching, material checklist, production integration, platform sync and mobile adaptation are explicitly excluded.
- Type consistency: backend uses `MaterialType`, `MaterialStatus`, `MaterialStatusFilter`; frontend uses `MaterialType`, `MaterialStatus`, `MaterialStatusFilter`; API values are `video | image | audio | subtitle` and `active | archived`.
- User constraints: no git staging or commit steps are included; formal Pencil prototype confirmation blocks frontend implementation.
