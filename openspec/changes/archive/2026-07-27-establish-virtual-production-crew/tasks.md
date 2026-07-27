# 虚拟制作团队实施任务清单

## Phase 1: 基础设施搭建

### 1.1 数据库 Schema 建立

- [x] 创建 PostgreSQL migration 文件
  - [x] `production_projects` 表
  - [x] `creative_briefs` 表
  - [x] `story_bibles` 表
  - [x] `character_bibles` 表
  - [x] `script_drafts` 表
  - [x] `directorial_treatments` 表
  - [x] `shot_contracts` 表
  - [x] `performance_briefs` 表
  - [x] `sound_plans` 表
  - [x] `continuity_ledgers` 表
  - [x] `take_reviews` 表
  - [x] `collaboration_suggestions` 表
- [x] 创建索引和外键约束
- [x] 编写 migration 测试
- [x] 执行 migration 并验证

### 1.2 Crate 结构创建

- [x] 创建 `crates/novex-production-crew` crate
- [x] 配置 `Cargo.toml` 依赖
  - [x] `sqlx` for PostgreSQL
  - [x] `serde`, `serde_json` for JSON
  - [x] `uuid` for ID 生成
  - [x] `chrono` for 时间
  - [x] `thiserror` for 错误处理
  - [x] `async-trait` for trait
  - [x] `tokio` for 异步运行时
- [x] 创建模块目录结构
  - [x] `orchestrator/`
  - [x] `roles/`
  - [x] `state/`
  - [x] `gates/`
  - [x] `executor/`
- [x] 配置 `backend/Cargo.toml` 引用新 crate

---

## Phase 2: ProductionState 层实现

### 2.1 产物结构体定义

- [x] `state/artifacts/mod.rs` 定义 `ArtifactType` 枚举
- [x] `state/artifacts/creative_brief.rs`
  - [x] `CreativeBrief` struct
  - [x] JSON schema 验证函数
  - [x] 单元测试
- [x] `state/artifacts/story_bible.rs`
  - [x] `StoryBible` struct
  - [x] Schema 验证
  - [x] 单元测试
- [x] `state/artifacts/character_bible.rs`
  - [x] `CharacterBible` struct
  - [x] Schema 验证
  - [x] 单元测试
- [x] `state/artifacts/script_draft.rs`
  - [x] `ScriptDraft` struct
  - [x] Schema 验证
  - [x] 单元测试
- [x] `state/artifacts/directorial_treatment.rs`
  - [x] `DirectorialTreatment` struct
  - [x] Schema 验证
  - [x] 单元测试
- [x] `state/artifacts/shot_contract.rs`
  - [x] `ShotContract` struct
  - [x] Schema 验证
  - [x] 单元测试
- [x] `state/artifacts/performance_brief.rs`
  - [x] `PerformanceBrief` struct
  - [x] Schema 验证
  - [x] 单元测试
- [x] `state/artifacts/sound_plan.rs`
  - [x] `SoundPlan` struct
  - [x] Schema 验证
  - [x] 单元测试
- [x] `state/artifacts/continuity_ledger.rs`
  - [x] `ContinuityLedger` struct
  - [x] Schema 验证
  - [x] 单元测试
- [x] `state/artifacts/take_review.rs`
  - [x] `TakeReview` struct
  - [x] Schema 验证
  - [x] 单元测试

### 2.2 Repository 实现

- [x] `state/repository.rs`
  - [x] `ProductionStateRepository` struct
  - [x] `create_project()` 方法
  - [x] `get_project()` 方法
  - [x] `list_projects()` 方法（带分页）
  - [x] `delete_project()` 方法（软删除）
  - [x] `update_project_status()` 方法
