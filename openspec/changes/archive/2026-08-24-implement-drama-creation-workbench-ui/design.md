## Context

当前 `apps/web` 是 React 19/Vite/React Router 的阶段 0 壳层，只探测 health；Project creative facts、AssetBible、Scene/Shot、Run 与文本候选仅在后续后端 change 中定义，尚未实现。本 change 只定义其桌面 UI 消费者，依赖 `extend-projects-episodes-creative-slice`、`implement-asset-bible-continuity-slice`、`implement-scenes-shots-storyboard-slice`、`implement-workflows-runs-slice`、`implement-provider-model-skill-catalog` 与 `integrate-agentscope-text-skills` 的 owner contract，不把这些依赖表述为当前可用 API。

## Goals / Non-Goals

**Goals:**

- 以 CreativeBrief、同事实源的工作流/故事板投影、文本候选审核和 Run 恢复组成可恢复创作闭环。
- 将领域 ID、revision、schemaVersion 和付费媒体审核门保留在命令/查询边界，避免 UI 自行推断领域状态。
- 让用户可查看 Run 节点详情并显式取消运行，同时以 owner 状态处理取消竞态和晚到结果。
- 以完整镜头卡片和共享项目壳层连接 Workbench、Review、Assets、Episode Timeline、Project Exports 与项目设置，而不复制领域事实。
- 由共享前端 harness 统一验证 300 个投影项的可操作性、普通本地 API P95 `<500ms` 以及桌面 Chrome/Edge 兼容性，避免各页面以不一致口径重复声明。
- 为 React 19、Vite、React Router、TanStack Query、Zustand、shadcn/Radix、Tailwind、Lucide 与 Zod 指定最小职责和测试入口。
- 支持先选 `creationMode=original|adaptation`：original 的 CreativeBrief 直接继续；adaptation 的 `materialType=novel|synopsis|existing_script` 与 `inputMode=inline_text|uploaded_file` SourceMaterial 显示解析/校验、失败恢复和 exact source binding snapshot。
- 支持 TextReview 中的初始 AssetBible typed specs、AssetBible entry/version/override/resolved snapshot、影响预览、显式接受和 continuity task 跟踪闭环。

**Non-Goals:**

- 不实现或聚合 CreativeBrief、AssetBible、StorySpec、ScriptSpec、Scene、Shot、WorkflowRun、Provider、Agent 或付费调用后端；UI 只发送用户明确触发的 owner commands。
- 不直接调用 Provider、保存媒体字节、替代 owner 的 Schema/版本映射，或自动接受候选/启动媒体生成。
- 不实现移动端、营销页、协作、画布执行器或后端聚合。
- 不实现 Workflow graph editor、节点/边编辑、连线校验、草稿保存或发布 UI；MVP-A 只读显示固定已发布默认 WorkflowVersion 的 source/run 投影。
- 不实现 storyboard insert/copy、Scene split/merge、Shot 跨场 move 或批量编辑；MVP-A 只提交同一明确父 scope 内排序。

## Decisions

### 1. 路由、数据边界与 DDD

前端路由为 `/projects/:projectId/workbench`，使用 `episodeId`、`workflowVersionId` 和 `view=storyboard|workflow|asset-bible` 查询参数；有效 project 即使尚无 Episode 也进入明确的 zero-episode 空态，只有 foreign/missing project 或 episode 深链才是错误。领域事实的唯一来源是 owner API：projects owner 返回 Project/creationMode/CreativeBrief/项目设置/预算，text owner 返回 adaptation SourceMaterial/candidate/TextReview，AssetBible owner 返回 entry/version/assignment/resolved snapshot/impact/task，episodes/scenes owner 返回 StorySpec/ScriptSpec/Scene/Shot 投影，workflows/runs owner 返回已发布 WorkflowVersion/Run；这些事实只以稳定 ID、revision、canonical schemaVersion 和不可变引用进入 Query cache。Zustand 仅保存当前视图、局部展开状态、未提交表单草稿和正在提交的 command correlation；不得复制 owner state 成为第二事实源。

