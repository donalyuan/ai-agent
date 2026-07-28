-- Durable Full Crew workflow. PostgreSQL owns all authoritative workflow state;
-- Redis messages may only carry run_id/step_id wake-up identities.

-- Preserve the first production-crew schema as readable history. Bound Full Crew
-- intents created after this migration must carry real project/topic identities.
ALTER TABLE production_projects
    ADD COLUMN project_id UUID REFERENCES projects(id) ON DELETE RESTRICT,
    ADD COLUMN topic_id UUID REFERENCES content_topics(id) ON DELETE RESTRICT,
    ADD COLUMN source_snapshot JSONB,
    ADD COLUMN source_fingerprint CHAR(64),
    ADD COLUMN source_locked_at TIMESTAMPTZ,
    ADD COLUMN script_promoted_at TIMESTAMPTZ,
    ADD COLUMN archived_at TIMESTAMPTZ;

ALTER TABLE production_projects DROP CONSTRAINT production_projects_status_check;
UPDATE production_projects
SET status = 'legacy_unbound'
WHERE project_id IS NULL OR topic_id IS NULL;
ALTER TABLE production_projects
    ADD CONSTRAINT production_projects_status_check CHECK (
        status IN (
            'legacy_unbound', 'created', 'active', 'briefing', 'scripting',
            'directing', 'generating', 'editing', 'qc', 'waiting_approval',
            'external_wait', 'attention_required', 'cancelling', 'cancelled',
            'failed', 'completed', 'approved', 'published', 'archived'
        )
    ),
    ADD CONSTRAINT production_projects_source_fingerprint_check CHECK (
        source_fingerprint IS NULL OR source_fingerprint ~ '^[0-9a-f]{64}$'
    ),
    ADD CONSTRAINT production_projects_bound_full_crew_check CHECK (
        project_type <> 'full_crew'
        OR status = 'legacy_unbound'
        OR (
            project_id IS NOT NULL
            AND topic_id IS NOT NULL
            AND source_snapshot IS NOT NULL
            AND jsonb_typeof(source_snapshot) = 'object'
            AND source_fingerprint IS NOT NULL
            AND source_locked_at IS NOT NULL
        )
    );

CREATE UNIQUE INDEX production_projects_one_active_intent_per_topic
    ON production_projects(topic_id)
    WHERE project_type = 'full_crew'
      AND topic_id IS NOT NULL
      AND archived_at IS NULL
      AND status IN (
          'created', 'active', 'briefing', 'scripting', 'directing',
          'generating', 'editing', 'qc', 'waiting_approval', 'external_wait',
          'attention_required', 'cancelling'
      );
CREATE INDEX idx_production_projects_source
    ON production_projects(project_id, topic_id, created_at DESC)
    WHERE status <> 'legacy_unbound';

COMMENT ON COLUMN production_projects.project_id IS 'Full Crew 制作意图绑定的真实内容项目；NULL 仅表示 legacy_unbound 历史。';
COMMENT ON COLUMN production_projects.topic_id IS 'Full Crew 制作意图锁定的 approved Topic；NULL 仅表示 legacy_unbound 历史。';
COMMENT ON COLUMN production_projects.source_snapshot IS '创建制作意图时不可变的项目策略与 Topic 来源快照。';
COMMENT ON COLUMN production_projects.source_fingerprint IS '项目身份与 Topic 可变业务字段的 canonical SHA-256。';

CREATE TABLE production_plan_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_key VARCHAR(120) NOT NULL,
    plan_version VARCHAR(32) NOT NULL,
    plan_digest CHAR(64) NOT NULL,
    plan JSONB NOT NULL,
    role_bindings JSONB NOT NULL,
    resource_limits JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_plan_snapshots_digest_check CHECK (plan_digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT production_plan_snapshots_json_check CHECK (
        jsonb_typeof(plan) = 'object'
        AND jsonb_typeof(role_bindings) = 'object'
        AND jsonb_typeof(resource_limits) = 'object'
    ),
    CONSTRAINT production_plan_snapshots_identity_unique UNIQUE (plan_key, plan_version, plan_digest)
);
COMMENT ON TABLE production_plan_snapshots IS 'Run 创建时冻结的固定 DAG、active Definition/model binding 和非金额资源限制。';

CREATE TABLE production_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    production_project_id UUID NOT NULL REFERENCES production_projects(id) ON DELETE RESTRICT,
    plan_snapshot_id UUID NOT NULL REFERENCES production_plan_snapshots(id) ON DELETE RESTRICT,
    status VARCHAR(32) NOT NULL DEFAULT 'created',
    quality_status VARCHAR(32) NOT NULL DEFAULT 'not_started',
    current_revision_epoch INT NOT NULL DEFAULT 0,
    resource_limits JSONB NOT NULL,
    binding_snapshot JSONB NOT NULL,
    source_snapshot JSONB NOT NULL,
    cancellation_intent JSONB,
    error_code VARCHAR(120),
    error_details JSONB,
    actor_type VARCHAR(32) NOT NULL,
    actor_id VARCHAR(120) NOT NULL,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_runs_status_check CHECK (
        status IN (
            'created', 'queued', 'running', 'waiting_approval', 'external_wait',
            'blocked', 'attention_required', 'cancelling', 'cancelled',
            'failed', 'completed'
        )
    ),
    CONSTRAINT production_runs_quality_status_check CHECK (
        quality_status IN ('not_started', 'reviewing', 'approved', 'rejected', 'needs_revision')
    ),
    CONSTRAINT production_runs_epoch_check CHECK (current_revision_epoch >= 0),
    CONSTRAINT production_runs_json_check CHECK (
        jsonb_typeof(resource_limits) = 'object'
        AND jsonb_typeof(binding_snapshot) = 'object'
        AND jsonb_typeof(source_snapshot) = 'object'
        AND (cancellation_intent IS NULL OR jsonb_typeof(cancellation_intent) = 'object')
        AND (error_details IS NULL OR jsonb_typeof(error_details) = 'object')
    ),
    CONSTRAINT production_runs_actor_check CHECK (
        actor_type = 'local_operator' AND length(btrim(actor_id)) > 0
    )
);
CREATE UNIQUE INDEX production_runs_one_per_intent ON production_runs(production_project_id);
CREATE INDEX idx_production_runs_recovery
    ON production_runs(status, updated_at)
    WHERE status IN ('queued', 'running', 'external_wait', 'cancelling');
