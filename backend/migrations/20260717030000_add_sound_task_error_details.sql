-- Persist only the provider diagnostics that are safe to expose to operators.

ALTER TABLE sound_subtitle_tasks
    ADD COLUMN error_details JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD CONSTRAINT sound_subtitle_tasks_error_details_check CHECK (
        jsonb_typeof(error_details) = 'object'
        AND (
            error_details - ARRAY[
                'http_status',
                'provider_error_code',
                'provider_error_message'
            ]::text[]
        ) = '{}'::jsonb
        AND (
            NOT error_details ? 'http_status'
            OR jsonb_typeof(error_details -> 'http_status') = 'number'
        )
        AND (
            NOT error_details ? 'provider_error_code'
            OR jsonb_typeof(error_details -> 'provider_error_code') = 'string'
        )
        AND (
            NOT error_details ? 'provider_error_message'
            OR jsonb_typeof(error_details -> 'provider_error_message') = 'string'
        )
    );

COMMENT ON COLUMN sound_subtitle_tasks.error_details IS
    '失败任务的脱敏诊断白名单，仅允许 HTTP 状态、供应商错误码和供应商错误消息；禁止保存原始请求头、响应头、响应体或凭据。';
COMMENT ON CONSTRAINT sound_subtitle_tasks_error_details_check ON sound_subtitle_tasks IS
    '强制结构化失败诊断为 JSON object，并拒绝白名单之外的字段。';
