# 交付验证

验证日期：2026-07-28

## 结论

本 change 的代码、数据库、零费用 E2E、故障恢复和跨服务回归已通过，OpenSpec 实现可以进入归档准备。当前结论不是“可直接生产部署”：真实模型 EvalRun 与真实视频/TTS/ASR/媒体分析验收未获用户成本授权，v3 角色仍为 candidate，真实 `MediaEvidenceProvider` 能力缺失时必须 fail-closed。

## 已运行验证

| 范围 | 命令/套件 | 结果 |
|---|---|---|
| Rust 格式 | `cargo fmt --all -- --check` | 通过 |
| Rust 构建 | `CARGO_INCREMENTAL=0 cargo build --workspace` | 通过 |
| Rust 全量 | `cargo test --workspace` | 所有可运行测试通过；共 5 个显式 ignored：1 个开发库 migration、1 个真实 provider、3 个 legacy orchestrator DB integration，相应 durable PostgreSQL 路径由临时库合同测试覆盖 |
| 数据库 replay | `backend/tests/database_migrations.rs` | `5 passed, 1 ignored`；临时空库与 legacy 升级均通过，ignored 项只针对配置中的开发库 |
| Full Crew Runner | `backend/tests/production_runner_contract.rs` | `2 passed`；PostgreSQL 恢复扫描、Redis 不可用容错、Gate package 单次 claim、重复 tick 和优雅停机通过 |
| 编排资源计量 | `production_repository_contract`、`durable_workflow_contract`、Video Worker `tests` | role retry、quality rework、视频任务/时长、TTS、ASR、并发、provider retry 和不确定结果预占/结算/释放全部通过，副作用前超限阻断保持有效 |
| Package provenance | `production_repository_contract::brief_and_script_package_builders_use_only_current_exact_role_attempt` | 通过；Brief/Script 只收当前 Run、revision epoch、role step、attempt 的精确产物，跨 Run/旧 epoch/错误 cardinality 被拒 |
| Append-only 保护 | `database_migrations` 与 `production_repository_contract` | 新增脚本/package invalidation、TakeReview-Ledger 映射、ContinuityLedger/TakeReview trigger 存在；直接 UPDATE/DELETE 被拒 |
| 11.2 零费用 E2E | `zero_cost_full_crew_e2e_reaches_completion_with_bounded_rework_policy` | 通过；Run=`completed`、quality=`approved`、模型调用 `0`、画面任务 `0` |
| 11.3 专项矩阵 | repository、recovery、routes、role prepare、promotion、durable workflow、application port | `76/76` 通过 |
| Video Worker | `pytest tests -q` | `196 passed`；只有既有 FastAPI `on_event` deprecation warning |
| Pi Runtime | `npm run lint`、`npm test`、`npm run build` | lint/build 通过，12 个测试文件 `76/76` 通过 |
| Fast Lane | `orchestrator::route::tests::test_fast_lane_route` | 通过，未复制 Full Crew durable 流程 |
| 发布运营 | publication database/domain/routes 与 work-library handoff | 全部通过，未创建自动发布旁路 |

Pi Runtime 生产镜像按设计仅安装 runtime dependencies，不包含 `typescript`/`vitest`；本次在仓库 `services/agent-runtime` 的完整 dev dependencies 下运行 lint/test/build，生产容器只承担运行时启动。未修改仓库所有权或生成文件。

## 零真实调用与候选状态

- `production_candidate_eval_authorization::unconfirmed_real_eval_plan_lists_exact_limits_without_calls_or_activation` 证明 `eval_runs=0`、provider calls=`0`，且 v3 九个角色全部保持 `candidate`。
- `production_role_prepare_contract::current_candidate_registry_never_enters_normal_runs_or_mutates_frozen_bindings` 证明历史 v2 Run binding 不漂移；当前 active v2 output schema 不满足 `durable-role-output-contract@1` 时，普通新 Run 在模型调用前稳定 fail-closed，不能回退为旧 schema 执行。
- `production_media_validation_authorization::real_media_validation_requires_exact_approval_and_complete_capabilities` 证明授权规划不创建 WorkGeneration、speech、asset 或 media evidence 记录。
- 零费用 Full Crew E2E 使用 fake generation 与 fake media provider，并断言 `model_calls=0`、`asset_generation_tasks=0`。
- `production_role_prepare_contract::current_candidate_registry_never_enters_normal_runs_or_mutates_frozen_bindings` 证明 candidate 零次生产执行，发布索引变化也不改写既有 binding。

待授权真实 EvalRun 的固定上限清单已经存在：18 个 eval run/case、最大输入 token `294912`、最大输出 token `50000`、重试 `0`、最大成本 `1800000` micros，并绑定精确模型 behavior fingerprint 与 authorization digest。只有用户明确确认完全一致的摘要后才能生成 EvalReport。

## 部署阻断

1. `production.producer/screenwriter/character_critic/director/cinematographer/performance_director/sound_director/editor/qc@3.0.0` 尚未产生符合门禁的真实不可变 EvalReport，状态必须保持 `candidate`，不得用于普通 ProductionRun。
2. 部署环境必须提供版本化且可审计的真实 `MediaEvidenceProvider`，同时证明 vision 与 audio/ASR 能力；缺失时 Editor/QC 保持 `capability/evidence blocker`。
3. 真实视频、TTS、ASR 与媒体分析仍需独立的精确授权摘要。当前验证未发出任何真实付费请求。
4. 本 change 不包含 Full Crew 审批前端；正式操作者体验仍需独立 Pencil 原型与 OpenSpec change。

在以上阻断解除前，只能声明 durable backend 与零费用合同完成，不能声明 Full Crew 生产可用，也不能激活 candidate。
