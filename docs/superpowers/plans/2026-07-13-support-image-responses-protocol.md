# Image Responses Protocol Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. This repository forbids `git add`, `git commit`, and `git push` without explicit user confirmation, so commit steps are intentionally omitted.

**Goal:** Allow only `model_type=image + api_protocol=openai_responses` and generate each requested image candidate through one non-streaming Responses API call with precise retry, audit, storage, and sanitized logging behavior.

**Architecture:** Extend the shared protocol compatibility matrix and database constraint without adding a duplicate protocol enum. The Rust control plane continues to validate and enqueue image tasks; the Python Worker selects a new per-candidate Responses provider, while existing OpenAI Images and Jimeng providers remain batch providers. The Admin form exposes the new compatible option after Pencil confirmation.

**Tech Stack:** Rust/Axum/SQLx/PostgreSQL, Python 3.12/FastAPI/urllib/pytest, Next.js 14/React/Vitest, OpenSpec, Pencil MCP, Docker Compose.

---

### Task 1: Lock The Compatibility Contract With Failing Rust Tests

**Files:**
- Modify: `crates/novex-model/src/registry.rs`
- Modify: `backend/tests/ai_model_routes.rs`
- Modify: `backend/tests/database_migrations.rs`
- Modify: `backend/src/application/asset_generation.rs`

- [ ] **Step 1: Add a unit test for the exact compatibility matrix**

Add a `#[cfg(test)]` module in `crates/novex-model/src/registry.rs` or extend its existing tests:

```rust
#[test]
fn responses_supports_text_and_image_but_not_video() {
    assert!(ApiProtocol::OpenAiResponses.supports(ModelType::Text));
    assert!(ApiProtocol::OpenAiResponses.supports(ModelType::Image));
    assert!(!ApiProtocol::OpenAiResponses.supports(ModelType::Video));
    assert!(!ApiProtocol::OpenAiChatCompletions.supports(ModelType::Image));
}
```

- [ ] **Step 2: Add an API test that creates an image Responses model**

In `backend/tests/ai_model_routes.rs`, construct a valid image payload with:

```rust
json!({
    "display_name": "Responses Image",
    "model_type": "image",
    "provider_name": "zeek-ai",
    "api_protocol": "openai_responses",
    "protocol_version": "v1",
    "auth_scheme": "bearer",
    "request_base_url": "https://api.example.com/v1",
    "upstream_model": "gpt-image-2",
    "api_key": "secret-key-1234",
    "api_secret": null,
    "timeout_seconds": 120,
    "reasoning_effort": null,
    "max_output_tokens": null,
    "settings": {
        "supported_sizes": ["1024x1024"],
        "default_size": "1024x1024",
        "max_images_per_request": 4
    },
    "sort_order": 0,
    "remark": "",
    "is_default": false
})
```

Assert `201 Created`, `model_type=image`, `api_protocol=openai_responses`, and masked credentials. Retain a separate assertion that `image + openai_chat_completions` returns `422 invalid_model_config`.

- [ ] **Step 3: Add a migration contract test for allowed and rejected rows**

After migrations, use direct SQL inserts inside a transaction to assert:

```sql
-- accepted
('image', 'openai_responses', 'bearer')

-- rejected by ai_models_type_protocol_check
('image', 'openai_chat_completions', 'bearer')
('video', 'openai_responses', 'bearer')
```

- [ ] **Step 4: Run the tests and verify RED**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc \
  'cd /app && /usr/local/cargo/bin/cargo test -p novex-model responses_supports_text_and_image_but_not_video && \
   /usr/local/cargo/bin/cargo test -p novex-api --test ai_model_routes && \
   /usr/local/cargo/bin/cargo test -p novex-api --test database_migrations'
```

Expected: the new compatibility assertions fail because `OpenAiResponses` only supports `ModelType::Text`, and the database rejects the image row.

### Task 2: Implement The Database And Rust Compatibility Matrix

**Files:**
- Create: `backend/migrations/20260713010000_image_responses_protocol.sql`
- Modify: `crates/novex-model/src/registry.rs`
- Modify: `backend/src/application/asset_generation.rs`

- [ ] **Step 1: Add the append-only migration**

Create:

```sql
ALTER TABLE ai_models
    DROP CONSTRAINT ai_models_type_protocol_check;

ALTER TABLE ai_models
    ADD CONSTRAINT ai_models_type_protocol_check CHECK (
        (model_type = 'text' AND api_protocol IN ('openai_responses', 'openai_chat_completions')) OR
        (model_type = 'image' AND api_protocol IN ('openai_images', 'openai_responses', 'jimeng_visual')) OR
        (model_type = 'video' AND api_protocol IN ('runway_api', 'kling_api'))
    );

COMMENT ON CONSTRAINT ai_models_type_protocol_check ON ai_models IS
    '限制模型类型与可执行协议组合；图片模型额外支持 Responses 图片工具协议。';
