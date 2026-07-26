-- Persist immutable, redacted Context compilation evidence before a Rust model call.
-- Historical bindings remain NULL until a later evidence-backed migration fixes them.

ALTER TABLE agent_conversation_bindings
    ADD COLUMN context_policy_bindings JSONB,
    ADD COLUMN tokenizer_profile_key VARCHAR(128),
    ADD COLUMN tokenizer_profile_version VARCHAR(32),
    ADD COLUMN tokenizer_profile_digest CHAR(64),
    ADD CONSTRAINT agent_conversation_bindings_context_binding_check CHECK (
        (context_policy_bindings IS NULL
            OR (jsonb_typeof(context_policy_bindings) = 'object'
                AND context_policy_bindings <> '{}'::jsonb))
        AND (
            (tokenizer_profile_key IS NULL
            AND tokenizer_profile_version IS NULL
            AND tokenizer_profile_digest IS NULL)
            OR
            (model_id IS NOT NULL
            AND tokenizer_profile_key ~ '^[a-z0-9][a-z0-9._-]{0,127}$'
            AND tokenizer_profile_version ~ '^[0-9]+\.[0-9]+\.[0-9]+$'
            AND tokenizer_profile_digest ~ '^[0-9a-f]{64}$')
        )
    );

ALTER TABLE agent_run_bindings
    ADD COLUMN context_policy_bindings JSONB,
    ADD COLUMN tokenizer_profile_key VARCHAR(128),
    ADD COLUMN tokenizer_profile_version VARCHAR(32),
    ADD COLUMN tokenizer_profile_digest CHAR(64),
    ADD CONSTRAINT agent_run_bindings_context_binding_check CHECK (
        (context_policy_bindings IS NULL
            OR (jsonb_typeof(context_policy_bindings) = 'object'
                AND context_policy_bindings <> '{}'::jsonb))
        AND (
            (tokenizer_profile_key IS NULL
            AND tokenizer_profile_version IS NULL
            AND tokenizer_profile_digest IS NULL)
            OR
            (tokenizer_profile_key ~ '^[a-z0-9][a-z0-9._-]{0,127}$'
            AND tokenizer_profile_version ~ '^[0-9]+\.[0-9]+\.[0-9]+$'
            AND tokenizer_profile_digest ~ '^[0-9a-f]{64}$')
        )
    );

COMMENT ON COLUMN agent_conversation_bindings.context_policy_bindings IS
    'node 到 ContextPolicy key/version/digest 的固定引用；历史未证明等价记录保持 NULL。';
COMMENT ON COLUMN agent_run_bindings.context_policy_bindings IS
    'Run 的 node 到 ContextPolicy key/version/digest 固定引用；不得以当前 active Policy 覆盖。';

CREATE OR REPLACE FUNCTION enforce_conversation_binding_update() RETURNS TRIGGER AS $$
BEGIN
    IF ROW(
        NEW.conversation_id, NEW.agent_key, NEW.agent_version, NEW.agent_digest,
        NEW.prompt_bindings, NEW.registry_digest, NEW.migration_source,
        NEW.parent_conversation_id, NEW.created_at
    ) IS DISTINCT FROM ROW(
        OLD.conversation_id, OLD.agent_key, OLD.agent_version, OLD.agent_digest,
        OLD.prompt_bindings, OLD.registry_digest, OLD.migration_source,
        OLD.parent_conversation_id, OLD.created_at
    ) THEN
        RAISE EXCEPTION 'conversation definition binding is immutable';
    END IF;

    IF OLD.context_policy_bindings IS NOT NULL
       AND NEW.context_policy_bindings IS DISTINCT FROM OLD.context_policy_bindings THEN
        RAISE EXCEPTION 'conversation context policy binding is immutable';
    END IF;

    IF OLD.tokenizer_profile_key IS NOT NULL
       AND ROW(
           NEW.tokenizer_profile_key, NEW.tokenizer_profile_version,
           NEW.tokenizer_profile_digest
       ) IS DISTINCT FROM ROW(
           OLD.tokenizer_profile_key, OLD.tokenizer_profile_version,
           OLD.tokenizer_profile_digest
       ) THEN
        RAISE EXCEPTION 'conversation tokenizer profile binding is immutable';
    END IF;

    IF OLD.model_id IS NULL
       AND OLD.behavior_fingerprint IS NULL
       AND OLD.model_capabilities IS NULL
       AND OLD.binding_status = 'definition_bound'
       AND NEW.model_id IS NOT NULL
       AND NEW.behavior_fingerprint IS NOT NULL
       AND NEW.model_capabilities IS NOT NULL
       AND NEW.binding_status = 'executable' THEN
        RETURN NEW;
    END IF;

    IF ROW(NEW.model_id, NEW.behavior_fingerprint, NEW.model_capabilities)
       IS DISTINCT FROM ROW(OLD.model_id, OLD.behavior_fingerprint, OLD.model_capabilities) THEN
        RAISE EXCEPTION 'conversation model binding is immutable';
    END IF;

    IF OLD.binding_status = NEW.binding_status
       OR (OLD.binding_status = 'definition_bound' AND NEW.binding_status = 'read_only')
       OR (OLD.binding_status = 'executable' AND NEW.binding_status = 'model_rebind_required') THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'invalid conversation binding status transition';