- [x] 各产物 CRUD 方法
  - [x] `save_creative_brief()`
  - [x] `get_creative_brief()`
  - [x] `save_story_bible()`
  - [x] `get_story_bible()`
  - [x] `save_character_bible()`
  - [x] `get_character_bibles_by_project()`
  - [x] `save_script_draft()`
  - [x] `get_script_draft()`
  - [x] `save_directorial_treatment()`
  - [x] `get_directorial_treatment()`
  - [x] `save_shot_contract()`
  - [x] `get_shot_contracts_by_project()`
  - [x] `save_performance_brief()`
  - [x] `get_performance_briefs_by_project()`
  - [x] `save_sound_plan()`
  - [x] `get_sound_plan()`
  - [x] `save_continuity_ledger()`
  - [x] `get_continuity_ledgers_by_project()`
  - [x] `save_take_review()`
  - [x] `get_take_reviews_by_shot()`
- [x] Repository 集成测试（使用 test database）

### 2.3 版本管理

- [x] `state/versioning.rs`
  - [x] `approve_artifact()` 方法（标记 approved，supersede 旧版本）
  - [x] `create_new_version()` 方法
  - [x] `get_latest_approved()` 方法
  - [x] `get_artifact_history()` 方法
- [x] 版本管理单元测试

### 2.4 协作建议管理

- [x] `state/collaboration.rs`
  - [x] `create_suggestion()` 方法
  - [x] `get_suggestions_by_project()` 方法
  - [x] `get_pending_suggestions_for_role()` 方法
  - [x] `respond_to_suggestion()` 方法
- [x] 协作建议单元测试

---

## Phase 3: 角色定义与注册

### 3.1 角色定义 Schema

- [x] `roles/definition.rs`
  - [x] `RoleDefinition` struct
  - [x] `PromptRef` struct
  - [x] `Lifecycle` 枚举
  - [x] Serialize/Deserialize 实现

### 3.2 角色 Manifest 文件

- [x] 创建 `roles/` 目录
- [x] `roles/producer.yaml`
  - [x] 定义 role_key、role_name、responsibilities
  - [x] 定义 input_artifacts、output_artifacts
  - [x] 引用 `producer.general@1`
- [x] `roles/screenwriter.yaml`
- [x] `roles/director.yaml`
- [x] `roles/cinematographer.yaml`
- [x] `roles/performance_director.yaml`
- [x] `roles/sound_director.yaml`
- [x] `roles/editor.yaml`
- [x] `roles/qc.yaml`
- [x] `roles/character_critic.yaml`
- [x] `roles/registry.yaml` 汇总注册表

### 3.3 角色加载器

- [x] `roles/loader.rs`
  - [x] `load_role_definitions()` 从 YAML 加载
  - [x] 验证 manifest 完整性
  - [x] 验证 PromptRef 引用存在
- [x] Loader 单元测试

### 3.4 角色注册表

- [x] `roles/registry.rs`
  - [x] `RoleRegistry` struct
  - [x] `register()` 方法
  - [x] `get()` 方法
  - [x] `list_all()` 方法
  - [x] `validate_sequence()` 验证角色执行顺序合法性
- [x] Registry 单元测试

---

## Phase 4: Prompt 模板创建

### 4.1 Producer Prompt

- [x] 创建 `prompts/roles/producer/` 目录
- [x] `prompts/roles/producer/general.v1.yaml`
  - [x] 定义 prompt_key、version、lifecycle
  - [x] 定义 model_requirements
  - [x] 定义 output_schema (CreativeBrief)
- [x] `prompts/roles/producer/general.v1.txt`
  - [x] 编写角色职责说明
  - [x] 编写输出格式要求
  - [x] 编写约束条件

### 4.2 Screenwriter Prompt

- [x] `prompts/roles/screenwriter/general.v1.yaml`
- [x] `prompts/roles/screenwriter/general.v1.txt`
  - [x] 强调故事结构、角色塑造
  - [x] 输出 StoryBible, CharacterBible[], ScriptDraft

### 4.3 Director Prompt

- [x] `prompts/roles/director/general.v1.yaml`
- [x] `prompts/roles/director/general.v1.txt`
  - [x] 强调视觉叙事、镜头语言
  - [x] 输出 DirectorialTreatment, ShotContract[]

