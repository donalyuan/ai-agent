-- Source deletion may redact case-level evidence, but every other completed report field stays immutable.
CREATE OR REPLACE FUNCTION reject_completed_eval_report_mutation() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'UPDATE'
       AND OLD.source_deleted = FALSE
       AND NEW.source_deleted = TRUE
       AND NEW.redacted_case_results = '[]'::jsonb
       AND ROW(
           NEW.id,
           NEW.eval_run_id,
           NEW.schema_version,
           NEW.passed,
           NEW.gate_results,
           NEW.aggregate_metrics,
           NEW.completed_at,
           NEW.context_node_results,
           NEW.context_selection_diff,
           NEW.context_budget_ledgers,
           NEW.tokenizer_metrics
       ) IS NOT DISTINCT FROM ROW(
           OLD.id,
           OLD.eval_run_id,
           OLD.schema_version,
           OLD.passed,
           OLD.gate_results,
           OLD.aggregate_metrics,
           OLD.completed_at,
           OLD.context_node_results,
           OLD.context_selection_diff,
           OLD.context_budget_ledgers,
           OLD.tokenizer_metrics
       ) THEN
        RETURN NEW;
    END IF;
    RAISE EXCEPTION 'completed eval reports are immutable';
END;
$$ LANGUAGE plpgsql;
