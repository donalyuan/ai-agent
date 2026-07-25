-- Versioned Agent execution evidence. Registry templates remain code-owned and are never stored here.
CREATE TABLE definition_releases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    definition_kind VARCHAR(16) NOT NULL,
    definition_key VARCHAR(160) NOT NULL,
    definition_version VARCHAR(32) NOT NULL,
    definition_digest CHAR(64) NOT NULL,
    registry_digest CHAR(64) NOT NULL,
    initial_status VARCHAR(16) NOT NULL,
    executor_owner VARCHAR(16) NOT NULL,
    published_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT definition_releases_kind_check CHECK (definition_kind IN ('agent', 'prompt')),
    CONSTRAINT definition_releases_status_check CHECK (initial_status IN ('candidate', 'active', 'supported', 'revoked')),
    CONSTRAINT definition_releases_owner_check CHECK (executor_owner IN ('rust', 'pi')),
    CONSTRAINT definition_releases_digest_check CHECK (
        definition_digest ~ '^[0-9a-f]{64}$' AND registry_digest ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT definition_releases_identity_unique UNIQUE (definition_kind, definition_key, definition_version)
);

COMMENT ON TABLE definition_releases IS '代码 Definition Registry 的不可变发布证据；禁止保存模板正文或由数据库覆盖运行定义。';

CREATE FUNCTION reject_definition_release_mutation() RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'definition releases are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER definition_releases_no_update
    BEFORE UPDATE OR DELETE ON definition_releases
    FOR EACH ROW EXECUTE FUNCTION reject_definition_release_mutation();

