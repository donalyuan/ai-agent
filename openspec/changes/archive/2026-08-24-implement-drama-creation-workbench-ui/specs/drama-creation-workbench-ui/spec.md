## ADDED Requirements

### Requirement:Episode 隔离的故事板浏览与筛选
工作台 SHALL 为每个 Episode 独立保存场次折叠/展开、镜头缩略图可见性，以及按 status、实际 model identity/revision 和 review result 的组合筛选。缩略图 MUST 只读取 owner/Media projection 的安全 ready reference；pending/failed/stale MUST 显示对应状态。筛选、折叠、切换 Episode 和读取缩略图 MUST NOT 创建或修改 Run、ProviderCall、TextReviewBatch、Scene/Shot 排序、AssetVersion 或审核决定。

#### Scenario:在单集内折叠并筛选镜头
- **WHEN** 用户在 Episode A 折叠场次并按状态、模型和审核结果筛选后切换到 Episode B
- **THEN** Episode B 使用自身隔离的交互状态，列表只显示 owner projection 匹配项；返回 Episode A 时可恢复其 UI slice，且整个过程零业务 mutation

#### Scenario:派生缩略图不可用时保持事实可见
- **WHEN** Shot 缩略图为 pending、failed、stale 或 AssetVersion fingerprint 不匹配
- **THEN** UI 显示 owner diagnostic 和稳定占位，不伪装 ready、不触发生成，也不改变候选或 current reference

### Requirement:CreativeBrief 工作台边界
系统 SHALL 在 `/projects/:projectId/workbench` 提供项目/剧集上下文明确的 CreativeBrief 表单，且只向 projects owner 提交 canonical payload：六项创作语义 `subject`、`genre`、`audience`、`characterPremise`、`style`、每集 `episodeDurationSeconds`，三个精确计数 `episodeCount`、`scenesPerEpisode`、`shotsPerScene`，以及 `schemaVersion`、`revision`。任何展示标签均为 presentation-only，不进入 command 或 schema。UI MUST 以 Zod 验证字段、稳定 ID 和版本；text owner 只消费保存成功的 validated snapshot，UI 与 text owner 均不得创建平行 CreativeBrief 或猜测字段映射。

#### Scenario:保存有效 Brief
- **WHEN** 用户提交已验证、携带当前 revision 的 CreativeBrief
- **THEN** UI 只调用 owner command、失效对应 Query key 并显示 owner 返回的版本

#### Scenario:拒绝不兼容的 owner DTO
- **WHEN** owner DTO 缺少、冲突或无法等值映射所需 brief 字段
- **THEN** contract test/运行时边界失败并显示可诊断错误，且 UI 不提交替代字段

#### Scenario:无 mutation 阻断不完整或冲突的 canonical payload
- **WHEN** canonical payload 缺少字段、包含未知/并行字段或 `schemaVersion`/`revision` 与 owner revision 冲突
- **THEN** Zod/contract adapter 在 owner command 前返回可诊断 validation，且不写入 Query mutation、command、领域事实或 audit

### Requirement:共享 storyboard 和 workflow projection
系统 SHALL 以分段控件切换 storyboard/workflow，两个视图 MUST 使用同一 Scene/Shot stable ID、revision、排序与引用。故事板 reorder MUST 发送 expected revision Compare-And-Swap；workflow 视图 MUST 只读显示固定、版本化、已发布默认 WorkflowVersion 的 source/run projection，MUST NOT 维护平行 ShotSpec 或执行领域规则，也 MUST NOT 提供 graph edit/connect/save/publish mutation。

#### Scenario:以当前 revision 重排 Shot
- **WHEN** 用户在 storyboard 拖动同一 Scene 内的 Shot 并提交当前 expectedRevision
- **THEN** UI 显示 owner 返回的排序投影，切换到 workflow 后仍引用同一 Shot ID/revision

