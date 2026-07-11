# AI Model Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 Novex 后台统一管理文本、图片、视频模型部署，并让视频工作台所有现有真实模型调用显式选择数据库模型。

**Architecture:** PostgreSQL `ai_models` 是运行时唯一配置来源；`backend` 提供管理、选项和模型解析服务，`crates/novex-model` 依据显式 API 调用协议构造文本客户端，Python Worker 按任务 `model_id` 构造图片 provider。新调用记录 `model_id` 与非敏感快照，运行时不再回退环境变量或硬编码供应商。

**Tech Stack:** Rust、Axum、SQLx、PostgreSQL、Python、FastAPI、Next.js 14、TypeScript、Vitest、Playwright、Pencil MCP、OpenSpec。

**Repository rule:** 未经用户明确确认不得执行 `git add`、`git commit` 或 `git push`；本计划不包含提交步骤。

---

### Task 1: 完成两套 Pencil 原型门禁

**Files:**
- Create: `docs/prototypes/admin/model-management.pen`
- Modify: `docs/prototypes/video-agent/video-agent.pen`
- Reference: `DESIGN.md`
- Reference: `docs/superpowers/specs/2026-07-10-ai-model-management-design.md`

- [x] **Step 1: 使用 Pencil MCP 创建后台模型管理主界面**

创建桌面端 `1440x960` 画板，包含现有 NOVEX ADMIN 导航、类型切换与筛选栏、模型表格、添加按钮。表格固定展示名称/模型标识、类型、供应商/API 协议、请求地址、默认、状态、最近调用、更新时间和行操作。

- [x] **Step 2: 创建后台编辑与破坏性操作状态**

在同一 `.pen` 文件增加编辑抽屉、停用默认模型时选择替代模型的弹窗、已引用模型逻辑删除确认弹窗。凭据输入显示掩码状态，不出现任何真实 Key。

- [x] **Step 3: 更新视频工作台六类调用状态**

在 `video-agent.pen` 覆盖账号策略草稿、当前选题池、历史补充、主题组评审、从选题生成脚本确认、脚本 Agent 和素材生成；复用紧凑模型选择控件，素材页将“供应商”改为“图片模型”。

- [x] **Step 4: 增加无可用模型状态**

至少在一个文本调用和图片调用画板中展示空选项、禁用命令和原地错误，不新增视频生成入口。

- [x] **Step 5: 验证并等待确认**

使用 Pencil MCP `batch_get`、`get_screenshot` 和 `snapshot_layout(problemsOnly=true)` 验证节点、视觉和布局；用户明确回复“按这个原型开发”之前不得执行 Task 2。

### Task 2: 建立模型协议领域类型与数据库 schema

**Files:**
- Create: `backend/migrations/20260710040000_ai_model_management.sql`
- Create: `crates/novex-model/src/registry.rs`
- Modify: `crates/novex-model/src/lib.rs`
- Modify: `backend/tests/database_migrations.rs`
- Test: `crates/novex-model/tests/model_registry.rs`

- [x] **Step 1: 先写协议兼容矩阵失败测试**

```rust
#[test]
fn protocol_must_match_model_type() {
    assert!(ApiProtocol::OpenAiResponses.supports(ModelType::Text));
    assert!(ApiProtocol::OpenAiImages.supports(ModelType::Image));
    assert!(!ApiProtocol::JimengVisual.supports(ModelType::Text));
    assert_eq!(ApiProtocol::RunwayApi.required_auth(), AuthScheme::Bearer);
}
```

- [x] **Step 2: 在 API 容器运行测试并确认失败**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-model --test model_registry'`

Expected: FAIL，提示 `ApiProtocol`、`ModelType` 或 `AuthScheme` 尚未定义。

- [x] **Step 3: 实现共享领域 enum 与类型化设置**

```rust
pub enum ModelType { Text, Image, Video }
pub enum ApiProtocol {
    OpenAiResponses,
    OpenAiChatCompletions,
    OpenAiImages,
    JimengVisual,
    RunwayApi,
    KlingApi,
}
pub enum AuthScheme { Bearer, AccessKeySecret }

pub struct ModelRuntimeConfig {
    pub model_id: Uuid,
    pub model_type: ModelType,
    pub api_protocol: ApiProtocol,
    pub request_base_url: String,
    pub upstream_model: String,
    pub api_key: String,
    pub api_secret: Option<String>,
    pub timeout_seconds: u64,
    pub reasoning_effort: Option<String>,
    pub max_output_tokens: Option<u32>,
    pub settings: ModelSettings,
}
```

