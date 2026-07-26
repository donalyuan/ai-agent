-- Context Policy/Profile evaluations freeze cross-runtime case evidence before activation.
ALTER TABLE eval_runs
    ADD COLUMN definition_kind VARCHAR(32) NOT NULL DEFAULT 'agent',
    ADD COLUMN context_case_set JSONB,
    ADD COLUMN context_policy JSONB,
    ADD COLUMN tokenizer_profile JSONB,
    ADD CONSTRAINT eval_runs_definition_kind_check CHECK (
        definition_kind IN ('agent', 'prompt', 'context_policy', 'tokenizer_profile')
    ),
    ADD CONSTRAINT eval_runs_context_evidence_check CHECK (
        (
            definition_kind IN ('context_policy', 'tokenizer_profile')
            AND jsonb_typeof(context_case_set) = 'object'
            AND jsonb_typeof(context_policy) = 'object'
            AND jsonb_typeof(tokenizer_profile) = 'object'
        )
        OR
        (
            definition_kind IN ('agent', 'prompt')
            AND context_case_set IS NULL
            AND context_policy IS NULL
            AND tokenizer_profile IS NULL
        )
    );

ALTER TABLE eval_reports
    ADD COLUMN context_node_results JSONB,
    ADD COLUMN context_selection_diff JSONB,
    ADD COLUMN context_budget_ledgers JSONB,
    ADD COLUMN tokenizer_metrics JSONB,
    ADD CONSTRAINT eval_reports_context_evidence_check CHECK (
        (
            context_node_results IS NULL
            AND context_selection_diff IS NULL
            AND context_budget_ledgers IS NULL
            AND tokenizer_metrics IS NULL
        )
        OR
        (
            jsonb_typeof(context_node_results) = 'array'
            AND jsonb_typeof(context_selection_diff) = 'array'
            AND jsonb_typeof(context_budget_ledgers) = 'array'
            AND jsonb_typeof(tokenizer_metrics) = 'array'
        )
    );

CREATE OR REPLACE FUNCTION enforce_eval_run_transition() RETURNS TRIGGER AS $$
BEGIN
    IF ROW(
        NEW.definition_kind, NEW.candidate_key, NEW.candidate_version, NEW.candidate_digest,
        NEW.baseline_key, NEW.baseline_version, NEW.case_set_version,
        NEW.evaluator_version, NEW.validation_mode, NEW.model_id,
        NEW.behavior_fingerprint, NEW.approved_real_calls, NEW.approval_snapshot,
        NEW.max_cases, NEW.max_input_tokens, NEW.max_output_tokens,
        NEW.max_retries, NEW.max_cost_micros, NEW.context_case_set,
        NEW.context_policy, NEW.tokenizer_profile
    ) IS DISTINCT FROM ROW(
        OLD.definition_kind, OLD.candidate_key, OLD.candidate_version, OLD.candidate_digest,
        OLD.baseline_key, OLD.baseline_version, OLD.case_set_version,
        OLD.evaluator_version, OLD.validation_mode, OLD.model_id,
        OLD.behavior_fingerprint, OLD.approved_real_calls, OLD.approval_snapshot,
        OLD.max_cases, OLD.max_input_tokens, OLD.max_output_tokens,
        OLD.max_retries, OLD.max_cost_micros, OLD.context_case_set,
        OLD.context_policy, OLD.tokenizer_profile
    ) THEN
        RAISE EXCEPTION 'eval run approval and configuration are immutable';
    END IF;
    IF OLD.status IN ('passed', 'failed', 'blocked', 'budget_exhausted') THEN
        RAISE EXCEPTION 'completed eval run is immutable';
    END IF;
    IF NEW.actual_cases < OLD.actual_cases
       OR NEW.actual_input_tokens < OLD.actual_input_tokens
       OR NEW.actual_output_tokens < OLD.actual_output_tokens
       OR NEW.actual_retries < OLD.actual_retries
       OR NEW.actual_cost_micros < OLD.actual_cost_micros
       OR NEW.actual_real_model_calls < OLD.actual_real_model_calls THEN
        RAISE EXCEPTION 'eval run usage cannot decrease';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION reject_completed_eval_report_mutation() RETURNS TRIGGER AS $$
BEGIN
    IF OLD.source_deleted = FALSE AND NEW.source_deleted = TRUE
       AND NEW.gate_results = OLD.gate_results
       AND NEW.aggregate_metrics = OLD.aggregate_metrics
       AND NEW.redacted_case_results = '[]'::jsonb
       AND NEW.context_node_results = OLD.context_node_results
       AND NEW.context_selection_diff = OLD.context_selection_diff
       AND NEW.context_budget_ledgers = OLD.context_budget_ledgers
       AND NEW.tokenizer_metrics = OLD.tokenizer_metrics THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'completed eval reports are immutable';
END;
$$ LANGUAGE plpgsql;

COMMENT ON COLUMN eval_runs.definition_kind IS
    '候选定义类型；Context Policy/Profile 必须固定 Context case set、Policy 与 Tokenizer Profile。';
COMMENT ON COLUMN eval_reports.context_selection_diff IS
    'Context candidate 相对 baseline 的逐 case 选择差异；完成后不可覆盖。';
COMMENT ON COLUMN eval_reports.context_budget_ledgers IS
    '逐 case BudgetLedger 审计证据；完成后不可覆盖。';
COMMENT ON COLUMN eval_reports.tokenizer_metrics IS
    '逐 case 跨语言 token、模式与预算浪费指标；完成后不可覆盖。';
