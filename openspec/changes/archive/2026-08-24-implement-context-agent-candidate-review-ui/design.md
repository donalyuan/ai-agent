## Context

## canonical review UI

UI action 和 audit projection 只使用 `accept|reject|retake`；legacy/unknown `approve` 显示 owner validation 且零 current/retake side effect。视频显示固定顺序 Provider terminal result candidate -> exact candidate/source/ShotSpec facts accept -> scenes exact current CAS -> MediaInspect/derivatives -> Timeline handoff；derivative pending/failed/stale 只阻断 Timeline/preview/export，不阻断或撤销 accepted/current。AssetEdit `accept` 仅显示 same-version AcceptDecision/audit 与 scenes eligibility CAS，不能暗示产生第二 AssetVersion、bytes/object/ref copy。

`implement-agent-asset-edit-review` 定义 AssetEditSession、Schema-valid AssetEditPlan、candidate、impact/stale 与显式接受；当前前端没有 selection、会话隔离或候选审核界面。本 change 消费 owner 事实，不能把 Agent 对话、AssetVersion、Plan 或接受决定复制到客户端长期状态。

## Goals / Non-Goals

**Goals:**

- 对 image/video AssetEdit 建立每会话一个 primary selection 和显式 refs；story/script 只展示 TextReview successor/stale closure，audio/TimelineVersion 只读取并跳转 Timeline editor typed commands。
- primary selection、base 和 refs 只表达完整 image/video `AssetVersion`；不提供 mask、选区、局部区域、图层或视频/音频时间范围控件。
- 让用户比较有效 Plan、费用、impact/staleTargets 与候选，并以精确引用集合全有或全无 accept 或 reject。
- 显示 AssetBible accepted resolved snapshot 与 continuity task owner projection，在 stale/pending 时阻断 Plan execute/accept，但不在浏览器复制 entry/override/resolver facts。
- 在 accepted-current 且 derivative ready 的视频重拍结果上提供显式 Timeline replacement handoff，由 Timeline owner 再完成 exact Clip compare 与 `ReplaceClipSource`。
- 将 Query、Zustand、Zod、路由、失败恢复和 TDD 边界写成可实现 contract。

**Non-Goals:**

- 不在浏览器实现 Agent 推理、Provider 或 AssetEditPlan 生成逻辑；UI 只显式调用 owner 的对话/Plan command。也不实现资产替换、费用结算、媒体编辑或自动接受。
- 不从当前画布/故事板选择隐式扩大目标；不把上次会话选择泄漏到新项目、集、场、镜头或版本。
- 不在 MVP-A 展示“即将支持”式的可交互 mask/选区/时间范围入口；这些能力延后 MVP-B。

## Decisions

### 1. DDD state boundary and routes

路由为 `/projects/:projectId/review`，以 `sessionId` 及显式 owner scope 参数进入；可执行 AssetEdit 深链的 selection type 仅为完整 image/video AssetVersion，并必须包含 stable owner ID、assetVersionId、revision/hash 与项目归属。story/script/audio/TimelineVersion 深链只允许进入只读 review 或 Timeline editor，不得生成 AssetEditPlan。任何 mask、选区、局部区域、图层或 start/end/time-range 深链参数都显示 `unsupported_feature` 并在 mutation 前停止。Zustand 只保存当前 session 的 `primarySelection`、显式 `refs`、暂存选择和 side-panel layout；切换 project/episode/scene/shot/asset/version 时先清除不再同 scope 的值。Session、Plan、candidate、impact、decision 与 revision 只在 TanStack Query cache 保存。

共享壳层可为每个 `projectId + episodeId` 保存 active session ID 和 presentation-only selection reference；进入或切回 Episode 时 Review 必须从 owner 读取 session scope/revision/message sequence 后再恢复。缓存不得保存消息正文、turn/Plan/candidate/decision 副本；session missing/foreign/stale 或 selection hash/revision 不匹配时，目标 Episode 的恢复值原子清除并显示 diagnostic，其他 Episode 的 session 不得作为 fallback。保存/恢复 active session 是只读 UI 行为，不重发用户消息、不生成 Plan、不 execute/accept 或触发 Provider。

### 2. SDD: owner adapter, DTO and cache