- [x] **Step 4: 先写 migration 断言**

在 `database_migrations.rs` 断言 `ai_models` 字段、状态/类型/协议 check、每类型默认部分唯一索引，以及 `agent_runs`、`asset_generation_tasks` 的 `model_id` 和 `model_snapshot`。

- [x] **Step 5: 新增追加式 migration**

SQL 必须创建 `ai_models`，为历史兼容把新增引用设为可空，外键删除策略为 `RESTRICT`；移除 `asset_generation_tasks.provider` 的固定两值 check，但保留该列作为历史供应商快照。

- [x] **Step 6: 运行 migration 与领域测试**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-model --test model_registry && cargo test -p novex-api --test database_migrations'`

Expected: PASS，且不启动 Worker。

### Task 3: 实现模型仓储和生命周期事务

**Files:**
- Create: `backend/src/repositories/ai_model_repository.rs`
- Modify: `backend/src/repositories/mod.rs`
- Create: `backend/tests/ai_model_repository_contract.rs`
- Modify: `backend/tests/support/test_database.rs`

- [x] **Step 1: 先写仓储契约失败测试**

```rust
#[tokio::test]
async fn replacing_default_is_atomic_and_versioned() {
    let text_a = repository.create(enabled_text("A")).await.unwrap();
    let text_b = repository.create(enabled_text("B")).await.unwrap();
    repository.set_default(text_b.id, text_b.version).await.unwrap();
    assert!(!repository.get(text_a.id).await.unwrap().unwrap().is_default);
    assert!(repository.get(text_b.id).await.unwrap().unwrap().is_default);
}
```

同时覆盖第一条启用模型自动默认、旧版本更新冲突、停用默认的替代模型、无替代关闭能力、已引用逻辑删除和未引用物理删除。

- [x] **Step 2: 运行契约测试并确认失败**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-api --test ai_model_repository_contract'`

Expected: FAIL，提示 repository 未定义。

- [x] **Step 3: 实现仓储 trait 与 PostgreSQL 实现**

```rust
#[async_trait]
pub trait AiModelRepository: Send + Sync {
    async fn create(&self, input: CreateAiModel) -> Result<AiModel, AiModelRepositoryError>;
    async fn update(&self, id: Uuid, version: i64, input: UpdateAiModel) -> Result<AiModel, AiModelRepositoryError>;
    async fn resolve_enabled(&self, id: Uuid, expected: ModelType) -> Result<ModelRuntimeConfig, AiModelRepositoryError>;
    async fn replace_default(&self, id: Uuid, version: i64) -> Result<AiModel, AiModelRepositoryError>;
    async fn change_status(&self, input: ChangeAiModelStatus) -> Result<AiModel, AiModelRepositoryError>;
    async fn delete(&self, input: DeleteAiModel) -> Result<DeleteAiModelOutcome, AiModelRepositoryError>;
}
```

- [x] **Step 4: 在事务中实现默认、状态和删除规则**

使用 `SELECT ... FOR UPDATE` 锁定同类型模型；所有更新带 `WHERE version = $expected`。逻辑删除必须清除默认状态，保留引用；物理删除前统一检查 `agent_runs` 和两类生成任务引用。

- [x] **Step 5: 运行仓储与 migration 测试**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-api --test ai_model_repository_contract --test database_migrations'`

Expected: PASS。

### Task 4: 实现管理 API 与只读模型选项 API

**Files:**
- Create: `backend/src/model_management.rs`
- Modify: `backend/src/lib.rs`
- Create: `backend/tests/ai_model_routes.rs`

- [ ] **Step 1: 先写路由失败测试**

```rust
#[tokio::test]
async fn admin_detail_masks_credentials_and_options_omit_them() {
    let created = post_model(&app, text_model_payload("secret-key")).await;
    assert_eq!(created["api_key_masked"], "secr****-key");
    assert!(created.get("api_key").is_none());
    let options = get_json(&app, "/api/model-options?type=text").await;
    assert!(options["models"][0].get("request_base_url").is_none());
    assert!(options["models"][0].get("api_key_masked").is_none());
}
```

再覆盖类型/协议不匹配、凭据留空保持、乐观锁、默认替换、停用和两类删除结果。

- [ ] **Step 2: 运行路由测试并确认失败**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-api --test ai_model_routes'`

