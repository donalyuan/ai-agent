## Stage 1 Provider/Model/Skill Settings Tasks

- [x] 1.1 Catalog, project threshold, workflow, provider/profile/model/skill/capability, credentials, sync, probe, quota and retention owner contracts verified.
- [x] 1.2 Settings Query state contains only drafts/tab/diff selection; plaintext credentials never enter localStorage, Query cache, logs or diagnostics.
- [x] 1.3 Zod/owner fixtures cover stale revision, unconfigured profile, unknown cost, threshold, quota, hold and redacted secret responses.
- [x] 1.4 `settingsApi` uses explicit Local/Mock profile, recursive owner mapping, redacted errors, Query invalidation and zero-mutation reads.
- [x] 2.1 Provider/profile/skill settings route displays owner IDs, revisions, provenance, approval and unavailable state.
- [x] 2.1a StorageProfile route consumes owner fields, masked credential status and project scope.
- [x] 2.2 Credential replacement is one-shot/password input, clears after submit and shows only masked/unconfigured status.
- [x] 2.3 Model sync is explicit and exposes candidate diff for human decision; no auto-accept occurs.
- [x] 2.4 Effective provider/profile values are revision-bound owner values; invalid/foreign updates are rejected by owner CAS.
- [x] 2.5 Probe is an explicit command and preserves unconfigured/auth/network/503 diagnostics without fallback or object mutation.
- [x] 2.5a Storage connection-test includes profile revision and probe correlation, preserves revision and unconfigured result.
- [x] 2.6 Budget and operation identity remain owner-bound; unknown cost requires explicit confirmation and no duplicate operation.
- [x] 2.11 Skill runtime/provenance remains separate from settings display and never resolves or launches a Run implicitly.
- [x] 2.7 Retention/hold facts remain read-only owner diagnostics and are not silently cleared.
- [x] 2.8 Provider/profile/model lifecycle commands use expected revision and refresh after 409; history is not overwritten.
- [x] 2.9 Operation policy/quota facts remain read-only or explicit owner commands; unknown quota is not inferred as available.
- [x] 2.10 Historical model references remain disable-only when delete proof is unavailable.
- [x] 3.1 Settings forms, dialogs, controls, Lucide icons, keyboard labels and responsive layout use existing design conventions.
- [x] 3.2 Browser evidence verifies settings load, redaction, explicit probe/sync and no plaintext or implicit Provider mutation.
- [x] 3.2a Lifecycle/CAS fixtures cover stale/foreign/409 zero-write and immutable SkillRevision/CapabilitySnapshot history.
- [x] 3.2b Storage fields, masked status, connection-test failure and disabled/unconfigured/master-key diagnostics are covered by owner tests/UI.
- [x] 3.3 Web/API tests, typecheck/lint/format, strict validation and diff checks pass.
- [x] 4.1 Default settings uses Mock Provider + Local offline (`local_workspace`) and read-only load.
- [x] 4.2 Live probe remains explicit `unconfigured` when provider/storage/renderer credentials are absent.
- [x] 4.3 Catalog CAS, redaction, master-key 503 and recoverable credential failure fixtures pass.

## 5. 表格与动态表单验收

- [x] 5.1 使用 TanStack Table + shared/ui DataTable 展示 Provider/Model/Skill revision、provenance、approval、enabled、policy 和 quota；覆盖稳定列、筛选、unknown/empty/error 状态。
- [x] 5.2 使用 React Hook Form + Zod 按 owner schema 生成动态参数表单，覆盖字段级错误、错误聚焦、未知 schema、dirty/submit、expectedRevision/If-Match 和 409 refetch。
- [x] 5.3 证明表格/表单只消费 shared/ui，不新增第二组件库或页面 CSS；读取、筛选、表单渲染和 quota 刷新零 ProviderCall/probe/Run mutation。
- [x] 5.4 运行设置 focused E2E、Web typecheck/lint/format、本 change strict validation 与 `git diff --check`；所有任务保持未勾选直至验收。
