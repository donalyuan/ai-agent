# 火山方舟图片协议替换 Jimeng Visual Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完整删除内部 `jimeng_visual`，新增可保存、可执行、可审计的 `volcengine_ark_images` Seedream 图片协议。

**Architecture:** PostgreSQL、Rust registry 与 Admin 维护同一协议矩阵；Backend 规范化 Ark 根地址并把协议映射成 `volcengine-ark` 任务审计值；Python Worker 使用独立 Ark HTTP adapter，并由任务编排层逐候选调用、重试和汇总。OpenAI Images 批量路径保持不变。

**Tech Stack:** PostgreSQL 16、SQLx、Rust/Axum、Python 3/pytest/urllib、Next.js 14/TypeScript/Vitest、Docker Compose、OpenSpec。

**执行约束:** 当前工作区包含用户未提交改动，只做逐块修改；未经用户明确授权不执行 `git add`、`git commit`、`git push`。自动化阶段禁止真实 Ark 请求。

---

### Task 1: 数据库约束与 Rust 协议矩阵

**Files:**
- Create: `backend/migrations/20260713030000_volcengine_ark_images.sql`
- Modify: `backend/tests/database_migrations.rs`
- Modify: `crates/novex-model/src/registry.rs`
- Modify: `crates/novex-model/tests/model_registry.rs`

- [ ] **Step 1: 写协议矩阵失败测试**

在 `model_registry.rs` 要求新协议只支持图片和 Bearer，并要求旧字符串解析失败：

```rust
assert!(ApiProtocol::VolcengineArkImages.supports(ModelType::Image));
assert!(!ApiProtocol::VolcengineArkImages.supports(ModelType::Text));
assert_eq!(
    ApiProtocol::VolcengineArkImages.required_auth(),
    AuthScheme::Bearer
);
assert!(ApiProtocol::from_str("jimeng_visual").is_err());
assert_eq!(
    ApiProtocol::from_str("volcengine_ark_images").unwrap(),
    ApiProtocol::VolcengineArkImages
);
```

- [ ] **Step 2: 运行 registry 测试并确认 RED**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api \
  sh -lc 'cd /app && cargo test -p novex-model --test model_registry'
```

Expected: 编译失败，提示 `VolcengineArkImages` 不存在。

- [ ] **Step 3: 写 migration 失败测试**

在 migration 测试中插入新组合并断言成功，同时插入旧协议/旧 provider 并断言失败：

```rust
let ark = insert_model("image", "volcengine_ark_images", "bearer").await;
assert!(ark.is_ok());
assert!(insert_model("image", "jimeng_visual", "access_key_secret").await.is_err());
assert!(insert_task_provider("volcengine-ark").await.is_ok());
assert!(insert_task_provider("jimeng").await.is_err());
```

- [ ] **Step 4: 运行 migration 测试并确认 RED**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api \
  sh -lc 'cd /app && cargo test -p novex-api --test database_migrations'
```

Expected: 新协议或新 provider 被现有 CHECK 拒绝。

- [ ] **Step 5: 实现 append-only migration**

迁移先执行冲突检查，再重建三个约束：

```sql
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM ai_models WHERE api_protocol = 'jimeng_visual') THEN
        RAISE EXCEPTION 'cannot remove jimeng_visual while matching ai_models records exist';
    END IF;
    IF EXISTS (SELECT 1 FROM asset_generation_tasks WHERE provider = 'jimeng') THEN
        RAISE EXCEPTION 'cannot remove jimeng provider while matching tasks exist';
    END IF;
END
$$;

ALTER TABLE ai_models DROP CONSTRAINT ai_models_protocol_check;
ALTER TABLE ai_models ADD CONSTRAINT ai_models_protocol_check CHECK (
    api_protocol IN (
        'openai_responses', 'openai_chat_completions', 'openai_images',
        'volcengine_ark_images', 'runway_api', 'kling_api'
    )
);
```

同一 migration 将图片类型约束改为 `openai_images | volcengine_ark_images`，任务 provider 改为 `gpt-image-2 | volcengine-ark`，并补充中文 COMMENT。

- [ ] **Step 6: 实现 Rust enum 与 settings 删除**

```rust
pub enum ApiProtocol {
    OpenAiResponses,
    OpenAiChatCompletions,
    OpenAiImages,
    VolcengineArkImages,
    RunwayApi,
    KlingApi,
}
```

