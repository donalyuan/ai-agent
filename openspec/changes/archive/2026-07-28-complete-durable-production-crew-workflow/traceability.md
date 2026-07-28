# Requirement / Scenario 追踪表

状态说明：除明确标为“门禁合同”的项外，下表证据均已在本轮自动化回归中通过。真实模型或媒体 provider 场景只验证授权与 fail-closed 合同，未执行真实调用。

证据简称：

- `PR`：`backend/tests/production_repository_contract.rs`
- `RR`：`backend/tests/production_recovery_contract.rs`
- `RUN`：`backend/tests/production_runner_contract.rs`
- `PA`：`backend/tests/production_routes.rs` 与 production DTO unit tests
- `PE`：`backend/tests/production_role_prepare_contract.rs`
- `SP`：`backend/tests/script_package_promotion_contract.rs`
- `DW`：`crates/novex-production-crew/tests/durable_workflow_contract.rs`
- `AP`：`crates/novex-production-crew/tests/production_application_port_contract.rs`
- `RO`：`crates/novex-production-crew/tests/role_output_contract.rs`
- `EA/MA`：production candidate/media authorization tests
- `DB`：`backend/tests/database_migrations.rs`
- `WL/WG`：Work Library、WorkGeneration backend/Worker tests

## content-topic-management

| Requirement | Scenario | 自动化证据 |
|---|---|---|
| Full Crew 必须受控使用已确认选题 | 从 approved 选题启动 Full Crew | `PR::zero_cost_full_crew_e2e_reaches_completion_with_bounded_rework_policy` |
| Full Crew 必须受控使用已确认选题 | 非 approved 或跨项目选题被拒绝 | `PA::full_crew_http_rejects_invalid_sources_plan_overrides_and_role_bypasses` |
| Full Crew 必须受控使用已确认选题 | 同一选题已有 active Full Crew | `PR::command_digest_active_intent_and_single_run_constraints_reject_conflicts` |
| Full Crew 必须受控使用已确认选题 | ScriptPackage 晋升成功 | `SP::approved_script_package_is_promoted_with_script_scenes_links_and_topic_atomically` |
| Full Crew 必须受控使用已确认选题 | ScriptPackage 晋升失败 | `SP::database_failure_rolls_back_every_promotion_side_effect` |
| Full Crew 必须受控使用已确认选题 | 选题已被其他脚本消费 | `SP::cross_project_deleted_and_consumed_topics_are_rejected_without_partial_writes` |
| Full Crew 活跃期间必须锁定来源选题 | 活跃制作期间编辑选题 | `PR::source_lifecycle_matrix_is_fail_closed_and_safe_failure_releases_the_lock` |
| Full Crew 活跃期间必须锁定来源选题 | 活跃制作期间软删除选题 | `PR::active_source_is_locked_and_safe_cancellation_releases_the_topic` |
| Full Crew 活跃期间必须锁定来源选题 | 安全终止后重新制作 | `PR::active_source_is_locked_and_safe_cancellation_releases_the_topic` |

## durable-production-crew-workflow

