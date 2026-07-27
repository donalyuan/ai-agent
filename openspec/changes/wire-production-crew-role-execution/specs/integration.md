# 集成规格：DefinitionRegistry + AuditedModelExecutor 对接

## RoleExecutionContext 依赖注入规格

`RoleExecutionContext` 封装单次角色执行所需的全部依赖，避免 `ProductionOrchestrator` 直接依赖 `AppState`：

```rust
pub struct RoleExecutionContext {
    /// 数据库连接池
    pub pool: PgPool,
    /// Agent/Prompt 定义注册表（只读）
    pub definition_registry: Arc<DefinitionRegistry>,
    /// 带审计的模型执行器
    pub audited_executor: Arc<AuditedModelExecutor>,
    /// 当前制作项目 ID
    pub project_id: Uuid,
    /// 角色标识（如 "producer"）
    pub role_key: String,
    /// 用户补充输入（可选）
    pub user_input: Option<String>,
    /// 优选模型 ID（从项目元数据或 AppConfig 获取）
    pub preferred_model_id: Uuid,
}
```

## AgentDefinition 文件组织规格

### 目录结构

```
agent-definitions/
  production/                    ← 新增目录
    producer.yaml
    screenwriter.yaml
    director.yaml
    cinematographer.yaml
    performance_director.yaml
    sound_director.yaml
    editor.yaml
    qc.yaml
    character_critic.yaml
  prompts/
    production/                  ← 新增目录
      producer.general.1.0.0.txt ← 引用 crates/novex-production-crew/prompts/roles/producer/general.v1.txt 内容
      screenwriter.general.1.0.0.txt
      ... (其余8个角色同理)
```

### PromptDefinition YAML 格式

```yaml
key: "production.producer.general"
version: "1.0.0"
status: "active"
schema_version: "1"
system:
  template: "production/producer.general.1.0.0.txt"
  trust: "confirmed_fact"
  source: "production_crew"
user_template: null   # user 层通过 fragments 注入产物 context
variables: []
fragments:
  - id: "context"
    trust: "confirmed_fact"
    source: "production_state"
    required: true
output_schema:
  type: "object"
  # production.producer 输出 schema（CreativeBrief 内容结构）
  required: ["target_audience", "key_messages"]
  properties:
    target_audience: { type: "string" }
    tone: { type: "array", items: { type: "string" } }
    key_messages: { type: "array", items: { type: "string" } }
    constraints: { type: "object" }
    success_criteria: { type: "array", items: { type: "string" } }
```

## registry.json 更新规格

`build-registry-v2.mjs` 脚本需要扫描 `production/` 目录并将新 agent 合并进 `registry.json`。

临时方案（本 change 范围内）：直接手工将9个角色的 `AgentDefinition` JSON append 到 `registry.json` 的 `"agents"` 数组，`"prompts"` 数组同理。构建脚本自动化作为后续优化。

## ContextCandidate 装配规格

### 优先级分配

| 内容 | trust | priority | required |
|---|---|---|---|
| 用户补充输入 | UserInstruction | P0 | false |
| approved 输入产物（必需） | ConfirmedFact | P1 | true |
| draft 输入产物（参考） | Reference | P2 | false |
| 项目元数据 | ConfirmedFact | P1 | false |

### 产物序列化格式

将产物 JSONB 内容序列化为带前缀的文本片段：

```
=== CreativeBrief (v1, approved) ===
{
  "target_audience": "...",
  ...
}
```

多个同类产物（如 CharacterBible[]）按 character_id 分段注入。

## AuditedModelExecutor 调用规格

调用序列严格遵循现有模式：

1. `prepare_call(owner, binding, prompt_snapshot)` → `PrepareAuditedCall`
2. 发起外部模型调用
3. `finish_call(prepared, response)` → `FinishAuditedCall`

`AuditedCallOwner` 使用 `AgentRun(run_id)`，run_id 在角色执行开始时创建。

## 模型 ID 解析规格

解析顺序（首个非空值生效）：

1. 请求体中的 `context.preferred_model_id`（可选覆盖）
2. `ProductionProject.metadata.preferred_model_id`
3. `AppConfig` 中的 `default_production_model_id` 环境变量
4. 若全部为空 → 返回 `500 internal_error: no model configured for production crew`

## 产物写入规格

写入规则：
- `version`：`SELECT COALESCE(MAX(version), 0) + 1 FROM {table} WHERE production_project_id = $1`
- `status`：`draft`（不自动 approve）
- `created_by`：角色的 `role_key`（如 "producer"）
- 写入后：更新 `production_projects.status` 至对应阶段（producer完成 → "scripting"，screenwriter完成 → "directing"，etc.）

## ProductionOrchestrator 新增字段

```rust
pub struct ProductionOrchestrator {
    pub role_registry: Arc<RoleRegistry>,
    pub state_repository: Arc<ProductionStateRepository>,
    pub gate_registry: Arc<GateRegistry>,
    pub pool: PgPool,
    // 新增
    pub audited_executor: Option<Arc<AuditedModelExecutor>>,
    pub definition_registry: Option<Arc<DefinitionRegistry>>,
}
```

`execute_role()` 方法新增：检查 `audited_executor` 和 `definition_registry` 是否已注入，若未注入返回 `AgentExecution("executor not configured")` 错误。