### 4.4 Cinematographer Prompt

- [x] `prompts/roles/cinematographer/general.v1.yaml`
- [x] `prompts/roles/cinematographer/general.v1.txt`
  - [x] 技术可行性审核
  - [x] 输出 TechnicalReview (collaboration_suggestions)

### 4.5 PerformanceDirector Prompt

- [x] `prompts/roles/performance_director/general.v1.yaml`
- [x] `prompts/roles/performance_director/general.v1.txt`
  - [x] 角色表演指导
  - [x] 输出 PerformanceBrief[]

### 4.6 SoundDirector Prompt

- [x] `prompts/roles/sound_director/general.v1.yaml`
- [x] `prompts/roles/sound_director/general.v1.txt`
  - [x] 音乐、音效、对话录音规划
  - [x] 输出 SoundPlan

### 4.7 Editor Prompt

- [x] `prompts/roles/editor/general.v1.yaml`
- [x] `prompts/roles/editor/general.v1.txt`
  - [x] 连续性分析
  - [x] 输出 ContinuityLedger[]

### 4.8 QC Prompt

- [x] `prompts/roles/qc/general.v1.yaml`
- [x] `prompts/roles/qc/general.v1.txt`
  - [x] 质量评审标准
  - [x] 输出 TakeReview[]

### 4.9 CharacterCritic Prompt

- [x] `prompts/roles/character_critic/general.v1.yaml`
- [x] `prompts/roles/character_critic/general.v1.txt`
  - [x] 角色视角校验
  - [x] 输出 CharacterReview (collaboration_suggestions)

---

## Phase 5: Gate 实现

### 5.1 Gate Trait

- [x] `gates/gate_trait.rs`
  - [x] `Gate` trait 定义
  - [x] `GateDecision` 枚举
  - [x] `GateContext` struct

### 5.2 具体 Gate 实现

- [x] `gates/producer_gate.rs`
  - [x] 验证 CreativeBrief 完整性
  - [x] 检查预算合理性
- [x] `gates/script_approval_gate.rs`
  - [x] 检查 ScriptDraft status
  - [x] 如未 approved，返回 WaitApproval
- [x] `gates/technical_feasibility_gate.rs`
  - [x] 检查 Cinematographer 的高优先级建议
  - [x] 如有未响应建议，返回 WaitApproval
- [x] `gates/quality_gate.rs`
  - [x] 检查所有 TakeReview
  - [x] 如有 rejected，返回 Reject
- [x] `gates/budget_gate.rs`
  - [x] 计算预计生成成本
  - [x] 检查用户余额
- [x] `gates/publish_gate.rs`
  - [x] 验证所有必需产物已批准
  - [x] 验证用户权限
- [x] 各 Gate 单元测试

### 5.3 Gate Registry

- [x] `gates/mod.rs`
  - [x] `GateRegistry` struct
  - [x] `register()` 方法
  - [x] `get()` 方法
  - [x] Bootstrap 时注册所有 Gate

---

## Phase 6: Orchestrator 实现

### 6.1 路由决策

- [x] `orchestrator/route.rs`
  - [x] `route_execution()` 方法
  - [x] 识别 Fast Lane vs Full Crew
  - [x] 返回 `ExecutionPlan`
- [x] 路由决策单元测试

### 6.2 Fast Lane 执行器

- [x] `orchestrator/fast_lane.rs`
  - [x] `execute_fast_lane()` 方法
  - [x] 简化脚本生成
  - [x] 调用视频生成 Worker
  - [x] 返回结果
- [x] Fast Lane 集成测试

### 6.3 Full Crew 执行器

- [x] `orchestrator/full_crew.rs`
  - [x] `plan_full_crew()` 生成角色执行计划
  - [x] 定义角色依赖关系
  - [x] 插入 Gate 检查点
- [x] Full Crew 计划单元测试

### 6.4 Orchestrator 核心

