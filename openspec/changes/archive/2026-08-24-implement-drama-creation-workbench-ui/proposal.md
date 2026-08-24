## Why

阶段 0 的 React 工作台只有工程壳层，尚不能让创作者从 CreativeBrief 建立、审核并恢复一条文本优先的短剧生产链路。需要把已定义的 Scene/Shot、WorkflowRun 与文本生成契约投影为可操作的桌面工作台，同时保留后端聚合与 Provider 调用的所有权。

## What Changes

- 新增 CreativeBrief 编辑、显式创建/启动文本生成 Run、重新生成、原 Run reconciliation、从失败节点创建 successor Run、故事板/工作流双视图和一次完整文本候选批量审核的前端闭环；UI 不得把 failed Run 原地恢复为 running。
- 新增历史 Run/input snapshot 选择与重新运行确认：用户先看到精确 Brief/SourceMaterial/WorkflowVersion/scope/selection 和新费用影响，再创建带 lineage 的新 rerun Run；不默认使用 current、不重启历史 Run 或隐式复用旧费用确认。
- 工作台先通过 projects owner 选择并保存 `creationMode=original|adaptation`：original 展示 CreativeBrief 的 `subject`、`genre`、`audience`、`characterPremise`、`style`、每集 `episodeDurationSeconds`、精确 `episodeCount`/`scenesPerEpisode`/`shotsPerScene` 及 schema/revision，且无需 SourceMaterial；adaptation 再展示 text owner 的 `materialType=novel|synopsis|existing_script`、`inputMode=inline_text|uploaded_file` SourceMaterial import/parse/validation/binding/recovery。UI 展示 immutable source revision/contentHash、精确 brief/run binding snapshots 与仅 uploaded_file 的 AssetVersion，恢复复用同一 source/brief revision。
- CreativeBrief 仅消费 projects owner 冻结的 canonical payload：`subject`、`genre`、`audience`、`characterPremise`、`style`、`episodeCount`、`scenesPerEpisode`、`shotsPerScene`、`episodeDurationSeconds`、`schemaVersion`、`revision`；展示标签如有需要仅为 presentation-only，不进入 command/schema。text owner 只消费该 validated snapshot，不得保存或改写 Project、creationMode、CreativeBrief、项目设置或预算。
- 让 StorySpec、ScriptSpec、Scene 与 Shot 使用同一稳定 ID、版本和排序事实；故事板排序以 expected revision 的 Compare-And-Swap 命令提交。
- 在任何付费媒体生成入口前强制展示完整 `TextReviewBatch`，只提供一次批量接受/拒绝；UI 不直接调用 Provider，也不聚合后端领域事实。
- `TextReviewBatch` 必须同时展示叙事候选实际引用的初始 AssetBible typed entry specs；接受后分别显示 Project/Episode/Scene/Shot 与 AssetBible owner ack，任何一个缺失都继续关闭媒体门。
- 在工作台提供 AssetBible 用户闭环：查看 Character/Look/Location/SceneVisual/Prop/VisualStyle 条目及不可变版本，管理 project/episode/scene/shot override，读取 resolved snapshot，预览修改的精确 Episode/Scene/Shot 影响集合，显式接受 successor，并跟踪 `ContinuityRevisionTask`；UI 不复制 entry/override、自动重生成或替换 current。
- 在 workflow/Run 面板显示项目文本费用阈值、图片/视频批量 BudgetGate、`cost=unknown` 强确认和精确 `runId + logicalOperation` 绑定；未经确认或超阈值时保持 `waiting_review`。
- 补齐 Run 用户控制闭环：工作台显示节点输入/输出安全摘要、耗时、最近事件和失败详情；对 `queued|running|waiting_review` Run 提供显式取消，展示 `cancel_requested|cancelled`、竞态和失败诊断，绝不把取消后的晚到结果显示为成功。
- 当 SkillRouter 返回歧义时，工作台展示按 owner 顺序给出的候选 SkillRevision、过滤/排序原因和不确定状态，要求用户从当前允许候选中显式选择；确认前不启动 Run/NodeRun/TextModel，确认后只显示 workflows/runs 冻结的最终 SkillRevision snapshot。
- 定义 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）的路由、DTO、Zod、TanStack Query/Zustand 缓存和失败可见性，供后续实现使用；Local 不是 TOS 失败 fallback。
- 新增按 Episode 隔离的场次折叠/展开、完整镜头卡片和按运行/生成状态、实际模型和审核结果组合筛选。镜头卡片显示场次、角色/场景引用、图片/视频候选、时长、提示词摘要、模型、成本、生成/审核/派生状态，并提供到候选审核、Agent、资产中心和显式 Episode Timeline 的项目安全入口；读取/筛选不产生业务 mutation。
- 切换 Episode 时按 `projectId + episodeId` 隔离保存并恢复 storyboard/workflow 视口、场次折叠、组合筛选、选中 Shot/Asset 和 active Agent session 引用；不兼容或已失效的 selection/session 必须清除并显示诊断，不能泄漏到另一集。
- ShotCard 还显示当前 AssetBible resolved snapshot ID/revision/hash、resolved chain 摘要和相关 ContinuityRevisionTask 状态；incomplete/stale/pending 时精确禁用依赖该连续性事实的图片、Agent 或 Timeline 动作。
- 由本 change 负责阶段一共享项目壳层：在保留 `projectId`、显式 `episodeId` 和 selection handoff 的前提下，连接 Workbench、Candidate Review、Project Asset Center、Episode Timeline、Project Exports 与项目模型设置；foreign/missing scope 可见失败，导航本身零业务 mutation。
- 由本 change 的共享前端 harness 负责阶段一非功能证据：以包含 300 个 fixed published Workflow node 的确定性只读投影（并关联必要 Scene/Shot）验证浏览、筛选、选择、详情和跨页导航可操作；普通本地 API 的 P95 必须 `<500ms`，明确排除外部生成、媒体传输、长连接等待、FFmpeg 预检/渲染/导出；桌面 Chrome 与 Edge 分别完成同一关键闭环并记录版本。

