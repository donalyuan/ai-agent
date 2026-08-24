## Phase One Coordination Tasks

追溯说明：2.2 实施 `implement-workflows-runs-slice`、`2.2` 的 child owner 任务与共享退出任务 `5.1`、`5.2`、`5.3`、`5.5` 已在对应 child `tasks.md` 和验证测试中闭环；本文件只协调 DAG，不承载业务实现。

- [x] 1. Foundation and architecture DAG verified against repository code, schemas, migrations and Compose runtime.
- [x] 2. Project/Episode creative owner, SourceMaterial and fixed Workflow/Run contracts implemented and tested.
- [x] 3. Structured text, AgentScope, Skill routing, TextReview closure and AssetBible continuity implemented and tested.
- [x] 4. Scene/Shot storyboard, AssetEdit owner, image/video provider operations and exact candidate CAS implemented and tested.
- [x] 5. Storage/TOS boundary, Local multipart, AssetVersion registration, Timeline/Media/Export owner and security contracts implemented and tested.
- [x] 6.1 `implement-drama-creation-workbench-ui` implemented, browser navigated and tasks/validation closed.
- [x] 6.2 `implement-context-agent-candidate-review-ui` implemented, browser navigated and tasks/validation closed.
- [x] 6.3 `implement-episode-timeline-editor-ui` implemented, browser navigated and tasks/validation closed.
- [x] 6.4 `implement-provider-model-skill-settings-ui` implemented, browser navigated and tasks/validation closed.
- [x] 6.5 `implement-project-asset-center` implemented with 2 GiB evidence and owner isolation.
- [x] 6.6 `implement-operations-resilience` implemented with threshold, recovery and checksum/ETag evidence.
- [x] 6.7 `implement-local-observability` implemented with W3C lineage, redaction, metrics and exporter-failure evidence.
- [x] 7.1 E2E-MVPA-001 evidence matrix and focused failure/no-side-effect records are maintained in `docs/evidence`.
- [x] 7.2 Mock and explicit live probe cardinality/profile boundaries are documented; missing external credentials remain `unconfigured`.
- [x] 7.3 Playwright CLI harness, Compose lifecycle and localhost-only runtime verification are available.
- [x] 7.4 Focused failure matrix covers CAS, scope, stale, capability, redaction, renderer/storage and no-duplicate-operation paths.
- [x] 7.5 Canonical RenderPlan/preview/export owner parity contracts and diagnostics are verified by API tests.
- [x] 7.6 Owner/spec/task traceability and no-GC/retention boundaries are recorded in ADRs and project memory.
- [x] 7.7 All active changes are status/strict-validated with tasks synchronized and `git diff --check` clean for touched artifacts.
- [x] 7.8 StorageProfile and SourceMaterial owner CRUD/handoff/recovery contracts are covered by tests and UI routes.
- [x] 7.9 Localhost performance/health and real navigation evidence are recorded without claiming provider/render success.
- [x] 7.10 2 GiB multipart and observability/resilience evidence files are present and tied to owner facts.
- [x] 7.11 Web/API/Worker/Compose listen only on localhost/127.0.0.1; LAN auth and public deployment remain MVP-B.
