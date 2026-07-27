# 虚拟制作团队设计文档

## 架构设计

### 核心设计原则

1. **受控编排优先**：Orchestrator 控制角色调度顺序和检查点，不采用多 Agent 自由群聊
2. **产物所有权清晰**：每个角色只拥有自己的产物，通过建议协作
3. **版本化与可审计**：所有定义、产物、模型调用完整审计
4. **复用现有基座**：基于 AgentDefinition、PromptCompiler、ModelCall 体系
5. **Fast Lane 并行存在**：简单场景快速通道，复杂场景完整团队

### 模块划分

```
crates/
  novex-production-crew/
    src/
      orchestrator/           # ProductionOrchestrator 核心编排逻辑
        mod.rs
        fast_lane.rs          # Fast Lane 执行器
        full_crew.rs          # Full Crew 执行器
        route.rs              # 路由决策
      
      roles/                  # 角色定义与注册
        mod.rs
        registry.rs           # RoleRegistry
        definition.rs         # RoleDefinition struct
        loader.rs             # 从 manifest 加载角色定义
      
      state/                  # ProductionState 管理
        mod.rs
        repository.rs         # 数据库 CRUD
        artifacts/            # 各类产物的结构体与 schema 验证
          creative_brief.rs
          story_bible.rs
          character_bible.rs
          script_draft.rs
          directorial_treatment.rs
          shot_contract.rs
          performance_brief.rs
          sound_plan.rs
          continuity_ledger.rs
          take_review.rs
        collaboration.rs      # 协作建议管理
        versioning.rs         # 版本管理逻辑
      
      gates/                  # 质量闸门
        mod.rs
        gate_trait.rs         # Gate trait 定义
        producer_gate.rs
        script_approval_gate.rs
        technical_feasibility_gate.rs
        quality_gate.rs
        budget_gate.rs
        publish_gate.rs
      
      executor/               # 角色执行器
        mod.rs
        role_executor.rs      # 执行单个角色的逻辑
        flow_executor.rs      # 批量执行流程
      
      lib.rs

backend/src/routes/production/  # HTTP API 路由
  mod.rs
  projects.rs                   # 项目 CRUD
  roles.rs                      # 角色执行
  artifacts.rs                  # 产物管理
  suggestions.rs                # 协作建议
  fast_lane.rs                  # 快速通道

prompts/roles/                  # 角色 Prompt 模板
  producer/
    general.v1.yaml
    general.v1.txt
  screenwriter/
    general.v1.yaml
    general.v1.txt
  director/
    general.v1.yaml
    general.v1.txt
  cinematographer/
    general.v1.yaml
    general.v1.txt
  performance_director/
    general.v1.yaml
    general.v1.txt
  sound_director/
    general.v1.yaml
    general.v1.txt
  editor/
    general.v1.yaml
    general.v1.txt
  qc/
    general.v1.yaml
    general.v1.txt
  character_critic/
    general.v1.yaml
    general.v1.txt

roles/                          # 角色定义 manifest
  producer.yaml
  screenwriter.yaml
  director.yaml
  cinematographer.yaml
  performance_director.yaml
  sound_director.yaml
  editor.yaml
  qc.yaml
  character_critic.yaml
  registry.yaml                 # 角色注册表
```

### 数据流设计

#### Full Crew 完整流程