| Requirement | Scenario | 自动化证据 |
|---|---|---|
| Full Crew 必须绑定 active 账号和唯一 active 制作意图 | 从有效选题创建制作意图 | `PR::intent_run_and_steps_are_created_atomically_and_reconstructed_from_postgres` |
| Full Crew 必须绑定 active 账号和唯一 active 制作意图 | 无效来源不能创建 Full Crew | `PA::full_crew_http_rejects_invalid_sources_plan_overrides_and_role_bypasses` |
| Full Crew 必须绑定 active 账号和唯一 active 制作意图 | 同一选题并发创建制作意图 | `PR::concurrent_intent_and_step_claim_have_single_winners` |
| Full Crew 必须绑定 active 账号和唯一 active 制作意图 | 同一制作意图重复创建 Run | `PR::command_digest_active_intent_and_single_run_constraints_reject_conflicts` |
| Full Crew 执行计划必须版本化并在 Run 创建时冻结 | 创建固定计划 Run | `DW::full_crew_v1_plan_is_fixed_versioned_and_deterministic`、`PR::intent_run_and_steps_are_created_atomically_and_reconstructed_from_postgres` |
| Full Crew 执行计划必须版本化并在 Run 创建时冻结 | 客户端试图改写计划 | `PA::full_crew_http_rejects_invalid_sources_plan_overrides_and_role_bypasses` |
| Full Crew 执行计划必须版本化并在 Run 创建时冻结 | 发布新计划版本 | `DW::full_crew_v1_plan_is_fixed_versioned_and_deterministic`、`PE::current_candidate_registry_never_enters_normal_runs_or_mutates_frozen_bindings` |
| PostgreSQL 必须是流程状态唯一事实源 | 服务重启后恢复流程 | `RR::approval_and_external_wait_states_are_reconstructed_without_redis_payload`、`RUN::runner_survives_redis_loss_and_stops_gracefully_without_duplicate_attempts` |
| PostgreSQL 必须是流程状态唯一事实源 | 收到重复唤醒消息 | `RR::redis_loss_duplicate_delivery_and_process_restart_preserve_postgres_truth`、`RUN::runner_recovery_executes_durable_domain_step_once` |
| PostgreSQL 必须是流程状态唯一事实源 | Redis 消息丢失 | `RR::redis_loss_duplicate_delivery_and_process_restart_preserve_postgres_truth`、`RUN::runner_survives_redis_loss_and_stops_gracefully_without_duplicate_attempts` |
| Full Crew Runner 必须完成进程级闭环 | 恢复扫描、claim/执行/finalize、Gate/domain/external-wait 调度和优雅停机 | `RUN::runner_recovery_executes_durable_domain_step_once`、`RUN::runner_survives_redis_loss_and_stops_gracefully_without_duplicate_attempts` |
| Step 推进必须具备并发租约和不确定副作用语义 | 两个 worker 并发认领 | `PR::concurrent_intent_and_step_claim_have_single_winners` |
| Step 推进必须具备并发租约和不确定副作用语义 | 纯数据库步骤租约过期 | `PR::leases_can_be_renewed_released_and_safely_reclaimed_without_live_attempt_leaks` |
| Step 推进必须具备并发租约和不确定副作用语义 | 外部调用结果不确定 | `PE::uncertain_provider_result_closes_audit_and_holds_resources_for_manual_attention` |
| 包级 Gate 必须绑定精确产物版本和 digest | 批准当前 package | `PR::package_gate_is_digest_bound_idempotent_and_unlocks_only_exact_successor`、`RUN::runner_recovery_executes_durable_domain_step_once` |
| 包级 Gate 必须绑定精确产物版本和 digest | 使用过期 package digest 审批 | `PR::package_gate_is_digest_bound_idempotent_and_unlocks_only_exact_successor` |
| 包级 Gate 必须绑定精确产物版本和 digest | 单产物 approved 不得替代包级审批 | `PR::package_gate_is_digest_bound_idempotent_and_unlocks_only_exact_successor` |
| 角色协作不得越过产物所有权 | 摄影指导提出阻断建议 | `PR::collaboration_suggestions_bind_source_audit_and_have_immutable_responses` |
| 角色协作不得越过产物所有权 | 接受建议后生成 owner 新版本 | `PR::production_package_builder_uses_exact_run_epoch_step_attempt_and_formal_script` |
| 角色协作不得越过产物所有权 | 拒绝建议 | `PR::collaboration_suggestions_bind_source_audit_and_have_immutable_responses` |
| ResourceSafetyGate 必须治理非金额资源 | 资源额度充足 | `PR::trusted_usage_is_settled_and_unused_reservations_are_released`、`PR::zero_cost_full_crew_e2e_reaches_completion_with_bounded_rework_policy` |
| ResourceSafetyGate 必须治理非金额资源 | 资源限制超出 | `DW::orchestration_resource_limit_blocks_model_side_effect_before_invocation`、`PR::resource_reservations_are_atomic_bounded_and_hold_unknown_results` |
| ResourceSafetyGate 必须治理非金额资源 | 查询资源审计 | `PR::resource_and_audit_snapshots_reject_pricing_and_credentials` |
| 编排层资源计量必须覆盖重试、返工和外部副作用 | role retry、quality rework、视频/TTS/ASR、并发、provider retry 及不确定结果 | `PR::role_retry_is_atomically_metered_idempotent_and_bounded`、`PR::resource_reservations_are_atomic_bounded_and_hold_unknown_results`、`DW::resource_gate_reserves_before_side_effect_and_holds_unknown_results` |
| Role step 必须原子保存产物并闭合审计终态 | 多产物角色执行成功 | `PE::successful_finalize_is_atomic_and_replays_the_same_attempt_result` |
| Role step 必须原子保存产物并闭合审计终态 | 输出 schema 不合法 | `PE::provider_parse_and_schema_failures_close_every_audit_anchor_without_artifacts` |
| Role step 必须原子保存产物并闭合审计终态 | finalize 重复提交 | `PE::successful_finalize_is_atomic_and_replays_the_same_attempt_result` |
| 正式角色执行不得绕过计划与 Gate | 执行当前合法角色 | `PE::prepare_uses_exact_package_and_persists_all_pre_provider_anchors` |
| 正式角色执行不得绕过计划与 Gate | 越级执行角色 | `PA::full_crew_http_rejects_invalid_sources_plan_overrides_and_role_bypasses` |
| 作品生成必须作为现有领域外部等待步骤接入 | 等待主画面完整 | `PR::missing_scene_visual_manifest_blocks_before_work_plan_run_or_provider_tasks` |
| 作品生成必须作为现有领域外部等待步骤接入 | 创建既有 WorkPlan | `PR::typed_production_package_plans_existing_work_with_auditable_sources_and_invalidation` |
| 作品生成必须作为现有领域外部等待步骤接入 | 等待作品运行终态 | `PR::existing_manual_confirmation_is_idempotent_and_enters_production_external_wait` |
| Editor 和 QC 必须基于真实媒体证据且 fail-closed | 使用完整媒体证据评审 | `PR::zero_cost_full_crew_e2e_reaches_completion_with_bounded_rework_policy` |
| Editor 和 QC 必须基于真实媒体证据且 fail-closed | 缺少媒体或能力 | `PE::editor_without_media_evidence_closes_the_owned_attempt_before_model_call`、`MA::real_media_validation_requires_exact_approval_and_complete_capabilities` |
| Editor 和 QC 必须基于真实媒体证据且 fail-closed | 空评审集合 | `DW::media_inventory_and_quality_gate_require_exact_current_coverage` |
| QC 返工必须派生新作品版本并受次数限制 | 局部 QC 返工 | `PR::controlled_media_provider_persists_only_immutable_redacted_snapshots` |
| QC 返工必须派生新作品版本并受次数限制 | 全局 QC 返工 | `AP::work_version_rework_reference_is_typed_digest_bound_and_requires_confirmation`、`WL::version_derivation_diff_and_confirmation_are_immutable_stale_safe_and_idempotent` |
| QC 返工必须派生新作品版本并受次数限制 | 达到返工上限 | `PR::controlled_media_provider_persists_only_immutable_redacted_snapshots` |
| 流程 API 必须返回可操作状态和审计关联 | 查询等待审批的 Run | `PA::production_run_status_exposes_waits_blockers_retryability_commands_and_audit_ids` |
| 流程 API 必须返回可操作状态和审计关联 | 异步命令被接受 | `PA::full_crew_http_commands_create_query_decide_resume_and_retry_durable_state` |
| 流程 API 必须返回可操作状态和审计关联 | 查询失败或注意状态 | `PA::production_run_status_exposes_waits_blockers_retryability_commands_and_audit_ids` |
| 行为变化角色必须通过版本化评测门禁 | 只完成零费用验证 | `EA::unconfirmed_real_eval_plan_lists_exact_limits_without_calls_or_activation` |
| 行为变化角色必须通过版本化评测门禁 | candidate 通过完整门禁 | `definition_release_repository::published_candidate_requires_an_exact_eval_report_before_activation`；门禁合同通过，v3 真实评测未授权、未激活 |
| 活跃 Full Crew 必须锁定来源生命周期 | 活跃流程期间修改或删除选题 | `PR::source_lifecycle_matrix_is_fail_closed_and_safe_failure_releases_the_lock` |
| 活跃 Full Crew 必须锁定来源生命周期 | 活跃流程期间归档账号 | `PR::source_lifecycle_matrix_is_fail_closed_and_safe_failure_releases_the_lock` |
| 活跃 Full Crew 必须锁定来源生命周期 | 终止后释放选题锁 | `PR::active_source_is_locked_and_safe_cancellation_releases_the_topic` |
| Gate reject 必须创建有界修订 epoch | BriefPackage 被拒绝 | `PR::all_package_reject_paths_are_append_only_owner_scoped_and_bounded` |
| Gate reject 必须创建有界修订 epoch | ScriptPackage 被拒绝 | `PR::all_package_reject_paths_are_append_only_owner_scoped_and_bounded` |
| Gate reject 必须创建有界修订 epoch | ProductionPackage 被拒绝 | `PR::all_package_reject_paths_are_append_only_owner_scoped_and_bounded` |
| Gate reject 必须创建有界修订 epoch | 达到 package 修订上限 | `PR::all_package_reject_paths_are_append_only_owner_scoped_and_bounded` |
| 脚本语义修订必须回流到新的 ScriptPackage | Director 提出脚本语义变更 | `SP::semantic_revision_creates_child_script_and_invalidates_only_unconfirmed_downstream` |
| 脚本语义修订必须回流到新的 ScriptPackage | 仅修改制作表达 | `SP::production_expression_revision_reopens_only_owner_without_creating_script_version` |
| Run 取消和 ProductionProject 删除必须保留审计真实性 | 在纯等待步骤取消 | `PR::active_source_is_locked_and_safe_cancellation_releases_the_topic` |
| Run 取消和 ProductionProject 删除必须保留审计真实性 | 外部调用期间取消 | `PR::cancellation_coordinator_calls_work_port_once_and_waits_for_a_true_terminal_result` |
| Run 取消和 ProductionProject 删除必须保留审计真实性 | 删除有历史的制作意图 | `PA::production_cancel_delete_and_archive_preserve_auditable_history` |
| Brief/Script package builder 必须使用当前精确来源 | current revision、step、attempt、跨 Run 和 cardinality | `PR::brief_and_script_package_builders_use_only_current_exact_role_attempt` |
| ProductionPackage 必须使用真实引用并满足集合完整性 | ProductionPackage 引用完整 | `PR::production_package_builder_uses_exact_run_epoch_step_attempt_and_formal_script` |
| ProductionPackage 必须使用真实引用并满足集合完整性 | 自由字符串或跨 Script 引用 | `SP::director_scene_references_accept_only_current_formal_script_scenes`、`DW::production_package_requires_closed_typed_scene_character_shot_sets` |
| ProductionPackage 必须使用真实引用并满足集合完整性 | Package 集合覆盖不完整 | `DW::production_package_requires_closed_typed_scene_character_shot_sets` |
| QualityPackage 必须基于确定性 take inventory 和追加版本 | 建立 required take inventory | `PR::required_take_inventory_builder_uses_exact_compose_chain_plan_order_and_package_shots` |
| QualityPackage 必须基于确定性 take inventory 和追加版本 | 质量产物跨版本或重复覆盖 | `PR::quality_package_isolates_work_versions_inventory_reviews_and_old_decisions` |
| QualityPackage 必须基于确定性 take inventory 和追加版本 | QC 返工后的新作品版本 | `PR::controlled_media_provider_persists_only_immutable_redacted_snapshots`、`PR::quality_package_isolates_work_versions_inventory_reviews_and_old_decisions` |
| WorkGeneration 外部终态必须显式映射 | 作品运行失败 | `AP::work_generation_terminal_state_mapping_is_fail_closed_until_media_evidence_is_complete` |
| WorkGeneration 外部终态必须显式映射 | 作品提交结果不确定 | `PR::controlled_media_provider_persists_only_immutable_redacted_snapshots` |
| WorkGeneration 外部终态必须显式映射 | 作品运行被独立取消 | `DW::external_terminal_mapping_is_fail_closed_and_reports_stable_waiting_reasons` |
| Production 命令必须治理 actor、幂等和动态输入 | 相同命令幂等重放 | `PA::production_api_uses_stable_actor_and_scopes_idempotency_by_command_and_aggregate` |
| Production 命令必须治理 actor、幂等和动态输入 | 相同 key 提交不同 payload | `PR::unified_production_command_store_scopes_replay_and_digest_conflicts` |
| Production 命令必须治理 actor、幂等和动态输入 | 请求覆盖模型或注入任意 context | production DTO `production_commands_reject_plan_role_model_actor_and_context_overrides`、`PA::full_crew_http_rejects_invalid_sources_plan_overrides_and_role_bypasses` |
| Production 命令必须治理 actor、幂等和动态输入 | 计划允许用户补充指令 | `DW::revision_directives_are_fixed_bounded_and_keep_old_epochs_immutable`、RoleExecutor `durable_revision_instruction_is_compiled_as_audited_user_context` |

