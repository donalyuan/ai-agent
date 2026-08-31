## 1. Owner Contract and Isolation Foundation

- [x] 1.1 取证 asset-edit review、Scene/Shot、WorkflowRun 与 AssetVersion owner 的已实现 API/DTO，确认 `/v1/projects/{projectId}/asset-edit-sessions`、target reference、费用、impact/stale 和 409 语义；缺失项显式阻塞。
- [x] 1.2 在已批准的前端基础上定义 primary selection、explicit refs、session/candidate/plan/impact Query key 与 Zustand scope-reset 边界，不保存 AssetVersion objectKey、媒体 bytes 或第二领域副本。
- [x] 1.2a 定义 `projectId + episodeId` active session/presentation selection slice 与 owner revalidation 恢复算法；覆盖切集往返、message sequence、selection revision/hash、missing/foreign/stale 清除、跨集无 fallback 和恢复零 message/Plan/Provider mutation。
- [x] 1.3 先编写 AssetEditPlan、Candidate、Impact、AcceptDecision Zod fixtures，覆盖 schema invalid、foreign/duplicate refs、base/version conflict 和 secret-free diagnostics。
- [x] 1.4 实现 `assetEditReviewApi` 的 Mock Provider + 显式 Local test/offline profile adapter（adapter identity=`local_workspace`）、Query invalidation 和纯读取路由加载；证明 selection/page load 不会执行 Agent、Provider、accept mutation 或 profile 切换。
- [x] 1.5 定义 conversation/session、message、turn、user input、Agent reply 的 Zod/owner DTO fixtures，绑定 project/scope/session、role、sequence、correlation 和状态；覆盖 foreign session、重复 sequence、刷新恢复和 secret-free diagnostic。
- [x] 1.6 定义 AssetBible accepted resolved snapshot/continuity task 与 Timeline replacement handoff Zod fixtures；覆盖 snapshot stale/foreign/hash-revision mismatch、pending task、retake accepted-current/derivative ready 和深链 scope，禁止 UI 复制/写入 entry/override 或推断 Clip。

## 2. Review and Decision Interaction

- [x] 2.1 实现 `/projects/:projectId/review`、深链 selection 解析及 project/episode/scene/shot/asset/version 切换时 primary/ref 原子清除。
- [x] 2.1a 接入共享壳层的 Episode active session handoff；返回目标 Episode 时只恢复 owner 校验通过的 session/selection，并从 Query 读取消息/轮次，不持久化 owner 正文或重复发送。
- [x] 2.2 先以 component/state 测试覆盖后实现完整 image/video AssetVersion 的 primary selection 与显式 ref 选择器；显示 owner ID/revision/hash，不展示 mask、选区、局部区域、图层或时间范围控件；story/script/audio/TimelineVersion 只显示只读 owner handoff 或 Timeline editor 入口，并断言不会生成 AssetEditPlan。
- [x] 2.3 先以 contract/BDD 测试覆盖后实现 Schema-valid Plan、费用、工具摘要、impact/staleTargets、基础/候选版本比较和过期禁用状态。
- [x] 2.4 先以 mutation 测试覆盖后实现 explicit execute 和 candidate reject，保留原始 owner 错误且不修改基础版本。
- [x] 2.5 先以 all-or-nothing/409 BDD 覆盖后实现单一 accept command，提交精确 refs 和 expected revisions，冲突时刷新/重新生成而不拆分或重试部分写入。
- [x] 2.6 先以对话状态/重复提交失败测试覆盖后实现 conversation panel、消息/轮次列表、用户输入、Agent 回复和断线/失败恢复；发送必须是明确 command，页面加载不产生 Agent mutation。
- [x] 2.7 先以 Plan schema/turn binding 测试覆盖后实现从指定 conversation/turn 生成 image/video `AssetEditPlan` 的显式入口；只创建待审核 Plan，不直接 execute/accept，绑定 selection/refs/base version 与 `runId + nodeRunId + logicalOperation`；story/script/audio/TimelineVersion 生成请求必须稳定拒绝并零副作用。
- [x] 2.8 先以 image/video candidate contract 与 review state tests 覆盖后实现图片 compare/accept/reject、视频 `VideoTakeCandidate` pending review/accept/reject/retake、derivative readiness 与 Timeline/Agnes gate；retake 绑定新 logical operation，候选 namespace 不混用，页面读取与比较零 Provider/Worker mutation。
- [x] 2.9 在 session/turn/Plan/candidate 显示 AssetBible snapshot/continuity task；stale/pending 时禁用 Plan generation/execute/accept。accepted-current 且 derivative ready 的 retake 只生成显式 Episode Timeline replacement handoff，不自动选择 Clip、调用 ReplaceClipSource、发布或导出。

## 3. Accessibility and Verification

- [x] 3.1 使用 shadcn/Radix、Tailwind、Lucide 完成可键盘操作的选择、diff、确认 dialog 和错误状态，避免自动默认范围与嵌套卡片。
- [x] 3.2 添加 Playwright E2E 入口，覆盖 scope leakage 防护、AssetBible snapshot/task stale gate、完整 AssetVersion 选择、mask/选区/局部区域/时间范围 `unsupported_feature` 且零 Plan/execute/Provider mutation、Plan 无效、执行显式确认、`accept|reject|retake`、legacy/unknown `approve` validation 零副作用、revision conflict 和网络失败；视频严格 Provider terminal result candidate -> exact candidate/source/ShotSpec accept -> scenes exact current CAS -> MediaInspect/derivatives -> 显式 Timeline replacement handoff，derivative pending/failed/stale 仅阻断 Timeline/preview/export，不阻断或撤销 accepted/current，AssetEdit accept 零第二 AssetVersion/bytes/object/ref copy；验证 handoff 不自动选择 Clip/ReplaceClipSource/publish/export；默认使用 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`）。
- [x] 3.3 运行定向 contract/component/store/E2E tests、Web typecheck/lint/format check、`openspec instructions apply --change "implement-context-agent-candidate-review-ui" --json`、strict validations 与 `git diff --check`；全部完成前保持 task 未勾选。

## 4. Turn-bound Plan

- [x] 4.1 添加 completed Agent reply turn -> 完整 image/video AssetVersion pending_review AssetEditPlan 的 request/response fixtures，冻结 turn/selection/refs/base versions 与 `runId + nodeRunId + logicalOperation`；覆盖 story/script/audio/TimelineVersion、mask/选区/局部区域/时间范围稳定拒绝或只读 owner 跳转且零写入。
- [x] 4.2 添加 foreign/pending/failed turn rejection 与 zero AssetVersion/Timeline mutation tests；明确 plan 创建不 execute/accept。
- [x] 4.3 添加 Text candidate successor/stale closure/regenerate UI fixtures，覆盖 partial/stale/foreign/duplicate batch diagnostics、immutable old batch 与 CAS all-or-nothing acceptance。

## 6. 共享 UI 复用验收

- [x] 6.1 使用创作工作台导出的 shared/ui 选择器、确认 Dialog、Tabs、Tooltip、Command 和 Toaster 完成候选审查；不新增基础变体、主题或页面级 CSS。
- [x] 6.2 添加键盘/ARIA、焦点回收、scope reset、候选 tabs、诊断 tooltip、命令导航和成功/失败通知测试；证明打开/刷新/切换不会触发隐式 mutation。
- [x] 6.3 运行本 change strict validation、共享组件复用检查和 Review focused E2E；保留 `unsupported_feature`、409、stale/foreign 与零副作用证据。
