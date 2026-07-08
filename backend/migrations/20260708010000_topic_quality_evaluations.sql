-- Topic quality gate snapshots for pre-ingest candidate filtering.
-- A snapshot records the quality gate decision before passed candidates are
-- written into content_topics; rejected candidates remain only in this report.

CREATE TABLE topic_quality_evaluations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    batch_id UUID NOT NULL REFERENCES topic_generation_batches(id) ON DELETE CASCADE,
    source_run_id UUID REFERENCES agent_runs(id) ON DELETE SET NULL,
    status VARCHAR(20) NOT NULL,
    pass_count INT NOT NULL DEFAULT 0,
    reject_count INT NOT NULL DEFAULT 0,
    rewrite_triggered BOOLEAN NOT NULL DEFAULT FALSE,
    result JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT topic_quality_evaluations_status_check CHECK (
        status IN ('succeeded', 'failed')
    )
);

COMMENT ON TABLE topic_quality_evaluations IS '选题候选入库前质量闸门评估快照，记录通过项与淘汰项。';
COMMENT ON COLUMN topic_quality_evaluations.batch_id IS '本次质量评估对应的 topic_generation_batches.id。';
COMMENT ON COLUMN topic_quality_evaluations.source_run_id IS '触发本次质量评估的 agent_runs.id，用于运行追踪。';
COMMENT ON COLUMN topic_quality_evaluations.result IS '结构化质量报告，包含摘要、候选质量分、决策、风险标记和原因。';
COMMENT ON COLUMN topic_quality_evaluations.rewrite_triggered IS '本批候选是否因首轮低通过率触发过一次自动重写。';

CREATE INDEX idx_topic_quality_evaluations_project_batch_created
    ON topic_quality_evaluations(project_id, batch_id, created_at DESC, id DESC);
CREATE INDEX idx_topic_quality_evaluations_source_run
    ON topic_quality_evaluations(source_run_id)
    WHERE source_run_id IS NOT NULL;
CREATE INDEX idx_topic_quality_evaluations_status
    ON topic_quality_evaluations(status);

CREATE TRIGGER trigger_topic_quality_evaluations_updated_at
    BEFORE UPDATE ON topic_quality_evaluations
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();
