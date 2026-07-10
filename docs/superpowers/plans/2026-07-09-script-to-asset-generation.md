# Script To Asset Generation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first script-to-asset-generation flow: reuse existing materials first, generate AI image candidates asynchronously, require manual confirmation for AI video tasks, and let users select one main asset per scene.

**Architecture:** Rust API owns validation, plan creation, candidate state, and stable database writes. Python `services/video-worker` owns provider calls, image download, local asset storage, and status callback. `apps/video-agent` adds the script-detail asset candidate UI, consuming backend APIs without exposing model-management configuration.

**Tech Stack:** Rust + Axum + SQLx + PostgreSQL; Python FastAPI worker + pytest; Next.js 14 + TypeScript + Vitest + Playwright; OpenSpec.

---

## File Structure

- Create: `backend/migrations/20260709020000_script_to_asset_generation.sql` — schema for `asset_generation_tasks` and `scene_asset_candidates`.
- Create: `backend/src/repositories/asset_generation_repository.rs` — domain enums, repository trait, SQLx implementation.
- Modify: `backend/src/repositories/mod.rs` — export asset generation repository.
- Modify: `backend/src/lib.rs` — route registration, app state accessor, route handlers, error mapping.
- Modify: `backend/src/agents/models/request.rs` — request/response DTOs for plans, tasks, candidates, selection.
- Create: `backend/tests/asset_generation_repository_contract.rs` — repository contract tests.
- Create: `backend/tests/asset_generation_routes.rs` — API route tests.
- Modify: `backend/tests/database_migrations.rs` — schema existence and constraints.
- Create: `services/video-worker/src/video_worker/asset_generation.py` — worker service, fakeable providers, storage writer.
- Modify: `services/video-worker/src/video_worker/main.py` — expose worker endpoint or service wiring for tests.
- Create: `services/video-worker/tests/test_asset_generation.py` — worker tests.
- Modify: `docker-compose.yml` — mount persistent local asset storage for API/worker if implementation requires shared path.
- Modify: `apps/video-agent/app/lib/api.ts` and `apps/video-agent/app/lib/api.test.ts` — asset candidate API types and wrappers.
- Create: `apps/video-agent/app/pages/script-creation/AssetCandidatePanel.tsx` — script asset candidate UI.
- Modify: `apps/video-agent/app/pages/script-creation/ScriptCreationPage.tsx` — render asset panel in script detail.
- Modify: `apps/video-agent/app/page.tsx` and `apps/video-agent/app/page.test.tsx` — state orchestration.
- Modify: `apps/video-agent/app/styles.css` — layout and candidate styling.
- Modify: `apps/video-agent/e2e/workspace.spec.ts` — end-to-end coverage.
- Modify: `docs/prototypes/video-agent/video-agent.pen` — Pencil prototype before frontend implementation.
- Modify: `openspec/changes/script-to-asset-generation/tasks.md` — check off tasks as implemented.

## Task 1: Schema And Repository

**Files:**
- Create: `backend/migrations/20260709020000_script_to_asset_generation.sql`
- Create: `backend/src/repositories/asset_generation_repository.rs`
- Modify: `backend/src/repositories/mod.rs`
- Modify: `backend/tests/database_migrations.rs`
- Test: `backend/tests/asset_generation_repository_contract.rs`

- [ ] **Step 1: Write migration/repository failing tests**

Add `backend/tests/asset_generation_repository_contract.rs` with tests for:

```rust
#[tokio::test]
async fn asset_repository_creates_tasks_candidates_and_selects_one_per_scene() {
    // Arrange migrated DB, project, script, two scenes, two materials.
    // Act: create one image generation task, create two candidates, select first, then select second.
    // Assert: only the second candidate is selected for the scene.
}

#[tokio::test]
async fn asset_repository_rejects_archived_material_selection() {
    // Arrange migrated DB with archived image material and candidate.
    // Act: select archived material candidate.
    // Assert: returns CandidateNotSelectable.
}
```

Add checks to `backend/tests/database_migrations.rs`:

```rust
assert!(table_exists(&test_pool, "asset_generation_tasks").await);
assert!(table_exists(&test_pool, "scene_asset_candidates").await);
assert!(constraint_exists(&test_pool, "scene_asset_candidates", "scene_asset_candidates_status_check").await);
assert!(constraint_exists(&test_pool, "asset_generation_tasks", "asset_generation_tasks_status_check").await);
```

- [ ] **Step 2: Run RED**

Run:

```bash
docker exec ai-agent-api cargo test --test asset_generation_repository_contract
docker exec ai-agent-api cargo test --test database_migrations
```

Expected: repository test target fails to compile because repository/module does not exist; migration test fails because tables do not exist.

- [ ] **Step 3: Add migration and repository implementation**

Implement migration with:

