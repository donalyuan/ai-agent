-- Move private TOS staging from ASR models into one versioned system tool.

CREATE TABLE tos_staging_tool_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    version BIGINT NOT NULL,
    is_current BOOLEAN NOT NULL DEFAULT FALSE,
    is_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    storage_provider VARCHAR(40) NOT NULL DEFAULT 'volcengine_tos',
    endpoint TEXT NOT NULL,
    region VARCHAR(80) NOT NULL,
    bucket VARCHAR(160) NOT NULL,
    object_prefix VARCHAR(240) NOT NULL,
    access_key TEXT NOT NULL,
    secret_key TEXT NOT NULL,
    signed_url_ttl_seconds INT NOT NULL,
    max_file_bytes BIGINT NOT NULL,
    max_audio_duration_seconds INT NOT NULL,
    last_check_status VARCHAR(20) NOT NULL DEFAULT 'never',
    last_checked_at TIMESTAMPTZ,
    last_check_error_summary TEXT,
    migration_source_model_id UUID REFERENCES ai_models(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT tos_staging_tool_configs_version_check CHECK (version > 0),
    CONSTRAINT tos_staging_tool_configs_identity_check CHECK (
        storage_provider = 'volcengine_tos'
        AND endpoint ~ '^https://'
        AND length(trim(region)) > 0
        AND length(trim(bucket)) > 0
        AND length(trim(object_prefix)) > 0
        AND length(trim(access_key)) > 0
        AND length(trim(secret_key)) > 0
    ),
    CONSTRAINT tos_staging_tool_configs_limits_check CHECK (
        signed_url_ttl_seconds BETWEEN 60 AND 3600
        AND max_file_bytes > 0
        AND max_audio_duration_seconds > 0
    ),
    CONSTRAINT tos_staging_tool_configs_check_status_check CHECK (
        last_check_status IN ('never', 'succeeded', 'failed')
    ),
    CONSTRAINT tos_staging_tool_configs_check_result_check CHECK (
        (last_check_status = 'never' AND last_checked_at IS NULL AND last_check_error_summary IS NULL)
        OR (last_check_status = 'succeeded' AND last_checked_at IS NOT NULL AND last_check_error_summary IS NULL)
        OR (last_check_status = 'failed' AND last_checked_at IS NOT NULL AND length(trim(last_check_error_summary)) > 0)
    ),
    UNIQUE (version),
    UNIQUE (id, version)
);

CREATE UNIQUE INDEX tos_staging_tool_configs_one_current
    ON tos_staging_tool_configs(is_current)
    WHERE is_current = TRUE;

CREATE TRIGGER trigger_tos_staging_tool_configs_updated_at
    BEFORE UPDATE ON tos_staging_tool_configs
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

COMMENT ON TABLE tos_staging_tool_configs IS
    'Admin 工具与 MCP 中的系统公用私有 TOS 暂存配置；新任务锁定不可变版本。';
COMMENT ON COLUMN tos_staging_tool_configs.access_key IS
    'TOS Access Key 明文；管理响应、任务快照和日志禁止返回。';
COMMENT ON COLUMN tos_staging_tool_configs.secret_key IS
    'TOS Secret Key 明文；管理响应、任务快照和日志禁止返回。';

WITH candidates AS (
    SELECT *
    FROM ai_models
    WHERE model_type = 'speech'
      AND api_protocol = 'volcengine_asr_v3'
      AND staging_storage_provider IS NOT NULL
), selected AS (
    SELECT id
    FROM candidates
    ORDER BY is_default DESC, (status = 'enabled') DESC, sort_order, created_at, id
    LIMIT 1
), ranked AS (
    SELECT
        candidates.*,
        ROW_NUMBER() OVER (
            ORDER BY (candidates.id = (SELECT id FROM selected)), candidates.created_at, candidates.id
        ) AS migrated_version,
        candidates.id = (SELECT id FROM selected) AS selected_current
    FROM candidates
)
INSERT INTO tos_staging_tool_configs (
    version, is_current, is_enabled, storage_provider, endpoint, region, bucket,
    object_prefix, access_key, secret_key, signed_url_ttl_seconds,
    max_file_bytes, max_audio_duration_seconds, migration_source_model_id
)
SELECT
    migrated_version,
    selected_current,
    selected_current AND status = 'enabled',
    staging_storage_provider,
    staging_endpoint,
    staging_region,
    staging_bucket,
    staging_object_prefix,
    staging_access_key,
    staging_secret_key,
    staging_signed_url_ttl_seconds,
    staging_max_file_bytes,
    staging_max_audio_duration_seconds,
    id
FROM ranked;

ALTER TABLE sound_subtitle_tasks
    ADD COLUMN tos_staging_config_id UUID,
    ADD COLUMN tos_staging_config_version BIGINT;

UPDATE sound_subtitle_tasks AS task
SET
    tos_staging_config_id = config.id,
    tos_staging_config_version = config.version
FROM tos_staging_tool_configs AS config
WHERE task.task_type = 'asr'
  AND config.migration_source_model_id = task.model_id;

ALTER TABLE sound_subtitle_tasks
    ADD CONSTRAINT sound_subtitle_tasks_tos_config_fk
        FOREIGN KEY (tos_staging_config_id, tos_staging_config_version)
        REFERENCES tos_staging_tool_configs(id, version) ON DELETE RESTRICT,
    ADD CONSTRAINT sound_subtitle_tasks_tos_config_check CHECK (
        (
            task_type = 'asr'
            AND tos_staging_config_id IS NOT NULL
            AND tos_staging_config_version IS NOT NULL
        ) OR (
            task_type <> 'asr'
            AND tos_staging_config_id IS NULL
            AND tos_staging_config_version IS NULL
        )
    );

CREATE INDEX idx_sound_subtitle_tasks_tos_config
    ON sound_subtitle_tasks(tos_staging_config_id, created_at DESC)
    WHERE tos_staging_config_id IS NOT NULL;

COMMENT ON COLUMN sound_subtitle_tasks.tos_staging_config_id IS
    'ASR 任务创建时锁定的系统 TOS 工具配置 ID。';
COMMENT ON COLUMN sound_subtitle_tasks.tos_staging_config_version IS
    'ASR 任务创建时锁定的系统 TOS 工具配置版本；Worker 禁止回退到当前配置。';

ALTER TABLE ai_models
    DROP CONSTRAINT ai_models_asr_staging_config_check,
    DROP COLUMN staging_storage_provider,
    DROP COLUMN staging_endpoint,
    DROP COLUMN staging_region,
    DROP COLUMN staging_bucket,
    DROP COLUMN staging_object_prefix,
    DROP COLUMN staging_access_key,
    DROP COLUMN staging_secret_key,
    DROP COLUMN staging_signed_url_ttl_seconds,
    DROP COLUMN staging_max_file_bytes,
    DROP COLUMN staging_max_audio_duration_seconds;