本 change 同时拥有阶段一共享项目壳层的 route composition，而不拥有子页面事实。壳层以 `/projects/:projectId/workbench`、`/projects/:projectId/review`、`/projects/:projectId/assets`、`/projects/:projectId/episodes/:episodeId/timeline`、`/projects/:projectId/exports` 和项目模型设置深链为规范入口；导航必须携带 owner-validated `projectId`，Timeline 还必须携带用户显式选择的 `episodeId`。镜头/候选/usage handoff 只传 stable owner IDs/revisions/hashes 和允许的 selection 参数，切换 project 或 Episode 时清除不兼容 selection；route load、back navigation、tab switch 和 breadcrumb 不触发 Run、Provider、上传、审核、Timeline、ExportBatch 或配置 mutation。zero-episode 只提供当前 original/adaptation 入口，不展示 MVP-B template action。

CreativeBrief 的 UI DTO 必须原样消费 projects owner 冻结的 canonical payload：`{subject, genre, audience, characterPremise, style, episodeCount, scenesPerEpisode, shotsPerScene, episodeDurationSeconds, schemaVersion, revision}`；其中六项创作语义、三个精确计数及 schema/revision 均为必填。旧版或产品展示标签若需显示，必须显式标记为 presentation-only，不能进入 command 或 Zod/schema。adapter 在 projects owner command 前逐字段拒绝缺失、未知、冲突或无法等值映射的 payload，并以零 mutation 阻塞；text owner 只消费成功保存的 snapshot，不得猜测、补齐、生成并行字段或改写 brief。

### 2. 前端 SDD：API adapter、Zod、缓存与 CAS

`workbenchApi` 只暴露 owner-resource 操作：通过 projects owner 选择/读取 creation mode、读取/保存 brief；通过 text owner 创建/读取/解析/校验/恢复 adaptation SourceMaterial；通过 AssetBible owner 读取/创建 entry successor、提交 assignment、resolve snapshot、preview impact、accept revision 和读取/ack task；通过 workflows/runs owner 显式创建并启动文本生成 Run、按同一 brief/run scope 重新生成、刷新/恢复/重试和 `submission_unknown` reconciliation；读取 storyboard 与固定 published WorkflowVersion projection；请求/读取/confirm/reject 文本候选；以 `expectedRevision` 提交同父 scope scene/shot reorder；读取 Run snapshot 与以 `Last-Event-ID` 恢复 SSE。最终 HTTP path 与 error envelope 由各 owner change 冻结。adaptation Zod schema 必须验证 `CreativeBriefSourceBindingSnapshot={projectId, sourceMaterialId, sourceMaterialRevision, sourceContentHash, creativeBriefId, creativeBriefRevision, creativeBriefPayloadHash, parseStatus, validationStatus, bindingStatus, bindingVersion}`；Run 创建后验证增加 `runId`、`runRevision` 的 `TextRunSourceBindingSnapshot`。AssetBible DTO 必须验证 typed entry、version/current map、scope assignment、resolved chain/snapshot ID/revision/hash、impact actual target set/hash、AcceptDecision 和 task status。uploaded_file 才验证 AssetVersion ref；所有 owner DTO 都拒绝 unknown/missing/冲突 ID、revision、hash、status/version 或 schemaVersion。

TanStack Query key 按 owner resource 构造，包含 `projectId`、可选 `episodeId`、published `workflowVersionId` 或对应 owner resource ID/revision；MVP-A key MUST NOT 依赖可编辑 workflow draft identity。成功 command 仅失效其 owner-resource key。reorder 乐观 UI 必须携带 previous projection，409/revision conflict 时回滚并 refetch，其他错误保留原始 code/message 与可重试动作。SSE 按 sequence 合并同一 run，断线后从最后持久化 event ID 重连；事件间隙或 foreign run 必须显示可诊断状态并触发 snapshot refetch。BudgetGate DTO 必须显示 estimate/actual、currency、source、`cost_status`、项目文本阈值、confirmationId 和 `runId + logicalOperation`；批量图片/视频、超阈值文本或 `cost=unknown` 只能由明确确认 command 解除，确认失配/过期时刷新 owner state，绝不在客户端推算为已确认。

