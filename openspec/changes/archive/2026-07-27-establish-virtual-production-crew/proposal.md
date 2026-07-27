# Proposal: 建立虚拟制作团队（Virtual Production Crew）

## 概述

为 Video Agent 建立完整的虚拟制作团队框架，实现从创意到成片的专业化、结构化视频生产流程。

## 背景

当前视频生成流程相对简化，缺乏专业视频制作的角色分工、质量把控和连续性保障。根据 `agent-foundation-direction.md`，Video Agent 的长期目标是实现受控的 Virtual Production Crew，参考专业制作团队的协作模式，通过多个专业角色、共享制作状态、结构化产物和质量闸门，提升视频内容的专业度和一致性。

## 目标

1. 建立 ProductionOrchestrator，支持 Fast Lane（快速通道）和 Full Crew（完整团队）两种执行模式
2. 实现 8 个核心专业角色：制片人、编剧、导演、摄影指导、表演指导、剪辑师、声音指导、QC
3. 建立 ProductionState 共享状态系统，支持 10 种结构化产物的版本管理和协作
4. 实现角色间通过版本或修改建议协作，不直接覆盖其他角色产物
5. 建立连续性保障机制，确保多镜头视频的视觉一致性
6. 实现质量闸门，包括剧本审核、技术可行性、QC 评审、预算控制
7. 复用现有 AgentDefinition、PromptCompiler、ModelCall 审计体系

## 非目标

- 多个角色实时群聊（文档明确排除）
- 角色直接调用付费生成 API
- 角色自主修改其他角色产物
- 外部真人协作者加入虚拟团队
- 移动端适配

## 成功标准

1. 能创建 Fast Lane 项目并在 2 分钟内完成简单视频生成
2. 能创建 Full Crew 项目并完整走完所有角色流程
3. 每个角色产出符合 schema 定义的结构化产物
4. 角色协作产生的修改建议可被追溯和应用
5. 连续性约束在后续镜头生成中生效
6. QC 不通过的镜头能重新生成
7. 所有角色执行过程可审计、可回放

## 风险与依赖

**风险**：
- 完整流程涉及多次模型调用，token 成本较高
- 角色产物 schema 设计不合理可能导致后续难以扩展
- 连续性约束的表达和执行可能复杂

**依赖**：
- 现有 AgentDefinition、PromptDefinition、PromptCompiler 体系
- 现有视频生成 Worker 和平台发布能力
- PostgreSQL、Redis、Milvus 基础设施

## 实施范围

本 change 覆盖：
1. ProductionOrchestrator 核心逻辑
2. 8 个角色的 RoleDefinition 和 Prompt 模板
3. ProductionState 数据库 schema 和 CRUD API
4. 10 种结构化产物的 JSONB schema 定义
5. 角色协作的修改建议机制
6. 连续性保障的 ContinuityLedger 实现
7. 质量闸门的 Gate 实现
8. HTTP API 接口
9. 集成测试覆盖 Fast Lane 和 Full Crew 完整流程

## 后续工作

- Admin 前端的制作项目管理界面（需独立 OpenSpec + Pencil 原型）
- 正式 Memory 系统集成角色记忆
- 局部 Planner 用于失败诊断和重试策略
- 角色 Prompt 的持续优化和评测
