-- Real evaluation attempts reuse the same immutable ModelCall audit path as production calls.
ALTER TABLE model_calls
    ADD COLUMN eval_run_id UUID REFERENCES eval_runs(id) ON DELETE RESTRICT;

ALTER TABLE model_calls DROP CONSTRAINT model_calls_owner_check;
ALTER TABLE model_calls
    ADD CONSTRAINT model_calls_owner_check
        CHECK (num_nonnulls(conversation_id, agent_run_id, eval_run_id) = 1);

CREATE INDEX idx_model_calls_eval_run
    ON model_calls(eval_run_id, prepared_at DESC)
    WHERE eval_run_id IS NOT NULL;

CREATE FUNCTION reject_model_call_eval_owner_mutation() RETURNS TRIGGER AS $$
BEGIN
    IF NEW.eval_run_id IS DISTINCT FROM OLD.eval_run_id THEN
        RAISE EXCEPTION 'model call eval owner is immutable';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER model_calls_eval_owner_immutable
    BEFORE UPDATE ON model_calls
    FOR EACH ROW EXECUTE FUNCTION reject_model_call_eval_owner_mutation();

COMMENT ON COLUMN model_calls.eval_run_id IS '真实评测 attempt 的唯一 EvalRun owner；预算预留与 prepared 记录在同一事务完成。';