#### Scenario:从 stale reorder 恢复
- **WHEN** reorder 返回 revision conflict
- **THEN** UI 回滚乐观顺序、读取 authoritative projection 并保留原始 conflict 诊断

#### Scenario:不提供延后的 storyboard 结构编辑
- **WHEN** 用户寻找或请求 storyboard insert/copy、Scene split/merge、Shot 跨场 move 或批量编辑
- **THEN** MVP-A UI 不呈现对应 affordance，兼容请求显示 `unsupported_feature`，且不提交 owner mutation；Timeline 的 `SplitClip` 仍按 Timeline owner contract 可用

#### Scenario:保持默认 workflow projection 只读
- **WHEN** 用户切换到 workflow 视图或尝试编辑节点/连线/保存草稿/发布版本
- **THEN** UI 显示 published WorkflowVersion 的 source/status，标记 graph editing 为 MVP-B，且不发起 WorkflowDraft/WorkflowVersion mutation

### Requirement:CreativeBrief 文本生成 Run 生命周期
系统 SHALL 在 projects owner 保存 CreativeBrief 成功后只响应用户明确的“生成”命令，调用 workflows/runs owner 幂等 ensure/freeze 固定的 published WorkflowVersion，并创建、启动冻结 projects owner CreativeBrief、adaptation SourceMaterial（仅 adaptation）与 WorkflowVersion snapshot 的文本生成 Run；不得由页面加载、自动保存、视图切换或选择变更隐式 ensure Workflow 或启动 Run。重新生成 MUST 绑定当前 Brief/source/Run revision、同一项目/集 scope 和新的 logical operation；失败或刷新后恢复 MUST 先读取 owner Run/intent snapshot，并在 `submission_unknown` 时先 reconciliation，不能盲目创建第二个可收费提交。

#### Scenario:提交 Brief 并启动文本生成
- **WHEN** 用户提交 schema-valid CreativeBrief 并明确点击生成
- **THEN** UI 以当前 brief/source revision、scope、provider/model/skill snapshot 引用调用 owner create/start command；owner 幂等返回并冻结 fixed published WorkflowVersion，UI 显示 `runId`、状态和 `logicalOperation`，且页面加载不会产生 ensure 或 Run mutation

#### Scenario:重新生成既有文本 Run
- **WHEN** 用户在失败或已被明确拒绝的文本候选上点击重新生成并确认
- **THEN** UI 提交当前 Brief/Run revision 与新的 logical operation，保留旧 Run/候选为只读审计，不覆盖原候选或复用不匹配的确认

#### Scenario:恢复失败或刷新后的 Run
- **WHEN** 用户刷新页面、Run 为 `failed`/`submission_unknown` 或 Worker/API 曾重启
- **THEN** UI 先读取持久化 Run/intent；`submission_unknown` 或重启只对同一 Run/operation reconciliation，failed Run 仅显示原始失败或显式创建 successor 的入口，绝不把终态 Run 原地改回 running，也不创建第二个未知外部提交

### Requirement:从失败节点继续的 successor Run UI
系统 SHALL 只在 workflows/runs owner 返回 `allowedActions.createSuccessor=true` 时提供“从失败节点继续”。确认视图 MUST 展示 predecessor Run/revision、失败节点、owner 提供的 reused success evidence、新执行节点、新 selection snapshot/费用影响；提交 MUST 使用 owner `CreateSuccessorRunFromFailure` command。成功后 UI SHALL 导航到新 runId，并保持 predecessor/旧候选/事件只读。stale/foreign/mismatch 或 `submission_unknown` MUST 显示原始诊断并零新 Run/Provider 副作用。

#### Scenario:显式创建并进入 successor Run
- **WHEN** 用户检查复用证据和费用影响后明确继续，owner 创建 successor 成功
- **THEN** UI 显示新 runId/predecessor lineage 和新 logical operations，旧 Run 保持 failed；reused 节点不显示为重新执行或重新收费