```sql
CREATE TABLE asset_generation_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    script_id UUID REFERENCES scripts(id) ON DELETE SET NULL,
    scene_id UUID REFERENCES scenes(id) ON DELETE SET NULL,
    provider VARCHAR(40) NOT NULL,
    task_type VARCHAR(40) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    candidate_count INT NOT NULL DEFAULT 0,
    reference_material_ids UUID[] NOT NULL DEFAULT ARRAY[]::UUID[],
    params JSONB NOT NULL DEFAULT '{}'::jsonb,
    result JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT,
    retry_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT asset_generation_tasks_provider_check CHECK (provider IN ('gpt-image-2', 'jimeng')),
    CONSTRAINT asset_generation_tasks_type_check CHECK (task_type IN ('image_candidates', 'video_draft', 'video_generation')),
    CONSTRAINT asset_generation_tasks_status_check CHECK (status IN ('draft', 'pending', 'processing', 'completed', 'failed')),
    CONSTRAINT asset_generation_tasks_candidate_count_check CHECK (candidate_count >= 0 AND candidate_count <= 48),
    CONSTRAINT asset_generation_tasks_retry_count_check CHECK (retry_count >= 0)
);

CREATE TABLE scene_asset_candidates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE CASCADE,
    scene_id UUID NOT NULL REFERENCES scenes(id) ON DELETE CASCADE,
    material_id UUID REFERENCES materials(id) ON DELETE SET NULL,
    candidate_type VARCHAR(20) NOT NULL,
    source VARCHAR(30) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'candidate',
    rank INT NOT NULL DEFAULT 0,
    generation_task_id UUID REFERENCES asset_generation_tasks(id) ON DELETE SET NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT scene_asset_candidates_type_check CHECK (candidate_type IN ('image', 'video')),
    CONSTRAINT scene_asset_candidates_source_check CHECK (source IN ('existing_material', 'ai_generated', 'video_task')),
    CONSTRAINT scene_asset_candidates_status_check CHECK (status IN ('candidate', 'selected', 'rejected', 'failed')),
    CONSTRAINT scene_asset_candidates_rank_check CHECK (rank >= 0)
);

CREATE UNIQUE INDEX scene_asset_candidates_one_selected_per_scene
    ON scene_asset_candidates(scene_id)
    WHERE status = 'selected';
```

Implement repository methods:

```rust
create_task(input) -> AssetGenerationTask
create_candidate(input) -> SceneAssetCandidate
list_candidates(script_id) -> Vec<SceneAssetCandidate>
select_candidate(scene_id, candidate_id) -> SceneAssetCandidate
reject_candidate(scene_id, candidate_id) -> SceneAssetCandidate
update_task_status(task_id, status, result, error) -> AssetGenerationTask
```

Selection must use a transaction: verify candidate belongs to scene, verify material is absent or `active`, set existing selected candidates for that scene to `candidate`, then set target to `selected`.

- [ ] **Step 4: Run GREEN**

Run:

```bash
docker exec ai-agent-api cargo test --test asset_generation_repository_contract
docker exec ai-agent-api cargo test --test database_migrations
```

Expected: tests pass.

## Task 2: Backend API And Validation

**Files:**
- Modify: `backend/src/lib.rs`
- Modify: `backend/src/agents/models/request.rs`
- Test: `backend/tests/asset_generation_routes.rs`
- Modify: `openspec/changes/script-to-asset-generation/tasks.md`

- [ ] **Step 1: Write route failing tests**

Add route tests for:

```rust
#[tokio::test]
async fn asset_generation_plan_rejects_more_than_48_images() {
    // POST /api/scripts/{script_id}/asset-generation-plan with candidate_count=4 on 13 scenes.
    // Expect 400 and Chinese message containing "48".
}

#[tokio::test]
async fn create_asset_generation_tasks_does_not_wait_for_worker() {
    // POST /api/scripts/{script_id}/asset-generation-tasks.
    // Expect 201, task status pending for image, draft for video.
}

#[tokio::test]
async fn selecting_candidate_replaces_existing_selected_candidate() {
    // Select candidate A, then candidate B for same scene.
    // GET candidates shows only B selected.
}
```

- [ ] **Step 2: Run RED**

Run:

```bash
docker exec ai-agent-api cargo test --test asset_generation_routes
```

Expected: fails because routes and DTOs do not exist.

- [ ] **Step 3: Implement DTOs and handlers**

Add DTOs:

```rust
AssetGenerationPlanRequest {
    provider: String,
    image_candidates_per_scene: i32,
    use_reference_materials: bool,
}

AssetGenerationPlanResponse {
    script_id: Uuid,
    scene_count: usize,
    image_candidate_count: i32,
    max_image_candidate_count: i32,
    provider: String,
    video_task_count: i32,
    can_create: bool,
    warnings: Vec<String>,
}

AssetGenerationTaskRequest {
    provider: String,
    image_candidates_per_scene: i32,
    use_reference_materials: bool,
}
```