END;
$$ LANGUAGE plpgsql;

CREATE TABLE context_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    schema_version VARCHAR(16) NOT NULL DEFAULT '1',
    source_runtime VARCHAR(16) NOT NULL DEFAULT 'rust',
    conversation_id UUID REFERENCES agent_conversations(id) ON DELETE CASCADE,
    agent_run_id UUID REFERENCES agent_runs(id) ON DELETE CASCADE,
    eval_run_id UUID REFERENCES eval_runs(id) ON DELETE RESTRICT,
    node_key VARCHAR(160) NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'succeeded',
    compiled_at TIMESTAMPTZ NOT NULL,
    policy_key VARCHAR(160) NOT NULL,
    policy_version VARCHAR(32) NOT NULL,
    tokenizer_profile_key VARCHAR(128) NOT NULL,
    tokenizer_profile_version VARCHAR(32) NOT NULL,
    tokenizer_mode VARCHAR(16) NOT NULL,
    model_context_window BIGINT NOT NULL,
    budget_ledger JSONB NOT NULL,
    decisions JSONB NOT NULL,
    selected_order JSONB NOT NULL,
    logical_input JSONB NOT NULL,
    context_digest CHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT context_snapshots_owner_check CHECK (
        num_nonnulls(conversation_id, agent_run_id, eval_run_id) = 1
    ),
    CONSTRAINT context_snapshots_runtime_check CHECK (source_runtime = 'rust'),
    CONSTRAINT context_snapshots_node_check CHECK (length(btrim(node_key)) > 0),
    CONSTRAINT context_snapshots_status_check CHECK (status = 'succeeded'),
    CONSTRAINT context_snapshots_policy_key_check CHECK (
        policy_key ~ '^[a-z0-9][a-z0-9._-]{0,159}$'
    ),
    CONSTRAINT context_snapshots_policy_version_check CHECK (
        policy_version ~ '^[0-9]+\.[0-9]+\.[0-9]+$'
    ),
    CONSTRAINT context_snapshots_profile_key_check CHECK (
        tokenizer_profile_key ~ '^[a-z0-9][a-z0-9._-]{0,127}$'
    ),
    CONSTRAINT context_snapshots_profile_version_check CHECK (
        tokenizer_profile_version ~ '^[0-9]+\.[0-9]+\.[0-9]+$'
    ),
    CONSTRAINT context_snapshots_mode_check CHECK (tokenizer_mode IN ('exact', 'conservative')),
    CONSTRAINT context_snapshots_window_check CHECK (model_context_window > 0),
    CONSTRAINT context_snapshots_payload_shape_check CHECK (
        jsonb_typeof(budget_ledger) = 'object'
        AND jsonb_typeof(decisions) = 'array'
        AND jsonb_typeof(selected_order) = 'array'
        AND jsonb_typeof(logical_input) = 'object'
    ),
    CONSTRAINT context_snapshots_selected_payload_check CHECK (
        NOT jsonb_path_exists(
            decisions,
            '$[*] ? (@.decision == "selected" && !exists(@.selected_payload))'
        )
        AND NOT jsonb_path_exists(
            decisions,
            '$[*] ? (@.decision != "selected" && exists(@.selected_payload))'
        )
    ),
    CONSTRAINT context_snapshots_digest_check CHECK (context_digest ~ '^[0-9a-f]{64}$')
);

COMMENT ON TABLE context_snapshots IS
    '成功 Context 编译的不可变、脱敏审计证据；采用项保留逻辑内容，排除项仅保存最小 decision 元数据。';
