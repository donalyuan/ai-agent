## Why

## review projection

视频 UI 只展示并提交 Provider terminal result candidate -> `accept|reject|retake` -> scenes exact current CAS -> MediaInspect/derivatives -> Timeline handoff 的 owner 状态；derivative pending/failed/stale 仅禁用 Timeline/preview/export，不表示撤销 accepted/current。`approve` 或未知 verb 只显示 validation，绝不产生 current/retake side effect。AssetEdit `accept` 复用同一已存在 AssetVersion，只显示 AcceptDecision/audit 与 eligibility CAS，不创建第二 version 或复制 bytes/object/ref。

素材上下文 Agent 的价值取决于它是否只携带用户明确选择的上下文，并把昂贵或影响广泛的修改保留为可检查候选。当前工作台没有会话隔离、EditPlan 审核、候选比较或精确引用替换的 UI，因此需要定义一个不自动接受、不泄漏上下文的前端闭环。

## What Changes

- 新增画布/故事板对 image/video AssetEdit 的 primary selection 和显式 refs 选择模型；story/script 仅进入 TextReview successor/stale closure，audio/Timeline 仅进入 Timeline editor 读取/命令入口，不在本 change 声称可编辑。
- MVP-A 选择器只允许选择完整的 image/video `AssetVersion`；不展示 mask、图片选区/局部区域、图层或视频/音频时间范围编辑控件。深链、恢复状态或 owner DTO 含这些字段时显示 `unsupported_feature`，且不提交 Plan/execute/Provider mutation。
- 新增 Schema-valid image/video AssetEditPlan、预计费用、impactAnalysis、staleTargets、候选比较、执行、reject 与全有或全无 accept 的审核界面。
- 在会话、Plan 和候选旁展示 AssetBible owner 的 accepted resolved snapshot ID/revision/hash、resolved chain 摘要和 `ContinuityRevisionTask` 状态；continuity stale/pending 时禁用 execute/accept，UI 不复制或修改 AssetBible 事实。
- 覆盖图片生成后的 Candidate compare/accept/reject 与视频 `VideoTakeCandidate` 的 pending review、accept/reject/retake、derivative readiness 和 Timeline gate；UI 按 owner API 区分 AssetEdit candidate、image storyboard eligibility 与 video take review，不把不同 candidate namespace 混用。
- 视频重拍 successor 只有在 accepted-current 且 derivative ready 后，才显示到目标 Episode Timeline 的 `ReplaceClipSource` 深链；该深链只携带 owner stable IDs/revisions/hashes/fingerprint，不能自动选择 Clip、替换 Cut、发布 TimelineVersion 或导出。
- 新增会话级 Agent 对话 UI：消息/轮次、用户输入、Agent 回复、加载/失败状态，以及从指定对话显式生成 Schema-valid `AssetEditPlan` 的入口；对话不会绕过计划审核直接执行。
- active Agent session 按 `projectId + episodeId` 隔离恢复：从 Workbench 切集或返回 Review 时只恢复目标 Episode 的合法 session/selection 引用，消息/轮次继续从 owner 读取；foreign/stale session 清除且不得借用其他 Episode 上下文。
- 将 revision conflict、候选过期、无效计划和网络失败显示为可恢复状态；接受范围只能是精确引用集合，绝不默认扩大。
- 规定 React Router、TanStack Query、Zustand、Zod 与 owner API DTO 的边界；不拥有 Agent、素材编辑、资产或工作流后端聚合。

## Capabilities

### New Capabilities
- `context-agent-candidate-review-ui`: 上下文绑定、候选审查与显式引用集合接受的桌面 UI contract。

### Modified Capabilities
- 无。

## Impact

- 后续实现将修改 `apps/web`，并消费 `implement-agent-asset-edit-review`、`implement-asset-bible-continuity-slice`、`implement-scenes-shots-storyboard-slice`、`implement-workflows-runs-slice`、`implement-episode-timeline-audio-export` 和已归档 AssetVersion 的 owner contract。
- 默认使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）数据；真实 Agent、Provider 或付费操作只能由明确用户操作触发的后端 command 启动，不能由 selection 或页面加载触发，Local 也不是 TOS 失败 fallback。

## Turn-bound Plan 闭合

用户可显式从同项目 completed Agent reply turn 为完整 image/video AssetVersion 创建 `pending_review` AssetEditPlan，冻结 turn/selection/refs/base versions 与 `runId + nodeRunId + logicalOperation`；story/script/audio/TimelineVersion 只能进入各自 owner surface，foreign/pending/failed turn 以及 mask/选区/时间范围输入必须拒绝。该操作不 execute/accept，不创建 AssetVersion 或 Timeline reference。

## 文本 stale UI 合同

**DDD**：UI 只显示 owner 的 successor/closure/batch 状态。**BDD**：stale closure 的 regenerate 与 partial/foreign/duplicate 拒绝可见。**SDD**：请求携带 source ids/hashes、expected revisions，不自行推断依赖。**TDD**：覆盖 immutability、全有或全无接受和零媒体副作用。

## 共享 UI 使用边界

本 change MUST 只消费创作工作台提供的 `shared/ui` 基线：选择器、确认 `Dialog`、`Tabs`、`Tooltip`、`Command` 和 `Toaster`/通知均复用既有封装，不再定义基础变体、第二套 token 或页面级 CSS。领域代码只负责会话、计划、候选比较、accept/reject/retake、错误恢复和 Timeline replacement handoff。

验收必须证明 selection/session scope reset、确认 dialog、候选 tabs、命令入口和通知均由共享组件呈现；打开 Review、切换项目/Episode、候选刷新和 409 恢复不得产生隐式 Agent/Provider/Timeline mutation。任何 mask/选区/时间范围编辑仍显示 `unsupported_feature`，不以共享控件绕过 MVP-A 边界。