COMMENT ON TABLE production_runs IS 'Full Crew 固定计划的一次持久执行；v1 每个制作意图最多一个 Run。';
COMMENT ON COLUMN production_runs.resource_limits IS '只包含调用、token、任务、时长、字符、并发和重试上限，不含金额。';
COMMENT ON COLUMN production_runs.cancellation_intent IS '取消先持久化意图；外部副作用确定停止后才可进入 cancelled。';

CREATE TABLE production_revision_epochs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    epoch INT NOT NULL,
    reason_type VARCHAR(40) NOT NULL,
    reason TEXT NOT NULL,
    affected_owners JSONB NOT NULL,
    source_package_id UUID,
    actor_type VARCHAR(32) NOT NULL,
    actor_id VARCHAR(120) NOT NULL,
    instruction_digest CHAR(64),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_revision_epochs_epoch_check CHECK (epoch >= 0),
    CONSTRAINT production_revision_epochs_reason_check CHECK (
        reason_type IN ('initial', 'brief_reject', 'script_reject', 'production_reject', 'script_semantic_revision', 'quality_rework')
        AND length(btrim(reason)) > 0
    ),
    CONSTRAINT production_revision_epochs_json_check CHECK (jsonb_typeof(affected_owners) = 'array'),
    CONSTRAINT production_revision_epochs_actor_check CHECK (
        actor_type = 'local_operator' AND length(btrim(actor_id)) > 0
    ),
    CONSTRAINT production_revision_epochs_instruction_digest_check CHECK (
        instruction_digest IS NULL OR instruction_digest ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT production_revision_epochs_run_epoch_unique UNIQUE (run_id, epoch)
);
COMMENT ON TABLE production_revision_epochs IS 'reject、语义回流和返工形成的 append-only 修订纪元。';

CREATE TABLE production_steps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    revision_epoch INT NOT NULL,
    plan_order INT NOT NULL,
    step_key VARCHAR(120) NOT NULL,
    step_type VARCHAR(32) NOT NULL,
    role_key VARCHAR(80),
    dependencies JSONB NOT NULL DEFAULT '[]'::jsonb,
    status VARCHAR(32) NOT NULL DEFAULT 'blocked',
    waiting_reason VARCHAR(120),
    error_code VARCHAR(120),
    error_details JSONB,
    retryable BOOLEAN NOT NULL DEFAULT FALSE,
    attempt INT NOT NULL DEFAULT 0,
    lease_owner VARCHAR(160),
    lease_expires_at TIMESTAMPTZ,
    side_effect_state VARCHAR(32) NOT NULL DEFAULT 'none',
    input_package_id UUID,
    input_digest CHAR(64),
    output_digest CHAR(64),
    agent_run_id UUID REFERENCES agent_runs(id) ON DELETE RESTRICT,
    model_call_id UUID REFERENCES model_calls(id) ON DELETE RESTRICT,
    context_snapshot_id UUID REFERENCES context_snapshots(id) ON DELETE RESTRICT,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_steps_epoch_fk FOREIGN KEY (run_id, revision_epoch)
        REFERENCES production_revision_epochs(run_id, epoch) ON DELETE RESTRICT,
    CONSTRAINT production_steps_type_check CHECK (
        step_type IN ('role', 'gate', 'domain_command', 'external_wait')
    ),
    CONSTRAINT production_steps_role_check CHECK (
        (step_type = 'role' AND role_key IS NOT NULL)
        OR (step_type <> 'role' AND role_key IS NULL)
    ),
    CONSTRAINT production_steps_status_check CHECK (
        status IN (
            'blocked', 'queued', 'running', 'waiting_approval', 'external_wait',
            'succeeded', 'failed', 'attention_required', 'cancelling', 'cancelled', 'superseded'
        )
    ),
    CONSTRAINT production_steps_attempt_check CHECK (attempt >= 0),
    CONSTRAINT production_steps_plan_order_check CHECK (plan_order >= 0),
    CONSTRAINT production_steps_dependencies_check CHECK (jsonb_typeof(dependencies) = 'array'),
    CONSTRAINT production_steps_error_details_check CHECK (
        error_details IS NULL OR jsonb_typeof(error_details) = 'object'
    ),
    CONSTRAINT production_steps_lease_pair_check CHECK (
        (lease_owner IS NULL AND lease_expires_at IS NULL)
        OR (lease_owner IS NOT NULL AND lease_expires_at IS NOT NULL)
    ),
    CONSTRAINT production_steps_side_effect_check CHECK (
        side_effect_state IN ('none', 'prepared', 'submitted', 'confirmed', 'unknown')
    ),
    CONSTRAINT production_steps_digest_check CHECK (
        (input_digest IS NULL OR input_digest ~ '^[0-9a-f]{64}$')
        AND (output_digest IS NULL OR output_digest ~ '^[0-9a-f]{64}$')
    ),
    CONSTRAINT production_steps_identity_unique UNIQUE (run_id, revision_epoch, step_key),
    CONSTRAINT production_steps_order_unique UNIQUE (run_id, revision_epoch, plan_order)
);
CREATE UNIQUE INDEX production_steps_one_active_lease
    ON production_steps(id) WHERE status = 'running' AND lease_owner IS NOT NULL;