COMMENT ON COLUMN context_snapshots.decisions IS
    'selected decision 必须含 selected_payload；所有排除 decision 禁止保存 payload 正文。';
COMMENT ON COLUMN context_snapshots.logical_input IS
    '已复核的最终逻辑模型输入；仅保存通过脱敏校验的内容。';

CREATE INDEX idx_context_snapshots_conversation
    ON context_snapshots(conversation_id, compiled_at DESC)
    WHERE conversation_id IS NOT NULL;
CREATE INDEX idx_context_snapshots_run
    ON context_snapshots(agent_run_id, compiled_at DESC)
    WHERE agent_run_id IS NOT NULL;
CREATE INDEX idx_context_snapshots_eval_run
    ON context_snapshots(eval_run_id, compiled_at DESC)
    WHERE eval_run_id IS NOT NULL;
CREATE INDEX idx_context_snapshots_node
    ON context_snapshots(node_key, compiled_at DESC);

CREATE TABLE context_compile_attempts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    schema_version VARCHAR(16) NOT NULL DEFAULT '1',
    source_runtime VARCHAR(16) NOT NULL DEFAULT 'rust',
    conversation_id UUID REFERENCES agent_conversations(id) ON DELETE CASCADE,
    agent_run_id UUID REFERENCES agent_runs(id) ON DELETE CASCADE,
    eval_run_id UUID REFERENCES eval_runs(id) ON DELETE RESTRICT,
    node_key VARCHAR(160) NOT NULL,
    status VARCHAR(16) NOT NULL DEFAULT 'failed',
    compiled_at TIMESTAMPTZ NOT NULL,
    stage VARCHAR(16) NOT NULL,
    code VARCHAR(80) NOT NULL,
    budget_ledger JSONB,
    decisions JSONB NOT NULL,
    attempt_digest CHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT context_compile_attempts_owner_check CHECK (
        num_nonnulls(conversation_id, agent_run_id, eval_run_id) = 1
    ),
    CONSTRAINT context_compile_attempts_runtime_check CHECK (source_runtime = 'rust'),
    CONSTRAINT context_compile_attempts_node_check CHECK (length(btrim(node_key)) > 0),
    CONSTRAINT context_compile_attempts_status_check CHECK (status = 'failed'),
    CONSTRAINT context_compile_attempts_stage_check CHECK (
        stage IN ('schema', 'eligibility', 'conflict', 'tokenizer', 'budget', 'finalize')
    ),
    CONSTRAINT context_compile_attempts_code_check CHECK (length(btrim(code)) > 0),
    CONSTRAINT context_compile_attempts_payload_shape_check CHECK (
        (budget_ledger IS NULL OR jsonb_typeof(budget_ledger) = 'object')
        AND jsonb_typeof(decisions) = 'array'
    ),
    CONSTRAINT context_compile_attempts_excluded_payload_check CHECK (
        NOT jsonb_path_exists(decisions, '$[*] ? (exists(@.selected_payload))')
    ),
    CONSTRAINT context_compile_attempts_digest_check CHECK (attempt_digest ~ '^[0-9a-f]{64}$')
);

COMMENT ON TABLE context_compile_attempts IS
    '失败 Context 编译的不可变最小证据；不对应 ModelCall，禁止保存任意候选 payload 正文。';
COMMENT ON COLUMN context_compile_attempts.decisions IS
    '仅保存候选 identity、来源/version、hash、token 与 decision code，不保存 selected_payload。';

CREATE INDEX idx_context_compile_attempts_conversation
    ON context_compile_attempts(conversation_id, compiled_at DESC)
    WHERE conversation_id IS NOT NULL;
CREATE INDEX idx_context_compile_attempts_run
    ON context_compile_attempts(agent_run_id, compiled_at DESC)
    WHERE agent_run_id IS NOT NULL;
CREATE INDEX idx_context_compile_attempts_eval_run
    ON context_compile_attempts(eval_run_id, compiled_at DESC)
    WHERE eval_run_id IS NOT NULL;
CREATE INDEX idx_context_compile_attempts_node
    ON context_compile_attempts(node_key, compiled_at DESC);

-- Explicit owner deletion is the only permitted removal path for immutable Context evidence.
CREATE FUNCTION reject_context_audit_mutation() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' AND (
        (OLD.conversation_id IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM agent_conversations WHERE id = OLD.conversation_id))
        OR (OLD.agent_run_id IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM agent_runs WHERE id = OLD.agent_run_id))
        OR (OLD.eval_run_id IS NOT NULL
            AND NOT EXISTS (SELECT 1 FROM eval_runs WHERE id = OLD.eval_run_id))
    ) THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION 'context audit records are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER context_snapshots_immutable
    BEFORE UPDATE OR DELETE ON context_snapshots
    FOR EACH ROW EXECUTE FUNCTION reject_context_audit_mutation();

