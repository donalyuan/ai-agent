# context-agent-candidate-review-ui Specification

## Purpose
TBD - created by archiving change implement-context-agent-candidate-review-ui. Update Purpose after archive.
## Requirements
### Requirement:视频 review 顺序与 canonical action
UI SHALL 将 video owner state 投影为 Provider terminal result candidate -> 人工 `accept` -> scenes exact current CAS -> MediaInspect/derivatives -> Timeline handoff。derivative `pending|failed|stale` 仅阻断 Timeline/preview/export，MUST NOT 撤销 accepted/current。UI action/audit projection 仅允许 `accept|reject|retake`；legacy/unknown `approve` MUST 显示 validation 且零 current/retake side effect。AssetEdit accept SHALL 只显示同一 existing AssetVersion 的 AcceptDecision/audit 与 scenes eligibility CAS，MUST NOT 表示复制 bytes/object/ref 或新增 AssetVersion。

#### Scenario:页面不把 derivative gate 表示为撤销
- **WHEN** 视频已 accepted/current 而 derivative 变为 pending、failed 或 stale，或用户提交 legacy action
- **THEN** UI 只阻断 Timeline/preview/export 并展示 validation，不显示 current 回滚或新的 AssetVersion

### Requirement:session 范围内的主选择与显式引用
系统 SHALL 为每个 AssetEditSession 保存一个 primary selection 与显式 refs，**可执行 AssetEditPlan 的类型只能为完整 image 或 video AssetVersion**，并使用 owner ID/revision/hash 验证。story/script 只能展示 TextReview successor/stale closure；audio/TimelineVersion 只能跳转 Timeline editor typed command。切换项目、集、场、镜头、AssetVersion 或 selection type 时，UI MUST 清除不属于新 scope 的 primary/ref，且不得从之前会话或当前画布隐式继承引用。

#### Scenario:从一个 Shot 切换到另一个 Shot
- **WHEN** 用户切换到不同 Shot 或 AssetVersion
- **THEN** UI 仅保留与新 scope 相符的显式选择，并要求用户重新确认其他 refs

#### Scenario:尝试跨项目引用
- **WHEN** 用户或深链提供其他项目/集的 reference
- **THEN** UI 拒绝该 reference、显示可诊断错误且不创建或更新 session

### Requirement:MVP-A 不展示局部媒体编辑能力
UI SHALL 只允许用户选择完整 image/video `AssetVersion`，MUST NOT 展示或提交 mask、图片选区、局部区域、图层、视频/音频 start/end time、time range 或 segment edit 控件/字段。若深链、恢复状态或 owner DTO 含这些输入，UI MUST 显示 `unsupported_feature` 并在 Plan/execute/Provider mutation 前停止；不得静默删除字段后按完整版本执行。

#### Scenario:完整版本选择可进入计划审核
- **WHEN** 用户选择同项目、revision/hash 可验证的完整 image/video AssetVersion
- **THEN** UI 允许将该完整版本加入 primary selection 或 explicit refs，并明确显示版本身份

#### Scenario:局部编辑输入被可见拒绝
- **WHEN** 用户状态、深链或 owner payload 含 mask、选区、局部区域、start/end time、time range 或 segment edit
- **THEN** UI 显示 `unsupported_feature`，不显示可执行的局部编辑控件，也不发出 Plan、execute、ProviderCall 或 Storage mutation

### Requirement:通过 Schema 校验的 plan 与 candidate 审核
系统 SHALL 只展示经 Zod 校验的 AssetEditPlan、费用、impactAnalysis、staleTargets 和候选比较。计划 MUST 显示 base AssetVersion、修改摘要、预期工具/输出和确认要求；未知字段、缺失 schemaVersion、无效费用来源或计划 hash 不一致 MUST 禁止 execute。

#### Scenario:查看有效编辑计划
- **WHEN** owner 返回 schema-valid、未过期的 Plan 与候选
- **THEN** UI 显示费用、影响、staleTargets、基础/候选差异和明确的 execute/reject/accept 操作