```
1. 用户创建项目 (POST /productions)
   ↓
2. Orchestrator.route_execution()
   - 检查 project_type = "full_crew"
   - 返回角色执行计划
   ↓
3. 执行 Producer 角色
   - RoleRegistry.get("producer")
   - 加载 RoleDefinition
   - PromptCompiler.compile(
       agent_def: producer.general@1,
       context: [用户需求]
     )
   - AgentRunCoordinator.execute()
   - 解析输出为 CreativeBrief
   - ProductionStateRepository.save_creative_brief()
   - 保存 ModelCall 审计
   ↓
4. ProducerGate.check()
   - 验证 CreativeBrief 完整性
   - 检查预算可行性
   - 通过 → 继续；拦截 → 返回错误
   ↓
5. 执行 Screenwriter 角色
   - 加载 CreativeBrief 作为 Context
   - 生成 StoryBible, CharacterBible[], ScriptDraft
   - 保存产物（status = draft）
   ↓
6. ScriptApprovalGate.check()
   - 等待人工审核
   - 用户调用 POST /artifacts/:id/approve
   - 产物 status = approved
   ↓
7. 执行 Director 角色
   - 加载 StoryBible, ScriptDraft, CharacterBible[]
   - 生成 DirectorialTreatment, ShotContract[]
   ↓
8. 执行 Cinematographer 角色
   - 加载 ShotContract[]
   - 生成 TechnicalReview (作为 collaboration_suggestions)
   - 如有高优先级建议 → 等待导演响应
   ↓
9. 执行 PerformanceDirector 角色
   - 加载 CharacterBible[], ScriptDraft
   - 生成 PerformanceBrief[] (按角色)
   ↓
10. 执行 SoundDirector 角色
    - 加载 DirectorialTreatment, ShotContract[]
    - 生成 SoundPlan
   ↓
11. 执行生成 (调用现有 Video Worker)
    - 遍历 ShotContract[]
    - 每个 shot 生成一个视频
    - 调用 POST /api/v1/video/generate
   ↓
12. 执行 Editor 角色
    - 检查已生成镜头
    - 提取视觉事实
    - 生成 ContinuityLedger[] (按 shot)
   ↓
13. 执行 QC 角色
    - 加载 ShotContract, ContinuityLedger, 生成结果
    - 评审每个 take
    - 生成 TakeReview[]
    - status = approved | rejected | needs_revision
   ↓
14. QualityGate.check()
    - 检查所有 TakeReview
    - 如有 rejected → 标记需重新生成的 shot
    - 全部 approved → 通过
   ↓
15. 用户确认
    - 调用 POST /productions/:id/publish
   ↓
16. PublishGate.check()
    - 验证所有必需产物已批准
    - 验证权限
    - 调用平台发布接口
```

#### Fast Lane 简化流程

```
1. 用户创建项目 (POST /productions, project_type = "fast_lane")
   ↓
2. Orchestrator.route_execution()
   - 识别为 Fast Lane
   - 跳过完整团队
   ↓
3. 简化脚本生成
   - 使用简化 Prompt
   - 直接生成 prompt for video generation
   ↓
4. 调用视频生成 Worker
   - POST /api/v1/video/generate
   ↓
5. 返回结果
   - 不保存完整制作产物
```

### 关键接口设计

#### ProductionOrchestrator

```rust
pub struct ProductionOrchestrator {
    role_registry: Arc<RoleRegistry>,
    state_repository: Arc<ProductionStateRepository>,
    gate_registry: Arc<GateRegistry>,
    agent_coordinator: Arc<AgentRunCoordinator>,
}

impl ProductionOrchestrator {
    /// 路由决策
    pub async fn route_execution(
        &self,
        project: &ProductionProject,
    ) -> Result<ExecutionPlan> {
        match project.project_type {
            ProjectType::FastLane => self.plan_fast_lane(project),
            ProjectType::FullCrew => self.plan_full_crew(project),
        }
    }
    
    /// 执行单个角色
    pub async fn execute_role(
        &self,
        project_id: Uuid,
        role_key: &str,
        user_input: Option<serde_json::Value>,
    ) -> Result<RoleExecutionResult> {
        // 1. 加载 RoleDefinition
        // 2. 检查输入产物是否就绪
        // 3. 调用 PromptCompiler
        // 4. 执行 ModelCall
        // 5. 验证输出 schema
        // 6. 保存产物
        // 7. 保存审计
    }
    
    /// 执行完整流程
    pub async fn execute_flow(
        &self,
        project_id: Uuid,
        roles: Vec<String>,
        auto_approve: bool,
    ) -> Result<FlowExecution> {
        // 顺序执行角色
        // 遇到 Gate 时暂停或自动通过
    }
}
```