```

- [ ] **Step 2: Extend `ApiProtocol::supports`**

Use an explicit match so only Responses crosses the text/image boundary:

```rust
pub const fn supports(self, model_type: ModelType) -> bool {
    matches!(
        (self, model_type),
        (Self::OpenAiResponses, ModelType::Text | ModelType::Image)
            | (Self::OpenAiChatCompletions, ModelType::Text)
            | (Self::OpenAiImages | Self::JimengVisual, ModelType::Image)
            | (Self::RunwayApi | Self::KlingApi, ModelType::Video)
    )
}
```

- [ ] **Step 3: Allow Responses image tasks to use the existing image provider audit value**

Update `image_provider_for_protocol`:

```rust
match protocol {
    ApiProtocol::OpenAiImages | ApiProtocol::OpenAiResponses => {
        Ok(AssetGenerationProvider::GptImage2)
    }
    ApiProtocol::JimengVisual => Ok(AssetGenerationProvider::Jimeng),
    _ => Err(ModelResolveError::InvalidConfig(Uuid::nil()).into()),
}
```

- [ ] **Step 4: Run the Rust tests and verify GREEN**

Run the command from Task 1 Step 4. Expected: all selected tests pass.

### Task 3: Accept Image Responses Models In The Worker Registry

**Files:**
- Modify: `services/video-worker/tests/test_model_registry.py`
- Modify: `services/video-worker/src/video_worker/model_registry.py`

- [ ] **Step 1: Add the failing registry test**

```python
def test_loader_returns_openai_responses_image_runtime_config():
    registry, _ = registry_for(
        image_model_row(api_protocol="openai_responses", auth_scheme="bearer")
    )

    config = registry.resolve_enabled(MODEL_ID, "image")

    assert config.api_protocol == "openai_responses"
    assert config.auth_scheme == "bearer"
    assert config.snapshot()["model_type"] == "image"
    assert "api_key" not in config.snapshot()
```

Retain `runway_api` as a rejected image protocol.

- [ ] **Step 2: Run the test and verify RED**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc \
  'cd /app && pytest tests/test_model_registry.py -q'
```

Expected: `ModelRegistryError("invalid_model_config")` for `openai_responses`.

- [ ] **Step 3: Extend the allowed protocol set**

```python
if protocol not in {"openai_images", "openai_responses", "jimeng_visual"}:
    raise ModelRegistryError("invalid_model_config", "图片模型协议无效")
```

Keep Bearer auth for both OpenAI protocols.

- [ ] **Step 4: Run the registry tests and verify GREEN**

Run the command from Step 2. Expected: all tests pass.

### Task 4: Add A Strict Non-Streaming Responses Image Provider

**Files:**
- Modify: `services/video-worker/tests/test_asset_generation.py`
- Modify: `services/video-worker/src/video_worker/asset_generation.py`

- [ ] **Step 1: Add failing request-shape and parsing tests**

Create tests that inject `http_post` and capture URL, headers, and payload:

```python
def test_responses_image_provider_posts_one_candidate_to_responses():
    requests = []

    def post(url, headers, payload):
        requests.append({"url": url, "headers": headers, "payload": payload})
        return {
            "output": [{"type": "image_generation_call", "result": base64.b64encode(b"png").decode()}]
        }

    provider = OpenAIResponsesImageProvider(
        api_key="test-key",
        model="gpt-image-2",
        base_url="https://proxy.example/v1",
        default_size="1024x1024",
        http_post=post,
    )

    images = provider.generate_images(image_task(candidate_count=1))

    assert requests[0]["url"] == "https://proxy.example/v1/responses"
    assert requests[0]["payload"]["model"] == "gpt-image-2"
    assert requests[0]["payload"]["tools"] == [{"type": "image_generation", "size": "1024x1024"}]
    assert requests[0]["payload"]["tool_choice"] == {"type": "image_generation"}
    assert "n" not in requests[0]["payload"]
    assert images[0].content == b"png"
```

Add tests for reference `input_image`, missing `image_generation_call`, and invalid base64. Assert no fallback fields are parsed.

- [ ] **Step 2: Add failing sanitized-log tests**

Use `caplog` and assert logs contain URL/model/prompt/candidate/attempt but exclude `test-key`, `Authorization`, input image base64, and result base64.

- [ ] **Step 3: Run focused tests and verify RED**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc \
  'cd /app && pytest tests/test_asset_generation.py -q -k "responses_image"'
```

Expected: import or assertion failures because the provider does not exist.

- [ ] **Step 4: Implement the provider and strict parser**

Add `OpenAIResponsesImageProvider` with `request_mode = "per_candidate"`. Build structured `input`, append data-URL `input_image` items for references, POST `/responses`, and parse only:

```python
output = response.get("output")
if not isinstance(output, list):
    raise PermanentProviderError("Responses image response missing output")

for item in output:
    if isinstance(item, dict) and item.get("type") == "image_generation_call":
        result = item.get("result")
        if isinstance(result, str) and result:
            try:
                content = base64.b64decode(result, validate=True)
            except (ValueError, binascii.Error) as error:
                raise PermanentProviderError("Responses image result is not valid base64") from error
            return [GeneratedImage(filename=f"{task.task_id}.png", content=content)]