CREATE TRIGGER context_compile_attempts_immutable
    BEFORE UPDATE OR DELETE ON context_compile_attempts
    FOR EACH ROW EXECUTE FUNCTION reject_context_audit_mutation();

ALTER TABLE model_calls
    ADD COLUMN context_snapshot_id UUID REFERENCES context_snapshots(id) ON DELETE SET NULL,
    ADD COLUMN context_digest CHAR(64),
    ADD COLUMN context_policy_key VARCHAR(160),
    ADD COLUMN context_policy_version VARCHAR(32),
    ADD COLUMN tokenizer_profile_key VARCHAR(128),
    ADD COLUMN tokenizer_profile_version VARCHAR(32),
    ADD COLUMN context_budget_summary JSONB,
    ADD CONSTRAINT model_calls_context_evidence_check CHECK (
        num_nonnulls(
            context_snapshot_id, context_digest, context_policy_key, context_policy_version,
            tokenizer_profile_key, tokenizer_profile_version, context_budget_summary
        ) IN (0, 7)
    ),
    ADD CONSTRAINT model_calls_context_digest_check CHECK (
        context_digest IS NULL OR context_digest ~ '^[0-9a-f]{64}$'
    ),
    ADD CONSTRAINT model_calls_context_policy_key_check CHECK (
        context_policy_key IS NULL OR context_policy_key ~ '^[a-z0-9][a-z0-9._-]{0,159}$'
    ),
    ADD CONSTRAINT model_calls_context_policy_version_check CHECK (
        context_policy_version IS NULL OR context_policy_version ~ '^[0-9]+\.[0-9]+\.[0-9]+$'
    ),
    ADD CONSTRAINT model_calls_context_profile_key_check CHECK (
        tokenizer_profile_key IS NULL OR tokenizer_profile_key ~ '^[a-z0-9][a-z0-9._-]{0,127}$'
    ),
    ADD CONSTRAINT model_calls_context_profile_version_check CHECK (
        tokenizer_profile_version IS NULL OR tokenizer_profile_version ~ '^[0-9]+\.[0-9]+\.[0-9]+$'
    ),
    ADD CONSTRAINT model_calls_context_budget_summary_check CHECK (
        context_budget_summary IS NULL OR jsonb_typeof(context_budget_summary) = 'object'
    );

ALTER TABLE model_calls DROP CONSTRAINT model_calls_owner_check;
ALTER TABLE model_calls
    ADD CONSTRAINT model_calls_owner_check
        CHECK (num_nonnulls(conversation_id, agent_run_id, eval_run_id) = 1);

CREATE UNIQUE INDEX idx_model_calls_context_snapshot
    ON model_calls(context_snapshot_id)
    WHERE context_snapshot_id IS NOT NULL;

COMMENT ON COLUMN model_calls.context_snapshot_id IS
    '调用前已持久化成功 ContextSnapshot；历史 ModelCall 可以为空。';
COMMENT ON COLUMN model_calls.context_digest IS
    '与 ContextSnapshot 一致的 canonical digest；prepared 后不可改变。';
COMMENT ON COLUMN model_calls.context_budget_summary IS
    'Context BudgetLedger 的审计摘要；prepared 后不可改变。';

CREATE FUNCTION reject_model_call_context_evidence_mutation() RETURNS TRIGGER AS $$
BEGIN
    IF ROW(
        NEW.context_snapshot_id, NEW.context_digest, NEW.context_policy_key,
        NEW.context_policy_version, NEW.tokenizer_profile_key,
        NEW.tokenizer_profile_version, NEW.context_budget_summary
    ) IS DISTINCT FROM ROW(
        OLD.context_snapshot_id, OLD.context_digest, OLD.context_policy_key,
        OLD.context_policy_version, OLD.tokenizer_profile_key,
        OLD.tokenizer_profile_version, OLD.context_budget_summary
    ) THEN
        RAISE EXCEPTION 'model call context evidence is immutable';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER model_calls_context_evidence_immutable
    BEFORE UPDATE ON model_calls
    FOR EACH ROW EXECUTE FUNCTION reject_model_call_context_evidence_mutation();