## script-agent-mvp

| Requirement | Scenario | 自动化证据 |
|---|---|---|
| Full Crew ScriptPackage 必须确定性晋升为正式脚本 | 编剧输出满足正式字段契约 | `RO::every_full_crew_role_accepts_a_complete_typed_fixture`、`DW::script_mapper_is_zero_call_deterministic_and_requires_complete_domain_fields` |
| Full Crew ScriptPackage 必须确定性晋升为正式脚本 | 正式字段缺失 | `RO::invalid_role_outputs_fail_closed_on_fields_references_order_and_duration` |
| Full Crew ScriptPackage 必须确定性晋升为正式脚本 | 批准 ScriptPackage 后晋升 | `SP::approved_script_package_is_promoted_with_script_scenes_links_and_topic_atomically` |
| Full Crew ScriptPackage 必须确定性晋升为正式脚本 | 晋升操作重复提交 | `SP::repeated_and_concurrent_promotion_return_the_original_script` |
| Full Crew ScriptPackage 必须确定性晋升为正式脚本 | 旧 ScriptPackage 不得晋升 | `SP::stale_package_is_rejected_when_a_constituent_has_a_newer_version` |
| Full Crew 后续产物不得静默修改已批准脚本 | ShotContract 引用正式 Scene | `SP::director_scene_references_accept_only_current_formal_script_scenes` |
| Full Crew 后续产物不得静默修改已批准脚本 | 导演修改已批准脚本语义 | `SP::semantic_revision_creates_child_script_and_invalidates_only_unconfirmed_downstream` |
| Full Crew 后续产物不得静默修改已批准脚本 | 脚本语义修订必须重新形成 ScriptPackage | `SP::semantic_revision_creates_child_script_and_invalidates_only_unconfirmed_downstream` |
| Full Crew 后续产物不得静默修改已批准脚本 | 新脚本晋升使旧下游失效 | `SP::semantic_revision_creates_child_script_and_invalidates_only_unconfirmed_downstream` |
| Full Crew ScriptPackage reject 必须保持来源和事务边界 | 拒绝后生成新 ScriptPackage | `PR::all_package_reject_paths_are_append_only_owner_scoped_and_bounded` |
| Full Crew ScriptPackage reject 必须保持来源和事务边界 | 重放旧 ScriptPackage approval | `PR::all_package_reject_paths_are_append_only_owner_scoped_and_bounded`、`SP::stale_package_is_rejected_when_a_constituent_has_a_newer_version` |