Expected: FAIL，路由为 404 或 DTO 未定义。

- [ ] **Step 3: 定义管理与工作台 DTO**

```rust
#[derive(Serialize)]
pub struct AiModelAdminResponse {
    pub model_id: Uuid,
    pub display_name: String,
    pub api_key_masked: String,
    pub api_secret_masked: Option<String>,
    pub credentials_configured: bool,
    pub version: i64,
}

#[derive(Serialize)]
pub struct ModelOptionResponse {
    pub model_id: Uuid,
    pub display_name: String,
    pub model_type: ModelType,
    pub provider_name: String,
    pub api_protocol: ApiProtocol,
    pub upstream_model: String,
    pub is_default: bool,
}
```

- [ ] **Step 4: 实现路由与稳定错误码**

注册 `/api/admin/models` 系列路由和 `/api/model-options`；映射 `model_not_found`、`model_disabled`、`model_type_mismatch`、`invalid_model_config`、`no_default_model`、`model_version_conflict`。不得增加伪鉴权中间件。

- [ ] **Step 5: 运行路由测试和敏感文本扫描**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-api --test ai_model_routes'`

Expected: PASS，测试响应和日志夹具不包含完整测试 Key。

### Task 5: 将文本客户端改为显式协议路由

**Files:**
- Modify: `crates/novex-model/src/llm.rs`
- Modify: `crates/novex-model/tests/openai_client.rs`
- Modify: `backend/src/lib.rs`
- Modify: `backend/tests/real_script_generation.rs`

- [ ] **Step 1: 先把客户端测试改为显式协议**

```rust
let config = OpenAIConfig {
    api_protocol: ApiProtocol::OpenAiResponses,
    request_base_url: server.uri(),
    upstream_model: "test-model".into(),
    api_key: "test-key".into(),
    timeout_seconds: 5,
    responses_reasoning_effort: Some("high".into()),
    responses_max_output_tokens: 3000,
};
```

断言 Responses 固定命中 `/responses`、Chat 固定命中 `/chat/completions`，URL 形态不能改变协议。

- [ ] **Step 2: 运行测试并确认失败**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-model --test openai_client'`

Expected: FAIL，`OpenAIConfig` 尚不接受显式协议。

- [ ] **Step 3: 实现显式协议分支**

删除 `uses_responses_api()` URL 推断；根据 `api_protocol` 分派 Responses 或 Chat。保留 SSE、JSON schema、prompt Token 覆盖、推理等级、超时和 User-Agent。

- [ ] **Step 4: 将 AppState 改为模型解析器注入**

```rust
#[async_trait]
pub trait ModelClientResolver: Send + Sync {
    async fn text_client(&self, model_id: Uuid) -> Result<ResolvedTextClient, ModelResolveError>;
}
```

生产实现从 `AiModelRepository` 创建客户端；测试实现返回 fake `LLMClient`，不要求数据库凭据。

- [ ] **Step 5: 运行 crate 与 backend 相关测试**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-model && cargo test -p novex-api --test real_script_generation --no-run'`

Expected: PASS/编译通过，不执行 ignored 真实模型测试。

### Task 6: 让全部 Rust 文本业务贯穿 model_id

**Files:**
- Modify: `backend/src/agents/conversational_runtime.rs`
- Modify: `backend/src/agents/script_agent.rs`
- Modify: `backend/src/agents/models/request.rs`
- Modify: `backend/src/lib.rs`
- Modify: `backend/tests/topic_agent_runtime.rs`
- Modify: `backend/tests/conversation_routes.rs`
- Modify: `backend/tests/script_routes.rs`
- Modify: `backend/tests/topic_review_routes.rs`
- Modify: `backend/tests/project_routes.rs`

- [ ] **Step 1: 先为所有请求补失败测试**

```rust
json!({ "content": "生成 8 个选题", "model_id": text_model_id })
json!({ "model_id": text_model_id }) // topic group review
json!({ "direction_notes": "更聚焦效率工具", "model_id": text_model_id })
json!({ "project_id": project_id, "topic_id": topic_id, "model_id": text_model_id })
```

覆盖缺失、停用和图片模型误传；断言未调用 fake LLM。

- [ ] **Step 2: 运行相关路由测试并确认失败**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-api --test conversation_routes --test topic_review_routes --test project_routes --test script_routes'`

Expected: FAIL，payload 或模型解析尚未接入。

