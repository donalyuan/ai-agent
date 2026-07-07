-- Content topic pool for VEDIO-AGENT phase 2.
-- Topics are concrete content ideas under a project, separate from projects
-- and linked to scripts only after an operator approves them.

CREATE TABLE topic_generation_batches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_run_id UUID REFERENCES agent_runs(id) ON DELETE SET NULL,
    prompt TEXT NOT NULL,
    requested_count INT NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'succeeded',
    error_message TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT topic_generation_batches_requested_count_check CHECK (
        requested_count > 0 AND requested_count <= 20
    ),
    CONSTRAINT topic_generation_batches_status_check CHECK (
        status IN ('running', 'succeeded', 'failed')
    )
);

COMMENT ON TABLE topic_generation_batches IS '选题 Agent 批量生成记录，用于追溯一次候选选题生成。';
COMMENT ON COLUMN topic_generation_batches.prompt IS '操作者补充要求和上下文摘要。';
COMMENT ON COLUMN topic_generation_batches.source_run_id IS '对应通用 Agent Runtime 的 agent_runs.id。';

CREATE INDEX idx_topic_generation_batches_project
    ON topic_generation_batches(project_id, created_at DESC);
CREATE INDEX idx_topic_generation_batches_run
    ON topic_generation_batches(source_run_id)
    WHERE source_run_id IS NOT NULL;
CREATE INDEX idx_topic_generation_batches_status
    ON topic_generation_batches(status);

CREATE TRIGGER trigger_topic_generation_batches_updated_at
    BEFORE UPDATE ON topic_generation_batches
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE content_topics (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    batch_id UUID REFERENCES topic_generation_batches(id) ON DELETE SET NULL,
    title VARCHAR(160) NOT NULL,
    angle TEXT NOT NULL DEFAULT '',
    target_audience TEXT NOT NULL DEFAULT '',
    hook_points TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    content_type VARCHAR(80) NOT NULL DEFAULT '',
    score DOUBLE PRECISION,
    score_reason TEXT NOT NULL DEFAULT '',
    tags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    source VARCHAR(20) NOT NULL DEFAULT 'manual',
    status VARCHAR(20) NOT NULL DEFAULT 'idea',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT content_topics_title_check CHECK (length(btrim(title)) > 0),
    CONSTRAINT content_topics_score_check CHECK (score IS NULL OR (score >= 0 AND score <= 100)),
    CONSTRAINT content_topics_source_check CHECK (source IN ('manual', 'agent')),
    CONSTRAINT content_topics_status_check CHECK (
        status IN ('idea', 'approved', 'scripted', 'archived')
    )
);

COMMENT ON TABLE content_topics IS '具体内容选题池，归属项目并跟踪从 idea 到 scripted 的状态。';
COMMENT ON COLUMN content_topics.batch_id IS 'Agent 批量生成时关联的 topic_generation_batches.id，人工选题为空。';
COMMENT ON COLUMN content_topics.hook_points IS '该选题的主要看点，可由人工或 Agent 生成。';
COMMENT ON COLUMN content_topics.score IS '0 到 100 的选题评分；人工选题可为空。';
COMMENT ON COLUMN content_topics.metadata IS '选题上下文扩展字段，保留生成提示、策略摘要等快照。';

CREATE INDEX idx_content_topics_project
    ON content_topics(project_id, created_at DESC);
CREATE INDEX idx_content_topics_status
    ON content_topics(status);
CREATE INDEX idx_content_topics_source
    ON content_topics(source);
CREATE INDEX idx_content_topics_batch
    ON content_topics(batch_id)
    WHERE batch_id IS NOT NULL;
CREATE INDEX idx_content_topics_created
    ON content_topics(created_at DESC);
CREATE INDEX idx_content_topics_tags
    ON content_topics USING GIN(tags);

CREATE TRIGGER trigger_content_topics_updated_at
    BEFORE UPDATE ON content_topics
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

ALTER TABLE scripts
    ADD COLUMN topic_id UUID REFERENCES content_topics(id) ON DELETE SET NULL;

COMMENT ON COLUMN scripts.topic_id IS '脚本由已确认选题生成时关联 content_topics.id；删除选题后保留脚本并置空。';

CREATE INDEX idx_scripts_topic
    ON scripts(topic_id)
    WHERE topic_id IS NOT NULL;