#### Scenario:收到无效或过期计划
- **WHEN** Plan 校验失败或 owner 标记 stale
- **THEN** UI 保留原始诊断、禁用执行/接受并提供刷新或重新生成入口

### Requirement:session 对话、消息和轮次
系统 SHALL 为当前 `AssetEditSession` 显示隔离的 Agent conversation、按持久化 sequence 排序的 message/turn，以及明确的 `user` 输入和 `agent` 回复。消息 MUST 绑定 project/scope/session、role、turnId、sequence、状态和 correlation；切换项目、集、场、镜头或 AssetVersion 时不得复用上一 session 的 conversation。页面加载、selection 改变或刷新只读取已持久化消息，不自动发送输入。

#### Scenario:发送用户输入并接收 Agent 回复
- **WHEN** 用户在当前 session 输入消息并明确提交
- **THEN** UI 追加一条 `user` message，显示 pending turn，并读取绑定同一 session/turn/correlation 的 Agent reply；刷新后按 sequence 恢复而不重复提交

#### Scenario:拒绝跨项目或格式错误的对话数据
- **WHEN** owner 返回其他 project/session 的 message、重复 sequence、未知 role、缺失 turn/correlation 或 plaintext secret
- **THEN** Zod adapter 拒绝数据，保留可诊断错误，不写入本地会话状态，也不发起 Agent/Provider mutation

### Requirement:从显式 conversation turn 生成 AssetEditPlan
系统 SHALL 仅在用户明确选择 conversation/turn 并点击生成计划后调用 owner，从已校验的对话上下文和 image/video 显式 selection/refs 生成 Schema-valid `AssetEditPlan`。生成请求 MUST 绑定 sessionId、conversationId、turnId、primary selection、explicit refs、base AssetVersion/revision、`runId + nodeRunId + logicalOperation`（如适用）和 correlation；Plan 必须先进入 review，不能由 Agent 回复直接执行或接受。story/script/audio/TimelineVersion 选择 MUST 被拒绝或转为只读 owner view。

#### Scenario:从已完成的 Agent turn 创建计划
- **WHEN** 当前 session 的 Agent turn 已完成且用户确认生成计划
- **THEN** UI 提交精确 conversation/turn 与 refs，显示经 Schema 校验的 Plan、费用、impact/staleTargets 和候选操作，且不修改基础 AssetVersion

#### Scenario:阻止从 pending 或 failed turn 生成计划
- **WHEN** turn 仍 pending、失败、跨 session、selection/refs 过期或对话内容 Schema 无效
- **THEN** UI 禁用生成或在 owner command 前拒绝，保留原始诊断且不创建 Plan、Candidate 或 ProviderCall

### Requirement:显式副作用与 candidate 比较
系统 MUST NOT 因页面加载、selection 改变或查看候选而调用 Agent、Provider 或执行 Plan。execute 仅可由用户在查看费用/影响后显式触发；reject 必须指定 candidate/plan，且不得删除基础版本。

#### Scenario:打开审核 session
- **WHEN** 用户进入 review 路由或改变 primary selection
- **THEN** UI 只执行读取查询，不发出 execute、Provider 或 accept mutation

#### Scenario:显式拒绝 candidate
- **WHEN** 用户选择一个 candidate 并确认 reject
- **THEN** UI 发送该 candidate 的 owner reject command、刷新会话，基础 AssetVersion 保持不变

### Requirement:对精确引用集合原子接受
系统 SHALL 用单一 accept command 提交 candidateId、expectedBaseVersionId、expectedPlanRevision 和无重复的显式 `referenceIds`/expected revisions。UI MUST NOT 默认选择当前场、当前集或所有草稿，且不得将一个 accept 拆为多个部分写入。

#### Scenario:一起接受所选引用
- **WHEN** 用户明确勾选同一项目内的全部目标引用且版本仍匹配
- **THEN** UI 提交一个全量集合，只有 owner 返回 all-or-nothing 成功后才刷新所有 target cache