raise PermanentProviderError("Responses image response missing image_generation_call result")
```

- [ ] **Step 5: Route the model protocol to the provider**

In `image_provider_from_model`, add `openai_responses` before `openai_images`, pass `settings.default_size`, and preserve the configured base URL and timeout.

- [ ] **Step 6: Run focused tests and verify GREEN**

Run the command from Step 3. Expected: all selected tests pass.

### Task 5: Execute Responses Calls Per Candidate Without Duplicating Successes

**Files:**
- Modify: `services/video-worker/tests/test_asset_generation.py`
- Modify: `services/video-worker/src/video_worker/asset_generation.py`

- [ ] **Step 1: Add a failing three-candidate test**

Use a per-candidate fake provider with outcomes `[success, success, temporary error, success-on-retry]`. Assert four total calls, three materials, and `retry_count == 1`.

- [ ] **Step 2: Add failing partial and permanent-error tests**

Cover:

```text
temporary + retry failure, then next candidate success -> completed, partial=true
success, then permanent error -> one success retained, remaining candidates/scenes not called
```

- [ ] **Step 3: Run focused tests and verify RED**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc \
  'cd /app && pytest tests/test_asset_generation.py -q -k "per_candidate or responses_candidate"'
```

- [ ] **Step 4: Split batch and per-candidate processing explicitly**

Keep the existing batch path in a focused helper and add a per-candidate helper that creates `candidate_count=1` sub-tasks. Retry only the current sub-task. Aggregate `materials`, `failed_count`, `retry_count`, `error_message`, and `fatal` into one `ImageTaskResult`.

- [ ] **Step 5: Run the full Worker suite**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc \
  'cd /app && pytest tests -q'
```

Expected: all tests pass, including existing OpenAI Images/Jimeng batch behavior.

### Task 6: Expose OpenAI Responses For Image Models In Admin

**Files:**
- Modify: `admin/app/models/page.test.tsx`
- Modify: `admin/app/models/ModelManagementPage.tsx`

- [ ] **Step 1: Add the failing page test after Pencil approval**

After changing the model type to image, assert the protocol select contains exactly:

```typescript
expect(screen.getByRole("option", { name: "OpenAI Images" })).toBeInTheDocument();
expect(screen.getByRole("option", { name: "OpenAI Responses" })).toBeInTheDocument();
expect(screen.getByRole("option", { name: "即梦 Visual" })).toBeInTheDocument();
expect(screen.queryByRole("option", { name: "OpenAI Chat Completions" })).not.toBeInTheDocument();
```

Select `openai_responses` and assert `API Secret` is absent because auth remains Bearer.

- [ ] **Step 2: Run the page test and verify RED**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc \
  'cd /app && npm test -- app/models/page.test.tsx'
```

Expected: `OpenAI Responses` is missing from the image protocol options.

- [ ] **Step 3: Add the compatible option**

```typescript
image: [
  { value: "openai_images", label: "OpenAI Images" },
  { value: "openai_responses", label: "OpenAI Responses" },
  { value: "jimeng_visual", label: "即梦 Visual" },
],
```

No layout or styling changes are required.

- [ ] **Step 4: Run Admin tests, lint, and build**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc \
  'cd /app && npm test && npm run lint && npm run build'
```

Expected: all commands exit `0`.

### Task 7: Verify OpenSpec, Workspace, And A Controlled Runtime Call

**Files:**
- Modify: `openspec/changes/support-image-responses-protocol/tasks.md`

- [ ] **Step 1: Format and run Rust verification**

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc \
  'cd /app && /usr/local/cargo/bin/cargo fmt --all --check && /usr/local/cargo/bin/cargo test --workspace'
```

- [ ] **Step 2: Re-run Worker and Admin verification**

Run the full commands from Tasks 5 and 6.

- [ ] **Step 3: Validate artifacts and diff hygiene**

```bash
openspec validate support-image-responses-protocol --strict
openspec instructions apply --change "support-image-responses-protocol" --json
git diff --check
git status --short
```

- [ ] **Step 4: Rebuild affected services**

```bash
docker compose -f /server/docker-compose.yml up -d --build \
  ai-agent-api ai-agent-video-worker ai-agent-admin
```

Verify `/health` for API and Worker, and confirm Worker reports `asset_generation_worker=enabled`.

- [ ] **Step 5: Configure and execute one paid candidate**

Update the existing target model through the Admin API/UI to:

```text
model_type=image
api_protocol=openai_responses
request_base_url=https://api.zeekai.cc/v1
upstream_model=gpt-image-2
```

Create a single-scene, single-candidate task. Verify exactly one initial external request in sanitized logs, then verify task/material/candidate database rows. A temporary error may produce exactly one retry for that candidate.

- [ ] **Step 6: Update OpenSpec task status immediately**

Mark each completed checkbox in `openspec/changes/support-image-responses-protocol/tasks.md`, then confirm `openspec instructions apply` reports progress consistent with the implementation.
