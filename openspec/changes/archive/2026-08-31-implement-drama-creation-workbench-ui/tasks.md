## Stage 1 Drama Creation Workbench Tasks

- [x] 1.1 Owner contracts for Project/CreativeBrief/SourceMaterial/Episode/Scene/Shot/Run/Catalog/AssetBible/TextReview verified and canonical payload frozen.
- [x] 1.2 Approved web dependencies and design primitives are configured.
- [x] 1.3 CreativeBrief/storyboard/workflow/text/run/SSE contracts cover invalid, foreign, stale and zero-mutation states.
- [x] 1.4 `workbenchApi`, Query keys, Local/Mock selection and presentation store keep owner facts out of durable UI state.
- [x] 1.4a Project+Episode presentation slices isolate viewport/filter/selection/session and clear foreign/stale state.
- [x] 1.5 Text Run create/start/regenerate/reconcile/successor contracts bind fixed workflow, source/brief, operation and cost identity.
- [x] 1.5a Historical RunInputSnapshot rerun contracts preserve exact inputs and create a new operation.
- [x] 1.6 BudgetGate confirmation fixtures cover unknown cost, threshold and fingerprint/user binding.
- [x] 1.7 Original/adaptation and SourceMaterial inline/uploaded owner binding is implemented without fake storage or reupload.
- [x] 1.8 Run/NodeRun detail/cancel DTOs preserve safe summaries, event sequence, failure/retryability and cancel races.
- [x] 1.9 Shared shell and ShotCard contracts preserve owner IDs/revisions/hashes/status and zero-episode behavior.
- [x] 1.10 AssetBible entry/version/assignment/snapshot/impact/task/ack fixtures enforce all-or-nothing owner acceptance.
- [x] 2.1 Workbench routes, fixed published workflow source and zero-episode/unavailable states are implemented.
- [x] 2.2 CreativeBrief form uses canonical Zod fields, expected revision and 409 refresh.
- [x] 2.3 Storyboard/workflow projections preserve stable IDs and scoped reorder/read-only boundaries.
- [x] 2.4 TextReview view displays candidate closure and uses explicit accept/reject/retake confirmation with owner gate.
- [x] 2.5 Run panel and owner recovery paths expose status, SSE/diagnostic contract and no implicit mutation.
- [x] 2.6 Brief-to-Run commands remain explicit, recoverable and never silently replace predecessor state.
- [x] 2.6a Failed successor flow creates new IDs/operations and preserves reuse evidence.
- [x] 2.6b Skill route candidates require explicit human selection and frozen revision.
- [x] 2.7 Budget gate and unknown-cost media entry remain blocked until explicit owner confirmation.
- [x] 2.8 Creation/source flows distinguish original/adaptation and inline/uploaded paths.
- [x] 2.9 Episode/scene/shot presentation state is scoped and read-only projection operations do not mutate owners.
- [x] 2.10 Run detail cancellation is explicit, single-shot and terminal-state safe.
- [x] 2.11 ShotCard exposes media/review/derivative status and owner-validated Review/Assets/Timeline actions.
- [x] 2.12 Shared project shell connects Workbench/Review/Assets/Timeline/Exports/Settings with scoped navigation.
- [x] 2.13 AssetBible view exposes entries/overrides/snapshot/impact/task and explicit owner acceptance.
- [x] 2.14 ShotCard shows continuity snapshot/chain/task and blocks dependent actions when stale/pending.
- [x] 3.1 Existing Tailwind/Lucide/Radix-compatible controls provide keyboard/focus/aria states.
- [x] 3.2 Browser navigation evidence covers project, source/brief, workflow/run, text review, AssetBible, media review, assets, timeline, exports and settings.
- [x] 3.3 Web tests/typecheck/lint/format, API tests, strict validation and diff checks pass.
- [x] 4.1 Project list/create/edit/If-Match/explicit selection is implemented.
- [x] 4.2 Zero-episode and explicit Episode selection are implemented; foreign deep links are diagnosed.
- [x] 4.3 Workflow view is published/read-only and does not author graph nodes.
- [x] 4.4 Shared Playwright/Mock+Local harness evidence is recorded through CLI navigation.
- [x] 4.5 E2E-MVPA-001 evidence stages and focused failures are recorded in `docs/evidence`.
- [x] 4.6 Text successor/stale closure and image accepted provenance fixtures are present in owner/UI contracts.
- [x] 4.6a Initial AssetBible specs/ack/snapshot/task gate is represented in Workbench/Review projections.
- [x] 4.7 Explicit project creation/source/brief/run binding and no-reupload boundary are covered.
- [x] 4.8 Cross-page navigation, Run cancellation and late-result diagnostics are covered by owner contracts.
- [x] 4.9 Read-only performance and localhost-only runtime constraints remain documented and verified by Compose health.

## 5. 共享组件基线与只读投影

- [x] 5.1 在 `shared/ui` 建立 shadcn/Radix 源码型基础变体、Tailwind/CSS Variables 语义 tokens、Lucide 图标和可访问性测试；禁止第二套组件库与页面级手写 CSS。
- [x] 5.2 封装有限 `react-resizable-panels`、Dialog/Tabs/Tooltip/Command/Toaster、DataTable/Form 与 VirtualList，定义稳定尺寸、键盘/ARIA 和业务页面只消费的导出边界。
- [x] 5.3 实现固定 published WorkflowVersion 的 300-node React Flow 只读投影及 TanStack Virtual 列表/日志投影；证明移动、连线、删除、保存、发布和版本升级为 MVP-B 且零写入。
- [x] 5.4 让 Review、Timeline、Provider/Model/Skill 和 Assets 的组件测试引用同一 shared/ui 基线，并以浏览器网络断言确认加载、导航、筛选和详情不产生 owner mutation。
