## 阶段 1 协调任务

追溯说明：2.2 实施 `implement-workflows-runs-slice`、`2.2` 的 child owner 任务与共享退出任务 `5.1`、`5.2`、`5.3`、`5.5` 已在对应 child `tasks.md` 和验证测试中闭环；本文件只协调 DAG，不承载业务实现。

- [x] 1. 已依据仓库代码、Schema、迁移和 Compose 运行时核验工程基线与架构 DAG。
- [x] 2. 已实现并测试 Project/Episode 创作 owner、SourceMaterial 及固定的 Workflow/Run 合同。
- [x] 3. 已实现并测试结构化文本、AgentScope、Skill 路由、TextReview 收口和 AssetBible 连续性。
- [x] 4. 已实现并测试 Scene/Shot 故事板、AssetEdit owner、图片/视频 Provider operation 与精确候选 CAS。
- [x] 5. 已实现并测试 Storage/TOS 边界、Local multipart、AssetVersion 登记、Timeline/Media/Export owner 与安全合同。
- [x] 6.1 已实现 `implement-drama-creation-workbench-ui`，完成浏览器导航及任务/验证收口。
- [x] 6.2 已实现 `implement-context-agent-candidate-review-ui`，完成浏览器导航及任务/验证收口。
- [x] 6.3 已实现 `implement-episode-timeline-editor-ui`，完成浏览器导航及任务/验证收口。
- [x] 6.4 已实现 `implement-provider-model-skill-settings-ui`，完成浏览器导航及任务/验证收口。
- [x] 6.5 已实现 `implement-project-asset-center`，并保留 2 GiB 证据与 owner 隔离。
- [x] 6.6 已实现 `implement-operations-resilience`，并保留阈值、恢复和 checksum/ETag 证据。
- [x] 6.7 已实现 `implement-local-observability`，并保留 W3C lineage、脱敏、metrics 和 exporter failure 证据。
- [x] 7.1 在 `docs/evidence` 中维护 `E2E-MVPA-001` 证据矩阵及聚焦失败/无副作用记录。
- [x] 7.2 记录 Mock 与显式 live probe 的基数/profile 边界；缺少外部凭据时保持 `unconfigured`。
- [x] 7.3 提供 Playwright CLI harness、Compose 生命周期与 localhost-only 运行时验证。
- [x] 7.4 聚焦失败矩阵覆盖 CAS、scope、stale、capability、脱敏、renderer/storage 和不重复操作路径。
- [x] 7.5 通过 API 测试验证 canonical RenderPlan/preview/export 的 owner 一致性合同和诊断。
- [x] 7.6 在 ADR 与项目记忆中记录 owner/spec/task 可追溯性及 no-GC/retention 边界。
- [x] 7.7 所有 active change 均完成 status/strict validation 且任务同步，受影响 artifacts 的 `git diff --check` 保持干净。
- [x] 7.8 通过测试与 UI 路由覆盖 StorageProfile 和 SourceMaterial owner 的 CRUD/handoff/recovery 合同。
- [x] 7.9 记录 localhost 性能/健康检查和真实导航证据，但不宣称 Provider/render 成功。
- [x] 7.10 2 GiB multipart 与 observability/resilience 证据文件存在且可追溯到 owner 事实。
- [x] 7.11 Web/API/Worker/Compose 仅监听 localhost/127.0.0.1；LAN auth 与公开部署仍属于 MVP-B。
- [x] 7.12 `fix-phase-one-review-vulnerabilities` 已关闭经确认的阶段一 scope、CAS、运行完整性、Storage binding 和 no-GC review 发现。