Run detail DTO 必须包含 Run/NodeRun stable identity、状态、开始/结束时间或 elapsed source、输入/输出的脱敏 owner summary、最近 RunEvent sequence、失败 code/message/retryability 和可用 command。只有 `queued|running|waiting_review` 可显示 cancel；cancel mutation 复用 workflows/runs owner 的 expected revision/correlation，先显示 `cancel_requested`，再通过 SSE/snapshot 收敛为 `cancelled`。HTTP/Temporal/Provider 晚到成功不得由 UI 覆盖 owner 的取消状态。完整原始 payload、secret、媒体 bytes 和 Provider response 不进入浏览器缓存或日志。

Storyboard `ShotCardView` 是不持久化、不可作为 command 前置的 presentation projection：Scene/角色/场景/duration/prompt 来自 scenes/ShotSpec owner，AssetBible owner 提供 resolved snapshot/chain/task，current eligibility 来自 scenes projection，candidate/review 来自对应 review owner，model/cost/generation 来自 Run/ProviderCall 的脱敏 owner query，derivative readiness 来自 Media projection。每个字段组保留 owner ID/revision/hash/status；UI 不跨 owner 合并出新的 accepted/current/ready 事实。任一 owner 不可用或 revision 对不上时该字段组显示 `partial|unavailable|stale`，相关动作禁用并由目标 owner 在 command 时重新校验。完整卡片必须显示 AssetBible snapshot/task、current/candidate image/video、duration、prompt summary、selected model identity/revision、cost value/status/source、generation/review/derivative readiness；按钮只生成 owner-validated review/Agent/assets/timeline deep link，不得从缩略图或 AssetVersion status 推断 current/eligibility。

### 3. BDD：双投影、审核与显式副作用

故事板和工作流分段控件读取同一 Scene/Shot stable ID/revision/reference；工作流仅只读显示 fixed published WorkflowVersion 的 node dependency/configuration/run command，故事板仅提交同一 Episode 内 Scene 排序或同一 Scene 内 Shot 排序，UI 不发送 storyboard insert/copy、Scene split/merge、Shot 跨场 move、批量编辑或 graph edit/connect/save/publish。工作台先渲染 projects owner creation mode：original 渲染完整 CreativeBrief；adaptation 只在选定 `novel|synopsis|existing_script` 与 `inline_text|uploaded_file` 后渲染 text owner SourceMaterial panel。该 panel 显示 input/type、immutable revision/contentHash、parse/validation、精确 brief/run binding snapshots、recover action 与仅 uploaded_file 的 AssetVersion ref；inline_text 明确无 storage session/StoredObject/AssetVersion。invalid enum/scope/revision/hash/status、adaptation invalid source 与 original/inline upload intent 显示 owner diagnostic，且不显示上传成功或发起 storage mutation。StorySpec、各集 ScriptSpec、Episode、Scene、Shot 与 ShotSpec 候选在同一个 `TextReviewBatch` 界面按依赖层级呈现 diff、版本、来源、Schema 状态与 stale 状态；允许编辑单个候选，但编辑后必须显示依赖候选待重新生成/校验。界面只提供 batch 级 `accept|reject`，不提供逐层必需审批；legacy/unknown `approve` 只显示 validation 且零 accepted handoff/媒体副作用，owner 未接受完整 batch 前不能解锁付费图片、视频、音频或后续媒体 command。Run 视图显示 `queued`、`running`、`waiting_review`、`succeeded`、`failed`、`cancel_requested`、`cancelled` 及恢复入口；有效 original 不得因无 source 阻断，adaptation 仅在 valid source 和完整 binding snapshot 后允许创建/启动文本 Run；重新生成和失败恢复必须绑定 owner 返回的 `runId + logicalOperation` 并显示原始诊断，且只发送 owner 已允许的 command。

