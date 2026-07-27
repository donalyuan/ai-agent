# 角色执行管道集成任务清单

## Phase 1: AgentDefinition 注册

### 1.1 创建角色 AgentDefinition 源文件

- [ ] 在 `agent-definitions/production/` 目录创建9个角色的 YAML 文件
  - [ ] `producer.yaml`
  - [ ] `screenwriter.yaml`
  - [ ] `director.yaml`
  - [ ] `cinematographer.yaml`
  - [ ] `performance_director.yaml`
  - [ ] `sound_director.yaml`
  - [ ] `editor.yaml`
  - [ ] `qc.yaml`
  - [ ] `character_critic.yaml`

### 1.2 创建 PromptDefinition 源文件

- [ ] 在 `agent-definitions/prompts/production/` 目录创建9个角色的 Prompt 模板
  - [ ] `producer.general.1.0.0.txt`（内容引用自 `crates/novex-production-crew/prompts/roles/producer/general.v1.txt`）
  - [ ] `screenwriter.general.1.0.0.txt`
  - [ ] `director.general.1.0.0.txt`
  - [ ] `cinematographer.general.1.0.0.txt`
  - [ ] `performance_director.general.1.0.0.txt`
  - [ ] `sound_director.general.1.0.0.txt`
  - [ ] `editor.general.1.0.0.txt`
  - [ ] `qc.general.1.0.0.txt`
  - [ ] `character_critic.general.1.0.0.txt`
- [ ] 创建对应的 PromptDefinition YAML 元数据文件（9个）

### 1.3 更新 registry.json

- [ ] 将9个角色的 `AgentDefinition` 合并进 `agent-definitions/registry.json` 的 `agents` 数组
- [ ] 将9个角色的 `PromptDefinition` 合并进 `registry.json` 的 `prompts` 数组
- [ ] 验证 `DefinitionRegistry::load()` 能成功加载新条目
- [ ] 验证 `PromptCompiler.compile("production.producer", ...)` 不报错

---

## Phase 2: RoleExecutor 实现

### 2.1 新增依赖

- [ ] 在 `crates/novex-production-crew/Cargo.toml` 添加 `novex-ai-core` 依赖
- [ ] 在 `crates/novex-production-crew/Cargo.toml` 添加 `novex-agent` 依赖
- [ ] 确认无循环依赖（novex-production-crew → novex-agent → novex-ai-core）

### 2.2 `RoleExecutionContext` 结构

- [ ] 在 `src/executor/role_executor.rs` 定义 `RoleExecutionContext` struct
  - [ ] `pool: PgPool`
  - [ ] `definition_registry: Arc<DefinitionRegistry>`
  - [ ] `audited_executor: Arc<AuditedModelExecutor>`
  - [ ] `project_id: Uuid`
  - [ ] `role_key: String`
  - [ ] `user_input: Option<String>`
  - [ ] `preferred_model_id: Uuid`

### 2.3 实现 `RoleExecutor::execute()`

- [ ] 从 `RoleRegistry` 加载 `RoleDefinition`
- [ ] 从 `ProductionStateRepository` 读取输入产物（最新 approved 或 draft 版本）
- [ ] 调用 `check_inputs_ready()` 验证输入就绪
- [ ] 实现 `build_context_candidates()`：将输入产物序列化为 `ContextCandidate` 列表
- [ ] 调用 `PromptCompiler::compile()` 编译角色 Prompt
- [ ] 调用 `AuditedModelExecutor` 发起模型调用（prepare → 调用 → finish）
- [ ] 解析 AI 输出为 `serde_json::Value`
- [ ] 调用 `validate_output()` 验证输出符合产物 schema
- [ ] 调用 `ProductionStateRepository::save_artifact()` 写入产物（version 自增，status=draft）
- [ ] 更新 `ProductionProject.status` 到对应阶段
- [ ] 构造并返回 `RoleExecutionResult`

### 2.4 RoleExecutor 单元测试

- [ ] 测试 `build_context_candidates()`：输入产物正确序列化
- [ ] 测试 `validate_output()`：有效输出通过，无效输出返回错误
- [ ] 测试 `execute()`：mock executor 场景验证全流程

---

## Phase 3: ProductionOrchestrator 更新

### 3.1 新增字段

- [ ] `ProductionOrchestrator` 添加 `audited_executor: Option<Arc<AuditedModelExecutor>>`
- [ ] `ProductionOrchestrator` 添加 `definition_registry: Option<Arc<DefinitionRegistry>>`

### 3.2 `execute_role()` 方法实现

- [ ] 在 `src/orchestrator/mod.rs` 实现 `execute_role(project_id, role_key, user_input) -> ProductionResult<RoleExecutionResult>`
  - [ ] 检查 executor 和 registry 是否已注入（未注入返回 AgentExecution 错误）
  - [ ] 解析 `preferred_model_id`（优先级：请求→项目元数据→AppConfig）
  - [ ] 构建 `RoleExecutionContext`
  - [ ] 调用 `RoleExecutor::execute(ctx)`
  - [ ] 触发 Gate 检查（在角色执行前）
  - [ ] 返回执行结果
- [ ] Orchestrator 集成测试（mock executor）

---

## Phase 4: AppState 集成