#### RoleDefinition

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleDefinition {
    pub role_key: String,
    pub role_name: String,
    pub responsibilities: Vec<String>,
    pub input_artifacts: Vec<ArtifactType>,
    pub output_artifacts: Vec<ArtifactType>,
    pub allowed_tools: Vec<String>,
    pub prompt_definition_ref: PromptRef,
    pub lifecycle: Lifecycle,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptRef {
    pub key: String,      // "producer.general"
    pub version: String,  // "@1"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactType {
    CreativeBrief,
    StoryBible,
    CharacterBible,
    ScriptDraft,
    DirectorialTreatment,
    ShotContract,
    PerformanceBrief,
    SoundPlan,
    ContinuityLedger,
    TakeReview,
}
```

#### Gate Trait

```rust
#[async_trait]
pub trait Gate: Send + Sync {
    fn name(&self) -> &str;
    
    async fn check(
        &self,
        project_id: Uuid,
        context: &GateContext,
    ) -> Result<GateDecision>;
}

pub enum GateDecision {
    Pass,
    Reject { reason: String },
    WaitApproval { artifact_id: Uuid },
}

pub struct GateContext {
    pub project: ProductionProject,
    pub artifacts: HashMap<ArtifactType, Vec<serde_json::Value>>,
    pub user_id: Uuid,
}
```

### 错误处理设计

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProductionError {
    #[error("Missing required input artifact: {artifact_type}")]
    MissingInputArtifact { artifact_type: String },
    
    #[error("Invalid artifact schema: {details}")]
    InvalidArtifactSchema { details: String },
    
    #[error("Gate rejected: {gate_name} - {reason}")]
    GateRejected { gate_name: String, reason: String },
    
    #[error("Role not found: {role_key}")]
    RoleNotFound { role_key: String },
    
    #[error("Invalid role sequence: {message}")]
    InvalidRoleSequence { message: String },
    
    #[error("Budget exceeded: requested {requested}, available {available}")]
    BudgetExceeded { requested: u64, available: u64 },
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Agent execution error: {0}")]
    AgentExecution(String),
}
```

### 性能考虑

1. **并行执行**：同一层级的独立角色可并行（如多个 CharacterBible 生成）
2. **缓存策略**：RoleDefinition、PromptDefinition 加载后缓存
3. **连接池**：PostgreSQL 连接池复用
4. **异步执行**：所有 I/O 操作异步
5. **流式输出**：长时间执行流程通过 SSE 推送进度

### 安全考虑

1. **权限校验**：每个 API 调用验证用户对项目的所有权
2. **输入验证**：所有 JSONB 产物在保存前验证 schema
3. **SQL 注入防护**：使用 sqlx 参数化查询
4. **成本控制**：BudgetGate 在生成前检查预算
5. **审计完整**：所有模型调用、产物修改、Gate 决策完整记录

### 测试策略

1. **单元测试**：每个 Gate、Repository、Executor 独立测试
2. **集成测试**：完整 Fast Lane 和 Full Crew 流程
3. **Contract 测试**：验证产物 schema
4. **负载测试**：并发项目执行
5. **Prompt 评测**：使用 EvalRun 对比角色输出质量

### 监控与可观测性

1. **结构化日志**：每个角色执行记录耗时、token 数、成本
2. **Metrics**：
   - 项目创建数
   - 角色执行成功率
   - Gate 拦截率
   - 平均执行时长
   - 平均 token 消耗
3. **Tracing**：完整请求链路追踪
4. **审计日志**：所有关键操作可追溯

## 技术债务与后续优化

1. **Memory 系统集成**：当前未集成正式 Memory，角色无长期记忆
2. **Planner 集成**：失败诊断和重试策略需要局部 Planner
3. **角色并行**：当前顺序执行，未来支持依赖图并行
4. **动态工作流**：支持用户自定义角色顺序
5. **外部协作**：支持真人加入虚拟团队
6. **实时协作**：多角色同时在线编辑（超出当前范围）