### Requirement:Skill 路由歧义必须人工裁决
当 owner `SkillRouteDecision.status=needs_human_selection` 时，系统 SHALL 展示候选 SkillRevision ID/digest、确定性过滤/排序原因、capability/policy 匹配、分数和歧义原因，并只允许用户从当前 candidate set 显式选择。提交 MUST 绑定 decision revision、candidate ID/digest、launch expected revision 和 project/node scope；关闭、刷新、默认第一项或设置页 enabled 状态 MUST NOT 自动选择。确认前不得启动 Run/NodeRun/TextModel/Provider；成功后只展示 workflows/runs 冻结的最终 SkillRevision snapshot。

#### Scenario:人工选择当前路由候选
- **WHEN** 路由存在并列/低置信候选，用户选择当前允许的 SkillRevision 且 expected revisions 匹配
- **THEN** UI 提交一次 owner selection command，随后显示冻结 SkillRevision 和 route audit；不修改 Registry 或加载其他候选正文

#### Scenario:拒绝过期或非候选 Skill
- **WHEN** decision/candidate/launch revision 过期、candidate 已 disabled/unapproved/不在集合，或用户未选择即启动
- **THEN** UI 显示 reroute/validation diagnostic，零 Run/NodeRun/TextModel/Provider mutation，不回退到默认第一项

### Requirement:SourceMaterial 导入和绑定
系统 SHALL 通过 projects owner 在 CreativeBrief 前选择并保存 `creationMode=original|adaptation`。original UI MUST 显示 CreativeBrief 的 `subject`、`genre`、`audience`、`characterPremise`、`style`、`episodeDurationSeconds`、精确 `episodeCount`/`scenesPerEpisode`/`shotsPerScene` 与 schema/revision，MUST NOT 要求或显示 SourceMaterial，且有效 original MUST 可继续到文本 Run。adaptation UI MUST 让用户选择 `materialType=novel|synopsis|existing_script` 与 `inputMode=inline_text|uploaded_file`，并显示 text owner SourceMaterial 的 immutable revision/contentHash、`parseStatus`、`validationStatus`，以及 `CreativeBriefSourceBindingSnapshot` 的 project/source/brief IDs、revisions、content/payload hashes、parse/validation/binding status/version；Run 创建后还显示增加 `runId`、`runRevision` 的 `TextRunSourceBindingSnapshot`。uploaded_file 才显示 verified `assetVersionId` 并通过 StoragePort/AssetVersion owner 交接；inline_text MUST 显示无 storage session、StoredObject 或 AssetVersion。UI 不保存媒体 bytes，且 invalid enum/scope/revision/hash/status 或 adaptation invalid source MUST 显示 owner diagnostic 并在 Text Run 或 Storage mutation 前阻断。

#### Scenario:创建保留的 SourceMaterial 输入
- **WHEN** 用户选择 adaptation 并提交 `novel|synopsis|existing_script` 和 `inline_text|uploaded_file` 的有效 SourceMaterial
- **THEN** UI 展示 parsing/validation、immutable revision/contentHash 与精确 brief binding snapshot；inline_text 不创建 storage session/StoredObject/AssetVersion，uploaded_file 在 verified AssetVersion 后才允许绑定，且只有全部 IDs/revisions/hashes/status/version 匹配、`validationStatus=valid` 后才允许创建/启动 adaptation 文本 Run并显示完整 run binding snapshot

#### Scenario:拒绝无效或 foreign source
- **WHEN** source parse/validation 失败、AssetVersion 未验证/跨项目、revision/hash 过期，或必需 source snapshot 不完整
- **THEN** UI 展示 owner 原始诊断，禁用文本 Run command，不上传替代内容且不产生付费提交

#### Scenario:恢复 source-bound 文本 Run
- **WHEN** SourceMaterial 解析或绑定的文本 Run 失败、页面刷新或 Worker/API 重启
- **THEN** UI 复用相同 source revision/hash 和 `runId + logicalOperation`，先 reconciliation unknown state，不重复上传/生成或把恢复失败显示为成功