Register routes:

```rust
.route("/api/scripts/:script_id/asset-generation-plan", post(create_asset_generation_plan))
.route("/api/scripts/:script_id/asset-generation-tasks", post(create_asset_generation_tasks))
.route("/api/scripts/:script_id/asset-candidates", get(list_asset_candidates))
.route("/api/scenes/:scene_id/asset-candidates/:candidate_id/select", put(select_asset_candidate))
.route("/api/scenes/:scene_id/asset-candidates/:candidate_id/reject", put(reject_asset_candidate))
.route("/api/scenes/:scene_id/asset-generation-tasks", post(create_scene_asset_generation_task))
.route("/api/asset-generation-tasks/:task_id/confirm", post(confirm_asset_generation_task))
```

Validation:

- Provider must be `gpt-image-2` or `jimeng`.
- `image_candidates_per_scene` must be 1-4.
- `scene_count * image_candidates_per_scene` must be <= 48.
- Image tasks are created as `pending`.
- Video tasks are created as `draft`.

- [ ] **Step 4: Run GREEN**

Run:

```bash
docker exec ai-agent-api cargo test --test asset_generation_routes
docker exec ai-agent-api cargo test --test material_routes
```

Expected: tests pass.

## Task 3: Worker Provider, Storage, And Tests

**Files:**
- Create: `services/video-worker/src/video_worker/asset_generation.py`
- Modify: `services/video-worker/src/video_worker/main.py`
- Test: `services/video-worker/tests/test_asset_generation.py`
- Modify: `docker-compose.yml`

- [ ] **Step 1: Write worker failing tests**

Add pytest tests:

```python
def test_worker_writes_generated_image_to_local_storage(tmp_path):
    provider = FakeImageProvider([GeneratedImage(filename="scene-1.png", content=b"png")])
    storage = LocalAssetStorage(tmp_path, public_prefix="/assets")
    result = process_image_task(task, provider, storage)
    assert result.materials[0].file_url.startswith("/assets/")
    assert (tmp_path / "generated" / "images").exists()

def test_worker_retries_temporary_error_once(tmp_path):
    provider = FakeImageProvider([TemporaryProviderError("timeout"), GeneratedImage(filename="ok.png", content=b"png")])
    result = process_image_task(task, provider, storage)
    assert result.retry_count == 1

def test_worker_does_not_create_material_when_download_fails(tmp_path):
    provider = FakeImageProvider([GeneratedImage(filename="bad.png", content=None)])
    result = process_image_task(task, provider, storage)
    assert result.status == "failed"
    assert result.materials == []
```

- [ ] **Step 2: Run RED**

Run:

```bash
docker exec ai-agent-video-worker pytest tests/test_asset_generation.py -q
```

Expected: fails because module/classes do not exist.

- [ ] **Step 3: Implement worker primitives**

Implement:

```python
@dataclass
class AssetGenerationTask:
    task_id: str
    provider: str
    prompt: str
    candidate_count: int
    reference_material_urls: list[str]

@dataclass
class GeneratedImage:
    filename: str
    content: bytes

class LocalAssetStorage:
    def save_image(self, task_id: str, image: GeneratedImage) -> str:
        path = self.root / "generated" / "images" / task_id / image.filename
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(image.content)
        return f"{self.public_prefix}/generated/images/{task_id}/{image.filename}"
```

Provider adapter boundaries:

- `OpenAIImageProvider` calls Image API using env key and `gpt-image-2`.
- `JimengImageProvider` is implemented behind env config and must fail fast with clear config error when credentials are absent.
- `FakeImageProvider` is used for tests.

- [ ] **Step 4: Run GREEN**

Run:

```bash
docker exec ai-agent-video-worker pytest tests/test_asset_generation.py -q
docker exec ai-agent-video-worker pytest tests -q
```

Expected: tests pass.

## Task 4: Frontend API And UI