CREATE TABLE agent_conversation_bindings (
    conversation_id UUID PRIMARY KEY REFERENCES agent_conversations(id) ON DELETE CASCADE,
    agent_key VARCHAR(160) NOT NULL,
    agent_version VARCHAR(32) NOT NULL,
    agent_digest CHAR(64) NOT NULL,
    prompt_bindings JSONB NOT NULL,
    registry_digest CHAR(64) NOT NULL,
    model_id UUID REFERENCES ai_models(id) ON DELETE RESTRICT,
    behavior_fingerprint CHAR(64),
    model_capabilities JSONB,
    binding_status VARCHAR(32) NOT NULL DEFAULT 'definition_bound',
    migration_source VARCHAR(60),
    parent_conversation_id UUID REFERENCES agent_conversations(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_conversation_bindings_model_pair CHECK (
        (model_id IS NULL AND behavior_fingerprint IS NULL)
        OR (model_id IS NOT NULL AND behavior_fingerprint ~ '^[0-9a-f]{64}$')
    ),
    CONSTRAINT agent_conversation_bindings_status_check CHECK (
        binding_status IN ('definition_bound', 'executable', 'model_rebind_required', 'read_only')
    )
);

COMMENT ON TABLE agent_conversation_bindings IS 'Conversation 的不可变 Agent/Prompt 绑定；模型绑定仅允许首次原子补齐。';
COMMENT ON COLUMN agent_conversation_bindings.prompt_bindings IS 'node 到 Prompt key/version/digest 的精确版本快照，不包含模板正文。';

CREATE TABLE agent_run_bindings (
    agent_run_id UUID PRIMARY KEY REFERENCES agent_runs(id) ON DELETE CASCADE,
    agent_key VARCHAR(160) NOT NULL,
    agent_version VARCHAR(32) NOT NULL,
    agent_digest CHAR(64) NOT NULL,
    prompt_bindings JSONB NOT NULL,
    registry_digest CHAR(64) NOT NULL,
    model_id UUID NOT NULL REFERENCES ai_models(id) ON DELETE RESTRICT,
    behavior_fingerprint CHAR(64) NOT NULL,
    model_capabilities JSONB NOT NULL,
    legacy_partial_audit BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_run_bindings_fingerprint_check CHECK (behavior_fingerprint ~ '^[0-9a-f]{64}$')
);

CREATE TABLE model_calls (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    schema_version VARCHAR(16) NOT NULL DEFAULT '1',
    source_runtime VARCHAR(16) NOT NULL DEFAULT 'rust',
    conversation_id UUID REFERENCES agent_conversations(id) ON DELETE CASCADE,
    agent_run_id UUID REFERENCES agent_runs(id) ON DELETE CASCADE,
    agent_step_id UUID REFERENCES agent_steps(id) ON DELETE SET NULL,
    root_call_id UUID REFERENCES model_calls(id) ON DELETE RESTRICT,
    parent_call_id UUID REFERENCES model_calls(id) ON DELETE RESTRICT,
    node_key VARCHAR(160) NOT NULL,
    attempt INT NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'prepared',
    agent_key VARCHAR(160) NOT NULL,
    agent_version VARCHAR(32) NOT NULL,
    prompt_key VARCHAR(160) NOT NULL,
    prompt_version VARCHAR(32) NOT NULL,
    registry_digest CHAR(64) NOT NULL,
    prompt_snapshot JSONB NOT NULL,
    context_sources JSONB NOT NULL DEFAULT '[]'::jsonb,
    memory_sources JSONB NOT NULL DEFAULT '[]'::jsonb,
    tool_schema JSONB,
    model_id UUID NOT NULL REFERENCES ai_models(id) ON DELETE RESTRICT,
    behavior_fingerprint CHAR(64) NOT NULL,
    model_snapshot JSONB NOT NULL,
    parameters JSONB NOT NULL DEFAULT '{}'::jsonb,
    asset_references JSONB NOT NULL DEFAULT '[]'::jsonb,
    output_snapshot JSONB,
    usage_snapshot JSONB,
    error_snapshot JSONB,
    structured_parse_status VARCHAR(32),
    prepared_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    CONSTRAINT model_calls_owner_check CHECK (num_nonnulls(conversation_id, agent_run_id) = 1),
    CONSTRAINT model_calls_source_check CHECK (source_runtime = 'rust'),
    CONSTRAINT model_calls_attempt_check CHECK (attempt > 0),
    CONSTRAINT model_calls_status_check CHECK (status IN ('prepared', 'succeeded', 'failed', 'aborted')),
    CONSTRAINT model_calls_terminal_time_check CHECK (
        (status = 'prepared' AND completed_at IS NULL)
        OR (status IN ('succeeded', 'failed', 'aborted') AND completed_at IS NOT NULL)
    ),
    CONSTRAINT model_calls_digest_check CHECK (
        registry_digest ~ '^[0-9a-f]{64}$' AND behavior_fingerprint ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT model_calls_attempt_unique UNIQUE (root_call_id, attempt)
);

COMMENT ON TABLE model_calls IS 'Rust 每次真实文本模型请求及每次显式重试的最小不可覆盖审计单元。';
COMMENT ON COLUMN model_calls.prompt_snapshot IS '调用前持久化的脱敏完整逻辑输入、System/User 消息与输出合同。';
COMMENT ON COLUMN model_calls.asset_references IS '仅保存资产 ID、版本/hash、MIME 和必要元数据；禁止 base64 和临时签名 URL。';

CREATE INDEX idx_model_calls_conversation ON model_calls(conversation_id, prepared_at DESC) WHERE conversation_id IS NOT NULL;
CREATE INDEX idx_model_calls_run ON model_calls(agent_run_id, prepared_at DESC) WHERE agent_run_id IS NOT NULL;
CREATE INDEX idx_model_calls_filter ON model_calls(status, node_key, model_id, prepared_at DESC);
CREATE UNIQUE INDEX idx_model_calls_step ON model_calls(agent_step_id) WHERE agent_step_id IS NOT NULL;

CREATE FUNCTION enforce_model_call_transition() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.status = OLD.status
       AND OLD.agent_step_id IS NULL
       AND NEW.agent_step_id IS NOT NULL
       AND ROW(
           NEW.output_snapshot, NEW.usage_snapshot, NEW.error_snapshot,
           NEW.structured_parse_status, NEW.completed_at
       ) IS NOT DISTINCT FROM ROW(
           OLD.output_snapshot, OLD.usage_snapshot, OLD.error_snapshot,
           OLD.structured_parse_status, OLD.completed_at
       )
       AND ROW(
           NEW.schema_version, NEW.source_runtime, NEW.conversation_id, NEW.agent_run_id,
           NEW.root_call_id, NEW.parent_call_id, NEW.node_key, NEW.attempt,
           NEW.agent_key, NEW.agent_version, NEW.prompt_key, NEW.prompt_version,
           NEW.registry_digest, NEW.prompt_snapshot, NEW.context_sources, NEW.memory_sources,
           NEW.tool_schema, NEW.model_id, NEW.behavior_fingerprint, NEW.model_snapshot,
           NEW.parameters, NEW.asset_references, NEW.prepared_at
       ) IS NOT DISTINCT FROM ROW(
           OLD.schema_version, OLD.source_runtime, OLD.conversation_id, OLD.agent_run_id,
           OLD.root_call_id, OLD.parent_call_id, OLD.node_key, OLD.attempt,
           OLD.agent_key, OLD.agent_version, OLD.prompt_key, OLD.prompt_version,
           OLD.registry_digest, OLD.prompt_snapshot, OLD.context_sources, OLD.memory_sources,
           OLD.tool_schema, OLD.model_id, OLD.behavior_fingerprint, OLD.model_snapshot,
           OLD.parameters, OLD.asset_references, OLD.prepared_at
       ) THEN
        RETURN NEW;
    END IF;
    IF OLD.status <> 'prepared' THEN
        RAISE EXCEPTION 'model call already has terminal status';
    END IF;
    IF NEW.status NOT IN ('succeeded', 'failed', 'aborted') THEN
        RAISE EXCEPTION 'model call may only transition from prepared to a terminal status';
    END IF;
    IF ROW(
        NEW.schema_version, NEW.source_runtime, NEW.conversation_id, NEW.agent_run_id,
        NEW.root_call_id, NEW.parent_call_id, NEW.node_key, NEW.attempt,
        NEW.agent_key, NEW.agent_version, NEW.prompt_key, NEW.prompt_version,
        NEW.registry_digest, NEW.prompt_snapshot, NEW.context_sources, NEW.memory_sources,
        NEW.tool_schema, NEW.model_id, NEW.behavior_fingerprint, NEW.model_snapshot,
        NEW.parameters, NEW.asset_references, NEW.prepared_at
    ) IS DISTINCT FROM ROW(
        OLD.schema_version, OLD.source_runtime, OLD.conversation_id, OLD.agent_run_id,
        OLD.root_call_id, OLD.parent_call_id, OLD.node_key, OLD.attempt,
        OLD.agent_key, OLD.agent_version, OLD.prompt_key, OLD.prompt_version,
        OLD.registry_digest, OLD.prompt_snapshot, OLD.context_sources, OLD.memory_sources,
        OLD.tool_schema, OLD.model_id, OLD.behavior_fingerprint, OLD.model_snapshot,
        OLD.parameters, OLD.asset_references, OLD.prepared_at
    ) THEN
        RAISE EXCEPTION 'model call prepared input is immutable';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER model_calls_terminal_once
    BEFORE UPDATE ON model_calls
    FOR EACH ROW EXECUTE FUNCTION enforce_model_call_transition();

ALTER TABLE agent_steps ADD COLUMN model_call_id UUID REFERENCES model_calls(id) ON DELETE SET NULL;
CREATE UNIQUE INDEX idx_agent_steps_model_call ON agent_steps(model_call_id) WHERE model_call_id IS NOT NULL;

CREATE TABLE eval_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    candidate_key VARCHAR(160) NOT NULL,
    candidate_version VARCHAR(32) NOT NULL,
    candidate_digest CHAR(64) NOT NULL,
    baseline_key VARCHAR(160),
    baseline_version VARCHAR(32),
    case_set_version VARCHAR(60) NOT NULL,
    evaluator_version VARCHAR(60) NOT NULL,
    model_id UUID REFERENCES ai_models(id) ON DELETE RESTRICT,
    behavior_fingerprint CHAR(64),
    approved_real_calls BOOLEAN NOT NULL DEFAULT FALSE,
    max_cases INT NOT NULL,
    max_input_tokens BIGINT NOT NULL,
    max_output_tokens BIGINT NOT NULL,
    max_retries INT NOT NULL,
    max_cost_micros BIGINT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'pending',
    actual_real_model_calls INT NOT NULL DEFAULT 0,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    CONSTRAINT eval_runs_budget_check CHECK (
        max_cases > 0 AND max_input_tokens >= 0 AND max_output_tokens >= 0
        AND max_retries >= 0 AND max_cost_micros >= 0
    ),
    CONSTRAINT eval_runs_status_check CHECK (status IN ('pending', 'running', 'passed', 'failed', 'blocked', 'budget_exhausted')),
    CONSTRAINT eval_runs_real_confirmation_check CHECK (
        approved_real_calls OR (model_id IS NULL AND behavior_fingerprint IS NULL AND max_cost_micros = 0)
    )
);

CREATE TABLE eval_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    eval_run_id UUID NOT NULL UNIQUE REFERENCES eval_runs(id) ON DELETE RESTRICT,
    schema_version VARCHAR(16) NOT NULL DEFAULT '1',
    passed BOOLEAN NOT NULL,
    gate_results JSONB NOT NULL,
    aggregate_metrics JSONB NOT NULL,
    redacted_case_results JSONB NOT NULL,
    source_deleted BOOLEAN NOT NULL DEFAULT FALSE,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE eval_reports IS '完成后不可覆盖的 candidate 门禁报告；来源删除后只保留聚合指标并标记 source_deleted。';

CREATE FUNCTION reject_completed_eval_report_mutation() RETURNS TRIGGER AS $$
BEGIN
    IF OLD.source_deleted = FALSE AND NEW.source_deleted = TRUE
       AND NEW.gate_results = OLD.gate_results
       AND NEW.aggregate_metrics = OLD.aggregate_metrics
       AND NEW.redacted_case_results = '[]'::jsonb THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'completed eval reports are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER eval_reports_immutable
    BEFORE UPDATE OR DELETE ON eval_reports
    FOR EACH ROW EXECUTE FUNCTION reject_completed_eval_report_mutation();