更新 `as_str`、`supports`、`required_auth`、`FromStr`；从 `ImageModelSettings` 删除 `request_key`。

- [ ] **Step 7: 运行两组测试并确认 GREEN**

Run: Task 1 Step 2 与 Step 4 的两个命令。

Expected: 两组测试全部通过，旧协议无法解析或写入。

### Task 2: Backend 地址规范化、任务映射和旧导入删除

**Files:**
- Modify: `backend/src/api/ai_models/dto.rs`
- Modify: `backend/src/application/asset_generation.rs`
- Modify: `backend/src/repositories/asset_generation_repository.rs`
- Modify: `backend/src/model_config_import.rs`
- Modify: `backend/tests/ai_model_routes.rs`
- Modify: `backend/tests/asset_generation_routes.rs`
- Modify: `backend/tests/model_config_import.rs`
- Modify: `.env.example`

- [ ] **Step 1: 写 Ark 创建/更新地址失败测试**

新增 `ark_image_payload`，分别提交根地址、完整端点和非法 query：

```rust
assert_eq!(created["request_base_url"], "https://ark.cn-beijing.volces.com/api/v3");
assert_eq!(created["auth_scheme"], "bearer");
assert_eq!(invalid_status, StatusCode::UNPROCESSABLE_ENTITY);
assert_eq!(invalid_body["error"]["code"], "invalid_model_config");
```

- [ ] **Step 2: 写任务映射和导入失败测试**

```rust
assert_eq!(ark_plan["provider"], "volcengine-ark");
assert_eq!(import_outcome.created.len(), 2);
assert!(rows.iter().all(|row| row.api_protocol != "jimeng_visual"));
```

- [ ] **Step 3: 运行相关 Rust 测试并确认 RED**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc \
  'cd /app && cargo test -p novex-api --test ai_model_routes --test asset_generation_routes --test model_config_import'
```

Expected: 新协议 payload 无法反序列化或任务映射仍缺失。

- [ ] **Step 4: 实现结构化地址规范化**

在 DTO 转 input 前调用：

```rust
fn normalize_request_base_url(
    protocol: ApiProtocol,
    value: String,
) -> Result<String, ModelApiError> {
    if protocol != ApiProtocol::VolcengineArkImages {
        return Ok(value);
    }
    let mut url = url::Url::parse(value.trim()).map_err(invalid_model_config)?;
    if !matches!(url.scheme(), "http" | "https") || url.query().is_some() || url.fragment().is_some() {
        return Err(invalid_model_config("invalid Ark request URL"));
    }
    let path = url.path().trim_end_matches('/');
    let root = path.strip_suffix("/images/generations").unwrap_or(path);
    if !root.ends_with("/api/v3") {
        return Err(invalid_model_config("Ark URL must end with /api/v3"));
    }
    url.set_path(root);
    Ok(url.to_string().trim_end_matches('/').to_string())
}
```

创建和更新走同一 helper。

- [ ] **Step 5: 替换任务 provider 枚举**

```rust
pub enum AssetGenerationProvider {
    GptImage2,
    VolcengineArk,
}
```

`ApiProtocol::VolcengineArkImages` 映射到 `VolcengineArk`，字符串固定 `volcengine-ark`。

- [ ] **Step 6: 删除旧 Jimeng 环境导入**

从 `LegacyModelImportConfig`、`from_env` 和 `import_legacy_model_config` 删除所有 `jimeng_*` 字段/分支；从 `.env.example` 删除 `JIMENG_*`。保留文本 OpenAI 和 OpenAI Images 导入。

- [ ] **Step 7: 运行相关测试并确认 GREEN**

Run: Task 2 Step 3 命令。

Expected: 三个测试目标通过，地址与 provider 快照符合规格。

### Task 3: Worker Ark HTTP adapter

**Files:**
- Modify: `services/video-worker/src/video_worker/model_registry.py`
- Modify: `services/video-worker/src/video_worker/asset_generation.py`
- Modify: `services/video-worker/tests/test_model_registry.py`
- Modify: `services/video-worker/tests/test_asset_generation.py`

- [ ] **Step 1: 写 registry 与 provider factory 失败测试**

```python
row = image_model_row(
    api_protocol="volcengine_ark_images",
    auth_scheme="bearer",
    request_base_url="https://ark.cn-beijing.volces.com/api/v3",
)
config = registry.resolve_enabled(MODEL_ID, "image")
assert config.api_protocol == "volcengine_ark_images"
assert isinstance(image_provider_from_model(config), VolcengineArkImageProvider)
```

- [ ] **Step 2: 写单次请求与响应失败测试**

fake transport 捕获 URL、headers、payload，并返回一张最小 PNG base64：

```python
assert url.endswith("/api/v3/images/generations")
assert headers["Authorization"] == "Bearer test-key"
assert payload["sequential_image_generation"] == "disabled"
assert payload["response_format"] == "b64_json"
assert payload["stream"] is False
assert payload["watermark"] is False
assert "n" not in payload
assert images[0].filename.endswith(".png")
```

- [ ] **Step 3: 写参考图与严格解析失败测试**

覆盖 PNG/JPEG/WebP data URL、`data[].error`、非法 base64 和未知 magic bytes；断言请求日志摘要不含 data URL 正文。

- [ ] **Step 4: 运行 Worker adapter 测试并确认 RED**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc \
  'cd /app && pytest tests/test_model_registry.py tests/test_asset_generation.py -q'
```