CREATE INDEX idx_production_steps_recovery
    ON production_steps(status, lease_expires_at, created_at)
    WHERE status IN ('queued', 'running', 'external_wait');
COMMENT ON TABLE production_steps IS '计划冻结后的 role、gate、domain_command 和 external_wait 步骤事实。';
COMMENT ON COLUMN production_steps.side_effect_state IS 'unknown 表示外部提交结果不确定，禁止租约过期后透明重试。';

CREATE TABLE production_step_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    step_id UUID NOT NULL REFERENCES production_steps(id) ON DELETE RESTRICT,
    attempt_no INT NOT NULL,
    status VARCHAR(32) NOT NULL,
    request_digest CHAR(64) NOT NULL,
    idempotency_key VARCHAR(200) NOT NULL,
    lease_owner VARCHAR(160) NOT NULL,
    side_effect_state VARCHAR(32) NOT NULL DEFAULT 'none',
    agent_run_id UUID REFERENCES agent_runs(id) ON DELETE RESTRICT,
    model_call_id UUID REFERENCES model_calls(id) ON DELETE RESTRICT,
    context_snapshot_id UUID REFERENCES context_snapshots(id) ON DELETE RESTRICT,
    result JSONB,
    error_details JSONB,
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    CONSTRAINT production_step_attempts_no_check CHECK (attempt_no > 0),
    CONSTRAINT production_step_attempts_status_check CHECK (
        status IN ('prepared', 'running', 'succeeded', 'failed', 'attention_required', 'cancelled')
    ),
    CONSTRAINT production_step_attempts_digest_check CHECK (request_digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT production_step_attempts_side_effect_check CHECK (
        side_effect_state IN ('none', 'prepared', 'submitted', 'confirmed', 'unknown')
    ),
    CONSTRAINT production_step_attempts_payload_check CHECK (
        (result IS NULL OR jsonb_typeof(result) = 'object')
        AND (error_details IS NULL OR jsonb_typeof(error_details) = 'object')
    ),
    CONSTRAINT production_step_attempts_identity_unique UNIQUE (step_id, attempt_no),
    CONSTRAINT production_step_attempts_idempotency_unique UNIQUE (step_id, idempotency_key, request_digest)
);
CREATE UNIQUE INDEX production_step_attempts_one_in_flight
    ON production_step_attempts(step_id)
    WHERE status IN ('prepared', 'running', 'attention_required');

CREATE TABLE artifact_package_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    source_step_id UUID NOT NULL REFERENCES production_steps(id) ON DELETE RESTRICT,
    source_attempt INT NOT NULL,
    revision_epoch INT NOT NULL,
    package_type VARCHAR(40) NOT NULL,
    package_version INT NOT NULL,
    package_digest CHAR(64) NOT NULL,
    schema_version VARCHAR(32) NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT artifact_package_snapshots_epoch_fk FOREIGN KEY (run_id, revision_epoch)
        REFERENCES production_revision_epochs(run_id, epoch) ON DELETE RESTRICT,
    CONSTRAINT artifact_package_snapshots_type_check CHECK (
        package_type IN ('brief', 'script', 'production', 'quality')
    ),
    CONSTRAINT artifact_package_snapshots_version_check CHECK (
        source_attempt > 0 AND package_version > 0
    ),
    CONSTRAINT artifact_package_snapshots_digest_check CHECK (package_digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT artifact_package_snapshots_metadata_check CHECK (jsonb_typeof(metadata) = 'object'),
    CONSTRAINT artifact_package_snapshots_identity_unique UNIQUE (run_id, package_type, revision_epoch, package_version),
    CONSTRAINT artifact_package_snapshots_digest_unique UNIQUE (run_id, package_digest),
    CONSTRAINT artifact_package_snapshots_id_run_unique UNIQUE (id, run_id)
);
COMMENT ON TABLE artifact_package_snapshots IS '绑定精确产物版本、来源 attempt 和 canonical digest 的不可变 Gate 检查点。';

ALTER TABLE production_revision_epochs
    ADD CONSTRAINT production_revision_epochs_source_package_fk
    FOREIGN KEY (source_package_id, run_id)
    REFERENCES artifact_package_snapshots(id, run_id) ON DELETE RESTRICT;

CREATE TABLE artifact_package_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    package_id UUID NOT NULL REFERENCES artifact_package_snapshots(id) ON DELETE RESTRICT,
    ordinal INT NOT NULL,
    artifact_type VARCHAR(60) NOT NULL,
    artifact_id UUID NOT NULL,
    artifact_version INT NOT NULL,
    content_digest CHAR(64) NOT NULL,
    source_step_id UUID NOT NULL REFERENCES production_steps(id) ON DELETE RESTRICT,
    source_attempt INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT artifact_package_items_ordinal_check CHECK (ordinal >= 0),
    CONSTRAINT artifact_package_items_version_check CHECK (artifact_version > 0 AND source_attempt > 0),
    CONSTRAINT artifact_package_items_digest_check CHECK (content_digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT artifact_package_items_ordinal_unique UNIQUE (package_id, ordinal),
    CONSTRAINT artifact_package_items_identity_unique UNIQUE (package_id, artifact_type, artifact_id, artifact_version)
);