### Requirement:付费媒体前的文本 candidate 审核
系统 SHALL 将 StorySpec、各集 ScriptSpec、Episode、Scene、Shot、ShotSpec，以及这些叙事候选实际引用的初始 AssetBible typed entry specs 作为一个 `pending_review` TextReviewBatch 展示，按依赖层级提供 diff、版本、来源、Schema 和 stale 状态，并只提供一次 batch 级 `accept` 或 `reject`。不被实际引用的 AssetBible 条目不得由 UI 隐式加入 handoff。legacy/unknown `approve` MUST 显示 validation 且零 accepted handoff/媒体副作用。用户编辑批次内上游候选后，UI MUST 显示受影响成员 stale/待重新生成，不得对 partial/stale batch 启用接受。任何付费媒体入口 MUST 在 TextReview accepted handoff及 Project/Episode/Scene/Shot/AssetBible 各 owner typed batch command ack 全部完成前保持不可执行；UI MUST NOT 提供逐层必需审批、自动确认、拼接 owner facts 或直接调用 Provider。

#### Scenario:批准文本 candidate
- **WHEN** 用户审核到 schema-valid 的文本候选并显式 accept
- **THEN** UI 提交 batch ID、完整 candidate IDs/hashes、实际引用的 AssetBible specs、expected revisions 与单次决定，并只在 TextReview accepted handoff 和 Project/Episode/Scene/Shot/AssetBible 全部 owner ack 匹配后刷新下游可用状态

#### Scenario:审核前尝试启动付费媒体
- **WHEN** TextReviewBatch 仍为 building/pending_review/rejected、包含 stale/无效成员或其确认状态未知
- **THEN** UI 显示审核阻断原因且不发起媒体或 Provider 请求

### Requirement:Run 状态与 SSE 恢复
系统 SHALL 显示 owner Run 状态及持久化 event sequence，并以 Last-Event-ID 恢复 SSE。断线、非法 cursor、foreign run 或 event gap MUST 保留原始诊断并读取 snapshot；UI MUST NOT 把连接恢复误报为 Run 成功。Run create/start/recover/reconcile 命令必须绑定同一项目、Brief revision、`runId + logicalOperation` 和 owner correlation。

#### Scenario:重连到运行中的 Run
- **WHEN** 连接在已接收 sequence 后中断并成功重连
- **THEN** UI 从最后 sequence 后合并事件并显示 owner 当前状态

#### Scenario:SSE 恢复失败
- **WHEN** owner 返回非法 cursor、权限错误或无法补齐的事件间隙
- **THEN** UI 显示可重试/刷新操作与原始失败信息，不改变 Run 为成功

### Requirement:从历史 Run 输入快照显式重新运行
Workbench SHALL 列出并展示同项目 immutable `RunInputSnapshot` 的 source Run ID/revision、CreativeBrief/SourceMaterial、固定 WorkflowVersion、scope、owner references、历史 selection 和 runnable/费用诊断。用户只有在明确选择一个精确 snapshot、检查目标 selection 与新 BudgetGate 后，才可提交 `CreateRunFromHistoricalSnapshot`；成功后导航到新 runId 并显示 `rerunOfRunId` lineage。UI MUST NOT 将“某版本”解析为 current/上一版、原地重启历史 Run、自动重基引用、复用 failed-successor evidence 或旧费用确认，也不得 fallback 到其他 Provider/Model/Skill。

#### Scenario:选择历史快照创建 rerun
- **WHEN** 用户选择精确历史 snapshot，检查输入、selection 和费用影响并明确确认，owner 创建新 Run 成功
- **THEN** UI 显示新的 runId/lineage/new operations，source Run/候选/审核/事件只读且不显示任何节点为隐式复用