TextReviewBatch 同时展示叙事候选实际引用的初始 AssetBible typed entry specs；不被引用的条目不得由 UI 添加到 handoff。batch accept 后，UI 分别显示 Project/Episode、Scene/Shot 和 AssetBible owner 的幂等 ack，全部 ack 前媒体门继续关闭。独立 AssetBible 视图只读取 owner 的 entry/version/current、assignment 与 resolved snapshot；创建 successor 前先提交 impact preview，展示完整实际 Episode/Scene/Shot target set、reason、revision 与 set hash。只有用户明确确认完整集合后才提交一次 accept；409/incomplete/foreign/hash mismatch 不拆分重试。成功只刷新 owner facts和 pending tasks，旧 ShotSpec/current media/Timeline 仍显示原 snapshot，绝不自动重生成或替换。

Run detail 的查看是只读操作；取消必须由明确 command 触发，并在确认后提交一次。重复点击、stale revision、终态 Run、foreign Run、SSE 断线或取消与 Activity 完成竞态都显示 owner 原始状态，不进行客户端补偿或重发。ShotCard 的 review/Agent/assets/timeline 行为只是带精确 scope 的导航；缺少显式 Episode 时不得跳到 Timeline，也不得静默选择第一集。

### 4. TDD、可访问性与视觉约束

先写组件、store、Zod contract、Query adapter 和路由测试，再实现最小界面；Playwright E2E 入口覆盖 brief 到候选审核、CAS 冲突和 SSE 断线恢复。使用 shadcn/Radix 原语、Tailwind token 与 Lucide 图标，工作台采用非嵌套卡片、分段控件和 tooltip；不增加营销文案。当前依赖尚未安装时，依赖变更属于后续实现并由 lockfile 审查。

非功能 harness 使用确定性 Compose/PostgreSQL、Mock Provider 与显式 Local test/offline profile，准备包含 300 个 fixed published Workflow node 的只读投影 fixture（每个 node 保留冻结 scope，按需关联 Scene/Shot），并执行加载、滚动或分页、筛选、选择、详情和跨 Workbench/Review/Assets/Timeline/Settings 导航；它只验证已有只读投影与业务入口，不恢复 MVP-B graph authoring。普通 API P95 以 localhost HTTP 请求端到端耗时计算，报告必须记录 route 集合、样本量、warm-up、环境、数据库数据量和 percentile；Provider/Agent/Temporal 等待、SSE 长连接、上传下载、媒体 probe/preview/render/export 不计入 `<500ms` 门。浏览器证据分别运行桌面 Chrome 与 Edge 的同一关键流程并记录实际版本；缺少任一浏览器证据不得通过该项验收。

### Run continuation 与 Skill route 裁决

刷新/API/Worker 重启和 `submission_unknown` 使用同一 Run、同一 operation 的只读恢复或 reconciliation。failed Run 的“从失败节点继续”按钮只在 owner `allowedActions.createSuccessor=true` 时出现，确认页显示 predecessor、失败节点、将复用的成功 evidence、新执行节点和费用影响；提交 `CreateSuccessorRunFromFailure` 后导航到新 runId，旧 Run 保持只读终态。UI 不构造 reuse set、不复用旧确认，也不把 successor 显示为原 Run 恢复。

Skill 路由由 AgentScope/text owner 返回 `SkillRouteDecision`。当状态为 `needs_human_selection` 时，UI 只按 owner 顺序显示候选 SkillRevision ID/digest、满足/淘汰的 capability/policy、lexical/semantic score 和歧义原因，并提交当前 decision revision、所选 candidate 与 expected launch revision。关闭/刷新不自动选择；候选过期时 refetch/reroute。选择成功后 UI 只读取 workflows/runs 冻结的最终 SkillRevision snapshot，不能更改 Registry、绕过 allowedSkills/requiredCapabilities 或在选择前启动 TextModel/Provider。

