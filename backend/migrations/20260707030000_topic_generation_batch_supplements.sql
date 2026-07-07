-- Supplement batch relationship for content strategy history.
-- A supplement is a new generation event that remains auditable while pointing
-- back to the original historical batch context.

ALTER TABLE topic_generation_batches
    ADD COLUMN supplement_of_batch_id UUID REFERENCES topic_generation_batches(id) ON DELETE SET NULL;

COMMENT ON COLUMN topic_generation_batches.supplement_of_batch_id IS '补充生成批次关联的原始 topic_generation_batches.id；普通原始批次为空。';

CREATE INDEX idx_topic_generation_batches_supplement_of
    ON topic_generation_batches(supplement_of_batch_id, created_at DESC)
    WHERE supplement_of_batch_id IS NOT NULL;