ALTER TABLE production_steps
    ADD CONSTRAINT production_steps_input_package_fk
    FOREIGN KEY (input_package_id, run_id)
    REFERENCES artifact_package_snapshots(id, run_id) ON DELETE RESTRICT;

CREATE TABLE production_gate_decisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    gate_step_id UUID NOT NULL REFERENCES production_steps(id) ON DELETE RESTRICT,
    package_id UUID NOT NULL,
    package_digest CHAR(64) NOT NULL,
    revision_epoch INT NOT NULL,
    decision VARCHAR(16) NOT NULL,
    reason TEXT,
    affected_owners JSONB NOT NULL DEFAULT '[]'::jsonb,
    actor_type VARCHAR(32) NOT NULL,
    actor_id VARCHAR(120) NOT NULL,
    command_id UUID,
    decided_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_gate_decisions_package_fk FOREIGN KEY (package_id, run_id)
        REFERENCES artifact_package_snapshots(id, run_id) ON DELETE RESTRICT,
    CONSTRAINT production_gate_decisions_epoch_fk FOREIGN KEY (run_id, revision_epoch)
        REFERENCES production_revision_epochs(run_id, epoch) ON DELETE RESTRICT,
    CONSTRAINT production_gate_decisions_decision_check CHECK (decision IN ('approved', 'rejected')),
    CONSTRAINT production_gate_decisions_reject_reason_check CHECK (
        decision = 'approved'
        OR (reason IS NOT NULL AND length(btrim(reason)) > 0 AND jsonb_array_length(affected_owners) > 0)
    ),
    CONSTRAINT production_gate_decisions_json_check CHECK (jsonb_typeof(affected_owners) = 'array'),
    CONSTRAINT production_gate_decisions_digest_check CHECK (package_digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT production_gate_decisions_actor_check CHECK (
        actor_type = 'local_operator' AND length(btrim(actor_id)) > 0
    ),
    CONSTRAINT production_gate_decisions_package_once UNIQUE (package_id)
);
CREATE INDEX idx_production_gate_decisions_run ON production_gate_decisions(run_id, decided_at DESC);

CREATE TABLE collaboration_suggestion_responses (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    suggestion_id UUID NOT NULL UNIQUE REFERENCES collaboration_suggestions(id) ON DELETE RESTRICT,
    decision VARCHAR(16) NOT NULL,
    reason TEXT,
    actor_type VARCHAR(32) NOT NULL,
    actor_id VARCHAR(120) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT collaboration_suggestion_responses_decision_check CHECK (decision IN ('accepted', 'rejected')),
    CONSTRAINT collaboration_suggestion_responses_reason_check CHECK (
        decision = 'accepted' OR (reason IS NOT NULL AND length(btrim(reason)) > 0)
    ),
    CONSTRAINT collaboration_suggestion_responses_actor_check CHECK (
        actor_type = 'local_operator' AND length(btrim(actor_id)) > 0
    )
);

ALTER TABLE collaboration_suggestions
    ADD COLUMN run_id UUID REFERENCES production_runs(id) ON DELETE RESTRICT,
    ADD COLUMN source_step_id UUID REFERENCES production_steps(id) ON DELETE RESTRICT,
    ADD COLUMN source_attempt INT,
    ADD COLUMN revision_epoch INT,
    ADD COLUMN source_model_call_id UUID REFERENCES model_calls(id) ON DELETE RESTRICT,
    ADD COLUMN target_artifact_version INT,
    ADD COLUMN target_content_digest CHAR(64),
    ADD COLUMN blocking BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN audit_status VARCHAR(32);
UPDATE collaboration_suggestions SET audit_status = 'legacy_partial_audit' WHERE audit_status IS NULL;
ALTER TABLE collaboration_suggestions ALTER COLUMN audit_status SET NOT NULL;
ALTER TABLE collaboration_suggestions ALTER COLUMN audit_status SET DEFAULT 'complete';
ALTER TABLE collaboration_suggestions
    ADD CONSTRAINT collaboration_suggestions_audit_check CHECK (
        (audit_status = 'legacy_partial_audit' AND run_id IS NULL)
        OR (
            audit_status = 'complete'
            AND run_id IS NOT NULL
            AND source_step_id IS NOT NULL
            AND source_attempt > 0
            AND revision_epoch >= 0
            AND source_model_call_id IS NOT NULL
            AND target_artifact_version > 0
            AND target_content_digest ~ '^[0-9a-f]{64}$'
        )
    );

CREATE TABLE production_resource_reservations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    step_id UUID NOT NULL REFERENCES production_steps(id) ON DELETE RESTRICT,
    attempt_no INT NOT NULL,
    resource_key VARCHAR(80) NOT NULL,
    reserved_value BIGINT NOT NULL,
    actual_value BIGINT,
    status VARCHAR(24) NOT NULL DEFAULT 'reserved',
    request_digest CHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    settled_at TIMESTAMPTZ,
    CONSTRAINT production_resource_reservations_values_check CHECK (
        attempt_no > 0 AND reserved_value >= 0 AND (actual_value IS NULL OR actual_value >= 0)
    ),
    CONSTRAINT production_resource_reservations_status_check CHECK (
        status IN ('reserved', 'settled', 'released', 'held_uncertain')
    ),
    CONSTRAINT production_resource_reservations_digest_check CHECK (request_digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT production_resource_reservations_identity_unique UNIQUE (step_id, attempt_no, resource_key)
);
CREATE INDEX idx_production_resource_reservations_run
    ON production_resource_reservations(run_id, resource_key, status);
