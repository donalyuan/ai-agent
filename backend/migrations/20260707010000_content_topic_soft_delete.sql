-- Soft-delete support for content topic management views.
-- `deleted_at` hides an item from default management lists without changing
-- its business lifecycle status such as idea/approved/scripted/archived.

ALTER TABLE content_topics
    ADD COLUMN deleted_at TIMESTAMPTZ;

COMMENT ON COLUMN content_topics.deleted_at IS '选题从管理视图移除的软删除时间；不改变选题业务状态。';

CREATE INDEX idx_content_topics_visible_project
    ON content_topics(project_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX idx_content_topics_visible_batch
    ON content_topics(batch_id)
    WHERE batch_id IS NOT NULL AND deleted_at IS NULL;