**Files:**
- Modify: `apps/video-agent/app/lib/api.ts`
- Modify: `apps/video-agent/app/lib/api.test.ts`
- Create: `apps/video-agent/app/pages/script-creation/AssetCandidatePanel.tsx`
- Modify: `apps/video-agent/app/pages/script-creation/ScriptCreationPage.tsx`
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/page.test.tsx`
- Modify: `apps/video-agent/app/styles.css`

- [ ] **Step 1: Write frontend API failing tests**

Add tests for wrappers:

```ts
await createAssetGenerationPlan(client, scriptId, {
  provider: "gpt-image-2",
  image_candidates_per_scene: 3,
  use_reference_materials: true,
});
expect(fetchMock).toHaveBeenCalledWith(
  "http://api.test/api/scripts/script-1/asset-generation-plan",
  expect.objectContaining({ method: "POST" }),
);
```

- [ ] **Step 2: Run API RED**

Run:

```bash
docker exec ai-agent-video-agent npm run test -- app/lib/api.test.ts
```

Expected: fails because wrapper does not exist.

- [ ] **Step 3: Implement API types and wrappers**

Add types:

```ts
export type AssetProvider = "gpt-image-2" | "jimeng";
export type AssetCandidateStatus = "candidate" | "selected" | "rejected" | "failed";
export type SceneAssetCandidate = {
  candidate_id: string;
  scene_id: string;
  material_id: string | null;
  candidate_type: "image" | "video";
  source: "existing_material" | "ai_generated" | "video_task";
  status: AssetCandidateStatus;
  file_url: string | null;
  thumbnail_url: string | null;
  file_name: string;
};
```

Add wrappers for plan, task creation, list, select, reject, scene regenerate, confirm video.

- [ ] **Step 4: Write UI failing tests**

Add `page.test.tsx` coverage:

```ts
it("脚本详情展示素材候选生成设置并选择主素材", async () => {
  // Mock selected script, candidates, and API calls.
  // Assert supplier buttons, candidate count input, old/AI sections, select button.
});
```

- [ ] **Step 5: Run UI RED**

Run:

```bash
docker exec ai-agent-video-agent npm run test -- app/page.test.tsx
```

Expected: fails because UI does not exist.

- [ ] **Step 6: Implement UI**

Create `AssetCandidatePanel.tsx` with:

- Left scene list.
- Middle existing material and AI image candidate sections.
- Right settings: provider segmented buttons, candidate count input 1-4, reference-material checkbox, plan warning, generate button, video confirm button.
- Select/reject/regenerate handlers passed from `page.tsx`.

Integrate into `ScriptDetailView` below source topic and above timeline, preserving the timeline comparison view.

- [ ] **Step 7: Run GREEN**

Run:

```bash
docker exec ai-agent-video-agent npm run test -- app/lib/api.test.ts app/page.test.tsx
docker exec ai-agent-video-agent npm run lint
```

Expected: tests and lint pass.

## Task 5: Prototype, E2E, OpenSpec, And Verification

**Files:**
- Modify: `docs/prototypes/video-agent/video-agent.pen`
- Modify: `apps/video-agent/e2e/workspace.spec.ts`
- Modify: `openspec/changes/script-to-asset-generation/tasks.md`

- [ ] **Step 1: Update Pencil prototype**

Use Pencil MCP to update `docs/prototypes/video-agent/video-agent.pen` with the confirmed script-detail asset candidate layout:

- Left scene list.
- Middle existing/AI candidate sections.
- Right generation settings and AI video confirmation.

- [ ] **Step 2: Verify prototype**

Use `mcp__pencil.batch_get` to read the relevant page nodes and confirm the prototype contains the new layout labels.

- [ ] **Step 3: Write E2E failing test**

Add E2E test:

```ts
test("脚本详情生成素材候选并选择主素材", async ({ page }) => {
  // Mock APIs for script detail, plan, candidate creation, candidate list, select.
  // Navigate to 脚本创作, open a script, generate candidates, select a candidate.
});
```

- [ ] **Step 4: Run E2E RED**

Run:

```bash
docker exec ai-agent-video-agent npm run test:e2e -- --grep "素材候选"
```

Expected: fails before UI is wired or before mocks are complete.

- [ ] **Step 5: Complete E2E and OpenSpec tasks**

Fix E2E wiring, then update `openspec/changes/script-to-asset-generation/tasks.md` checkboxes only for completed tasks.

- [ ] **Step 6: Final verification**

Run:

```bash
docker exec ai-agent-api cargo fmt -- --check
docker exec ai-agent-api cargo test
docker exec ai-agent-api cargo clippy --all-targets --all-features -- -D warnings
docker exec ai-agent-video-worker pytest tests -q
docker exec ai-agent-video-agent npm run lint
docker exec ai-agent-video-agent npm run test
docker exec ai-agent-video-agent npm run build
docker exec ai-agent-video-agent npm run test:e2e
openspec instructions apply --change "script-to-asset-generation" --json
openspec validate --all
git diff --check
```

Expected: all commands pass; OpenSpec task state matches actual implementation.

## Self-Review

- Spec coverage: tasks cover schema, API, worker, storage, frontend, E2E, Pencil, OpenSpec validation, and cost/failure constraints.
- Placeholder scan: plan contains no `TBD`, `TODO`, or unbounded “handle edge cases” instructions.
- Type consistency: provider values use `gpt-image-2 | jimeng`; task statuses use `draft | pending | processing | completed | failed`; candidate statuses use `candidate | selected | rejected | failed`.