COMMENT ON TABLE production_resource_reservations IS '外部调用前原子预占的非金额资源；结果不确定时保持 held_uncertain。';

CREATE TABLE production_resource_usage (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    step_id UUID REFERENCES production_steps(id) ON DELETE RESTRICT,
    reservation_id UUID REFERENCES production_resource_reservations(id) ON DELETE RESTRICT,
    resource_key VARCHAR(80) NOT NULL,
    used_value BIGINT NOT NULL,
    usage_digest CHAR(64) NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_resource_usage_value_check CHECK (used_value >= 0),
    CONSTRAINT production_resource_usage_digest_check CHECK (usage_digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT production_resource_usage_event_unique UNIQUE (run_id, resource_key, usage_digest)
);

CREATE TABLE production_commands (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    actor_type VARCHAR(32) NOT NULL,
    actor_id VARCHAR(120) NOT NULL,
    command_type VARCHAR(40) NOT NULL,
    aggregate_type VARCHAR(32) NOT NULL,
    aggregate_id UUID NOT NULL,
    idempotency_key VARCHAR(200) NOT NULL,
    request_digest CHAR(64) NOT NULL,
    result JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_commands_actor_check CHECK (
        actor_type = 'local_operator' AND length(btrim(actor_id)) > 0
    ),
    CONSTRAINT production_commands_digest_check CHECK (request_digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT production_commands_result_check CHECK (jsonb_typeof(result) = 'object'),
    CONSTRAINT production_commands_idempotency_unique UNIQUE (
        actor_type, actor_id, command_type, aggregate_type, aggregate_id, idempotency_key
    )
);
CREATE INDEX idx_production_commands_aggregate
    ON production_commands(aggregate_type, aggregate_id, created_at DESC);
COMMENT ON TABLE production_commands IS '统一命令幂等事实；同作用域同 key 必须比较 canonical request digest。';

ALTER TABLE production_gate_decisions
    ADD CONSTRAINT production_gate_decisions_command_fk
    FOREIGN KEY (command_id) REFERENCES production_commands(id) ON DELETE RESTRICT;

CREATE TABLE production_domain_links (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    source_step_id UUID NOT NULL REFERENCES production_steps(id) ON DELETE RESTRICT,
    revision_epoch INT NOT NULL,
    link_type VARCHAR(40) NOT NULL,
    script_id UUID REFERENCES scripts(id) ON DELETE RESTRICT,
    scene_id UUID REFERENCES scenes(id) ON DELETE RESTRICT,
    work_id UUID REFERENCES works(id) ON DELETE RESTRICT,
    work_version_id UUID REFERENCES work_versions(id) ON DELETE RESTRICT,
    work_plan_id UUID REFERENCES work_plans(id) ON DELETE RESTRICT,
    work_generation_run_id UUID REFERENCES work_generation_runs(id) ON DELETE RESTRICT,
    target_version VARCHAR(80) NOT NULL,
    target_digest CHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_domain_links_epoch_fk FOREIGN KEY (run_id, revision_epoch)
        REFERENCES production_revision_epochs(run_id, epoch) ON DELETE RESTRICT,
    CONSTRAINT production_domain_links_target_check CHECK (
        num_nonnulls(script_id, scene_id, work_id, work_version_id, work_plan_id, work_generation_run_id) = 1
    ),
    CONSTRAINT production_domain_links_type_check CHECK (
        link_type IN ('script', 'scene', 'work', 'work_version', 'work_plan', 'work_generation_run')
    ),
    CONSTRAINT production_domain_links_digest_check CHECK (target_digest ~ '^[0-9a-f]{64}$'),
    CONSTRAINT production_domain_links_identity_unique UNIQUE (run_id, link_type, target_version, target_digest)
);
CREATE INDEX idx_production_domain_links_run ON production_domain_links(run_id, created_at);

CREATE TABLE production_wakeups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    step_id UUID NOT NULL REFERENCES production_steps(id) ON DELETE RESTRICT,
    status VARCHAR(24) NOT NULL DEFAULT 'pending',
    delivery_attempts INT NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delivered_at TIMESTAMPTZ,
    CONSTRAINT production_wakeups_status_check CHECK (status IN ('pending', 'delivered', 'superseded')),
    CONSTRAINT production_wakeups_attempts_check CHECK (delivery_attempts >= 0),
    CONSTRAINT production_wakeups_identity_unique UNIQUE (step_id, status)
);
CREATE INDEX idx_production_wakeups_pending ON production_wakeups(created_at) WHERE status = 'pending';
COMMENT ON TABLE production_wakeups IS 'Redis 派发 outbox；消息载荷只能由 run_id 和 step_id 构成。';

-- Deterministic media evidence for the exact WorkVersion consumed by final compose.
CREATE TABLE required_take_inventories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    source_step_id UUID NOT NULL REFERENCES production_steps(id) ON DELETE RESTRICT,
    source_attempt INT NOT NULL,
    revision_epoch INT NOT NULL,
    work_id UUID NOT NULL REFERENCES works(id) ON DELETE RESTRICT,
    work_version_id UUID NOT NULL REFERENCES work_versions(id) ON DELETE RESTRICT,
    work_generation_run_id UUID NOT NULL REFERENCES work_generation_runs(id) ON DELETE RESTRICT,
    final_artifact_id UUID NOT NULL REFERENCES work_artifacts(id) ON DELETE RESTRICT,
    work_version_hash CHAR(64) NOT NULL,
    inventory_digest CHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT required_take_inventories_epoch_fk FOREIGN KEY (run_id, revision_epoch)
        REFERENCES production_revision_epochs(run_id, epoch) ON DELETE RESTRICT,
    CONSTRAINT required_take_inventories_attempt_check CHECK (source_attempt > 0),
    CONSTRAINT required_take_inventories_digest_check CHECK (
        work_version_hash ~ '^[0-9a-f]{64}$' AND inventory_digest ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT required_take_inventories_identity_unique UNIQUE (run_id, work_version_id, inventory_digest)
);

CREATE TABLE required_takes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    inventory_id UUID NOT NULL REFERENCES required_take_inventories(id) ON DELETE RESTRICT,
    ordinal INT NOT NULL,
    take_key VARCHAR(160) NOT NULL,
    generation_step_id UUID NOT NULL REFERENCES work_generation_steps(id) ON DELETE RESTRICT,
    generation_attempt_id UUID NOT NULL REFERENCES work_generation_attempts(id) ON DELETE RESTRICT,
    output_artifact_id UUID NOT NULL REFERENCES work_artifacts(id) ON DELETE RESTRICT,
    segment_key VARCHAR(160) NOT NULL,
    scene_ids JSONB NOT NULL,
    scene_shot_map JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT required_takes_ordinal_check CHECK (ordinal >= 0),
    CONSTRAINT required_takes_mapping_check CHECK (
        jsonb_typeof(scene_ids) = 'array'
        AND jsonb_array_length(scene_ids) > 0
        AND jsonb_typeof(scene_shot_map) = 'object'
        AND scene_shot_map <> '{}'::jsonb
    ),
    CONSTRAINT required_takes_key_unique UNIQUE (inventory_id, take_key),
    CONSTRAINT required_takes_ordinal_unique UNIQUE (inventory_id, ordinal),
    CONSTRAINT required_takes_attempt_unique UNIQUE (inventory_id, generation_attempt_id, output_artifact_id)
);
COMMENT ON TABLE required_takes IS 'final compose 实际消费的生成输出；segment 可覆盖多个 Scene，不虚构 take/Shot 一对一。';

