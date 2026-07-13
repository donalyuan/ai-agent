# Rust API 与 Agent Runtime 分层重构实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. 当前仓库未授权委派或提交，默认使用 `superpowers:executing-plans` 在当前会话内执行。

**Goal:** 在保持全部外部行为不变的前提下，将 Rust API 与 Agent Runtime 重构为按业务模块组织的 `bootstrap/api/application/domain/repositories` 分层结构。

**Architecture:** `bootstrap` 负责依赖组装，`api` 负责 HTTP，`application` 负责用例编排，`domain` 保存无框架依赖的业务类型，`repositories` 负责持久化。统一 `AgentRuntime` 继续作为入口，并委派给脚本、选题生成、质量闸门和主题组评审模块。

**Tech Stack:** Rust 2021、Axum 0.7、SQLx 0.8、Tokio、PostgreSQL、Redis、Cargo Workspace、Docker Compose、OpenSpec。

**Specifications:**

- `openspec/changes/refactor-rust-api-agent-runtime-layers/proposal.md`
- `openspec/changes/refactor-rust-api-agent-runtime-layers/design.md`
- `openspec/changes/refactor-rust-api-agent-runtime-layers/specs/novex-foundation-architecture/spec.md`
- `openspec/changes/refactor-rust-api-agent-runtime-layers/tasks.md`

**Repository constraints:** 不执行 `git add`、`git commit` 或 `git push`。每完成一个实施步骤，只同步勾选对应 OpenSpec task，并运行该步骤指定的验证命令。

---

## 目标文件结构

```text
backend/src/
  lib.rs                         模块入口与应用构建导出
  bootstrap/
    mod.rs                       bootstrap 公共入口
    config.rs                    AppConfig 与环境读取
    state.rs                     AppState 与依赖/Service 组装
    runtime.rs                   PostgreSQL、Redis 与运行时状态构建
  api/
    mod.rs                       API 模块入口
    router.rs                    总 Router、CORS 与静态素材服务
    error.rs                     公共 JSON rejection 和错误响应
    health.rs                    health/ready
    workspace/{mod,dto,handlers}.rs
    ai_models/{mod,dto,handlers}.rs
    projects/{mod,dto,handlers}.rs
    topics/{mod,dto,handlers}.rs
    materials/{mod,dto,handlers}.rs
    scripts/{mod,dto,handlers}.rs
    conversations/{mod,dto,handlers}.rs
    asset_generation/{mod,dto,handlers}.rs
  application/
    mod.rs
    workspace.rs
    ai_models.rs
    projects.rs
    topics.rs
    materials.rs
    scripts.rs
    conversations.rs
    asset_generation.rs
    agents/
      mod.rs
      runtime/
        mod.rs                   AgentRuntime 统一入口
        types.rs                 AgentTurnRequest/Response
        error.rs                 AgentRuntimeError
        script.rs                脚本生成与分镜修改
        topic_generation.rs      普通与补充选题生成
        topic_quality.rs         质量闸门与最多一次重写
        topic_review.rs          主题组评审
        prompt.rs                共享上下文和 Prompt 工具
  domain/
    mod.rs
    script.rs
    topic.rs
    conversation.rs
  repositories/                 保持现有按领域文件，改用 domain 类型
```

## Task 1：建立测试与路由基线

**Files:**

- Create: `backend/tests/module_boundaries.rs`
- Modify: `openspec/changes/refactor-rust-api-agent-runtime-layers/tasks.md`
- Inspect: `backend/src/lib.rs`

- [ ] **Step 1：确认 API 容器运行状态**

Run:

```bash
docker compose -f /server/docker-compose.yml ps ai-agent-api
```

Expected: `ai-agent-api` 状态为 running；若未运行，使用现有 Compose 配置启动该服务后再继续。

