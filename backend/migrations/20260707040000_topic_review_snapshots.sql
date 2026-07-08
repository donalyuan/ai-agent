-- Topic-group review snapshots for content strategy screening.
-- A snapshot belongs to an original topic generation batch and records AI
-- decision-support output without mutating content topic lifecycle status.

CREATE TABLE topic_review_snapshots (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    root_batch_id UUID NOT NULL REFERENCES topic_generation_batches(id) ON DELETE CASCADE,
    source_run_id UUID REFERENCES agent_runs(id) ON DELETE SET NULL,
    status VARCHAR(20) NOT NULL,
    review_summary TEXT NOT NULL DEFAULT '',
    result JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_message TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT topic_review_snapshots_status_check CHECK (
        status IN ('succeeded', 'failed')
    )
);

COMMENT ON TABLE topic_review_snapshots IS '主题组选题 AI 评审快照，按原始批次聚合原始和补充批次选题。';
COMMENT ON COLUMN topic_review_snapshots.root_batch_id IS '主题组原始 topic_generation_batches.id。';
COMMENT ON COLUMN topic_review_snapshots.source_run_id IS '触发本次评审的 agent_runs.id，用于运行追踪。';
COMMENT ON COLUMN topic_review_snapshots.result IS '结构化评审结果，包含选题层级、风险标记和相似选题引用。';

CREATE INDEX idx_topic_review_snapshots_project_root_latest
    ON topic_review_snapshots(project_id, root_batch_id, created_at DESC, id DESC);
CREATE INDEX idx_topic_review_snapshots_run
    ON topic_review_snapshots(source_run_id)
    WHERE source_run_id IS NOT NULL;
CREATE INDEX idx_topic_review_snapshots_status
    ON topic_review_snapshots(status);

CREATE TRIGGER trigger_topic_review_snapshots_updated_at
    BEFORE UPDATE ON topic_review_snapshots
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();
