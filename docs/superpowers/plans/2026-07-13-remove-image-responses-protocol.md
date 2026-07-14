# Remove Image Responses Protocol Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 彻底移除 `model_type=image + api_protocol=openai_responses`，恢复数据库、Rust、Worker、Admin 与原型的一致严格协议矩阵，同时保留默认模型 `POST` 修复和 `/assets` 本地参考图读取。

**Architecture:** 保留已执行的放开 migration，新增 append-only 收紧 migration；应用层删除所有 Responses 图片入口和不可达运行代码。通过数据库、Rust、Worker、Admin 四层拒绝测试锁定最终行为，历史 change、migration 和任务快照保持不变。

**Tech Stack:** PostgreSQL 16、SQLx migrations、Rust/Axum、Python 3.12/pytest、Next.js 14/TypeScript/Vitest、Pencil MCP、OpenSpec。

**执行约束:** 当前工作区包含用户未提交改动，必须逐块编辑，不得整文件还原；未经用户明确授权不得执行 `git add`、`git commit` 或 `git push`。

---

## 文件职责映射

- `backend/migrations/20260713020000_remove_image_responses_protocol.sql`：在历史放开 migration 之后恢复数据库最终约束。
- `crates/novex-model/src/registry.rs`：Rust 领域层模型类型与协议兼容矩阵。
- `backend/src/application/asset_generation.rs`：图片任务协议到业务 provider 的映射。
- `services/video-worker/src/video_worker/model_registry.py`：Worker 执行前的图片模型协议校验。
- `services/video-worker/src/video_worker/asset_generation.py`：合法图片 provider 和批量图片任务执行；保留本地参考图读取。
- `admin/app/models/ModelManagementPage.tsx`：Admin 按模型类型展示协议选项。
- `docs/prototypes/admin/model-management.pen`：模型管理原型状态，只通过 Pencil MCP 修改。
- `MEMORY.md`、`docs/memory/video-agent-workspace-flow.md`：记录已确认的新稳定边界。
- `openspec/changes/remove-image-responses-protocol/tasks.md`：实施状态单一清单。

### Task 1: 确认规格并删除 Pencil 原型状态

**Files:**
- Modify: `openspec/changes/remove-image-responses-protocol/tasks.md`
- Modify through Pencil MCP: `docs/prototypes/admin/model-management.pen`

- [x] **Step 1: 标记书面规格已确认**

把任务 `1.2` 改为：

```markdown
- [x] 1.2 取得用户对书面规格的明确确认。
```

- [x] **Step 2: 删除已定位的 Responses 图片原型状态**

调用 Pencil MCP：

```text
batch_design(filePath="docs/prototypes/admin/model-management.pen", operations='D("cdD3D")')
```

预期：删除顶层 frame `cdD3D`，名称为“状态 - 添加图片模型（OpenAI Responses）”。

- [x] **Step 3: 验证原型删除与布局**

调用 `batch_get` 搜索精确名称，预期 `nodes=[]`；再调用 `snapshot_layout(problemsOnly=true, maxDepth=2)`，确认没有因本次删除新增布局问题。

- [x] **Step 4: 更新 OpenSpec 原型任务**

把任务 `1.3` 改为已完成；不直接编辑 `.pen` JSON。

### Task 2: 编写跨层拒绝测试并确认 RED

**Files:**
- Modify: `crates/novex-model/tests/model_registry.rs:4`
- Modify: `backend/tests/ai_model_routes.rs:130`
- Modify: `backend/tests/database_migrations.rs:280`
- Modify: `services/video-worker/tests/test_model_registry.py:83`
- Modify: `services/video-worker/tests/test_asset_generation.py:853`
- Modify: `admin/app/models/page.test.tsx:74`

- [x] **Step 1: Rust 领域测试改为拒绝图片 Responses**

将图片断言改为：

```rust
assert!(!ApiProtocol::OpenAiResponses.supports(ModelType::Image));
assert!(!ApiProtocol::OpenAiChatCompletions.supports(ModelType::Image));
assert!(!ApiProtocol::OpenAiResponses.supports(ModelType::Video));
```

- [x] **Step 2: API 测试改为拒绝唯一被回退的组合**

保留 `image_responses_payload()` 作为非法请求构造器，将测试主体改为：

```rust
#[tokio::test]
async fn admin_rejects_image_responses_protocol() {
    let (admin_pool, pool, database_name, test_url) = migrated_pool().await;
    let app = build_app_with_state(app_state(test_url, pool.clone()));

    let (status, body) = send(
        &app,
        "POST",
        "/api/admin/models",
        Some(image_responses_payload("Responses Image")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["code"], "invalid_model_config");

    pool.close().await;
    drop_database(&admin_pool, &database_name).await;
    admin_pool.close().await;
}
```