## Capabilities

### New Capabilities
- `drama-creation-workbench-ui`: 面向桌面 React 工作台的 CreativeBrief、双视图、文本审核与 Run 状态/恢复交互契约。

### Modified Capabilities
- 无。

## Impact

- 后续实现将修改 `apps/web`，并消费 `extend-projects-episodes-creative-slice`、`implement-asset-bible-continuity-slice`、`implement-scenes-shots-storyboard-slice`、`implement-workflows-runs-slice`、`implement-provider-model-skill-catalog` 和 `integrate-agentscope-text-skills` 定义的 owner API contract。
- 使用现有 React 19、Vite、React Router、Lucide React；目标架构要求 TanStack Query、Zustand、Zod、shadcn/Radix 与 Tailwind，但它们尚未在当前 `apps/web/package.json` 安装，依赖引入属于后续实现任务。

## 阶段一闭合补充

本 change 还拥有浏览器可见的 `/projects` 项目列表、创建、`If-Match` 编辑、显式选择和进入 `/projects/:projectId/workbench`，以及共享 Playwright harness 的后续实现合同。工作台可在零 Episode 时打开；文本批次接受后按 `(number,id)` 列出 Episode，用户必须显式选择，foreign/missing 深链报错而不得静默选择第一集；“全部集”仅是项目视图，episode-specific timeline 必须显式选集。它消费 `workflows/runs` 的 project-scope 默认已发布版本，不提供图编辑、连线、保存或发布 UI。默认 E2E 是 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），真实 AgentScope/Provider/TOS/FFmpeg 只可由显式 probe 触发，且页面加载/视图切换不得创建或切换 profile。

本 change 的 `E2E-MVPA-001` 实现和报告 MUST 引用总体 `plan-phase-one-drama-mvp-a/design.md` 的 canonical `S01`-`S11` 加 `S08a asset center` stage evidence matrix；UI 只观察 owner response/state，不重定义领域事实。每个阶段报告必须逐行记录 DDD owner、exact prerequisite snapshot、可读取 success artifact/assertion、对应 `F01`-`F11`/`F08a` focused failure diagnostic 与 no-side-effect invariant；缺少任一字段不得报告 E2E 通过。storyboard 仅显示同一 Episode 内 Scene 排序和同一 Scene 内 Shot 排序，不提供 insert/copy、Scene split/merge、Shot 跨场 move 或批量编辑。

矩阵补入不重编号的 `S04a asset bible continuity` 后，Workbench 必须记录初始 entry handoff/ack、accepted resolved snapshot、影响集合/hash、显式 successor accept、pending/resolved task 与 `F04a asset_bible_impact_or_snapshot_conflict`。该行失败不得打开图片/Agent/Timeline 连续性门，也不得自动修改 ShotSpec/current media/Timeline。

共享壳层不得把当前全局 `/assets`、`/runs` 或 `/settings` 链接当作项目业务上下文。阶段一导航必须生成 owner-validated project-scoped route，只有 Timeline 要求显式 Episode；从镜头、候选、usage 或导出返回时必须保留来源上下文。空项目不展示或启动 MVP-B 模板流程，只提供 original/adaptation 的当前阶段入口。

## Candidate gate UI 合同

**DDD**：workbench 只显示 TextReviewBatch 和 storyboard eligibility owner facts。**BDD**：successor/stale closure、unaccepted image、Agnes preflight rejection 可见且没有“已提交”假象。**SDD**：显示 candidate/provenance/revision/hash/target snapshot，不构造 accept 或 submit。**TDD**：聚焦 stale/foreign/mismatch、零外部副作用与 strict E2E 回归。

## SourceMaterial 与默认 Workflow 边界

**DDD**：projects owner 管理 Project、creationMode、CreativeBrief、项目设置和预算；text owner 只读消费 validated CreativeBrief snapshot，并管理 adaptation SourceMaterial、文本候选与 TextReview；Run 由 workflows/runs owner 管理。上传文件本体仍由 StoragePort/AssetVersion owner 管理，inline_text 不创建 storage session、StoredObject 或 AssetVersion。**BDD**：invalid enum/scope/revision/hash/status 或 adaptation parse/validation 失败可恢复且不启动付费 Run；original 无 source 仍可继续。**SDD**：UI 分别传 projects owner 的 brief DTO、text owner 的 source DTO、project/source/brief IDs/revisions/content/payload hashes 与 parse/validation/binding status/version；Run 创建后再增加 run ID/revision。默认 Workflow 只读展示 published source。**TDD**：覆盖 original、两种 adaptation 输入、foreign/stale/invalid、recovery/no-reupload、original/inline upload intent 零 storage mutation、MVP-B storyboard 结构编辑和 graph mutation zero-write。