## Dependency DAG

```text
projects creative + AssetBible + scenes/shots
           \             |            /
        workflows/runs + provider catalog + AgentScope text
                            |
             drama creation workbench UI
```

## Current / Defined / Todo

- **Current**：只有 health 壳层；没有 Query/Zustand/Zod、业务路由、owner API adapter 或审核 UI。
- **Defined**：本设计的 brief 字段、AssetBible 管理/影响/任务、文本 Run create/start/regenerate/recover/reconcile、BudgetGate、双投影、审核门、CAS、Run/SSE 恢复与测试契约。
- **Todo**：在 owner contract 已实现后安装/配置计划依赖，完成 adapter、组件、`Mock Provider +` 显式 Local test/offline profile fixture 与端到端测试。

## Risks / Trade-offs

- [owner DTO 未实现或字段名不等值] -> contract test 阻塞 adapter，不在 UI 兼容猜测。
- [SSE 重放不完整] -> 保留最后 sequence、显示原始错误并 refetch snapshot；不得报告已恢复。
- [乐观排序覆盖他人编辑] -> expectedRevision、回滚和 authoritative refetch。
- [文本审核被绕过] -> 媒体按钮由已批准 TextReviewBatch 与 owner precondition 共同门控，stale/partial batch 不可接受，后端仍为权威。
- [AssetBible 修改静默改写下游] -> UI 强制 impact preview 与完整集合确认，只显示 ContinuityRevisionTask，不自动替换 ShotSpec/current media/Timeline。
- [费用确认被重试/恢复复用] -> UI 对 confirmation 的 run/logical operation/fingerprint/revision 做 Zod 校验，失配即刷新并保持 waiting_review。
- [后端支持 cancel 但 UI 只展示状态] -> 将 cancel DTO、按钮、竞态和 E2E 固定为本 change 的显式任务。
- [五个页面各自可打开但用户无法连续操作] -> 共享项目壳层统一 route composition 和 selection handoff，页面 owner 仍保持独立。
- [ShotCard 只显示缩略图导致生成/审核入口断裂] -> 冻结完整卡片 DTO 和 owner-validated actions，不从前端推断 eligibility。
- [性能数据被外部调用或缓存偶然性污染] -> 使用冻结 fixture、明确 warm-up/样本/route/环境并排除外部生成、媒体和长连接；原始报告保留 P95 与失败请求。

## Migration Plan

1. 先接入 `Mock Provider +` 显式 Local test/offline profile fixture 与显式 `workbenchApi` interface，不改变阶段 0 shell 行为；profile 由 harness 明确选择，不能因 TOS 失败切换。
2. owner endpoints 可用后以 additive routes 接入，逐项验证 schema/409/SSE；关闭 feature 不删除现有数据。
3. 回滚仅移除新路由和 UI state，server version、候选和 Run 事实保持 owner 管理。

## Open Questions

- owner 冻结的 Run/SourceMaterial command、SSE path 与稳定错误 envelope 的精确名称；UI 只依赖其语义，不自行发明第二套状态机。
- 工作流只读投影的最小节点 DTO 与大图虚拟化阈值；graph editor UI 明确不在 MVP-A。

## Acceptance Commands

`openspec validate implement-drama-creation-workbench-ui --strict --json`、`pnpm --filter @video-agent/web test`、`pnpm --filter @video-agent/web typecheck`、`pnpm --filter @video-agent/web lint`、`pnpm --filter @video-agent/web format:check`、`git diff --check -- openspec/changes/implement-drama-creation-workbench-ui`。

## 阶段一入口与 E2E 闭合