- [x] **Step 3: migration 测试要求最终 schema 拒绝图片 Responses**

将现有 `image_responses_insert` 断言改为：

```rust
assert!(
    image_responses_insert.is_err(),
    "image models should reject openai_responses after the rollback migration"
);
```

继续保留 `image + openai_chat_completions` 与 `video + openai_responses` 的拒绝断言。

- [x] **Step 4: Worker 注册表测试改为非法配置**

删除成功解析测试，把参数加入 `test_loader_rejects_unavailable_or_invalid_models`：

```python
(image_model_row(api_protocol="openai_responses"), "invalid_model_config"),
```

- [x] **Step 5: Worker provider factory 测试改为拒绝**

将 Responses provider 构造测试替换为：

```python
def test_image_provider_factory_rejects_openai_responses():
    with pytest.raises(
        ProviderConfigError,
        match="unsupported image protocol: openai_responses",
    ):
        image_provider_from_model(image_model_config("openai_responses"))
```

- [x] **Step 6: Admin 测试要求图片下拉不含 Responses**

保留文本和视频协议断言，将图片期望改为：

```typescript
expect(protocolNames()).toEqual(["OpenAI Images", "即梦 Visual"]);
expect(protocolNames()).not.toContain("OpenAI Responses");
```

删除对图片 `openai_responses` 的切换动作；保留 `jimeng_visual` 显示 `API Secret` 和“设为默认使用 POST”测试。

- [x] **Step 7: 在容器中运行聚焦测试并确认 RED**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-model --test model_registry protocol_must_match_model_type_and_auth_scheme'
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test ai_model_routes admin_rejects_image_responses_protocol'
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test database_migrations migrations_create_video_agent_core_schema'
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc 'cd /app && pytest tests/test_model_registry.py tests/test_asset_generation.py -q -k "openai_responses or protocol_must"'
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc 'cd /app && npm test -- app/models/page.test.tsx'
```

预期：至少 Rust 兼容矩阵、API/migration、Worker 注册表/provider factory 和 Admin 协议选项因当前仍支持该组合而失败。记录实际失败原因后再实施。

### Task 3: 新增收紧 migration 并恢复 Rust 矩阵

**Files:**
- Create: `backend/migrations/20260713020000_remove_image_responses_protocol.sql`
- Modify: `crates/novex-model/src/registry.rs:98`
- Modify: `backend/src/application/asset_generation.rs:479`
- Test: `crates/novex-model/tests/model_registry.rs`
- Test: `backend/tests/ai_model_routes.rs`
- Test: `backend/tests/database_migrations.rs`

- [x] **Step 1: 新增 append-only migration**

文件内容固定为：

```sql
-- Restore the original image protocol boundary without rewriting model records.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM ai_models
        WHERE model_type = 'image'
          AND api_protocol = 'openai_responses'
    ) THEN
        RAISE EXCEPTION
            'cannot remove image responses protocol while matching ai_models records exist';
    END IF;
END
$$;

ALTER TABLE ai_models
    DROP CONSTRAINT ai_models_type_protocol_check;

ALTER TABLE ai_models
    ADD CONSTRAINT ai_models_type_protocol_check CHECK (
        (model_type = 'text' AND api_protocol IN (
            'openai_responses', 'openai_chat_completions'
        )) OR
        (model_type = 'image' AND api_protocol IN (
            'openai_images', 'jimeng_visual'
        )) OR
        (model_type = 'video' AND api_protocol IN ('runway_api', 'kling_api'))
    );

COMMENT ON CONSTRAINT ai_models_type_protocol_check ON ai_models IS
    '限制模型类型与可执行协议组合；Responses 仅用于文本模型。';
```

- [x] **Step 2: 恢复 Rust 领域矩阵**

用以下完整 match 替换 `ApiProtocol::supports` 的模式：

```rust
matches!(
    (self, model_type),
    (Self::OpenAiResponses | Self::OpenAiChatCompletions, ModelType::Text)
        | (Self::OpenAiImages | Self::JimengVisual, ModelType::Image)
        | (Self::RunwayApi | Self::KlingApi, ModelType::Video)
)
```

- [x] **Step 3: 删除素材任务的 Responses 图片映射**

恢复为：

```rust
match protocol {
    ApiProtocol::OpenAiImages => Ok(AssetGenerationProvider::GptImage2),
    ApiProtocol::JimengVisual => Ok(AssetGenerationProvider::Jimeng),
    _ => Err(ModelResolveError::InvalidConfig(Uuid::nil()).into()),
}
```

- [x] **Step 4: 运行 Rust 聚焦测试并确认 GREEN**

运行 Task 2 的三条 Rust 命令。预期全部通过；migration 测试证明按完整迁移链执行后的最终约束拒绝图片 Responses。

- [x] **Step 5: 更新 OpenSpec 数据库/Rust 任务**

将 `2.1`、`3.1`、`3.2`、`3.3` 依据实际测试状态勾选；Worker/Admin RED 任务在各自实现完成前保持未完成。

### Task 4: 删除 Worker Responses 图片运行时并保留本地参考图

**Files:**
- Modify: `services/video-worker/src/video_worker/model_registry.py:79`
- Modify: `services/video-worker/src/video_worker/asset_generation.py:1-1225`
- Modify: `services/video-worker/tests/test_model_registry.py`
- Modify: `services/video-worker/tests/test_asset_generation.py`

- [x] **Step 1: 收紧 Worker 注册表**

替换协议集合：

```python
if protocol not in {"openai_images", "jimeng_visual"}:
    raise ModelRegistryError("invalid_model_config", "图片模型协议无效")