CREATE TABLE media_evidence_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    source_step_id UUID NOT NULL REFERENCES production_steps(id) ON DELETE RESTRICT,
    source_attempt INT NOT NULL,
    revision_epoch INT NOT NULL,
    work_version_id UUID NOT NULL REFERENCES work_versions(id) ON DELETE RESTRICT,
    inventory_id UUID NOT NULL REFERENCES required_take_inventories(id) ON DELETE RESTRICT,
    final_artifact_id UUID NOT NULL REFERENCES work_artifacts(id) ON DELETE RESTRICT,
    asset_hash CHAR(64) NOT NULL,
    mime_type VARCHAR(160) NOT NULL,
    duration_ms BIGINT NOT NULL,
    vision_capability_version VARCHAR(120) NOT NULL,
    audio_capability_version VARCHAR(120) NOT NULL,
    redacted_analysis JSONB NOT NULL,
    evidence_digest CHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT media_evidence_snapshots_epoch_fk FOREIGN KEY (run_id, revision_epoch)
        REFERENCES production_revision_epochs(run_id, epoch) ON DELETE RESTRICT,
    CONSTRAINT media_evidence_snapshots_values_check CHECK (source_attempt > 0 AND duration_ms > 0),
    CONSTRAINT media_evidence_snapshots_digest_check CHECK (
        asset_hash ~ '^[0-9a-f]{64}$' AND evidence_digest ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT media_evidence_snapshots_analysis_check CHECK (jsonb_typeof(redacted_analysis) = 'object'),
    CONSTRAINT media_evidence_snapshots_no_secret_check CHECK (
        NOT jsonb_path_exists(redacted_analysis, '$.**.api_key')
        AND NOT jsonb_path_exists(redacted_analysis, '$.**.authorization')
        AND NOT jsonb_path_exists(redacted_analysis, '$.**.signed_url')
        AND NOT jsonb_path_exists(redacted_analysis, '$.**.base64')
    ),
    CONSTRAINT media_evidence_snapshots_identity_unique UNIQUE (run_id, work_version_id, inventory_id, evidence_digest)
);

-- Existing quality rows lack exact WorkVersion/take/media provenance. Keep them
-- as legacy_partial_audit rather than fabricating links.
ALTER TABLE continuity_ledgers
    DROP CONSTRAINT continuity_ledgers_production_project_id_shot_id_key,
    ALTER COLUMN shot_id DROP NOT NULL,
    ADD COLUMN run_id UUID REFERENCES production_runs(id) ON DELETE RESTRICT,
    ADD COLUMN step_id UUID REFERENCES production_steps(id) ON DELETE RESTRICT,
    ADD COLUMN attempt INT,
    ADD COLUMN revision_epoch INT,
    ADD COLUMN work_version_id UUID REFERENCES work_versions(id) ON DELETE RESTRICT,
    ADD COLUMN inventory_id UUID REFERENCES required_take_inventories(id) ON DELETE RESTRICT,
    ADD COLUMN evidence_snapshot_id UUID REFERENCES media_evidence_snapshots(id) ON DELETE RESTRICT,
    ADD COLUMN shot_contract_id UUID REFERENCES shot_contracts(id) ON DELETE RESTRICT,
    ADD COLUMN version INT,
    ADD COLUMN content_digest CHAR(64),
    ADD COLUMN audit_status VARCHAR(32);