## work-generation

| Requirement | Scenario | 自动化证据 |
|---|---|---|
| 已批准 ProductionPackage 必须复用现有作品计划链路 | 主画面不完整时等待 | `PR::missing_scene_visual_manifest_blocks_before_work_plan_run_or_provider_tasks` |
| 已批准 ProductionPackage 必须复用现有作品计划链路 | 从 ProductionPackage 创建 WorkPlan | `PR::typed_production_package_plans_existing_work_with_auditable_sources_and_invalidation` |
| 已批准 ProductionPackage 必须复用现有作品计划链路 | ProductionPackage 变化使旧计划失效 | `PR::typed_production_package_plans_existing_work_with_auditable_sources_and_invalidation` |
| 已批准 ProductionPackage 必须复用现有作品计划链路 | 操作者修改 Full Crew 下游方案 | `PR::typed_production_package_plans_existing_work_with_auditable_sources_and_invalidation`、`WL::version_derivation_diff_and_confirmation_are_immutable_stale_safe_and_idempotent` |
| Full Crew 作品运行必须继续人工确认非金额资源 | 确认 Full Crew 作品计划 | `PR::existing_manual_confirmation_is_idempotent_and_enters_production_external_wait` |
| Full Crew 作品运行必须继续人工确认非金额资源 | 相同确认重复提交 | `PR::existing_manual_confirmation_is_idempotent_and_enters_production_external_wait` |
| Full Crew 作品运行必须继续人工确认非金额资源 | 资源限制不满足 | `DW::orchestration_resource_limit_blocks_model_side_effect_before_invocation`、Worker `test_work_generation.py` 全量合同 |
| Full Crew QC 返工必须遵守作品版本治理 | 局部返工派生 edit 版本 | `PR::controlled_media_provider_persists_only_immutable_redacted_snapshots` |
| Full Crew QC 返工必须遵守作品版本治理 | 全局返工派生 full regeneration 版本 | `AP::work_version_rework_reference_is_typed_digest_bound_and_requires_confirmation`、`WL::version_derivation_diff_and_confirmation_are_immutable_stale_safe_and_idempotent` |
| Full Crew QC 返工必须遵守作品版本治理 | QC 不通过不得伪装作品生成失败或成功批准 | `PR::controlled_media_provider_persists_only_immutable_redacted_snapshots` |
| WorkGenerationRun 技术终态必须真实传播到 Full Crew | 作品运行失败 | `AP::work_generation_terminal_state_mapping_is_fail_closed_until_media_evidence_is_complete` |
| WorkGenerationRun 技术终态必须真实传播到 Full Crew | 作品运行需要人工处理 | `DW::external_terminal_mapping_is_fail_closed_and_reports_stable_waiting_reasons` |
| WorkGenerationRun 技术终态必须真实传播到 Full Crew | 作品运行成功但成片证据不完整 | `AP::work_generation_terminal_state_mapping_is_fail_closed_until_media_evidence_is_complete` |
| WorkGenerationRun 技术终态必须真实传播到 Full Crew | Full Crew 请求取消作品运行 | `PR::cancellation_coordinator_calls_work_port_once_and_waits_for_a_true_terminal_result` |
| Full Crew 作品幂等必须校验请求内容 | 相同 key 确认不同计划修订 | `WL::version_derivation_diff_and_confirmation_are_immutable_stale_safe_and_idempotent` |
| Full Crew 作品幂等必须校验请求内容 | 相同 key 和相同计划重放 | `PR::existing_manual_confirmation_is_idempotent_and_enters_production_external_wait` |

