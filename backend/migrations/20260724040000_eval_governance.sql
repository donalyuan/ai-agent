-- Candidate evaluation runs freeze approval and budget evidence before any paid call.
ALTER TABLE eval_runs
    ADD COLUMN validation_mode VARCHAR(32) NOT NULL DEFAULT 'zero_cost',
    ADD COLUMN approval_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN actual_cases INT NOT NULL DEFAULT 0,
    ADD COLUMN actual_input_tokens BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN actual_output_tokens BIGINT NOT NULL DEFAULT 0,
    ADD COLUMN actual_retries INT NOT NULL DEFAULT 0,
    ADD COLUMN actual_cost_micros BIGINT NOT NULL DEFAULT 0;

ALTER TABLE eval_runs DROP CONSTRAINT eval_runs_real_confirmation_check;
ALTER TABLE eval_runs
    ADD CONSTRAINT eval_runs_validation_mode_check
        CHECK (validation_mode IN ('golden_baseline', 'zero_cost', 'real_model')),
    ADD CONSTRAINT eval_runs_candidate_digest_check
        CHECK (candidate_digest ~ '^[0-9a-f]{64}$'),
    ADD CONSTRAINT eval_runs_fingerprint_check
        CHECK (behavior_fingerprint IS NULL OR behavior_fingerprint ~ '^[0-9a-f]{64}$'),
    ADD CONSTRAINT eval_runs_mode_approval_check CHECK (
        (validation_mode = 'real_model'
            AND approved_real_calls
            AND model_id IS NOT NULL
            AND behavior_fingerprint IS NOT NULL)
        OR
        (validation_mode IN ('golden_baseline', 'zero_cost')
            AND NOT approved_real_calls
            AND model_id IS NULL
            AND behavior_fingerprint IS NULL
            AND max_cost_micros = 0)
    ),
    ADD CONSTRAINT eval_runs_actual_budget_check CHECK (
        actual_cases >= 0 AND actual_cases <= max_cases
        AND actual_input_tokens >= 0 AND actual_input_tokens <= max_input_tokens
        AND actual_output_tokens >= 0 AND actual_output_tokens <= max_output_tokens
        AND actual_retries >= 0 AND actual_retries <= max_retries
        AND actual_cost_micros >= 0 AND actual_cost_micros <= max_cost_micros
        AND actual_real_model_calls >= 0
    );

CREATE FUNCTION enforce_eval_run_transition() RETURNS TRIGGER AS $$
BEGIN
    IF ROW(
        NEW.candidate_key, NEW.candidate_version, NEW.candidate_digest,
        NEW.baseline_key, NEW.baseline_version, NEW.case_set_version,
        NEW.evaluator_version, NEW.validation_mode, NEW.model_id,
        NEW.behavior_fingerprint, NEW.approved_real_calls, NEW.approval_snapshot,
        NEW.max_cases, NEW.max_input_tokens, NEW.max_output_tokens,
        NEW.max_retries, NEW.max_cost_micros
    ) IS DISTINCT FROM ROW(
        OLD.candidate_key, OLD.candidate_version, OLD.candidate_digest,
        OLD.baseline_key, OLD.baseline_version, OLD.case_set_version,
        OLD.evaluator_version, OLD.validation_mode, OLD.model_id,
        OLD.behavior_fingerprint, OLD.approved_real_calls, OLD.approval_snapshot,
        OLD.max_cases, OLD.max_input_tokens, OLD.max_output_tokens,
        OLD.max_retries, OLD.max_cost_micros
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

CREATE TRIGGER eval_runs_immutable_configuration
    BEFORE UPDATE OR DELETE ON eval_runs
    FOR EACH ROW EXECUTE FUNCTION enforce_eval_run_transition();

COMMENT ON COLUMN eval_runs.approval_snapshot IS '创建 EvalRun 时固定的模型、case/token/retry/cost 预算确认；后续不可覆盖。';
COMMENT ON COLUMN eval_runs.validation_mode IS 'golden_baseline 与 zero_cost 必须保持零真实调用；real_model 必须显式批准。';
