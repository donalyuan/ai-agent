ALTER TABLE tos_staging_tool_configs
    DROP CONSTRAINT tos_staging_tool_configs_check_status_check,
    DROP CONSTRAINT tos_staging_tool_configs_check_result_check,
    ADD COLUMN last_check_requested_at TIMESTAMPTZ,
    ADD COLUMN check_locked_at TIMESTAMPTZ,
    ADD COLUMN check_worker_id VARCHAR(160);

UPDATE tos_staging_tool_configs
SET last_check_requested_at = COALESCE(last_checked_at, updated_at)
WHERE last_check_status IN ('succeeded', 'failed');

ALTER TABLE tos_staging_tool_configs
    ADD CONSTRAINT tos_staging_tool_configs_check_status_check CHECK (
        last_check_status IN ('never', 'queued', 'running', 'succeeded', 'failed')
    ),
    ADD CONSTRAINT tos_staging_tool_configs_check_result_check CHECK (
        (
            last_check_status = 'never'
            AND last_check_requested_at IS NULL
            AND last_checked_at IS NULL
            AND last_check_error_summary IS NULL
            AND check_locked_at IS NULL
            AND check_worker_id IS NULL
        ) OR (
            last_check_status = 'queued'
            AND last_check_requested_at IS NOT NULL
            AND last_checked_at IS NULL
            AND last_check_error_summary IS NULL
            AND check_locked_at IS NULL
            AND check_worker_id IS NULL
        ) OR (
            last_check_status = 'running'
            AND last_check_requested_at IS NOT NULL
            AND last_checked_at IS NULL
            AND last_check_error_summary IS NULL
            AND check_locked_at IS NOT NULL
            AND length(trim(check_worker_id)) > 0
        ) OR (
            last_check_status = 'succeeded'
            AND last_check_requested_at IS NOT NULL
            AND last_checked_at IS NOT NULL
            AND last_check_error_summary IS NULL
            AND check_locked_at IS NULL
            AND check_worker_id IS NULL
        ) OR (
            last_check_status = 'failed'
            AND last_check_requested_at IS NOT NULL
            AND last_checked_at IS NOT NULL
            AND length(trim(last_check_error_summary)) > 0
            AND check_locked_at IS NULL
            AND check_worker_id IS NULL
        )
    );

CREATE INDEX idx_tos_staging_tool_configs_pending_check
    ON tos_staging_tool_configs(last_check_requested_at, id)
    WHERE last_check_status IN ('queued', 'running');

COMMENT ON COLUMN tos_staging_tool_configs.last_check_status IS
    '真实 Bucket 连接检查状态；queued/running 由语音 Worker 执行，禁止用本地字段校验写入 succeeded。';
COMMENT ON COLUMN tos_staging_tool_configs.last_check_requested_at IS
    '管理员最后一次发起真实 Bucket 连接检查的时间。';
