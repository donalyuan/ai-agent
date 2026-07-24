---
name: agent-foundation-direction
description: Novex Agent 基座、角色分化和虚拟制作团队的长期方向
metadata:
  type: project
---

# Agent 基座与虚拟制作团队方向

## 基座执行模式

- Novex Agent 基座默认采用“受控工作流型 Agent”：业务状态机、固定检查点、成本限制、权限和最终写入权由代码控制。
- 自主 `Planner` 作为可选的局部能力，只在调研、开放式分析、失败诊断等授权节点中提出或执行有限子计划；不得绕过预算、权限、质量闸门、人工确认或业务状态规则。
- 最终形态采用“受控工作流外壳 + 局部 Planner”，不以多个 Agent 自由群聊作为默认架构。

## Agent 角色与 Prompt

- 每类 Agent 使用版本化 `AgentDefinition`，至少描述角色、目标、约束、上下文策略、记忆策略和允许能力。
- 每个 LLM 节点使用独立、版本化的 `PromptDefinition`。实际模型输入由平台规则、Agent 角色、节点职责、动态上下文、用户指令和输出契约共同编译，形成不可变 `PromptSnapshot`。
- 同一 Agent 的生成、评审和重写节点可以使用不同专业角色。例如选题生成使用策划角色，质量闸门使用独立评审角色，避免一个万能 Prompt 同时承担冲突职责。
- 读取资源、状态流转、持久化等确定性步骤不需要角色 Prompt，也不得伪装成模型推理。
- 第一版角色和 Prompt 随代码版本治理，不先提供后台任意在线编辑；Prompt 行为变化必须经过版本、回放和评测。

## Context 与 Memory

- `Context` 是本轮动态装配的数据，应包含来源、优先级、有效期和 token 预算，并通过统一 Context Compiler 选择和裁剪。
- `Memory` 只保存已确认、稳定、可复用的信息，例如账号策略、用户明确偏好、已确认反馈和长期约束；瞬时数据、未确认推断和临时对话摘要不得自动升级为长期记忆。
- 记忆必须具备作用域、来源、置信度、状态、更新时间、失效和删除语义。模型提出的新记忆先作为候选，经过人工确认或确定性规则后才能生效。
- 每次运行必须保存模型、Prompt、Context、Memory 和后续 Tool 调用的必要快照，以支持审计、回放和评测。

## 虚拟制作团队

- Video Agent 的长期目标是受控的 `Virtual Production Crew Agent`，参考 `Emily2040/seedance-2.0` 的“主路由器 + 按需专业 Skill + 共享项目状态 + Prompt Compiler + Gate”思想，但不把其文档式 Skill 包直接当作后端多 Agent Runtime。
- 第一版采用一个 `ProductionOrchestrator`、多个版本化 `RoleDefinition`、共享 `ProductionState`、结构化阶段产物、固定 Gate 和 `PromptCompiler`，而不是多个独立 Agent 自由讨论。
- 专业角色方向包括制片人、编剧、导演、摄影指导、表演指导、剪辑师、声音指导和 QC。简单短视频应走 Fast Lane，不强制启动完整制作团队。
- “演员”优先建模为持久化 `CharacterBible` 与受约束的 `PerformanceDirector`；需要角色视角校验时可以增加 `CharacterCritic`，但角色不得拥有发布、付费生成、删除或正式记忆写入权。
- 共享制作状态至少逐步覆盖 `CreativeBrief`、`StoryBible`、`CharacterBible`、`ScriptDraft`、`DirectorialTreatment`、`ShotContract`、`PerformanceBrief`、`SoundPlan`、`ContinuityLedger` 和 `TakeReview`。
- 各角色只拥有自己的结构化产物，通过新版本或修改建议协作，不直接覆盖其他角色的已确认产物。实际生成结果经接受后覆盖计划状态，成为后续镜头的连续性事实来源。

## 实施顺序与边界

1. 先提取执行内核：`AgentAdapter`、Registry、ExecutionContext、Run Recorder 和统一失败收尾。
2. 再建设通用模型请求响应、Prompt 版本、Context Compiler 和运行快照。
3. 在回放和评测基础上建设 Memory，然后扩展受权限控制的 Tool 与局部 Planner。
4. 可复用基座分别归入 `novex-ai-core`、`novex-agent`、`novex-model`、`novex-memory`、`novex-tools` 和 `novex-eval`；`backend` 保留 HTTP、PostgreSQL Repository、业务 Adapter 和依赖组装。
5. 任何落地实现必须先建立对应 OpenSpec change，并保持现有选题、脚本、声音和作品 Agent 的外部行为可回归验证。

### 已确认第一步

- 第一项正式变更命名为 `stabilize-agent-runtime-kernel`，目标是完成外部行为不变的 `Agent Runtime Kernel v1` 重构。
- 本步建立 `AgentKey`、`AgentAdapter`、`AgentRegistry`、`AgentExecutionContext`、`AgentRunCoordinator` 和 Run/Step 记录契约，以注册机制替代 Runtime 核心中的业务类型分派。
- 业务 Repository 归对应 Adapter 持有；Kernel 不认识选题、脚本、声音或作品等业务概念，缺失依赖在 Bootstrap 注册阶段失败，不延迟到运行时。
- 本步不引入 Memory、Tool Loop、自主 Planner、虚拟制作团队业务流或新的角色 Prompt；只为后续 `AgentDefinition` 的版本和能力声明保留稳定边界。
- 本步不得改变现有 HTTP API、数据库结构、模型选择、Prompt、消息 metadata、Run/Step 审计语义及选题、脚本、声音、作品 Agent 的业务结果。

## 暂缓范围

- 网络热门视频采集与总结暂缓，不纳入本轮 Agent 基座方向；后续如恢复，必须另行确认数据来源、合规边界和证据时效规则。