#### Scenario:历史快照不可运行时保持只读
- **WHEN** snapshot/revision/hash 缺失或 stale/foreign，历史 selection 不再 runnable，或关联 operation 为 `submission_unknown`
- **THEN** UI 显示逐项 owner diagnostic，禁用 rerun 或要求用户显式选择新配置并重新确认，不提交新 Run/Provider mutation、不改用 current

### Requirement:Workflow 费用与 BudgetGate 交互
系统 SHALL 在 workflow/Run 面板显示 owner 的项目文本费用阈值、每个付费 operation 的 estimate/actual、currency、source、`cost_status` 与确认状态。图片/视频批量生成 MUST 在提交前确认；文本估算超过项目阈值和 `cost=unknown` MUST 保持 `waiting_review` 并要求明确确认。确认 mutation MUST 绑定当前 `runId + logicalOperation`、request fingerprint、revision 和稳定本地 user UUID；页面刷新、重试、恢复或参数变化不得复用失配确认。

#### Scenario:确认精确的付费 batch operation
- **WHEN** 用户查看当前图片/视频批量 operation 的费用与影响并明确确认
- **THEN** UI 只提交 owner 返回的 run/logical operation/fingerprint/revision，成功后刷新同一 BudgetGate；重复确认不创建第二个收费 mutation

#### Scenario:阻断超阈值或 unknown cost
- **WHEN** 文本 estimate 超过项目阈值、成本为 unknown、确认缺失/过期或绑定不匹配
- **THEN** UI 显示原因并保持付费 command 禁用/Run `waiting_review`，不直接调用 Provider，也不把恢复视为新确认

### Requirement:Frontend 验证边界
系统 SHALL 以组件、state、Zod contract、Query adapter 与 Playwright E2E 入口覆盖 brief、双投影、审核门、CAS 和 SSE 失败场景。默认测试 MUST 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）；真实 Provider 调用不属于本 UI change，且页面加载/视图切换不得创建或切换 profile。

#### Scenario:运行 Frontend 回归套件
- **WHEN** 维护者执行本 change 的验收命令
- **THEN** 测试可定位组件、state、contract 或 E2E 失败，且全部 OpenSpec task 在实现前保持未勾选

### Requirement:Project 入口和共享 E2E harness
系统 SHALL 在 `/projects` 提供 list/create/If-Match edit/explicit select，并只经既有项目 owner API 进入 `/projects/:projectId/workbench`。工作台 SHALL 支持 zero Episode、按 `(number,id)` 列出并显式选择 Episode；foreign/missing deep link MUST 显示 error，项目“全部集”视图 MUST NOT 隐式选择 episode-specific timeline。此 change SHALL 拥有 direct `@playwright/test`、config、dedicated E2E reset/lifecycle、根 `pnpm run test:e2e` 与 `E2E-MVPA-001` 的后续实现；默认 MUST 是 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），Local 不是 TOS 失败 fallback，运行开始后 Adapter/Profile MUST 冻结。`E2E-MVPA-001` MUST 引用总体 `design.md` 的 canonical `S01`-`S11` 加 `S04a asset bible continuity`、`S08a asset center` stage evidence matrix；每个阶段 MUST 在 UI E2E report 中记录 DDD owner、exact prerequisite snapshot、success artifact/assertion、对应 `F01`-`F11`/`F04a`/`F08a` diagnostic 和 no-side-effect invariant，UI MUST NOT 改写或重新定义 owner fact。

#### Scenario:进入空 Project 工作台
- **WHEN** 用户创建、编辑并显式选择一个尚无 Episode 的项目
- **THEN** UI 进入 zero-episode workbench，不创建 Episode、Run 或隐式 workflow mutation

#### Scenario:拒绝不安全的 Episode deep link
- **WHEN** 路由 episodeId 不存在或属于其他项目
- **THEN** UI 显示 owner error，不选取第一集或任何替代 Episode