- [ ] **Step 3: 扩展 DTO 与运行上下文**

`SendAgentMessageRequest`、`StrategyProfileDraftRequest`、主题组评审请求和 `GenerateScriptRequest` 增加 `model_id: Uuid`。Runtime 创建一次 `ResolvedTextClient` 后传入意图解析、脚本服务、选题生成、质量闸门和重写。

- [ ] **Step 4: 写入实际运行快照**

```rust
pub struct ModelExecutionSnapshot {
    pub model_id: Uuid,
    pub provider_name: String,
    pub api_protocol: ApiProtocol,
    pub protocol_version: String,
    pub request_base_url: String,
    pub upstream_model: String,
    pub reasoning_effort: Option<String>,
    pub timeout_seconds: u64,
}
```

保存前显式删除凭据字段；内部重试只能复用当前 client。

- [ ] **Step 5: 运行全部直接相关 Rust 测试**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-api --test topic_agent_runtime --test conversation_routes --test script_routes --test topic_review_routes --test project_routes'`

Expected: PASS。

### Task 7: 将图片任务与 Worker 切换到数据库模型

**Files:**
- Modify: `backend/src/agents/models/request.rs`
- Modify: `backend/src/repositories/asset_generation_repository.rs`
- Modify: `backend/src/lib.rs`
- Modify: `backend/tests/asset_generation_routes.rs`
- Modify: `backend/tests/asset_generation_repository_contract.rs`
- Create: `services/video-worker/src/video_worker/model_registry.py`
- Modify: `services/video-worker/src/video_worker/asset_generation.py`
- Modify: `services/video-worker/src/video_worker/main.py`
- Create: `services/video-worker/tests/test_model_registry.py`
- Modify: `services/video-worker/tests/test_asset_generation.py`

- [ ] **Step 1: 先把后端图片 payload 测试改为 model_id**

```json
{
  "model_id": "00000000-0000-0000-0000-000000000001",
  "candidate_count": 3,
  "use_reference_materials": true
}
```

断言图片模型通过，文本/停用模型拒绝，任务保存 `model_id`，现有幂等键与 `1-4`/`48` 上限不变。

- [ ] **Step 2: 先写 Worker 数据库模型加载失败测试**

```python
def test_loader_returns_openai_images_runtime_config(fake_connection):
    config = PostgresModelRegistry(fake_connection).resolve_enabled(IMAGE_MODEL_ID, "image")
    assert config.api_protocol == "openai_images"
    assert config.api_key == "test-key"
    assert config.settings.default_size == "1024x1024"
```

- [ ] **Step 3: 运行后端与 Worker 测试并确认失败**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-api --test asset_generation_routes --test asset_generation_repository_contract'`

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc 'cd /app && pytest tests/test_model_registry.py tests/test_asset_generation.py -q'`

Expected: FAIL，仍使用 provider 字符串和环境变量 factory。

- [ ] **Step 4: 实现数据库模型加载与 provider factory**

```python
def image_provider_from_model(config: ImageModelRuntimeConfig) -> ImageProvider:
    if config.api_protocol == "openai_images":
        return OpenAIImageProvider.from_model_config(config)
    if config.api_protocol == "jimeng_visual":
        return JimengImageProvider.from_model_config(config)
    raise ProviderConfigError("protocol_not_supported")
```

`run_next_image_task` 在外部调用前重新解析启用状态并写入不含凭据的快照。删除 `image_provider_from_env` 运行时路径。

- [ ] **Step 5: 覆盖停用、在途与重试语义**

停用模型的待执行任务标记失败且 fake provider 调用次数为 0；已构造并发出的 fake 调用允许完成；临时错误最多同模型重试一次，永久错误不跨模型。

- [ ] **Step 6: 运行后端与 Worker 全量相关测试**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-api --test asset_generation_routes --test asset_generation_repository_contract'`

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc 'cd /app && pytest tests -q'`

Expected: PASS，Worker 保持关闭。

### Task 8: 实现一次性环境配置导入

**Files:**
- Create: `backend/src/bin/import_model_config.rs`
- Create: `backend/tests/model_config_import.rs`
- Modify: `.env.example`
- Modify: `docker-compose.yml`
- Modify: `docs/memory/project-tech-stack.md`

- [ ] **Step 1: 先写导入器测试**

覆盖旧文本 `/responses` 地址规范化、OpenAI 图片独立记录、即梦双凭据、缺失凭据跳过、相同 `source_key` 重跑跳过和后台编辑后不覆盖。

- [ ] **Step 2: 运行导入测试并确认失败**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test -p novex-api --test model_config_import'`

