-- Lock the validated script narration source used by TTS tasks without trusting client snapshots.

ALTER TABLE sound_subtitle_tasks
    ADD COLUMN source_script_id UUID REFERENCES scripts(id) ON DELETE RESTRICT,
    ADD COLUMN source_script_snapshot JSONB;

ALTER TABLE sound_subtitle_tasks
    ADD CONSTRAINT sound_subtitle_tasks_script_source_pair_check CHECK (
        (source_script_id IS NULL AND source_script_snapshot IS NULL)
        OR (
            source_script_id IS NOT NULL
            AND source_script_snapshot IS NOT NULL
            AND jsonb_typeof(source_script_snapshot) = 'object'
        )
    );

CREATE INDEX idx_sound_subtitle_tasks_source_script
    ON sound_subtitle_tasks(source_script_id, created_at DESC)
    WHERE source_script_id IS NOT NULL;

COMMENT ON COLUMN sound_subtitle_tasks.source_script_id IS
    '旁白来源脚本；仅在应用服务重新读取并校验脚本后写入。';
COMMENT ON COLUMN sound_subtitle_tasks.source_script_snapshot IS
    '不可变来源快照，包含脚本版本及按 sequence 排序的所选分镜原始旁白。';