#### Scenario:将每个浏览器阶段追溯到 canonical matrix
- **WHEN** 维护者运行 `E2E-MVPA-001` 的 workbench flow 或 focused failure suite
- **THEN** 报告按 `S01`-`S11` 加 `S04a`、`S08a` 逐行记录 owner response/state、exact prerequisites、success evidence、`F01`-`F11`/`F04a`/`F08a` diagnostic 和 no-side-effect assertion；缺行、泛化最终绿色状态或客户端拼接 owner 事实均使验收失败
### Requirement:媒体前必须可见 candidate gate
Workbench MUST 展示 text closure/batch 和 image eligibility provenance facts；对于 stale、foreign、未接受或不匹配的输入，MUST NOT 将 media submission 显示为可用。

#### Scenario:Preflight 拒绝不得产生伪成功
- **WHEN** candidate gate 失败
- **THEN** UI 展示 owner diagnostic，且不报告 ProviderCall 或 external submit 成功。

### Requirement:Run 详情和显式取消闭环
Workbench SHALL 显示 Run/NodeRun 的稳定 ID、状态、owner 提供的开始/结束时间或耗时、脱敏输入输出摘要、最近 RunEvent sequence、失败 code/message/retryability 和可用动作。只有 `queued|running|waiting_review` Run 可以显示取消；取消 MUST 以当前 expected revision/correlation 调用 workflows/runs owner，一次提交后显示 `cancel_requested` 并通过 SSE/snapshot 收敛到 `cancelled`。UI MUST NOT 保存或展示 plaintext secret、媒体 bytes、完整原始 Provider payload，也 MUST NOT 让取消后的晚到 Activity/Provider success 覆盖 owner 的取消状态。

#### Scenario:取消运行并观察确定状态
- **WHEN** 用户明确取消当前 `queued|running|waiting_review` Run
- **THEN** UI 只提交一次 owner cancel command，显示 `cancel_requested` 和后续 `cancelled`；重复点击、刷新或晚到结果不创建第二 command、不报告 succeeded

#### Scenario:拒绝过期或终态取消
- **WHEN** Run 已终态、属于其他项目或 expected revision 已过期
- **THEN** UI 显示 owner validation/conflict、refetch authoritative snapshot，且不发送补偿 mutation、不改变 Run/NodeRun/Event

### Requirement:完整镜头卡片与项目安全操作入口
Storyboard SHALL 为每个 Shot 显示所属 Scene、角色/场景引用、AssetBible resolved snapshot ID/revision/hash/chain 和 ContinuityRevisionTask、current/candidate image/video、duration、prompt summary、实际 model identity/revision、cost value/status/source、generation/review/derivative readiness。该 `ShotCardView` MUST 是不持久化的 presentation projection：Scene/ShotSpec、AssetBible snapshot/task、current eligibility、candidate/review、Run/ProviderCall 安全摘要和 Media derivative 各自保留 owner ID/revision/hash/status，不得由 UI 合并成新的 accepted/current/ready 事实。任一 owner unavailable、partial、stale 或 revision mismatch 时 MUST 对应显示并禁用依赖动作。卡片动作 SHALL 只生成 owner-validated Candidate Review/Agent/Asset Center/显式 Episode Timeline handoff，并由目标 owner 重新校验；MUST NOT 从缩略图或 AssetVersion status 推断 accepted/current。未接受、continuity pending、stale、foreign 或 derivative-not-ready 媒体 MUST 显示精确阻断原因并禁用 Agnes 或 Timeline handoff。

#### Scenario:从镜头进入候选审核再返回
- **WHEN** 用户从有效 ShotCard 打开 image/video candidate review
- **THEN** route 携带 project/episode/scene/shot/asset version 的稳定 ID/revision/hash，返回时保留来源 Episode 和筛选状态，导航本身零审核/Provider mutation

#### Scenario:阻断不可用镜头媒体
- **WHEN** candidate 未接受、scope/revision/hash 不匹配或必需 derivative 未 ready
- **THEN** 卡片显示 owner diagnostic，不显示 Agnes/Timeline 可执行状态，也不创建 Run、ProviderCall、Clip 或 AssetVersion