- [ ] **Step 2：运行重构前全量测试**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test --workspace'
```

Expected: 命令退出码为 0。若存在基线失败，先记录并查明原因，不得把原有失败归因于后续重构。

- [ ] **Step 3：记录路由清单**

Run:

```bash
sed -n '418,540p' backend/src/lib.rs | rg '\.route\(|\.nest_service\('
```

Expected: 清单覆盖 health、ready、models、workspace、projects、topics、materials、conversations、scripts 和 asset generation 路由；迁移完成后使用同一清单逐项核对。

- [ ] **Step 4：添加新模块路径编译契约测试**

Create `backend/tests/module_boundaries.rs`:

```rust
use novex_api::application::agents::runtime::{AgentRuntime, AgentTurnRequest};
use novex_api::bootstrap::{AppConfig, AppState};
use novex_api::domain::conversation::{AgentConversation, AgentMessage};
use novex_api::domain::script::{Scene, Script, ScriptStatus};
use novex_api::domain::topic::{ContentTopic, ContentTopicStatus};

#[test]
fn layered_public_modules_are_available() {
    fn assert_type<T>() {}

    assert_type::<AgentRuntime>();
    assert_type::<AgentTurnRequest>();
    assert_type::<AppConfig>();
    assert_type::<AppState>();
    assert_type::<AgentConversation>();
    assert_type::<AgentMessage>();
    assert_type::<Scene>();
    assert_type::<Script>();
    assert_type::<ScriptStatus>();
    assert_type::<ContentTopic>();
    assert_type::<ContentTopicStatus>();
}
```

- [ ] **Step 5：验证模块边界测试先失败**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test module_boundaries'
```

Expected: 编译失败，错误指出 `application`、`bootstrap` 或 `domain` 尚不存在。

- [ ] **Step 6：勾选 OpenSpec 1.1 至 1.3**

在 `openspec/changes/refactor-rust-api-agent-runtime-layers/tasks.md` 将 1.1、1.2、1.3 更新为 `- [x]`，不执行 Git 提交。

## Task 2：迁移 Domain 类型

**Files:**

- Create: `backend/src/domain/mod.rs`
- Move: `backend/src/agents/models/script.rs` -> `backend/src/domain/script.rs`
- Move: `backend/src/agents/models/topic.rs` -> `backend/src/domain/topic.rs`
- Move: `backend/src/agents/conversation.rs` -> `backend/src/domain/conversation.rs`
- Modify: `backend/src/repositories/*.rs`
- Modify: `backend/src/agents/*.rs`
- Modify: `backend/tests/*.rs`

- [ ] **Step 1：创建 Domain 模块入口**

Create `backend/src/domain/mod.rs`:

```rust
//! 与 HTTP、数据库驱动和 Agent 编排无关的业务实体与状态规则。

pub mod conversation;
pub mod script;
pub mod topic;
```

在 `backend/src/lib.rs` 添加：

```rust
pub mod domain;
```

- [ ] **Step 2：机械迁移三个领域文件**

保持类型、字段、序列化、状态解析和校验实现不变，只修改文件位置。迁移后生产代码统一使用：

```rust
use crate::domain::conversation::*;
use crate::domain::script::*;
use crate::domain::topic::*;
```

测试统一使用：

```rust
use novex_api::domain::{conversation::*, script::*, topic::*};
```

- [ ] **Step 3：修正 Repository 依赖方向**

将 `script_repository.rs`、`topic_repository.rs`、`conversation_repository.rs` 以及引用这些类型的其他 Repository 从 `crate::agents::*` 改为 `crate::domain::*`。不得在 `domain` 中引入 `axum` 或 `sqlx`。

- [ ] **Step 4：验证旧 Domain 路径已清空**

Run:

```bash
rg -n 'agents::(models::(script|topic)|conversation)' backend/src backend/tests
```

Expected: 无输出；若匹配是将在后续删除的模块声明，也必须在本 Task 内切换。