**DDD**：projects owner 管理 Project/creationMode/CreativeBrief/项目设置/预算，AssetBible owner 管理 entry/version/override/resolved snapshot/impact/task，episodes/scenes owner 管理 Episode/Scene/Shot，text owner 管理 adaptation SourceMaterial/candidate/TextReview，workflows/runs owner 管理默认 Workflow binding 和 Run；UI 只保存显式 selection 和未提交表单。**BDD**：空项目进入工作台、original CreativeBrief 继续、adaptation source import/parse/validate/recover、初始 AssetBible handoff、影响预览/显式接受/task、两种 input 分支、invalid enum/scope/revision/hash/status、foreign/missing episode 深链、zero-episode、明确选择 Episode 和“全部集”边界均有可观察状态。**SDD**：`/projects` 使用 owner 的 create/If-Match update/list/select/brief contract；workbench 读取 AssetBible 和 immutable WorkflowVersion/Run/审核/费用/恢复 owner facts，而不推断节点/端口或维护第二事实源。**TDD**：先固定各 owner 的 project/episode/creation/brief/source/AssetBible route、selection、error DTO 与 Mock fixtures，再接入 owner command。

本 change 后续安装 direct `@playwright/test`、config、dedicated E2E environment/reset、Web/API/worker lifecycle、根 `pnpm run test:e2e` 和 CI/phase-one acceptance entry。`E2E-MVPA-001` 必须引用总体 `design.md` 的 canonical `S01`-`S11` 加 `S04a asset bible continuity`、`S08a asset center` matrix，逐阶段展示 owner response、prerequisite snapshot、success evidence、对应 `F01`-`F11`/`F04a`/`F08a` diagnostic 和 no-side-effect assertion；本 UI 只提供浏览器观察与 command 入口，不复制或重定义 owner 事实。浏览器 oracle 使用 Mock preview；真实 FFmpeg 仅 media adapter probe。

workbench 必须渲染 Text successor/stale closure、实际引用的初始 AssetBible specs、全部 owner ack 和 batch CAS 状态，以及 AssetBible resolved snapshot/task、image candidate accepted provenance/id/revision/hash/target eligibility；Agnes 操作只在 owner projection exact current 且 continuity task 不阻断时可用。partial/stale/foreign/hash/revision mismatch 展示 diagnostic，客户端不发 ProviderCall/submit。

storyboard projection 以 Episode 为交互隔离键，场次可折叠，Shot 行显示 owner 提供的安全缩略图 readiness/reference，并允许组合筛选 `status`、实际 `modelId/modelRevision` 与 `reviewResult`。折叠、筛选和展开仅存在 Zustand 的当前 Episode UI slice；切换 Episode 清空或恢复对应 slice，不修改 owner 排序、审核、Run 或媒体事实。

Episode UI slice 的 key 固定为 `projectId + episodeId`，至少保存 storyboard/workflow 视口、场次折叠集合、组合筛选、当前 Shot/Asset selection 和 active Agent session ID；不得保存 message/turn、Run、candidate、AssetVersion 或任何 owner revision 正文。切换 Episode 时先保存离开集的 display slice，再读取目标集 slice，并用目标 owner scope/revision 验证 selection/session；合法值恢复，missing/foreign/stale 值清除并显示诊断。返回原 Episode 时可恢复原展示状态，但不得重发 message、重建 Plan、触发 Run/Provider 或把会话上下文复制到另一集。

历史重新运行与 failed continuation 是两个独立命令。前者展示用户选中的 immutable `RunInputSnapshot`、历史输入/selection、当前 runnable 检查和新 BudgetGate，提交 `CreateRunFromHistoricalSnapshot` 后进入带 `rerunOfRunId` 的新 Run，所有节点使用新 logical operations；后者只用于 failed predecessor 并允许 owner 验证过的 success evidence reuse。UI 不从“上一版/current”猜测 snapshot，也不自动升级输入、重基引用、复用费用确认或 fallback。