Expected: `VolcengineArkImageProvider` 不存在或新协议被 registry 拒绝。

- [ ] **Step 5: 实现 registry 与 Ark provider**

`model_registry.py` 只允许 `openai_images | volcengine_ark_images`，两者都要求 `bearer`。Provider 公开单候选模式：

```python
class VolcengineArkImageProvider:
    request_mode = "per_candidate"

    def generate_images(self, task: AssetGenerationTask) -> list[GeneratedImage]:
        if task.candidate_count != 1:
            raise ProviderConfigError("Ark provider requires one candidate per request")
        payload = self._build_payload(task)
        response = self.http_post(
            f"{self.base_url}/images/generations",
            {"Authorization": f"Bearer {self.api_key}", "Content-Type": "application/json"},
            payload,
        )
        return [parse_ark_image(task.task_id, response)]
```

参考图使用现有 `default_binary_get`，新增 `detect_image_type` 和 `image_data_url`；响应按 magic bytes选择扩展名。

- [ ] **Step 6: 实现脱敏日志模型**

日志 payload 使用副本，把 `image` 替换成 `"<redacted:N reference image(s)>"`，Authorization 固定 `Bearer ***`；响应只记录 data 数量与字节数。通过标准 `logging` 输出 JSON，不输出 `b64_json`。

- [ ] **Step 7: 运行 adapter 测试并确认 GREEN**

Run: Task 3 Step 4 命令。

Expected: provider、参考图、解析和日志测试通过，旧 Jimeng 测试已删除。

### Task 4: Worker 逐候选执行与费用边界

**Files:**
- Modify: `services/video-worker/src/video_worker/asset_generation.py`
- Modify: `services/video-worker/tests/test_asset_generation.py`

- [ ] **Step 1: 写逐候选失败测试**

构造 3 候选 fake provider，断言每次 task 的 `candidate_count == 1` 且共调用 3 次。

- [ ] **Step 2: 写重试/永久错误失败测试**

覆盖：候选 2 临时失败后成功只多一次调用；临时重试耗尽后继续候选 3；永久错误后不调用剩余候选；此前成功素材不重复。

- [ ] **Step 3: 运行逐候选测试并确认 RED**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc \
  'cd /app && pytest tests/test_asset_generation.py -q'
```

Expected: 现有按场景批量调用导致 call count 或失败停止断言不符。

- [ ] **Step 4: 实现显式执行模式**

`process_image_task` 保留 OpenAI batch 路径，新增 `process_per_candidate_image_task`：

```python
for candidate_index in range(task.candidate_count):
    single = replace(task, task_id=f"{task.task_id}-{candidate_index + 1}", candidate_count=1)
    result = process_single_candidate(single, provider, storage, candidate_index)
    if result.fatal:
        failed_count += task.candidate_count - candidate_index - 1
        break
```

临时错误重试耗尽返回 `fatal=False`，永久错误返回 `fatal=True`；累计真实 retry_count 和候选数量。

- [ ] **Step 5: 运行逐候选和 Worker 全量测试并确认 GREEN**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc \
  'cd /app && pytest tests -q'
```

Expected: 全量通过，OpenAI Images 既有 batch 行为不变。

### Task 5: Admin 协议与表单联动