#### Scenario:ShotCard owner projection 部分不可用
- **WHEN** Run/ProviderCall、review、scenes eligibility 或 Media projection 任一 owner query 不可用、超时或 revision 不匹配
- **THEN** 卡片只将对应字段组标记为 partial/unavailable/stale，不用其他字段猜测结果、不持久化合成状态，并禁用依赖该事实的动作

### Requirement:AssetBible 管理、影响确认与连续性任务 UI
Workbench SHALL 提供 `view=asset-bible`，只通过 AssetBible owner 管理 Character、Look、Location、SceneVisual、Prop、VisualStyle 的稳定 entry 与不可变 version，提交 project/episode/scene/shot assignment，读取 resolved chain/snapshot，并执行 `PreviewAssetBibleRevisionImpact` 与 `AcceptAssetBibleRevision`。UI MUST 在接受前展示完整实际 Episode/Scene/Shot target set、每项 revision/reason、analysis/set hash 和 expected revisions；只可用一个 all-or-nothing command 显式接受。成功后 SHALL 显示 successor/current pointer、AcceptDecision 和 `ContinuityRevisionTask` 状态；MUST NOT 复制 entry/override/resolver facts、自动修改 ShotSpec/current media/Timeline、自动执行 Provider 或把 incomplete analysis 当作可接受。

#### Scenario:预览并显式接受 AssetBible successor
- **WHEN** 用户编辑合法 entry candidate，owner 返回完整 impact target set/hash，且用户确认全部精确范围
- **THEN** UI 提交一次 owner accept command，显示 successor/current 与 pending tasks；旧 entry version、ShotSpec、current media 和 Timeline 继续引用原 snapshot，直到各 owner 显式修订

#### Scenario:影响集合冲突时全有或全无失败
- **WHEN** analysis incomplete、target set 缺失/重复/foreign、hash/revision 过期或 owner unavailable
- **THEN** UI 显示原始 diagnostic、禁用或拒绝 accept、刷新 owner state，且不拆分提交、不创建 successor/task、ProviderCall 或媒体 mutation

#### Scenario:resolved snapshot 或 task 阻断依赖动作
- **WHEN** Shot 的 snapshot incomplete/stale/hash mismatch，或相关 `ContinuityRevisionTask` 为 pending
- **THEN** Workbench/ShotCard 显示 snapshot/task 状态，禁用依赖该连续性事实的图片/Agent/Timeline command，并提供显式 owner 修订入口，不自动重基

### Requirement:阶段一共享项目导航闭环
系统 SHALL 用共享项目壳层连接 `/projects/:projectId/workbench`、`/projects/:projectId/review`、`/projects/:projectId/assets`、`/projects/:projectId/episodes/:episodeId/timeline`、`/projects/:projectId/exports` 和项目模型设置。所有入口 MUST 保留并验证 projectId；Timeline MUST 要求用户显式选择 episodeId，Project Exports 只允许用户显式选择 published TimelineVersion。镜头、候选和 usage handoff 只可携带 owner stable IDs/revisions/hashes；切换 project/Episode MUST 清除不兼容 selection。route load、back、breadcrumb 和 tab switch MUST 为只读，不得创建或修改 Run、ProviderCall、UploadSession、审核、Timeline、EpisodeExportBatch 或设置。zero-episode 空态 MUST 只提供 original/adaptation 当前入口，不展示或启动 MVP-B 推荐模板/从模板开始。

#### Scenario:沿项目上下文完成跨页面导航
- **WHEN** 用户从 Workbench 进入 Review、Assets、显式 Episode Timeline、Project Exports 或项目设置并返回
- **THEN** 每个 route 保持同一 owner-validated project、必要时保持显式 Episode/selection，且导航过程零业务 mutation

