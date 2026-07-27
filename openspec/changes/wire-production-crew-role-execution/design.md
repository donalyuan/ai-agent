# 设计文档：打通虚拟制作团队角色执行管道

## 架构概览

```
HTTP Handler
    │ execute_role()
    ▼
AppState.production_orchestrator()
    │ 注入: PgPool + DefinitionRegistry + AuditedModelExecutor
    ▼
ProductionOrchestrator.execute_role(project_id, role_key, user_input)
    │
    ├─→ RoleRegistry.get(role_key)       → RoleDefinition
    ├─→ Repository.get_project()          → ProductionProject
    ├─→ Repository.get_input_artifacts()  → Vec<Value>（已就绪产物）
    ├─→ RoleExecutor.check_inputs_ready() → OK | Err(MissingInputArtifact)
    │
    ▼
RoleExecutor.execute(ctx: RoleExecutionContext)
    │
    ├─→ build_context_candidates()         → Vec<ContextCandidate>
    │     └─ 输入产物 JSON → ContextCandidate（trust=ConfirmedFact, priority=P1）
    │     └─ 用户输入 → ContextCandidate（trust=UserInstruction, priority=P0）
    │
    ├─→ PromptCompiler.compile(agent_key, "1.0.0", node_key, input, ...)
    │     └─ 从 DefinitionRegistry 加载角色的 AgentDefinition + PromptDefinition
    │
    ├─→ AuditedModelExecutor.call(snapshot, ...) → AuditedParsedModelResponse
    │     └─ 持久化 ModelCall 审计记录（调用前 prepareAudit()）
    │
    ├─→ RoleExecutor.validate_output(output)  → OK | Err(InvalidArtifactSchema)
    │
    └─→ Repository.save_artifact(project_id, artifact_type, content)
          └─ 写入对应产物表（version = max(existing)+1，status = draft）
```

## AgentDefinition 注册方案

### 问题

现有 `DefinitionRegistry` 从 `/app/agent-definitions/registry.json` 加载，该文件由构建脚本生成。制作角色的 `AgentDefinition` 需要注册进去。

### 方案

在 `agent-definitions/` 目录下新建 `production/` 子目录，存放9个角色的 YAML source 文件，由构建脚本一同编译进 `registry.json`。

格式对齐现有 agent（`executor_owner: rust`，节点名规范：`{role_key}.execute`）：

```yaml
# agent-definitions/production/producer.yaml
agent_key: "production.producer"
version: "1.0.0"
status: "active"
executor_owner: "rust"
role: "制片人"
goals:
  - "根据用户需求产出结构化创意简报（CreativeBrief）"
constraints:
  - "不得自行安排视频生成或发布"
  - "输出必须是符合 creative_brief schema 的纯 JSON"
model_requirements:
  text: true
  tool_calling: false
  structured_output: true
  vision: false
  reasoning: false
  min_context_window: 8192
tool_profiles:
  - "chat"
tools: []
nodes:
  production.producer.execute:
    key: "production.producer.general"
    version: "1.0.0"
```

对应的 `PromptDefinition` 源文件放在 `agent-definitions/prompts/production/`，引用现有的 `prompts/roles/producer/general.v1.txt` 内容。

### 构建流程调整

在 `build-registry-v2.mjs` 脚本或同级新脚本中，加入 `production/` 目录的扫描，将新角色合并进 `registry.json`。

### 临时方案（本 change 范围）

为避免阻塞集成测试，本 change 直接将9个角色的 `AgentDefinition` 以 JSON 写入 `registry.json`，并将 prompt 模板写入对应的模板目录。完整的构建脚本集成作为后续优化项（在 tasks.md 中标注）。

## RoleExecutionContext 设计

```rust
pub struct RoleExecutionContext {
    pub pool: PgPool,
    pub definition_registry: Arc<DefinitionRegistry>,
    pub audited_executor: Arc<AuditedModelExecutor>,
    pub project_id: Uuid,
    pub role_key: String,
    pub user_input: Option<String>,
    /// 使用项目首选模型 ID（从 ProductionProject.metadata 获取，或 AppConfig 默认值）
    pub preferred_model_id: Option<Uuid>,
}
```

## 输入产物 Context 装配

```rust
fn build_context_candidates(
    role_def: &RoleDefinition,
    input_artifacts: &HashMap<ArtifactType, Vec<Value>>,
    user_input: Option<&str>,
) -> Vec<ContextCandidate> {
    let mut candidates = vec![];
    
    // 用户输入（P0, UserInstruction）
    if let Some(input) = user_input {
        candidates.push(ContextCandidate {
            id: "user_input".into(),
            source: "user".into(),
            source_version: None,
            trust: TrustLevel::UserInstruction,
            priority: ContextPriority::P0,
            required: false,
            content: ContextPayload::Text(input.to_string()),
            ..Default::default()
        });
    }
    
    // 各输入产物（P1, ConfirmedFact）
    for artifact_type in &role_def.input_artifacts {
        if let Some(artifacts) = input_artifacts.get(artifact_type) {
            if let Some(latest) = artifacts.first() { // 已按 version DESC 排序
                candidates.push(ContextCandidate {
                    id: format!("{:?}", artifact_type).to_lowercase(),
                    source: "production_state".into(),
                    trust: TrustLevel::ConfirmedFact,
                    priority: ContextPriority::P1,
                    required: true,
                    content: ContextPayload::Text(
                        serde_json::to_string_pretty(latest).unwrap_or_default()
                    ),
                    ..Default::default()
                });
            }
        }
    }
    
    candidates
}
```

## 输出解析策略

角色输出预期为 `structured_output = true` 的纯 JSON。解析流程：

1. 直接尝试 `serde_json::from_str` 解析
2. 若解析成功，调用各产物的 `validate(content)` 校验 schema
3. 校验通过 → 写入对应产物表（status = draft，version = 上一版 +1）
4. 校验失败 → 返回 `ProductionError::InvalidArtifactSchema`

## AppState 集成

```rust
// backend/src/bootstrap/state.rs 新增方法
pub(crate) fn production_orchestrator(&self) -> Result<ProductionOrchestrator, AppStateError> {
    let pool = self.database_pool()?;
    let definition_registry = self.definition_registry()?;
    let audited_executor = self.audited_model_executor(pool.clone())?;
    let role_registry = Arc::new(RoleRegistry::bootstrap());
    let gate_registry = Arc::new(GateRegistry::bootstrap());
    Ok(ProductionOrchestrator {
        role_registry,
        state_repository: Arc::new(ProductionStateRepository::new(pool)),
        gate_registry,
        audited_executor: Some(audited_executor),
        definition_registry: Some(definition_registry),
        pool,
    })
}
```

## 错误处理映射

| ProductionError | HTTP 状态码 | error_code |
|---|---|---|
| MissingInputArtifact | 400 | missing_input_artifact |
| InvalidArtifactSchema | 422 | invalid_artifact_schema |
| RoleNotFound | 404 | role_not_found |
| GateRejected | 409 | gate_rejected |
| GateWaitApproval | 409 | waiting_approval |
| AgentExecution | 502 | agent_execution_failed |
| Database | 500 | internal_error |

## 测试策略

- **单元测试**：mock `AuditedModelExecutor`，验证 Prompt 编译和产物解析逻辑
- **集成测试**：在 `#[ignore]` 测试中使用真实 DB + mock LLM client，验证端到端角色执行
- **HTTP 测试**：使用 `tower::ServiceExt`，验证 200/400/422 响应格式