## 关键横切证明

| 必证项 | 自动化证据 |
|---|---|
| candidate 零次生产执行 | `EA::unconfirmed_real_eval_plan_lists_exact_limits_without_calls_or_activation`；`PE::current_candidate_registry_never_enters_normal_runs_or_mutates_frozen_bindings` |
| 旧 revision 不串用 | `DW::revision_directives_are_fixed_bounded_and_keep_old_epochs_immutable`；`PR::all_package_reject_paths_are_append_only_owner_scoped_and_bounded` |
| 旧 WorkVersion 不串用 | `PR::quality_package_isolates_work_versions_inventory_reviews_and_old_decisions` |
| 任意 context/model override 被拒 | production DTO unit test；`PA::full_crew_http_rejects_invalid_sources_plan_overrides_and_role_bypasses` |
| actor 与幂等冲突 | `PA::production_api_uses_stable_actor_and_scopes_idempotency_by_command_and_aggregate`；`PR::unified_production_command_store_scopes_replay_and_digest_conflicts` |
| 删除后审计保留 | `PA::production_cancel_delete_and_archive_preserve_auditable_history`；`DW::intent_delete_and_archive_commands_preserve_history` |
| 新增失效事实与 TakeReview-Ledger 映射不可变 | `database_migrations` trigger/replay assertions；`PR::quality_package_isolates_work_versions_inventory_reviews_and_old_decisions` 的直接 mutation rejection |
| 脚本/package 失效与质量映射必须数据库级 append-only | migration replay、直接 UPDATE/DELETE 拒绝和跨版本映射隔离 | `DB::migrations_create_durable_full_crew_schema`、`PR::quality_package_isolates_work_versions_inventory_reviews_and_old_decisions`、`PR::all_package_reject_paths_are_append_only_owner_scoped_and_bounded` |
