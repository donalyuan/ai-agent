# 虚拟制作团队架构规格

## 系统架构

### 核心组件

```
┌─────────────────────────────────────────────────────────────┐
│                    HTTP API Layer                            │
│  /api/productions, /api/productions/:id/roles/:role/execute │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│              ProductionOrchestrator                          │
│  - 路由决策 (Fast Lane / Full Crew)                          │
│  - 角色调度                                                   │
│  - 检查点管理                                                 │
└─────────────────────────────────────────────────────────────┘
                              ↓
        ┌─────────────────────┴─────────────────────┐
        ↓                                           ↓
┌──────────────────┐                    ┌──────────────────────┐
│  RoleRegistry    │                    │  ProductionState     │
│  - RoleDefinition│                    │  - 10种结构化产物     │
│  - 8个专业角色    │                    │  - 版本管理          │
└──────────────────┘                    │  - 协作建议          │
        ↓                                └──────────────────────┘
┌──────────────────────────────────────────────────────────────┐
│              AgentRunCoordinator                              │
│  - 调用 PromptCompiler 生成角色 Prompt                        │
│  - 执行 ModelCall                                             │
│  - 保存审计快照                                                │
└──────────────────────────────────────────────────────────────┘
        ↓
┌──────────────────────────────────────────────────────────────┐
│                    Gate System                                │
│  ProducerGate | ScriptApprovalGate | TechnicalFeasibilityGate│
│  QualityGate | BudgetGate | PublishGate                      │
└──────────────────────────────────────────────────────────────┘
```

### 数据流

**Fast Lane 流程**：
```
用户输入 → ProductionOrchestrator (识别为Fast Lane)
         → 简化脚本生成
         → 直接调用视频生成 Worker
         → 返回结果
```

**Full Crew 流程**：
```
用户输入 → ProductionOrchestrator (识别为Full Crew)
         → Producer 产出 CreativeBrief
         → Screenwriter 产出 StoryBible + CharacterBible + ScriptDraft
         → ScriptApprovalGate (人工审核)
         → Director 产出 DirectorialTreatment + ShotContract
         → Cinematographer 技术审核
         → PerformanceDirector 产出 PerformanceBrief
         → SoundDirector 产出 SoundPlan
         → 执行生成 (调用 Worker)
         → Editor 维护 ContinuityLedger
         → QC 评审 TakeReview
         → QualityGate
         → 用户确认
         → 发布
```

## 技术栈

- **后端编排层**：Rust + Axum
- **数据存储**：PostgreSQL (ProductionState)
- **角色定义**：版本化 YAML manifest + 独立 Prompt 模板
- **模型调用**：复用现有 AgentRunCoordinator + ModelCall 审计
- **视频生成**：复用现有 Python Worker
- **API 协议**：RESTful JSON

## 部署架构

```
┌──────────────┐       ┌──────────────┐       ┌──────────────┐
│  ai-agent    │       │ PostgreSQL   │       │ Python Worker│
│  -api        │◄─────►│ video_agent  │       │              │
│  :18180      │       │   database   │       │   :18181     │
└──────────────┘       └──────────────┘       └──────────────┘
       │                                              ▲
       │                                              │
       └──────────────────────────────────────────────┘
                 (通过 Redis 队列调度)
```

## 扩展点

1. **新增角色**：在 RoleRegistry 注册 RoleDefinition + Prompt 模板
2. **新增产物类型**：在 ProductionState 添加表 + JSONB schema
3. **新增 Gate**：实现 Gate trait 并注册到 Orchestrator
4. **自定义工作流**：Orchestrator 支持可配置的角色调度顺序