**Files:**
- Modify: `admin/app/lib/api.ts`
- Modify: `admin/app/models/ModelManagementPage.tsx`
- Modify: `admin/app/models/page.test.tsx`
- Modify: `admin/app/lib/api.test.ts`
- Modify: `admin/e2e/workspace.spec.ts`

- [ ] **Step 1: 写协议选项和认证失败测试**

```typescript
expect(screen.getByRole("option", { name: "火山方舟图片生成" })).toBeInTheDocument();
expect(screen.queryByRole("option", { name: "即梦 Visual" })).not.toBeInTheDocument();
fireEvent.change(protocol, { target: { value: "volcengine_ark_images" } });
expect(screen.queryByLabelText("API Secret")).not.toBeInTheDocument();
```

- [ ] **Step 2: 写空尺寸和单次最大数失败测试**

提交 Ark 表单后断言 payload：

```typescript
expect(payload.settings).toEqual({
  supported_sizes: [],
  default_size: null,
  max_images_per_request: 1,
});
expect(payload.auth_scheme).toBe("bearer");
```

- [ ] **Step 3: 运行 Admin 测试并确认 RED**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc \
  'cd /app && npm test -- --run app/models/page.test.tsx app/lib/api.test.ts'
```

Expected: 新协议类型或选项不存在，旧 Secret 仍显示。

- [ ] **Step 4: 实现 Admin 类型与联动**

```typescript
image: [
  { value: "openai_images", label: "OpenAI Images" },
  { value: "volcengine_ark_images", label: "火山方舟图片生成" },
]
```

`changeProtocol` 对 Ark 设置 `bearer`；图片尺寸 trim 后为空时提交空数组/null；Ark 强制最大数 1，表单显示该固定值。

- [ ] **Step 5: 运行 Admin 测试并确认 GREEN**

Run: Task 5 Step 3 命令。

Expected: 页面与 API helper 测试通过。

### Task 6: 综合验证、部署和 see-dream 保存

**Files:**
- Modify: `openspec/changes/replace-jimeng-with-volcengine-ark-images/tasks.md`
- Verify: runtime PostgreSQL、API、Worker、Admin

- [ ] **Step 1: 运行全量自动化**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api \
  sh -lc 'cd /app && cargo test --workspace'
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker \
  sh -lc 'cd /app && pytest tests -q'
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin \
  sh -lc 'cd /app && npm test -- --run && npm run lint && npm run build'
```

Expected: 全部命令退出码 0，不出现真实 Ark 网络请求。

- [ ] **Step 2: 运行规格与差异校验**

```bash
openspec validate replace-jimeng-with-volcengine-ark-images --strict --no-interactive
openspec instructions apply --change replace-jimeng-with-volcengine-ark-images --json
git diff --check
```

Expected: OpenSpec valid；实现任务与实际状态一致；diff 无空白错误。

- [ ] **Step 3: 部署前检查费用与迁移冲突**

查询 `jimeng_visual` 模型、`provider='jimeng'` 任务和 `pending/processing` 图片任务，三项必须均为 0；确认 `ASSET_GENERATION_WORKER_ENABLED` 未开启。

- [ ] **Step 4: 重建并验证服务**

```bash
docker compose -f /server/docker-compose.yml up -d --build \
  ai-agent-api ai-agent-video-worker ai-agent-admin
curl -fsS http://127.0.0.1:18180/health
curl -fsS http://127.0.0.1:18181/health
```

Expected: API 与 Worker 健康，migration 已应用。

- [ ] **Step 5: 保存 see-dream 配置但不生成图片**

读取当前模型版本后，通过管理 API 更新为：

```json
{
  "api_protocol": "volcengine_ark_images",
  "auth_scheme": "bearer",
  "request_base_url": "https://ark.cn-beijing.volces.com/api/v3/images/generations",
  "upstream_model": "doubao-seedream-5-0-260128",
  "settings": {
    "supported_sizes": [],
    "default_size": null,
    "max_images_per_request": 1
  }
}
```

Expected: 保存成功，响应根地址为 `https://ark.cn-beijing.volces.com/api/v3`，Key 仍为 configured，Secret 不再需要；没有生成任务或上游调用。

- [ ] **Step 6: 停在真实调用门禁**

向用户报告自动化、部署和保存结果，并再次请求单分镜、单候选真实验证确认。未确认前不得启用 Worker 或创建计费任务。