#### Scenario:收到 revision 冲突
- **WHEN** accept 返回 base version、plan 或 target revision conflict
- **THEN** UI 不重试写入，刷新 owner state、标记候选过期并要求重新选择或重新生成计划

### Requirement:审核 UI 验证边界
系统 SHALL 为 scope isolation、Zod/DTO、Plan/cost/impact、execute/reject/accept、409 与网络失败提供组件、store、contract 和 Playwright E2E 入口。默认 test fixture MUST 使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），且不包含媒体 bytes 或真实调用；fixture/profile 选择不得因页面加载或失败自动切换。

#### Scenario:执行审核回归测试
- **WHEN** 维护者运行前端测试与此 change 验收命令
- **THEN** 失败可定位为 component/state/contract/E2E，且实现前 tasks 全部保持未勾选

### Requirement:绑定 turn 的 pending review 计划
系统 SHALL 允许用户显式从同项目、已完成 Agent reply turn 为 image/video 生成 `pending_review` AssetEditPlan，并冻结 conversation/turn/selection/refs/base versions 与 `runId + nodeRunId + logicalOperation`。story/script/audio/TimelineVersion、foreign、pending 或 failed turn MUST 被拒绝或转到只读 owner surface；生成 plan MUST NOT execute 或 accept。

#### Scenario:拒绝未完成的 turn
- **WHEN** 用户选择 foreign、pending 或 failed turn
- **THEN** UI 显示 owner diagnostic，不创建 plan 或任何 AssetVersion/Timeline mutation

### Requirement:文本闭包审核 UI
UI MUST 渲染 owner-provided successor 和 stale-closure state，并只提交已绑定的 regenerate input；MUST NOT 合成 candidate 或绕过 batch CAS。

#### Scenario:可见地阻断无效批次
- **WHEN** batch 为 partial、stale、foreign 或 duplicate
- **THEN** UI 展示 diagnostic，且不将 media handoff 显示为 accepted。

### Requirement:image 和 video 媒体 candidate 审核 UI
UI SHALL 渲染 owner-provided image candidate provenance/base-result revision、fee/impact/stale 和 accept/reject action，并 SHALL 渲染 video `VideoTakeCandidate` status、ShotSpec/duration/aspect snapshot、normalized metadata、derivative readiness 和显式 `accept|reject|retake` action。image accept MUST 使用 owner candidate/scene eligibility contract；video retake MUST 使用新的 `logicalOperation` 提交 successor input；UI MUST NOT 混用 candidate namespace 或从 AssetVersion status 推断 current。unaccepted/stale/foreign media MUST 禁用 Agnes/Timeline handoff；derivative-not-ready 仅禁用 Timeline/preview/export，MUST NOT 禁用 video accept/current。

#### Scenario:审核生成的 image 和 video 结果
- **WHEN** 用户打开 persisted image candidate 或 video take review
- **THEN** UI 比较 base/result fact 并展示精确 owner command；页面加载和比较不执行 Provider/Worker mutation

#### Scenario:拒绝或重拍 video take
- **WHEN** 用户 reject take，或以变更的 prompt/ShotSpec/duration/aspect 提交 retake
- **THEN** UI 使用 candidate revision 和新的 logical operation 调用 owner review command，保留旧 take，并保持 Timeline handoff 阻断直至 successor 被接受

### Requirement:AssetBible continuity projection 与 stale gate
UI SHALL 在 AssetEditSession、conversation turn、Plan 和 candidate 上显示 AssetBible owner 的 accepted `ResolvedContinuitySnapshot` ID/revision/hash、resolved chain 摘要和 `ContinuityRevisionTask` 状态。UI MUST NOT 复制或修改 entry/override/resolver facts。snapshot incomplete/stale/foreign/hash-revision mismatch 或 target task 为 pending 时，UI MUST 显示 `continuity_stale`，禁用 Plan generation/execute/accept，并提供 owner-validated Workbench/Shot 修订入口；不得自动重基或解决任务。