Expected: FAIL，导入器尚不存在。

- [ ] **Step 3: 实现显式确认的导入命令**

```bash
cargo run -p novex-api --bin import-model-config -- --confirm-plaintext-credentials
```

缺少确认参数必须退出非零；输出只包含创建/跳过的显示名称和 ID，不输出地址中的凭据或 Key。

- [ ] **Step 4: 更新环境和运行文档**

明确 `OPENAI_*`、`OPENAI_IMAGE_*`、`JIMENG_*` 仅供一次性导入；API 和 Worker 的运行时配置来源为 PostgreSQL。不得保留隐式兜底开关。

- [ ] **Step 5: 使用测试环境验证两次导入**

第一次创建记录，第二次全部跳过；修改一条记录后再次导入仍不得覆盖。执行期间保持 `ASSET_GENERATION_WORKER_ENABLED=false`。

### Task 9: 实现管理后台 AI 模型管理页

**Files:**
- Create: `admin/app/components/AdminShell.tsx`
- Create: `admin/app/models/page.tsx`
- Create: `admin/app/models/ModelManagementPage.tsx`
- Create: `admin/app/models/page.test.tsx`
- Modify: `admin/app/page.tsx`
- Modify: `admin/app/lib/api.ts`
- Modify: `admin/app/lib/api.test.ts`
- Modify: `admin/app/styles.css`
- Modify: `admin/e2e/workspace.spec.ts`

- [ ] **Step 1: 先写 API wrapper 失败测试**

```ts
await createAiModel(client, payload);
await updateAiModel(client, modelId, { ...payload, version: 2, api_key: "" });
await changeAiModelStatus(client, modelId, { status: "disabled", version: 3 });
await deleteAiModel(client, modelId, { version: 4 });
```

断言方法、路径、筛选 query 和请求体，不允许响应类型包含 `api_key` 原文字段。

- [ ] **Step 2: 先写页面失败测试**

覆盖类型 tab、筛选、表格列、添加抽屉、按类型切换字段、密钥掩码、默认替代弹窗、删除确认、版本冲突刷新和无模型状态。

- [ ] **Step 3: 运行 Admin 单测并确认失败**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc 'cd /app && npm test -- --run app/lib/api.test.ts app/models/page.test.tsx'`

Expected: FAIL，页面和 API 方法尚不存在。

- [ ] **Step 4: 实现共享壳层与模型页面**

`/models` 使用现有导航与 DESIGN tokens；工具栏和表格不放入装饰卡片。编辑抽屉按基础、协议凭据、运行、类型专属四组渲染，协议选项随类型过滤。

- [ ] **Step 5: 实现生命周期交互**

默认替代、无替代关闭能力、逻辑/物理删除响应、版本冲突、加载/错误/空状态均在操作所在区域反馈。API Key/API Secret 输入为密码框，编辑默认空值。

- [ ] **Step 6: 运行 Admin 验证**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc 'cd /app && npm test -- --run && npm run lint && npm run build'`

Expected: 全部 PASS。

### Task 10: 实现视频工作台全部模型选择入口

**Files:**
- Create: `apps/video-agent/app/components/models/ModelSelect.tsx`
- Create: `apps/video-agent/app/components/models/modelSelection.ts`
- Modify: `apps/video-agent/app/lib/api.ts`
- Modify: `apps/video-agent/app/lib/api.test.ts`
- Modify: `apps/video-agent/app/page.tsx`
- Modify: `apps/video-agent/app/pages/content-strategy/AccountStrategyPage.tsx`
- Modify: `apps/video-agent/app/pages/content-strategy/ContentStrategyPage.tsx`
- Modify: `apps/video-agent/app/pages/content-strategy/TopicHistoryPage.tsx`
- Modify: `apps/video-agent/app/pages/script-creation/ScriptCreationPage.tsx`
- Modify: `apps/video-agent/app/pages/script-creation/AssetCandidatePanel.tsx`
- Modify: `apps/video-agent/app/pages/asset-generation/AssetGenerationPage.tsx`
- Modify: `apps/video-agent/app/page.test.tsx`
- Modify: `apps/video-agent/app/styles.css`
- Modify: `apps/video-agent/e2e/workspace.spec.ts`