UPDATE continuity_ledgers SET audit_status = 'legacy_partial_audit' WHERE audit_status IS NULL;
ALTER TABLE continuity_ledgers ALTER COLUMN audit_status SET NOT NULL;
ALTER TABLE continuity_ledgers ALTER COLUMN audit_status SET DEFAULT 'complete';
ALTER TABLE continuity_ledgers
    ADD CONSTRAINT continuity_ledgers_audit_check CHECK (
        (audit_status = 'legacy_partial_audit' AND run_id IS NULL)
        OR (
            audit_status = 'complete'
            AND run_id IS NOT NULL AND step_id IS NOT NULL AND attempt > 0
            AND revision_epoch >= 0 AND work_version_id IS NOT NULL
            AND inventory_id IS NOT NULL AND evidence_snapshot_id IS NOT NULL
            AND shot_contract_id IS NOT NULL AND version > 0
            AND content_digest ~ '^[0-9a-f]{64}$'
        )
    );
CREATE UNIQUE INDEX continuity_ledgers_append_only_identity
    ON continuity_ledgers(run_id, work_version_id, inventory_id, shot_contract_id, version)
    WHERE audit_status = 'complete';

ALTER TABLE take_reviews
    ALTER COLUMN shot_id DROP NOT NULL,
    ALTER COLUMN take_number DROP NOT NULL,
    ADD COLUMN run_id UUID REFERENCES production_runs(id) ON DELETE RESTRICT,
    ADD COLUMN step_id UUID REFERENCES production_steps(id) ON DELETE RESTRICT,
    ADD COLUMN attempt INT,
    ADD COLUMN revision_epoch INT,
    ADD COLUMN work_version_id UUID REFERENCES work_versions(id) ON DELETE RESTRICT,
    ADD COLUMN inventory_id UUID REFERENCES required_take_inventories(id) ON DELETE RESTRICT,
    ADD COLUMN evidence_snapshot_id UUID REFERENCES media_evidence_snapshots(id) ON DELETE RESTRICT,
    ADD COLUMN required_take_id UUID REFERENCES required_takes(id) ON DELETE RESTRICT,
    ADD COLUMN version INT,
    ADD COLUMN content_digest CHAR(64),
    ADD COLUMN audit_status VARCHAR(32);
UPDATE take_reviews SET audit_status = 'legacy_partial_audit' WHERE audit_status IS NULL;
ALTER TABLE take_reviews ALTER COLUMN audit_status SET NOT NULL;
ALTER TABLE take_reviews ALTER COLUMN audit_status SET DEFAULT 'complete';
ALTER TABLE take_reviews
    ADD CONSTRAINT take_reviews_audit_check CHECK (
        (audit_status = 'legacy_partial_audit' AND run_id IS NULL)
        OR (
            audit_status = 'complete'
            AND run_id IS NOT NULL AND step_id IS NOT NULL AND attempt > 0
            AND revision_epoch >= 0 AND work_version_id IS NOT NULL
            AND inventory_id IS NOT NULL AND evidence_snapshot_id IS NOT NULL
            AND required_take_id IS NOT NULL AND version > 0
            AND content_digest ~ '^[0-9a-f]{64}$'
        )
    );
CREATE UNIQUE INDEX take_reviews_append_only_identity
    ON take_reviews(run_id, work_version_id, inventory_id, required_take_id, version)
    WHERE audit_status = 'complete';

-- All process artifacts created by the durable executor carry exact provenance.
DO $$
DECLARE
    artifact_table TEXT;
BEGIN
    FOREACH artifact_table IN ARRAY ARRAY[
        'creative_briefs', 'story_bibles', 'character_bibles', 'script_drafts',
        'directorial_treatments', 'shot_contracts', 'performance_briefs', 'sound_plans'
    ] LOOP
        EXECUTE format(
            'ALTER TABLE %I ADD COLUMN run_id UUID REFERENCES production_runs(id) ON DELETE RESTRICT, '
            'ADD COLUMN step_id UUID REFERENCES production_steps(id) ON DELETE RESTRICT, '
            'ADD COLUMN attempt INT, ADD COLUMN revision_epoch INT, '
            'ADD COLUMN content_digest CHAR(64), '
            'ADD COLUMN applied_suggestion_ids JSONB NOT NULL DEFAULT ''[]''::jsonb, '
            'ADD COLUMN audit_status VARCHAR(32)',
            artifact_table
        );
        EXECUTE format('UPDATE %I SET audit_status = ''legacy_partial_audit'' WHERE audit_status IS NULL', artifact_table);
        EXECUTE format('ALTER TABLE %I ALTER COLUMN audit_status SET NOT NULL', artifact_table);
        EXECUTE format('ALTER TABLE %I ALTER COLUMN audit_status SET DEFAULT ''complete''', artifact_table);
        EXECUTE format(
            'ALTER TABLE %I ADD CONSTRAINT %I CHECK ('
            '(audit_status = ''legacy_partial_audit'' AND run_id IS NULL) OR '
            '(audit_status = ''complete'' AND run_id IS NOT NULL AND step_id IS NOT NULL '
            'AND attempt > 0 AND revision_epoch >= 0 AND content_digest ~ ''^[0-9a-f]{64}$'' '
            'AND jsonb_typeof(applied_suggestion_ids) = ''array''))',
            artifact_table,
            artifact_table || '_durable_audit_check'
        );
    END LOOP;
END $$;