- [ ] **Step 5：运行领域与 Repository 测试**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test script_domain_models --test script_repository_contract --test topic_repository_contract --test conversation_repository_contract'
```

Expected: 全部通过。

- [ ] **Step 6：勾选 OpenSpec 2.1 至 2.5**

同步任务状态，不执行 Git 提交。

## Task 3：拆分 Bootstrap

**Files:**

- Create: `backend/src/bootstrap/mod.rs`
- Create: `backend/src/bootstrap/config.rs`
- Create: `backend/src/bootstrap/state.rs`
- Create: `backend/src/bootstrap/runtime.rs`
- Modify: `backend/src/lib.rs`
- Modify: `backend/src/main.rs`
- Modify: `backend/tests/*_routes.rs`

- [ ] **Step 1：创建 Bootstrap 公共入口**

Create `backend/src/bootstrap/mod.rs`:

```rust
//! 组装配置、基础设施连接和应用依赖，不承载业务规则。

mod config;
mod runtime;
mod state;

pub use config::AppConfig;
pub use runtime::{build_runtime_state, connect_runtime_pg_pool};
pub use state::AppState;
```

- [ ] **Step 2：迁移配置与状态**

将 `AppConfig`、`AppState` 及其现有构造方法机械迁入 `config.rs` 和 `state.rs`。保留测试注入 `LLMClient`、`ModelClientResolver` 和数据库连接的能力；为 `AppState` 添加说明其只保存依赖、不保存请求状态的文档注释。

- [ ] **Step 3：迁移运行时构建**

将以下函数迁入 `runtime.rs`，签名保持不变：

```rust
pub async fn build_runtime_state() -> Result<AppState, Box<dyn std::error::Error + Send + Sync>>;
pub async fn connect_runtime_pg_pool(database_url: &str, max_connections: u32) -> Result<PgPool, sqlx::Error>;
```

`sync_content_strategy_menu_state` 保持私有，并注明启动同步用于确保菜单种子状态与当前能力一致。

- [ ] **Step 4：切换 main 与测试 import**

生产入口使用：

```rust
use novex_api::{build_app_with_state, bootstrap::build_runtime_state};
```

所有测试使用：

```rust
use novex_api::bootstrap::{AppConfig, AppState};
```

- [ ] **Step 5：运行 Bootstrap 相关测试**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test health --test cors --test database_migrations --test model_routing'
```

Expected: 全部通过，且 Task 1 的 `module_boundaries` 只剩 `application::agents::runtime` 未满足。

- [ ] **Step 6：勾选 OpenSpec 3.1 至 3.5**

同步任务状态，不执行 Git 提交。

## Task 4：建立公共 API 与首批 Application Service

**Files:**

- Create: `backend/src/api/mod.rs`
- Create: `backend/src/api/router.rs`
- Create: `backend/src/api/error.rs`
- Create: `backend/src/api/health.rs`
- Create: `backend/src/api/workspace/mod.rs`
- Create: `backend/src/api/workspace/dto.rs`
- Create: `backend/src/api/workspace/handlers.rs`
- Create: `backend/src/api/ai_models/mod.rs`
- Create: `backend/src/api/ai_models/dto.rs`
- Create: `backend/src/api/ai_models/handlers.rs`
- Create: `backend/src/application/mod.rs`
- Create: `backend/src/application/workspace.rs`
- Create: `backend/src/application/ai_models.rs`
- Modify: `backend/src/lib.rs`
- Delete after migration: `backend/src/model_management.rs`

- [ ] **Step 1：创建 Application 与 API 模块入口**

`backend/src/application/mod.rs`:

```rust
//! 面向用例的业务编排层；依赖领域类型和持久化端口，不依赖 Axum。

pub mod ai_models;
pub mod workspace;
```

`backend/src/api/mod.rs`:

```rust
//! Axum 传输层；只负责提取、校验、调用应用用例和响应转换。

pub mod ai_models;
pub mod error;
pub mod health;
pub mod router;
pub mod workspace;
```

- [ ] **Step 2：迁移公共 HTTP 错误与有效 JSON 提取器**

把 `ValidJson<T>`、`invalid_json_response` 和公共错误 body 迁入 `api/error.rs`。保留现有 `FromRequest` 行为、状态码和 JSON 文案，不把 Repository 具体错误放进公共文件。

- [ ] **Step 3：迁移 health、ready、workspace 和 AI models**

每个 API 模块提供：

```rust
pub fn router() -> axum::Router<AppState>;
```

handler 保持 `pub(super)`，DTO 放 `dto.rs`。AI model 的 Repository 协作和错误枚举迁入 `application/ai_models.rs`；workspace 查询迁入 `application/workspace.rs`。

- [ ] **Step 4：建立总 Router**

`api/router.rs` 提供：

```rust
pub fn build_app() -> Router;
pub fn build_app_with_state(state: AppState) -> Router;
```

通过 `merge` 组合 feature Router，统一添加 CORS、`/assets` 和 state。`lib.rs` 只重新导出这两个应用构建函数，不重新导出旧 DTO 路径。

- [ ] **Step 5：运行首批 API 测试**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test health --test cors --test workspace_menu_routes --test ai_model_routes'
```

Expected: 全部通过。

- [ ] **Step 6：勾选 OpenSpec 4.1 至 4.5**

同步任务状态，不执行 Git 提交。

## Task 5：迁移业务 API 与 Application Service

**Files:**

- Create: `backend/src/api/projects/mod.rs`
- Create: `backend/src/api/projects/dto.rs`
- Create: `backend/src/api/projects/handlers.rs`
- Create: `backend/src/api/topics/mod.rs`
- Create: `backend/src/api/topics/dto.rs`
- Create: `backend/src/api/topics/handlers.rs`
- Create: `backend/src/api/materials/mod.rs`
- Create: `backend/src/api/materials/dto.rs`
- Create: `backend/src/api/materials/handlers.rs`
- Create: `backend/src/api/scripts/mod.rs`
- Create: `backend/src/api/scripts/dto.rs`
- Create: `backend/src/api/scripts/handlers.rs`
- Create: `backend/src/api/asset_generation/mod.rs`
- Create: `backend/src/api/asset_generation/dto.rs`
- Create: `backend/src/api/asset_generation/handlers.rs`
- Create: `backend/src/application/projects.rs`
- Create: `backend/src/application/topics.rs`
- Create: `backend/src/application/materials.rs`
- Create: `backend/src/application/scripts.rs`
- Create: `backend/src/application/asset_generation.rs`
- Modify: `backend/src/api/router.rs`
- Modify: `backend/src/agents/models/request.rs`（迁出内容后删除）

- [ ] **Step 1：按 DTO 边界先迁移请求响应类型**

从旧 `request.rs` 按以下范围迁移，字段、Serde 名称、默认值和校验实现保持原样：

```text
projects: 1-234
materials: 235-411
asset_generation: 412-611
topics: 612-998
workspace: 999-1021（已在 Task 4 完成）
conversations: 1022-1229（留给 Task 6）
scripts: 1230-1448
```

模块内部共享的规范化函数保持私有；真正跨模块的类型使用 `domain` 类型或独立 Application 输入，不互相 import DTO。

- [ ] **Step 2：迁移 projects 用例**

将项目 CRUD、策略资料更新、策略草稿 Prompt/Schema、同模型重试和 JSON 输出解析迁入 `application/projects.rs`。API handler 只完成 DTO 与 Application 输入输出转换。

- [ ] **Step 3：迁移 topics 与 materials 用例**

将选题批次、主题组、评审快照、质量结果、状态流转和 prepare-script 编排迁入 `application/topics.rs`；将素材筛选、详情、更新和状态修改迁入 `application/materials.rs`。

- [ ] **Step 4：迁移 scripts 与 asset generation 用例**

将脚本生成/查询/状态更新迁入 `application/scripts.rs`。将素材生成计划、任务、候选选择/拒绝、确认/忽略、幂等键和已有素材候选构建迁入 `application/asset_generation.rs`，并为幂等键稳定性添加原因注释。

- [ ] **Step 5：组合业务 Router 并验证无直接 SQL**

Run:

```bash
rg -n 'sqlx::|query!|query_as!|LLMPrompt|LLMJsonSchema' backend/src/api
```

Expected: API 模块无 SQL 和 Prompt 构建；允许仅在 DTO 中引用序列化类型。

- [ ] **Step 6：运行各业务路由测试**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test project_routes --test topic_routes --test topic_review_routes --test topic_group_priority_routes --test material_routes --test script_routes --test asset_generation_routes'
```

Expected: 全部通过。

- [ ] **Step 7：勾选 OpenSpec 5.1 至 5.6**

同步任务状态，不执行 Git 提交。

## Task 6：拆分 Agent Runtime 与对话 API

**Files:**

- Create: `backend/src/application/agents/mod.rs`
- Create: `backend/src/application/agents/runtime/mod.rs`
- Create: `backend/src/application/agents/runtime/types.rs`
- Create: `backend/src/application/agents/runtime/error.rs`
- Create: `backend/src/application/agents/runtime/script.rs`
- Create: `backend/src/application/agents/runtime/topic_generation.rs`
- Create: `backend/src/application/agents/runtime/topic_quality.rs`
- Create: `backend/src/application/agents/runtime/topic_review.rs`
- Create: `backend/src/application/agents/runtime/prompt.rs`
- Create: `backend/src/application/conversations.rs`
- Create: `backend/src/api/conversations/mod.rs`
- Create: `backend/src/api/conversations/dto.rs`
- Create: `backend/src/api/conversations/handlers.rs`
- Modify: `backend/src/api/router.rs`
- Delete after migration: `backend/src/agents/conversational_runtime.rs`

- [ ] **Step 1：创建 Runtime 模块骨架和公共类型**

`application/agents/mod.rs`:

```rust
//! 业务 Agent 的应用编排入口。

pub mod runtime;
```

`application/agents/runtime/mod.rs`:

```rust
//! 统一加载会话并分派到业务 Agent 能力；具体 Prompt 和解析由子模块承担。

mod error;
mod prompt;
mod script;
mod topic_generation;
mod topic_quality;
mod topic_review;
mod types;

pub use error::AgentRuntimeError;
pub use types::{AgentTurnRequest, AgentTurnResponse};

pub struct AgentRuntime {
    conversation_repository: Arc<dyn ConversationRepository>,
    script_repository: Arc<dyn ScriptRepository>,
    project_repository: Arc<dyn ProjectRepository>,
    topic_repository: Option<Arc<dyn TopicRepository>>,
    llm_client: Arc<dyn LLMClient>,
    model_execution: Option<ModelExecutionSnapshot>,
}
```

- [ ] **Step 2：迁移统一入口，先不改执行顺序**

把旧 `AgentRuntime::handle_turn`、会话校验、用户消息写入、run/step 建立、成功回复和失败收尾机械迁入 `mod.rs`。添加文档注释说明“先持久化用户消息、再建立运行记录、失败时完成 run/step”的不变量。

- [ ] **Step 3：拆分 script 能力**

迁移脚本生成意图、分镜补丁、相关 Prompt、Schema 和输出解析到 `script.rs`。保持 `ScriptGenerationMode`、脚本绑定、assistant metadata 和错误转换不变。

- [ ] **Step 4：拆分 topic generation 与 quality**

普通/补充批次生成、上下文归一和选题输出解析进入 `topic_generation.rs`；质量评分、通过判定、低通过率检测和最多一次重写进入 `topic_quality.rs`。在重写逻辑前注释说明成本上限和不得跨模型切换。

- [ ] **Step 5：拆分 topic review 与共享 Prompt**

主题组评审输出、风险标记、上下文格式化进入 `topic_review.rs`；真正被两个以上能力复用的截断、历史消息和账号策略上下文工具进入 `prompt.rs`。

- [ ] **Step 6：迁移对话 Application 与 API**

`application/conversations.rs` 负责创建会话、列消息和向 Runtime 发送消息。对话 DTO 从旧 `request.rs` 迁入 `api/conversations/dto.rs`，保持字段和 metadata 转换不变。

- [ ] **Step 7：运行 Runtime 和对话测试**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test -p novex-api --test conversation_routes --test conversational_script_agent --test topic_agent_runtime --test topic_review_routes --test script_agent_service --test module_boundaries'
```

Expected: 全部通过，包括新模块路径契约。

- [ ] **Step 8：勾选 OpenSpec 6.1 至 6.8**

同步任务状态，不执行 Git 提交。

## Task 7：删除旧入口并完成注释审查

**Files:**

- Delete: `backend/src/agents/models/request.rs`
- Delete: `backend/src/agents/conversational_runtime.rs`
- Delete or reduce: `backend/src/agents/models/mod.rs`
- Modify: `backend/src/agents/mod.rs`
- Modify: `backend/src/lib.rs`
- Modify: all changed Rust modules

- [ ] **Step 1：删除旧模块与声明**

确认所有类型和行为已迁移后删除旧聚合文件；`agents` 仅保留仍有明确职责的 LLM 与 script agent 实现。不得添加旧路径 `pub use`。

- [ ] **Step 2：收敛 lib.rs**

目标形态：

```rust
pub mod agents;
pub mod api;
pub mod application;
pub mod bootstrap;
pub mod domain;
pub mod model_config_import;
pub mod model_routing;
pub mod repositories;

pub use api::router::{build_app, build_app_with_state};
```

允许保留少量确有二进制入口需要的导出，但不得包含 handler、DTO、Repository error mapper 或 Prompt。

- [ ] **Step 3：检查旧路径与文件体量**

Run:

```bash
rg -n 'agents::models::|conversational_runtime|pub use .*agents' backend/src backend/tests
find backend/src -type f -name '*.rs' -printf '%p ' -exec wc -l {} \; | sort -k2 -nr | head -20
```

Expected: 无旧路径；没有新的千行 API/Runtime 聚合文件。

- [ ] **Step 4：完成注释审查**

逐个检查模块头、公共 Application Service、Agent 入口、重试、幂等、质量闸门、主题组归一、失败收尾和事务顺序。删除仅复述赋值、分支或 CRUD 的注释。

- [ ] **Step 5：格式化并运行快速编译**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo fmt --all && /usr/local/cargo/bin/cargo check --workspace'
```

Expected: 退出码为 0。

- [ ] **Step 6：勾选 OpenSpec 7.1 至 7.6**

同步任务状态，不执行 Git 提交。

## Task 8：全量验证与 OpenSpec 收尾

**Files:**

- Modify: `openspec/changes/refactor-rust-api-agent-runtime-layers/tasks.md`
- Verify: all changed files

- [ ] **Step 1：运行格式检查**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo fmt --all -- --check'
```

Expected: 退出码为 0。

- [ ] **Step 2：运行 Workspace 全量测试**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo test --workspace'
```

Expected: 全部测试通过，失败数为 0。

- [ ] **Step 3：运行 Clippy**

Run:

```bash
docker compose -f /server/docker-compose.yml exec -T ai-agent-api sh -lc 'cd /app && /usr/local/cargo/bin/cargo clippy --workspace --all-targets -- -D warnings'
```

Expected: 退出码为 0，无 warning。

- [ ] **Step 4：核对路由和架构要求**

将 Task 1 路由清单与 `api/router.rs` 及各 feature `router()` 对比；运行：

```bash
rg -n 'sqlx::|LLMPrompt|LLMJsonSchema' backend/src/api
rg -n 'axum::|sqlx::' backend/src/domain
rg -n 'agents::models::|conversational_runtime' backend/src backend/tests
```

Expected: API 无 SQL/Prompt，Domain 无 Axum/SQLx，旧路径无残留。

- [ ] **Step 5：完成 OpenSpec tasks 并验证 change**

勾选 8.1 至 8.5，保留 8.6 到执行 apply instructions 后再勾选。Run:

```bash
openspec validate refactor-rust-api-agent-runtime-layers
openspec instructions apply --change "refactor-rust-api-agent-runtime-layers" --json
```

Expected: change valid，apply 状态与实际完成任务一致。

- [ ] **Step 6：勾选 OpenSpec 8.6 并复核工作区**

Run:

```bash
git diff --check
git status --short
```

Expected: 无 whitespace error；只包含本次设计、OpenSpec、计划、Rust 源码和测试改动。不得执行 Git 提交。
