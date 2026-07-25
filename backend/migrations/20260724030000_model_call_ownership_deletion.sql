-- Preserve aggregate evaluation evidence while deleting explicitly owned ModelCall content.
ALTER TABLE model_calls DROP CONSTRAINT model_calls_root_call_id_fkey;
ALTER TABLE model_calls DROP CONSTRAINT model_calls_parent_call_id_fkey;
ALTER TABLE model_calls
    ADD CONSTRAINT model_calls_root_call_id_fkey
    FOREIGN KEY (root_call_id) REFERENCES model_calls(id) ON DELETE CASCADE;
ALTER TABLE model_calls
    ADD CONSTRAINT model_calls_parent_call_id_fkey
    FOREIGN KEY (parent_call_id) REFERENCES model_calls(id) ON DELETE CASCADE;

CREATE TABLE eval_report_sources (
    eval_report_id UUID NOT NULL REFERENCES eval_reports(id) ON DELETE CASCADE,
    model_call_id UUID NOT NULL REFERENCES model_calls(id) ON DELETE CASCADE,
    PRIMARY KEY (eval_report_id, model_call_id)
);

COMMENT ON TABLE eval_report_sources IS '不可变 EvalReport 到源 ModelCall 的所有权索引；源删除时只保留聚合指标。';

CREATE FUNCTION mark_eval_report_model_call_source_deleted() RETURNS TRIGGER AS $$
BEGIN
    UPDATE eval_reports AS report
    SET source_deleted = TRUE,
        redacted_case_results = '[]'::jsonb
    WHERE report.source_deleted = FALSE
      AND EXISTS (
          SELECT 1 FROM eval_report_sources AS source
          WHERE source.eval_report_id = report.id
            AND source.model_call_id = OLD.id
      );
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER model_calls_mark_eval_report_source_deleted
    BEFORE DELETE ON model_calls
    FOR EACH ROW EXECUTE FUNCTION mark_eval_report_model_call_source_deleted();