#### Scenario:拒绝缺失或 foreign 导航 scope
- **WHEN** route/handoff 缺少必需 project/Episode、引用其他项目，或 Timeline 没有显式 Episode
- **THEN** UI 显示选择或 owner error，不静默选择第一集/current TimelineVersion、不使用全局 `/assets|/runs|/settings` 替代项目上下文，也不提交下游 command

### Requirement:Episode 展示状态和 Agent 会话上下文隔离恢复
Workbench SHALL 以 `projectId + episodeId` 为 key 隔离保存 storyboard/workflow 视口、场次折叠、`status/model/review` 组合筛选、当前 Shot/Asset selection 和 active Agent session ID。切换 Episode MUST 先保存离开集的 display slice，再验证并恢复目标集 slice；返回原集时恢复其合法状态。客户端 MUST NOT 在 slice 中复制 message/turn、Run、candidate、AssetVersion 或 owner revision 正文，也 MUST NOT 将 selection/session 泄漏到其他 Episode。missing/foreign/stale selection 或 session 必须清除并显示 owner diagnostic；保存、切换和恢复 MUST 为本地只读交互，不能重发消息、生成 Plan、启动 Run/Provider 或修改审核/排序。

#### Scenario:切回 Episode 恢复合法展示状态
- **WHEN** 用户从 Episode A 切到 B 后再返回 A，A 的视口、折叠、筛选、selection 和 active session 仍属于当前 owner scope/revision
- **THEN** UI 恢复 A 的展示状态和 session 引用，B 的上下文不可见，且没有产生 owner mutation、消息重发或收费操作

#### Scenario:清除过期或跨集上下文
- **WHEN** 目标 Episode slice 中的 Shot/Asset/session 已删除、revision 过期或属于其他 project/Episode
- **THEN** UI 原子清除不兼容值并显示 diagnostic，只保留仍合法的视口/筛选，不用其他集的状态兜底且不产生业务 mutation

### Requirement:阶段一前端性能与桌面浏览器验收
共享前端 harness SHALL 在确定性本地环境中，以包含 300 个 fixed published Workflow node 的只读投影 fixture（每个 node 使用冻结 scope，按需关联 Scene/Shot）验证加载、浏览、滚动或分页、筛选、选择、详情和跨 Workbench/Review/Assets/Timeline/Settings 导航可完成，且不得崩溃、丢失当前 project/Episode scope、触发隐式业务 mutation 或借机实现 MVP-B graph authoring。普通 API 的 localhost HTTP 请求 P95 MUST `<500ms`；报告 MUST 记录 route 集合、样本量、warm-up、环境、数据量、成功/失败数和 percentile，并 MUST 排除 Provider/Agent/Temporal 等待、SSE 长连接、上传下载、媒体 probe/preview/render/export。关键业务闭环 MUST 分别在桌面 Chrome 与 Edge 验证并记录实际浏览器版本。

#### Scenario:300 个固定工作流节点投影保持可操作
- **WHEN** harness 加载包含 300 个 fixed published Workflow node、冻结 scope 及必要 Scene/Shot 关联的只读项目投影并执行浏览、筛选、选择、详情和真实跨页导航
- **THEN** 操作均有可观察结果，project/Episode/selection 保持正确，无崩溃、未处理错误或隐式 Run/Provider/upload/review/Timeline/settings mutation

#### Scenario:普通 API 达到 P95 门槛
- **WHEN** harness 在记录的 warm-up 后对声明的普通 API route 集合采样
- **THEN** localhost HTTP 成功请求 P95 `<500ms`，报告可复算且未混入外部生成、长连接、媒体传输或渲染耗时

#### Scenario:桌面 Chrome 与 Edge 均完成关键闭环
- **WHEN** 维护者运行阶段一浏览器兼容验收
- **THEN** Chrome 与 Edge 分别完成项目入口、工作台、审核、资产、Timeline 和设置导航及关键操作，并保存各自版本与失败证据；缺少任一浏览器结果时该验收失败