ALTER TABLE shot_contracts ADD COLUMN domain_scene_id UUID REFERENCES scenes(id) ON DELETE RESTRICT;
ALTER TABLE performance_briefs
    ADD COLUMN character_bible_id UUID REFERENCES character_bibles(id) ON DELETE RESTRICT,
    ADD COLUMN script_id UUID REFERENCES scripts(id) ON DELETE RESTRICT;
ALTER TABLE sound_plans ADD COLUMN script_id UUID REFERENCES scripts(id) ON DELETE RESTRICT;

ALTER TABLE scripts
    ADD COLUMN production_run_id UUID REFERENCES production_runs(id) ON DELETE RESTRICT,
    ADD COLUMN script_package_id UUID REFERENCES artifact_package_snapshots(id) ON DELETE RESTRICT,
    ADD COLUMN script_package_digest CHAR(64),
    ADD COLUMN topic_snapshot JSONB,
    ADD COLUMN source_artifacts JSONB,
    ADD COLUMN source_revision_epoch INT;
ALTER TABLE scripts
    ADD CONSTRAINT scripts_full_crew_source_check CHECK (
        production_run_id IS NULL
        OR (
            topic_id IS NOT NULL
            AND script_package_id IS NOT NULL
            AND script_package_digest ~ '^[0-9a-f]{64}$'
            AND jsonb_typeof(topic_snapshot) = 'object'
            AND jsonb_typeof(source_artifacts) = 'array'
            AND source_revision_epoch >= 0
        )
    );

-- Source lock lives at the Topic/Project application boundary and therefore
-- protects every caller, not only the Production API.
CREATE FUNCTION reject_locked_full_crew_topic_mutation() RETURNS TRIGGER AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM production_projects production
        WHERE production.topic_id = OLD.id
          AND production.project_type = 'full_crew'
          AND production.archived_at IS NULL
          AND production.status IN (
              'created', 'active', 'briefing', 'scripting', 'directing',
              'generating', 'editing', 'qc', 'waiting_approval', 'external_wait',
              'attention_required', 'cancelling'
          )
    ) AND ROW(
        NEW.project_id, NEW.title, NEW.angle, NEW.target_audience,
        NEW.hook_points, NEW.content_type, NEW.tags, NEW.status, NEW.deleted_at
    ) IS DISTINCT FROM ROW(
        OLD.project_id, OLD.title, OLD.angle, OLD.target_audience,
        OLD.hook_points, OLD.content_type, OLD.tags, OLD.status, OLD.deleted_at
    ) THEN
        IF current_setting('novex.production_script_promotion', TRUE) = 'on'
           AND OLD.status = 'approved' AND NEW.status = 'scripted'
           AND ROW(
               NEW.project_id, NEW.title, NEW.angle, NEW.target_audience,
               NEW.hook_points, NEW.content_type, NEW.tags, NEW.deleted_at
           ) IS NOT DISTINCT FROM ROW(
               OLD.project_id, OLD.title, OLD.angle, OLD.target_audience,
               OLD.hook_points, OLD.content_type, OLD.tags, OLD.deleted_at
           ) THEN
            RETURN NEW;
        END IF;
        RAISE EXCEPTION 'source_locked' USING ERRCODE = 'P0001';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER content_topics_full_crew_source_lock
    BEFORE UPDATE ON content_topics
    FOR EACH ROW EXECUTE FUNCTION reject_locked_full_crew_topic_mutation();

CREATE FUNCTION reject_project_archive_with_active_full_crew() RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status = 'active' AND NEW.status = 'archived' AND EXISTS (
        SELECT 1 FROM production_projects production
        WHERE production.project_id = OLD.id
          AND production.project_type = 'full_crew'
          AND production.archived_at IS NULL
          AND production.status IN (
              'created', 'active', 'briefing', 'scripting', 'directing',
              'generating', 'editing', 'qc', 'waiting_approval', 'external_wait',
              'attention_required', 'cancelling'
          )
    ) THEN
        RAISE EXCEPTION 'source_locked' USING ERRCODE = 'P0001';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER projects_full_crew_source_lock
    BEFORE UPDATE OF status ON projects
    FOR EACH ROW EXECUTE FUNCTION reject_project_archive_with_active_full_crew();

CREATE FUNCTION reject_production_append_only_mutation() RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'production audit record is append-only';
END;
$$ LANGUAGE plpgsql;

DO $$
DECLARE
    audit_table TEXT;
BEGIN
    FOREACH audit_table IN ARRAY ARRAY[
        'production_plan_snapshots', 'production_revision_epochs',
        'artifact_package_snapshots', 'artifact_package_items',
        'production_gate_decisions', 'collaboration_suggestion_responses',
        'production_resource_usage', 'production_commands', 'production_domain_links',
        'required_take_inventories', 'required_takes', 'media_evidence_snapshots'
    ] LOOP
        EXECUTE format(
            'CREATE TRIGGER %I BEFORE UPDATE OR DELETE ON %I '
            'FOR EACH ROW EXECUTE FUNCTION reject_production_append_only_mutation()',
            audit_table || '_append_only', audit_table
        );
    END LOOP;
END $$;

COMMENT ON COLUMN media_evidence_snapshots.redacted_analysis IS '脱敏分析结果；禁止 base64、签名 URL、凭据和原始请求头。';
COMMENT ON COLUMN continuity_ledgers.audit_status IS 'legacy_partial_audit 不得进入新 QualityPackage；禁止伪造历史媒体映射。';
COMMENT ON COLUMN take_reviews.audit_status IS 'legacy_partial_audit 不得进入新 QualityPackage；complete 必须绑定真实 required take。';