```

- [x] **Step 2: 删除 Responses provider 与专用依赖**

删除以下符号及只被它们使用的代码：

```text
OpenAIResponsesImageProvider
Per-candidate candidate_index / attempt fields
ImageProvider.request_mode
FakeImageProvider/OpenAIImageProvider/JimengImageProvider.request_mode
_process_per_candidate_image_task
_log_structured
_sanitized_error_summary
```

从 imports 删除 `binascii`、`logging` 和 `replace`，保留 `base64`、`json`、`os`、`re`。

- [x] **Step 3: 恢复单一批量图片处理入口**

将 `_process_batch_image_task` 主体并回 `process_image_task`，删除基于 `request_mode` 的分派。保存素材时使用原有逻辑或保留纯内部 `_save_generated_image` 均可，但最终不得残留逐候选分支、候选 attempt 或不可达 mode。

- [x] **Step 4: 删除 provider factory 的 Responses 分支**

`image_provider_from_model()` 第一分支恢复为 `openai_images`，然后是 `jimeng_visual`，其他协议统一抛出：

```python
raise ProviderConfigError(f"unsupported image protocol: {config.api_protocol}")
```

- [x] **Step 5: 清理仅服务 Responses 的测试**

删除 `PerCandidateImageProvider`、四个 `test_per_candidate_*`、batch/request_mode 测试，以及所有 `test_responses_image_provider_*`。保留并运行现有批量重试、部分成功、OpenAI Images、即梦和任务领取测试。

- [x] **Step 6: 把本地参考图测试从 Responses 解耦**

用独立测试替换 `test_responses_image_provider_reads_reference_from_managed_asset_storage`：

```python
def test_default_binary_get_reads_managed_asset_and_rejects_escape(tmp_path, monkeypatch):
    asset_root = tmp_path / "assets"
    reference_path = asset_root / "existing" / "reference.png"
    reference_path.parent.mkdir(parents=True)
    reference_path.write_bytes(b"managed-reference")
    monkeypatch.setenv("ASSET_STORAGE_ROOT", str(asset_root))
    monkeypatch.setenv("ASSET_PUBLIC_PREFIX", "/assets")

    assert asset_generation.default_binary_get(
        "/assets/existing/reference.png"
    ) == b"managed-reference"
    with pytest.raises(
        asset_generation.PermanentProviderError,
        match="escapes asset storage root",
    ):
        asset_generation.default_binary_get("/assets/../outside.png")
```

- [x] **Step 7: 运行 Worker 聚焦与全量测试**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc 'cd /app && pytest tests/test_model_registry.py tests/test_asset_generation.py -q'
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc 'cd /app && pytest tests -q'
```

预期：全部通过，测试收集结果不再包含 `responses_image_provider` 或 `per_candidate`，本地参考图测试通过。

- [x] **Step 8: 更新 OpenSpec Worker 任务**

根据实际结果勾选 `2.2`、`2.4`、`4.1`、`4.2`、`4.3`。

### Task 5: 删除 Admin 图片协议选项并保留默认 POST

**Files:**
- Modify: `admin/app/models/ModelManagementPage.tsx:23`
- Modify: `admin/app/models/page.test.tsx:74`
- Preserve: `admin/app/lib/api.ts:268`
- Preserve: `admin/app/lib/api.test.ts:284`

- [x] **Step 1: 删除图片协议选项**

图片映射最终为：

```typescript
image: [
  { value: "openai_images", label: "OpenAI Images" },
  { value: "jimeng_visual", label: "即梦 Visual" },
],
```

文本映射中的 `openai_responses` 保持不变。

- [x] **Step 2: 确认默认模型请求修复未被改动**

`setDefaultAiModel()` 必须继续包含：

```typescript
return request<AiModel>(client, `/api/admin/models/${modelId}/default`, {
  method: "POST",
  body: payload,
});
```