- [ ] **Step 1: 先写模型选项与 payload API 测试**

```ts
const options = await listModelOptions(client, "text");
await sendAgentMessage(client, conversationId, { content, model_id: textModelId });
await createTopicGroupReview(client, batchId, { model_id: textModelId });
await createAssetGenerationTasks(client, scriptId, {
  model_id: imageModelId,
  candidate_count: 3,
  use_reference_materials: true,
});
```

- [ ] **Step 2: 先写页面失败测试**

逐项断言账号策略、当前选题、历史补充、主题组评审、脚本确认、脚本对话、素材批量和单镜头重生携带选中模型。断言无模型时禁用、停用错误后保留输入并刷新，不出现视频模型入口。

- [ ] **Step 3: 运行工作台单测并确认失败**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-agent sh -lc 'cd /app && npm test -- --run app/lib/api.test.ts app/page.test.tsx'`

Expected: FAIL，模型选项和 payload 尚未接入。

- [ ] **Step 4: 实现共享选择组件和状态模型**

```ts
export type ModelOption = {
  model_id: string;
  display_name: string;
  model_type: "text" | "image" | "video";
  provider_name: string;
  api_protocol: string;
  upstream_model: string;
  is_default: boolean;
};
```

按类型加载一次选项，为每个业务操作保存独立选择；初始选默认，选中项消失时不静默切换正在编辑的请求。

- [ ] **Step 5: 接入全部文本调用点**

策略草稿、topic 首次/补充、评审、脚本确认和每轮 script Agent 消息分别提交当前操作的 `model_id`；topic 内部步骤不在前端暴露额外选择器。

- [ ] **Step 6: 接入图片调用点**

将 `AssetGenerationProvider` 状态替换为图片 `model_id`；批量和单分镜重生复用当前图片选择，保留候选数、参考素材、幂等锁和费用提示。

- [ ] **Step 7: 运行工作台验证**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-agent sh -lc 'cd /app && npm test -- --run && npm run lint && npm run build'`

Expected: 全部 PASS。

### Task 11: 完成无费用 E2E、迁移切换和规格收尾

**Files:**
- Modify: `openspec/changes/manage-ai-models-and-workspace-selection/tasks.md`
- Modify: `MEMORY.md`
- Modify: `docs/memory/project-tech-stack.md`
- Verify: `docs/prototypes/admin/model-management.pen`
- Verify: `docs/prototypes/video-agent/video-agent.pen`

- [ ] **Step 1: 运行完整后端与 Worker 测试**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && cargo test --workspace'`

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-worker sh -lc 'cd /app && pytest tests -q'`

Expected: 全部 PASS；忽略的真实 provider 测试不得执行。

- [ ] **Step 2: 运行两套前端验证**

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-admin sh -lc 'cd /app && npm test -- --run && npm run lint && npm run build && npm run test:e2e'`

Run: `docker compose -f /server/docker-compose.yml exec -T ai-agent-video-agent sh -lc 'cd /app && npm test -- --run && npm run lint && npm run build && npm run test:e2e'`

Expected: 全部 PASS，E2E 使用路由 mock，不调用供应商。

- [ ] **Step 3: 在 Worker 关闭状态应用 migration 与导入**

确认 `ASSET_GENERATION_WORKER_ENABLED=false`，应用 SQLx migration，运行一次导入命令并核验默认模型、协议和掩码字段。不得在此步骤开启 Worker。

- [ ] **Step 4: 执行 fake provider 端到端验证**

验证文本和图片选择、快照、停用待执行任务、同模型重试与无跨模型调用；断言 fake provider 调用计数符合预期，真实费用为 0。

- [ ] **Step 5: 完成 Pencil 与 OpenSpec 校验**

使用 Pencil MCP 截图和 `snapshot_layout(problemsOnly=true)` 检查两套原型；运行：

```bash
openspec validate manage-ai-models-and-workspace-selection --strict --no-interactive
openspec instructions apply --change "manage-ai-models-and-workspace-selection" --json
git diff --check
```

Expected: OpenSpec valid，任务状态与实际一致，`git diff --check` 无错误。

- [ ] **Step 6: 更新稳定项目记忆并做敏感信息扫描**

只记录已确认的统一模型管理、协议驱动、工作台选择和运行时数据库单一来源；不得记录任何真实 Key。使用 `rg` 检查新增文件不存在真实凭据、调试打印或未确认的认证实现。