`assetEditReviewApi` 暴露 session read/create、conversation read/create、message/turn list、用户消息发送、Agent 回复读取、从指定 conversation/turn 生成 Plan、Plan read/validate、candidate list、impact read、execute、reject、accept。owner change 已定义资源根为 `/v1/projects/{projectId}/asset-edit-sessions`；UI DTO 必须以 Zod 验证 conversation/session scope、message/turn sequence、`user`/`agent` role、pending/failed/completed 状态、canonical schemaVersion、baseAssetVersionId、plan hash、费用/货币/估计来源、candidate、impact/staleTargets、accepted AssetBible snapshot ID/revision/hash、continuity task status、expectedBaseVersionId、expected target revisions 和 `referenceIds[]`。accept payload 是 `{candidateId, expectedBaseVersionId, expectedPlanRevision, resolvedContinuitySnapshot:{id,revision,hash}, references:[{referenceId, expectedRevision}]}`；无 refs、重复 refs、跨项目 refs、stale snapshot 或范围别名在客户端和 owner 都拒绝。

Query key 含 projectId/sessionId/planId/candidateId/baseAssetVersionId；execute/reject/accept 后只失效该会话和显式 targets。任何 409 `base_version_conflict`、stale 或 revision conflict 都不重试写入：刷新 owner state，标记 plan 过期并要求重新生成/重新选择。

### 3. BDD: preview before side effect

执行按钮只在 Schema-valid、未过期 Plan、明示费用与影响、用户显式确认后可用；selection 或打开页面不触发 Agent/Provider。候选比较显示基础和结果版本、修改摘要、费用、impact/stale。accept 按 owner transaction 语义只接受所有精确 refs 成功的结果，UI 不逐项发送补偿写入；reject 只拒绝指定 candidate/plan。

对话面板以 session scope 隔离消息/轮次：用户输入作为不可变 message 追加，Agent 回复必须带 turn、schema/correlation 与可诊断状态。只有用户明确点击“从本轮生成编辑计划”才调用 owner `generateAssetEditPlanFromConversation`；生成结果先进入 Plan review，不得直接 execute、accept 或替换任何引用。刷新、断线或失败只读取并恢复已持久化消息/轮次，不重复发送用户消息或重新生成可收费计划。

AssetBible 投影与 session/turn/Plan 使用同一 accepted snapshot identity；UI 只显示 resolved chain 的 owner 摘要和 pending/acknowledged/resolved/superseded task 状态。snapshot 或 task 变化时失效当前 Plan/candidate action，并提供回到 Workbench AssetBible/Shot 修订入口；不得由 UI 修改 task、自动重建 Plan 或把旧 snapshot 重基到新版本。

视频 retake successor 被 accept 且 scenes current CAS 成功后，仍须等待 matching derivative ready。两项都成立时，Review 只生成到 `/projects/:projectId/episodes/:episodeId/timeline` 的 replacement handoff，携带 candidate/take、Shot、new AssetVersion ID/revision/hash 与 derivative fingerprint；不携带“当前 Clip”推断。Timeline 页面读取 owner 候选 Clip matches，由用户选择一个精确 Clip、比较 old/new source 后明确提交 `ReplaceClipSource`，再单独发布新 TimelineVersion。

### 4. TDD and compatibility

组件测试覆盖 scope reset、完整版本选择 chips、Plan/fee/impact、disabled state 与 conflict banner；store 测试覆盖跨上下文清除；Zod/adapter contract 测试覆盖 schema invalid、foreign refs、duplicate refs、mask/选区/时间范围 `unsupported_feature` 和 secret-free error rendering；Playwright E2E 覆盖 create -> review -> execute -> accept/reject、局部编辑负例及 409 刷新。默认 fixture MUST 是 `Mock Provider +` 显式 Local test/offline profile（adapter identity=`local_workspace`），与 owner HTTP schema 兼容；AssetVersion 仍只消费 owner ID/revision/schemaVersion，不读取 objectKey 或媒体字节。

## Dependency DAG

```text
AssetVersion + scenes/shots + workflows/runs + AssetBible
                 \             /
          asset edit review owner
                    |
     context agent candidate review UI
                    |
       explicit Timeline replacement handoff
```

## Current / Defined / Todo