#### Scenario:连续性任务阻断过期候选
- **WHEN** 当前 Plan 冻结的 snapshot 与 owner 不匹配，或目标出现 pending `ContinuityRevisionTask`
- **THEN** UI 保留旧 Plan/candidate 只读，显示精确 snapshot/task diagnostic，不发送 Agent、Provider、execute、accept 或 AssetBible mutation

### Requirement:已接受重拍只提供显式 Timeline replacement handoff
video retake successor 只有在 scenes owner 已确认 accepted-current 且 matching derivative ready 时，UI SHALL 提供到显式 Episode Timeline 的 replacement handoff。handoff MUST 只携带 project/episode/shot、candidate/take、new AssetVersion ID/revision/hash 和 derivative fingerprint；MUST NOT 自动选择 Clip、调用 `ReplaceClipSource`、修改 Cut、发布 TimelineVersion 或启动导出。Timeline owner SHALL 重新查询候选 Clip matches，并要求用户比较 old/new source 后明确确认。

#### Scenario:从已接受且 ready 的重拍进入替换确认
- **WHEN** retake successor 已 accepted-current 且 matching derivative ready，用户点击替换入口
- **THEN** UI 导航到同项目同集 Timeline 并传递 owner references；Timeline 仍要求用户选择精确 Clip 和提交 `ReplaceClipSource`

#### Scenario:重拍未 ready 时不暗示已装入时间线
- **WHEN** retake 未接受、scope/hash/revision 不匹配或 derivative pending/failed/stale
- **THEN** replacement handoff 禁用并显示 owner diagnostic，现有 Clip/Cut/TimelineVersion/ExportJob 保持不变

### Requirement:按 Episode 恢复 active Agent session
Review UI SHALL 以 `projectId + episodeId` 隔离 active session ID 和 presentation-only primary/ref selection。进入、切换或返回 Episode 时 MUST 从 AssetEdit owner 重新验证 session project/Episode/scope/revision、message sequence 与 selection version/hash，合法时恢复相同 session；message/turn/Plan/candidate/decision 正文只从 owner Query 读取，不得复制到持久 UI slice。missing/foreign/stale session 或 selection MUST 原子清除并显示 diagnostic，MUST NOT 使用其他 Episode session 兜底。恢复、清除或浏览 MUST NOT 重发 message、生成/执行 Plan、accept candidate 或触发 Provider/Storage/Timeline mutation。

#### Scenario:返回 Episode 恢复原 Agent 会话
- **WHEN** 用户从 Episode A 的 Review 切到 B 后返回 A，A 的 active session 和 selection 仍通过 owner scope/revision/hash 校验
- **THEN** UI 恢复 A session 并从 owner 继续读取消息/轮次，B session/selection 不可见且没有重复 message、Plan 或收费 operation

#### Scenario:过期或跨集会话不会恢复
- **WHEN** 保存的 active session 不存在、属于其他 project/Episode、revision 已过期或 selection fingerprint 不匹配
- **THEN** UI 清除目标 Episode 的无效引用并显示 owner diagnostic，不借用其他会话、不提交任何业务 mutation

### Requirement:候选审查复用共享基础控件
Review UI SHALL 使用 `shared/ui` 提供的选择器、确认 `Dialog`、`Tabs`、`Tooltip`、`Command` 和 `Toaster`/通知；MUST NOT 再造基础变体、页面级手写 CSS 或第二套组件库。共享控件只承载候选审查领域交互，不改变 owner facts。

#### Scenario:确认候选操作
- **WHEN** 用户比较候选并显式确认 accept、reject 或 retake
- **THEN** 确认 dialog、差异 tabs、诊断 tooltip、命令入口和结果通知均来自共享 UI，且只提交一次 owner command

#### Scenario:页面读取不触发副作用
- **WHEN** 用户打开 Review、切换 tab、使用命令或刷新候选
- **THEN** 共享控件只更新读取/展示状态，不创建 Plan、ProviderCall、Timeline reference 或其他 owner mutation