### 4.1 新增 `production_orchestrator()` 方法

- [ ] 在 `backend/src/bootstrap/state.rs` 实现 `production_orchestrator() -> Result<ProductionOrchestrator, AppStateError>`
  - [ ] 获取 `PgPool`
  - [ ] 获取 `DefinitionRegistry`（复用现有 `definition_registry()` 方法）
  - [ ] 获取 `AuditedModelExecutor`（复用现有 `audited_model_executor()` 方法）
  - [ ] 创建 `RoleRegistry::bootstrap()`（从 `crates/novex-production-crew/roles/` 加载 YAML）
  - [ ] 创建 `GateRegistry::bootstrap()`
  - [ ] 构造并返回 `ProductionOrchestrator`

### 4.2 `RoleRegistry::bootstrap()` 实现

- [ ] 在 `src/roles/registry.rs` 添加 `bootstrap(roles_dir: &Path) -> ProductionResult<Self>` 方法
  - [ ] 调用 `RoleLoader::load_from_dir(roles_dir)`
  - [ ] 注册所有已加载角色
- [ ] 单元测试：加载真实 roles 目录

---

## Phase 5: HTTP Handler 接入

### 5.1 更新 `execute_role` handler

- [ ] 在 `backend/src/api/production/handlers.rs` 中更新 `execute_role()`
  - [ ] 调用 `state.production_orchestrator()?`
  - [ ] 调用 `orchestrator.execute_role(id, role_key, req.user_input)`
  - [ ] 将 `ProductionError` 映射到对应 HTTP 状态码
  - [ ] 返回 `RoleExecutionResult` 的 JSON 表示

### 5.2 错误码映射

- [ ] 在 `handlers.rs` 中实现 `production_error_response(error: ProductionError) -> impl IntoResponse`
  - [ ] `MissingInputArtifact` → 400 `missing_input_artifact`
  - [ ] `InvalidArtifactSchema` → 422 `invalid_artifact_schema`
  - [ ] `RoleNotFound` → 404 `role_not_found`
  - [ ] `GateRejected` → 409 `gate_rejected`
  - [ ] `GateWaitApproval` → 409 `waiting_approval`
  - [ ] `AgentExecution` → 502 `agent_execution_failed`
  - [ ] `Database` → 500 `internal_error`

---

## Phase 6: 集成测试

### 6.1 Producer 角色端到端测试（`#[ignore]` + 真实 DB）

- [ ] 创建测试项目（project_type = fast_lane，初始 metadata 含 preferred_model_id）
- [ ] 调用 `execute_role("producer")` with mock LLM client
- [ ] 验证 CreativeBrief 产物写入 DB（version=1，status=draft）
- [ ] 验证 ModelCall 审计记录存在
- [ ] 验证 ProductionProject.status 更新为 "scripting"
- [ ] 清理测试数据

### 6.2 输入产物缺失测试

- [ ] 创建空项目，直接执行 Director 角色
- [ ] 验证返回 `missing_input_artifact` 错误
- [ ] 验证错误响应包含 `required_artifacts` 列表

### 6.3 输出 schema 验证测试

- [ ] mock LLM 返回不符合 schema 的输出
- [ ] 验证返回 `invalid_artifact_schema` 错误
- [ ] 验证无产物写入 DB

### 6.4 HTTP 测试（tower::ServiceExt）

- [ ] Producer 执行成功：200 + output_artifacts
- [ ] 角色不存在：404 + role_not_found
- [ ] 执行器未配置（test state）：500

---

## Phase 7: 构建脚本（可选优化）

- [ ] 更新 `agent-definitions/scripts/build-registry-v2.mjs`（或新建构建脚本）自动扫描 `production/` 目录并合并进 `registry.json`
  - 注：本 change 范围内手工维护 registry.json；自动化构建作为后续优化项，标记此任务为可选

---

## Phase 8: 验证与收尾

- [ ] `cargo build --workspace` 无错误
- [ ] `cargo test --workspace` 全量通过
- [ ] 在开发环境中手动 curl 测试 `execute_role("producer")`
- [ ] 验证 ModelCall 审计记录可从 `/model-calls` 查询
- [ ] 更新 `docs/memory/agent-foundation-direction.md`：标记角色执行已接通 AI 基础设施

---

## 总计

- **Phase 1**: 27 任务（AgentDefinition 注册）
- **Phase 2**: 14 任务（RoleExecutor 实现）
- **Phase 3**: 5 任务（Orchestrator 更新）
- **Phase 4**: 6 任务（AppState 集成）
- **Phase 5**: 11 任务（Handler 接入）
- **Phase 6**: 10 任务（集成测试）
- **Phase 7**: 1 任务（可选优化）
- **Phase 8**: 5 任务（验证收尾）

**总计约 79 个任务**

## 实施优先级

1. **P0（阻塞后续）**: Phase 1（AgentDefinition 注册）→ Phase 2（RoleExecutor 实现）
2. **P1（核心功能）**: Phase 3（Orchestrator）→ Phase 4（AppState）→ Phase 5（Handler）
3. **P2（质量保障）**: Phase 6（集成测试）
4. **P3（可选优化）**: Phase 7（构建脚本）
5. **P4（收尾）**: Phase 8（验证）