- **Current**：无前端 Agent conversation/message/turn、selection/session/Plan/candidate UI；AssetEditPlan 后端也尚未实现。
- **Defined**：会话消息/轮次、从指定 turn 生成 Plan、AssetBible continuity projection、primary/ref 隔离、Plan/成本/影响比较、精确 CAS accept/reject、显式 Timeline replacement handoff 和错误恢复。
- **Todo**：实现 conversation/Plan owner adapter、Query/store、组件、`Mock Provider +` 显式 Local test/offline profile fixtures、E2E 及依赖安装。

## Risks / Trade-offs

- [selection 跨上下文泄漏] -> scope change 原子清空选择并在测试断言。
- [客户端接受部分引用] -> 单一 accept command 和 owner all-or-nothing 响应；失败不补偿。
- [成本或影响陈旧] -> revision/base version 双重检查，409 后强制刷新。
- [自动产生外部副作用] -> execute/probe 只由明确 button command，页面加载零 mutation。
- [把 accepted retake 误当作 Timeline 已替换] -> Review 只生成 owner-validated 深链；Clip 选择、ReplaceClipSource、发布和导出均留在 Timeline UI 的独立确认步骤。

## Migration Plan

先在 `Mock Provider +` 显式 Local test/offline profile 资源上实现独立 review route；owner API 到位后 additive 接入，使用 capability flag 显示 unavailable 而非伪造候选。回滚只移除 UI route/cache，保留 owner session 和资产版本。

## Open Questions

- owner 对只读 TextReview closure 与 Timeline editor command 的最终 DTO 只作为跨 change 读取契约；本 change 不实现这些类型的 Agent edit command。
- cost estimate 的单位/区间和 staleTargets 的分页/刷新语义。

## Acceptance Commands

`openspec validate implement-context-agent-candidate-review-ui --strict --json`、`pnpm --filter @video-agent/web test`、`pnpm --filter @video-agent/web typecheck`、`pnpm --filter @video-agent/web lint`、`pnpm --filter @video-agent/web format:check`、`git diff --check -- openspec/changes/implement-context-agent-candidate-review-ui`。

## Turn-bound pending review

**DDD**：conversation/turn 是 owner facts，UI 只为 image/video 提交冻结 turn-bound plan request。**BDD**：only completed reply 可生成 pending_review plan；story/script/audio/TimelineVersion、foreign/pending/failed 请求拒绝且零 mutation。**SDD**：Zod 验证 turn status、project、selection/refs/base version 与 `runId + nodeRunId + logicalOperation`；兼容 exact CAS accept。**TDD**：先建 turn/type/status/失败 fixtures，再接入 button/diagnostic；非目标是隐式 execute/accept、真实调用或 Timeline 写入。默认使用 `Mock Provider +` 显式 Local test/offline profile，验收使用既有 E2E/strict 命令。

文本审查视图必须显示 successor/stale closure 与 immutable batch，且仅提交 owner 定义的 regenerate input（run/brief/batch expected revisions、source ids/hashes）；它不得在客户端拼接 candidate 或绕过 batch CAS。

### Media candidate review UI boundary

图片生成结果必须显示 candidateId、source/base AssetVersion、result AssetVersion revision/hash、accepted provenance、费用、impact/stale 和 derivative readiness；用户可通过 owner 的单一 image candidate compare/reject/accept command 操作，accept 成功前不显示 storyboard current 或 Agnes submit 可用。视频视图必须显示 `VideoTakeCandidate` revision、ShotSpec/duration/aspect snapshot、normalized metadata、proxy/thumbnail/keyframe/waveform readiness 和 review status，并提供显式 accept/reject/retake；accept 先以 exact candidate/source/ShotSpec facts 请求 scenes current CAS，不能由 derivative 状态阻断或撤销。retake 只提交 successor input 与新 `logicalOperation`，不复用旧确认；未接受或派生未 ready 时 Timeline handoff disabled。页面加载、候选比较和刷新均为读取，不触发 Provider/Worker/accept mutation。

Session/turn/Plan/candidate 还必须显示 AssetBible accepted resolved snapshot ID/revision/hash 与 continuity task 状态。continuity stale/pending 禁用 execute/accept；accepted-current retake 且 derivative ready 后只显示 Timeline replacement handoff，不表示 Clip 已替换或 TimelineVersion 已发布。