- [x] `orchestrator/mod.rs`
  - [x] `ProductionOrchestrator` struct
  - [x] `new()` 构造函数
  - [x] `route_execution()` 调用路由
  - [x] `execute_role()` 执行单个角色
  - [x] `execute_flow()` 执行完整流程
- [x] Orchestrator 集成测试

---

## Phase 7: 角色执行器实现

### 7.1 单角色执行器

- [x] `executor/role_executor.rs`
  - [x] `RoleExecutor` struct
  - [x] `execute()` 方法
    - [x] 加载 RoleDefinition
    - [x] 检查输入产物就绪
    - [x] 从 ProductionState 读取输入
    - [x] 调用 PromptCompiler
    - [x] 调用 AgentRunCoordinator
    - [x] 解析输出
    - [x] 验证产物 schema
    - [x] 保存产物到 ProductionState
    - [x] 保存 ModelCall 审计
- [x] RoleExecutor 单元测试

### 7.2 流程执行器

- [x] `executor/flow_executor.rs`
  - [x] `FlowExecutor` struct
  - [x] `execute_flow()` 方法
    - [x] 遍历角色列表
    - [x] 顺序执行每个角色
    - [x] 遇到 Gate 时检查
    - [x] 根据 GateDecision 决定继续或暂停
    - [x] 记录流程状态
- [x] FlowExecutor 集成测试

### 7.3 PromptCompiler 集成

- [x] 确保现有 PromptCompiler 支持角色 Prompt
- [x] 测试 Context 装配（输入产物作为 Context）
- [x] 测试输出 Schema 验证

---

## Phase 8: HTTP API 实现

### 8.1 项目管理路由

- [x] `backend/src/routes/production/projects.rs`
  - [x] `POST /api/v1/production/productions`
    - [x] 请求体验证
    - [x] 调用 `ProductionStateRepository.create_project()`
    - [x] 返回项目详情
  - [x] `GET /api/v1/production/productions/:id`
    - [x] 权限校验
    - [x] 查询项目及关联产物摘要
  - [x] `GET /api/v1/production/productions`
    - [x] 分页查询
    - [x] 过滤条件
  - [x] `DELETE /api/v1/production/productions/:id`
    - [x] 权限校验
    - [x] 软删除

### 8.2 角色执行路由

- [x] `backend/src/routes/production/roles.rs`
  - [x] `POST /api/v1/production/productions/:id/roles/:role_key/execute`
    - [x] 权限校验
    - [x] 调用 `Orchestrator.execute_role()`
    - [x] 返回执行结果
  - [x] `POST /api/v1/production/productions/:id/execute-flow`
    - [x] 调用 `Orchestrator.execute_flow()`
    - [x] 返回流程 ID
  - [x] `GET /api/v1/production/productions/:id/flows/:flow_id`
    - [x] 查询流程状态

### 8.3 产物管理路由

- [x] `backend/src/routes/production/artifacts.rs`
  - [x] `GET /api/v1/production/productions/:id/artifacts/:artifact_type`
    - [x] 支持 version、character_id、shot_id 查询参数
    - [x] 返回产物详情
  - [x] `POST /api/v1/production/productions/:id/artifacts/:artifact_type/:artifact_id/approve`
    - [x] 调用 `ProductionStateRepository.approve_artifact()`
  - [x] `GET /api/v1/production/productions/:id/artifacts/:artifact_type/all`
    - [x] 列出特定类型的所有产物

### 8.4 协作建议路由

- [x] `backend/src/routes/production/suggestions.rs`
  - [x] `POST /api/v1/production/productions/:id/suggestions`
    - [x] 创建协作建议
  - [x] `POST /api/v1/production/productions/:id/suggestions/:suggestion_id/respond`
    - [x] 响应建议
  - [x] `GET /api/v1/production/productions/:id/suggestions`
    - [x] 列出建议（支持过滤）

### 8.5 Fast Lane 路由