- [x] **Step 3: 运行 Admin 聚焦和全量验证**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc 'cd /app && npm test -- app/models/page.test.tsx app/lib/api.test.ts'
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc 'cd /app && npm test'
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc 'cd /app && npm run lint'
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc 'cd /app && npm run build'
```

预期：全部通过；页面测试同时证明图片协议下拉不含 Responses、默认切换使用 `POST`。

- [x] **Step 4: 更新 OpenSpec Admin 任务**

勾选 `2.3`、`4.4`。

### Task 6: 更新记忆、验证迁移并重建服务

**Files:**
- Modify: `MEMORY.md:53`
- Modify: `docs/memory/video-agent-workspace-flow.md:147`
- Modify: `openspec/changes/remove-image-responses-protocol/tasks.md`

- [x] **Step 1: 更新稳定记忆**

用以下新决策覆盖原支持条目，不记录凭据或一次性请求 ID：

```markdown
- 2026-07-13 用户确认回退 `model_type=image + api_protocol=openai_responses`：图片模型恢复只允许 `openai_images | jimeng_visual`，文本 Responses 不变。已执行的放开 migration 作为历史保留，并通过追加 migration 恢复最终数据库约束；Rust、Worker、Admin 和 Pencil 原型同步删除该组合。Admin “设为默认”使用 `POST` 与 Worker `/assets/...` 本地参考图安全读取属于独立修复，继续保留。
```

- [x] **Step 2: 运行格式化、全量测试与静态检查**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo fmt --all -- --check'
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test --workspace'
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc 'cd /app && pytest tests -q'
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc 'cd /app && npm test && npm run lint && npm run build'
openspec validate remove-image-responses-protocol --strict
openspec instructions apply --change remove-image-responses-protocol --json
git diff --check
```

预期：所有命令退出码为 `0`；OpenSpec apply state 与实际任务一致。

- [x] **Step 3: 部署前再次检查冲突记录**

```bash
docker compose -f /server/docker-compose.yml exec -T biga-postgres psql -U postgres -d video_agent -v ON_ERROR_STOP=1 -c "SELECT COUNT(*) FROM ai_models WHERE model_type='image' AND api_protocol='openai_responses';"
```

预期：`0`。若非 `0`，停止部署并报告具体记录，不自动修改协议。

- [x] **Step 4: 重建并启动 API、Worker、Admin**

```bash
docker compose -f /server/docker-compose.yml up -d --build ai-agent-api ai-agent-video-worker ai-agent-admin
```

等待命令完成，不保留未收尾的构建会话。

- [x] **Step 5: 验证 migration、约束和服务健康**

```bash
docker compose -f /server/docker-compose.yml exec -T biga-postgres psql -U postgres -d video_agent -v ON_ERROR_STOP=1 -c "SELECT version, description, success FROM _sqlx_migrations WHERE version IN (20260713010000, 20260713020000) ORDER BY version;" -c "SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE conrelid='ai_models'::regclass AND conname='ai_models_type_protocol_check';"
curl -fsS http://localhost:18180/health
curl -fsS http://localhost:18181/health
curl -fsS http://localhost:18182/models >/dev/null
```

预期：两条 migration 均成功；最终约束的图片集合只有 `openai_images`、`jimeng_visual`；三个服务请求成功。

- [x] **Step 6: 验证数据库直接拒绝并回滚测试事务**

```bash
docker compose -f /server/docker-compose.yml exec -T biga-postgres psql -U postgres -d video_agent -v ON_ERROR_STOP=1 -c "BEGIN; INSERT INTO ai_models (display_name, model_type, provider_name, api_protocol, auth_scheme, request_base_url, upstream_model, api_key) VALUES ('Rollback Constraint Probe', 'image', 'test', 'openai_responses', 'bearer', 'https://example.invalid/v1', 'test-image', 'test-key'); ROLLBACK;"
```

预期：INSERT 因 `ai_models_type_protocol_check` 失败；测试记录不会持久化。失败是本步骤成功证据，需单独捕获预期非零退出码，不能让后续验证被跳过。

- [x] **Step 7: 验证 API 与 Admin 最终行为且不调用上游**

使用无真实凭据的图片 Responses 创建请求调用本地 Admin API，预期 `422 invalid_model_config`；打开或测试 `/models` 确认图片协议只有 OpenAI Images 和即梦。不得创建素材任务或调用真实供应商。

- [x] **Step 8: 完成 OpenSpec 状态**

逐项核对后勾选 `5.1` 至 `5.5` 以及所有已完成任务，再运行：

```bash
openspec instructions apply --change remove-image-responses-protocol --json
```

预期：`state=all_done`。最终再次运行 `git diff --check` 和 `git status --short`，确认没有覆盖工作区其他改动。
