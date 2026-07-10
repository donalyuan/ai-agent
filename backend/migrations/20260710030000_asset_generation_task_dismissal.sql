ALTER TABLE asset_generation_tasks
    ADD COLUMN dismissed_at TIMESTAMPTZ;

COMMENT ON COLUMN asset_generation_tasks.dismissed_at IS
    '失败任务从素材生成页面隐藏的时间；任务、错误、候选和费用审计仍保留。';

CREATE INDEX idx_asset_generation_tasks_visible_script
    ON asset_generation_tasks(script_id, created_at ASC, id ASC)
    WHERE dismissed_at IS NULL;

COMMENT ON INDEX idx_asset_generation_tasks_visible_script IS
    '支持按脚本读取未清理的素材生成任务。';