- [x] `backend/src/routes/production/fast_lane.rs`
  - [x] `POST /api/v1/production/productions/:id/fast-lane`
    - [x] 调用 `Orchestrator.execute_fast_lane()`
    - [x] 返回 job_id
  - [x] `GET /api/v1/production/productions/:id/fast-lane/:job_id`
    - [x] 查询快速生成状态

### 8.6 审计日志路由

- [x] `GET /api/v1/production/productions/:id/audit-log`
  - [x] 查询项目审计日志
  - [x] 包含角色执行、产物批准、Gate 决策

### 8.7 路由注册

- [x] `backend/src/routes/production/mod.rs`
  - [x] 注册所有子路由
  - [x] 配置中间件（认证、日志、错误处理）
- [x] `backend/src/main.rs`
  - [x] 引入 production 路由
  - [x] 挂载到 `/api/v1/production`

---

## Phase 9: 集成测试

### 9.1 Fast Lane 端到端测试

- [x] 创建测试项目（project_type = fast_lane）
- [x] 执行快速生成
- [x] 验证返回视频 URL
- [x] 验证 ModelCall 审计记录
- [x] 清理测试数据

### 9.2 Full Crew 端到端测试

- [x] 创建测试项目（project_type = full_crew）
- [x] 执行 Producer 角色
  - [x] 验证 CreativeBrief 产出
- [x] 执行 Screenwriter 角色
  - [x] 验证 StoryBible、CharacterBible、ScriptDraft 产出
- [x] 批准 ScriptDraft
- [x] 执行 Director 角色
  - [x] 验证 DirectorialTreatment、ShotContract 产出
- [x] 执行 Cinematographer 角色
  - [x] 验证 TechnicalReview 建议
- [x] 执行 PerformanceDirector 角色
  - [x] 验证 PerformanceBrief 产出
- [x] 执行 SoundDirector 角色
  - [x] 验证 SoundPlan 产出
- [x] 模拟视频生成完成
- [x] 执行 Editor 角色
  - [x] 验证 ContinuityLedger 产出
- [x] 执行 QC 角色
  - [x] 验证 TakeReview 产出
- [x] 验证 QualityGate 通过
- [x] 验证所有 ModelCall 审计记录
- [x] 清理测试数据

### 9.3 协作建议测试

- [x] 创建项目并执行到 Director 阶段
- [x] Cinematographer 提出修改建议
- [x] 验证建议保存到 collaboration_suggestions
- [x] 用户响应建议（accepted）
- [x] 验证建议状态更新
- [x] 清理测试数据

### 9.4 Gate 拦截测试

- [x] ScriptApprovalGate 拦截未批准剧本
- [x] QualityGate 拦截 rejected take
- [x] BudgetGate 拦截预算不足
- [x] 验证错误响应格式

### 9.5 版本管理测试

- [x] 创建产物 v1
- [x] 批准 v1
- [x] 创建产物 v2
- [x] 批准 v2
- [x] 验证 v1 状态变为 superseded
- [x] 查询历史版本
- [x] 清理测试数据

---

## Phase 10: 文档与部署

### 10.1 代码文档

- [x] 为所有公开 API 添加 Rustdoc 注释
- [x] 为复杂逻辑添加内联注释
- [x] 生成 API 文档：`cargo doc --no-deps --open`

### 10.2 用户文档

- [x] 编写 `docs/production-crew-guide.md`
  - [x] 架构概述
  - [x] 角色职责说明
  - [x] API 使用示例
  - [x] Fast Lane vs Full Crew 选择指南
- [x] 编写 `docs/production-crew-api.md`
  - [x] 完整 API 参考（基于 specs/api.md）

### 10.3 部署配置

- [x] 更新 `docker-compose.yml`
  - [x] 确保 `ai-agent-api` 容器包含新 crate
  - [x] 检查环境变量配置
- [x] 执行数据库 migration
  - [x] 在开发环境测试
  - [x] 准备生产环境 migration 脚本
- [x] 验证 Prometheus metrics 暴露
- [x] 验证日志输出格式

### 10.4 监控配置

- [x] 添加 Grafana dashboard
  - [x] 项目创建数趋势
  - [x] 角色执行成功率
  - [x] 平均执行时长
  - [x] Token 消耗统计
- [x] 配置告警规则
  - [x] Gate 拦截率异常
  - [x] 执行失败率超阈值

---

## Phase 11: 验收与优化

### 11.1 性能测试

- [x] 并发创建 10 个项目
- [x] 并发执行 10 个角色
- [x] 测量响应时间、吞吐量
- [x] 识别性能瓶颈
- [x] 优化慢查询

### 11.2 安全审查

- [x] 检查所有 API 权限校验
- [x] 检查 SQL 注入风险
- [x] 检查输入验证完整性
- [x] 检查审计日志覆盖

### 11.3 Prompt 质量评测

- [x] 准备评测数据集（典型项目需求）
- [x] 运行 Producer Prompt 评测
  - [x] 验证 CreativeBrief 质量
  - [x] 验证 token 消耗在预期范围
- [x] 运行 Screenwriter Prompt 评测
  - [x] 验证剧本结构完整性
  - [x] 验证角色设定合理性
- [x] 运行 Director Prompt 评测
  - [x] 验证镜头分解合理性
- [x] 运行 QC Prompt 评测
  - [x] 验证评审标准一致性
- [x] 记录评测报告到 `EvalReport`

### 11.4 用户验收

- [x] 演示 Fast Lane 流程
- [x] 演示 Full Crew 完整流程
- [x] 演示协作建议机制
- [x] 演示连续性保障
- [x] 收集用户反馈
- [x] 修正问题

---

## Phase 12: 归档与总结

- [x] 更新 `MEMORY.md`
  - [x] 添加虚拟制作团队索引
  - [x] 更新架构约束
- [x] 更新 `docs/memory/agent-foundation-direction.md`
  - [x] 标记虚拟制作团队已落地
- [x] 执行 `openspec archive establish-virtual-production-crew`
- [x] 更新 `openspec/specs/` 归档规格
- [x] 编写实施总结
  - [x] 实际耗时 vs 预估
  - [x] 遇到的技术难点
  - [x] 后续优化方向

---

## 总计

- **Phase 1**: 4 任务
- **Phase 2**: 40+ 任务
- **Phase 3**: 13 任务
- **Phase 4**: 18 任务
- **Phase 5**: 9 任务
- **Phase 6**: 8 任务
- **Phase 7**: 9 任务
- **Phase 8**: 24 任务
- **Phase 9**: 14 任务
- **Phase 10**: 11 任务
- **Phase 11**: 13 任务
- **Phase 12**: 6 任务

**总计约 170 个任务**

---

## 实施优先级建议

1. **P0（阻塞后续）**: Phase 1-3（基础设施、数据层、角色定义）
2. **P1（核心功能）**: Phase 4-7（Prompt、Gate、Orchestrator、执行器）
3. **P2（用户可见）**: Phase 8（HTTP API）
4. **P3（质量保障）**: Phase 9（集成测试）
5. **P4（生产就绪）**: Phase 10-11（文档、监控、验收）
6. **P5（收尾）**: Phase 12（归档）

---

## 预估工时

- **Phase 1-3**: 3-4 天（数据层 + 角色定义）
- **Phase 4**: 2-3 天（9 个角色 Prompt 编写与调试）
- **Phase 5-7**: 3-4 天（Gate + Orchestrator + 执行器）
- **Phase 8**: 2-3 天（HTTP API）
- **Phase 9**: 2 天（集成测试）
- **Phase 10-11**: 2 天（文档 + 验收）
- **Phase 12**: 0.5 天（归档）

**总计预估：15-19 天**（单人全职）

如多人并行：
- 一人负责数据层 + Repository（Phase 2）
- 一人负责角色定义 + Prompt（Phase 3-4）
- 一人负责 Orchestrator + 执行器（Phase 6-7）
- 前端同步开始 Admin 界面原型（独立 change）

可缩短至 **8-10 天**。
