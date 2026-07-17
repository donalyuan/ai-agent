-- Add speech models, versioned voice catalogs and auditable sound/subtitle tasks.

ALTER TABLE ai_models
    ADD COLUMN catalog_access_key TEXT,
    ADD COLUMN catalog_secret_key TEXT,
    ADD COLUMN staging_storage_provider VARCHAR(40),
    ADD COLUMN staging_endpoint TEXT,
    ADD COLUMN staging_region VARCHAR(80),
    ADD COLUMN staging_bucket VARCHAR(160),
    ADD COLUMN staging_object_prefix VARCHAR(240),
    ADD COLUMN staging_access_key TEXT,
    ADD COLUMN staging_secret_key TEXT,
    ADD COLUMN staging_signed_url_ttl_seconds INT,
    ADD COLUMN staging_max_file_bytes BIGINT,
    ADD COLUMN staging_max_audio_duration_seconds INT;

ALTER TABLE ai_models
    DROP CONSTRAINT ai_models_type_check,
    DROP CONSTRAINT ai_models_protocol_check,
    DROP CONSTRAINT ai_models_auth_scheme_check,
    DROP CONSTRAINT ai_models_type_protocol_check;

ALTER TABLE ai_models
    ADD CONSTRAINT ai_models_type_check CHECK (
        model_type IN ('text', 'image', 'video', 'speech')
    ),
    ADD CONSTRAINT ai_models_protocol_check CHECK (
        api_protocol IN (
            'openai_responses', 'openai_chat_completions', 'openai_images',
            'volcengine_ark_images', 'runway_api', 'kling_api',
            'volcengine_tts_v3', 'volcengine_asr_v3'
        )
    ),
    ADD CONSTRAINT ai_models_auth_scheme_check CHECK (
        auth_scheme IN ('bearer', 'access_key_secret', 'api_key')
    ),
    ADD CONSTRAINT ai_models_type_protocol_check CHECK (
        (model_type = 'text' AND api_protocol IN (
            'openai_responses', 'openai_chat_completions'
        )) OR
        (model_type = 'image' AND api_protocol IN (
            'openai_images', 'volcengine_ark_images'
        )) OR
        (model_type = 'video' AND api_protocol IN ('runway_api', 'kling_api')) OR
        (model_type = 'speech' AND api_protocol IN (
            'volcengine_tts_v3', 'volcengine_asr_v3'
        ))
    ),
    ADD CONSTRAINT ai_models_catalog_credentials_pair_check CHECK (
        (catalog_access_key IS NULL AND catalog_secret_key IS NULL) OR
        (
            length(trim(catalog_access_key)) > 0
            AND length(trim(catalog_secret_key)) > 0
        )
    ),
    ADD CONSTRAINT ai_models_catalog_credentials_protocol_check CHECK (
        (catalog_access_key IS NULL AND catalog_secret_key IS NULL) OR
        (model_type = 'speech' AND api_protocol = 'volcengine_tts_v3')
    ),
    ADD CONSTRAINT ai_models_asr_staging_config_check CHECK (
        (
            model_type = 'speech'
            AND api_protocol = 'volcengine_asr_v3'
            AND staging_storage_provider = 'volcengine_tos'
            AND staging_endpoint ~ '^https://'
            AND length(trim(staging_region)) > 0
            AND length(trim(staging_bucket)) > 0
            AND length(trim(staging_object_prefix)) > 0
            AND length(trim(staging_access_key)) > 0
            AND length(trim(staging_secret_key)) > 0
            AND staging_signed_url_ttl_seconds BETWEEN 60 AND 3600
            AND staging_max_file_bytes > 0
            AND staging_max_audio_duration_seconds > 0
        ) OR (
            api_protocol <> 'volcengine_asr_v3'
            AND staging_storage_provider IS NULL
            AND staging_endpoint IS NULL
            AND staging_region IS NULL
            AND staging_bucket IS NULL
            AND staging_object_prefix IS NULL
            AND staging_access_key IS NULL
            AND staging_secret_key IS NULL
            AND staging_signed_url_ttl_seconds IS NULL
            AND staging_max_file_bytes IS NULL
            AND staging_max_audio_duration_seconds IS NULL
        )
    );

COMMENT ON TABLE ai_models IS
    '文本、图片、视频和语音模型部署注册表；运行时模型配置的唯一来源。';
COMMENT ON COLUMN ai_models.catalog_access_key IS
    'TTS 音色目录 OpenAPI Access Key；管理响应和运行快照不得返回明文。';
COMMENT ON COLUMN ai_models.catalog_secret_key IS
    'TTS 音色目录 OpenAPI Secret Key；管理响应和运行快照不得返回明文。';
COMMENT ON COLUMN ai_models.staging_access_key IS
    'ASR 私有 TOS 暂存 Access Key；不得进入 settings、响应明文、日志或运行快照。';
COMMENT ON COLUMN ai_models.staging_secret_key IS
    'ASR 私有 TOS 暂存 Secret Key；不得进入 settings、响应明文、日志或运行快照。';
COMMENT ON CONSTRAINT ai_models_type_protocol_check ON ai_models IS
    '限制文本、图片、视频和语音模型类型与可执行协议组合。';

DROP INDEX ai_models_one_default_per_type;

CREATE UNIQUE INDEX ai_models_one_default_per_type
    ON ai_models(model_type)
    WHERE is_default = TRUE
      AND deleted_at IS NULL
      AND model_type <> 'speech';

CREATE UNIQUE INDEX ai_models_one_default_per_speech_protocol
    ON ai_models(api_protocol)
    WHERE is_default = TRUE
      AND deleted_at IS NULL
      AND model_type = 'speech';

COMMENT ON INDEX ai_models_one_default_per_type IS
    '现有文本、图片和视频模型继续按类型各维护一个默认模型。';
COMMENT ON INDEX ai_models_one_default_per_speech_protocol IS
    'TTS 与 ASR 按语音协议分别维护默认模型，互不替换。';

ALTER TABLE agent_conversations
    DROP CONSTRAINT agent_conversations_agent_type_check,
    ADD CONSTRAINT agent_conversations_agent_type_check CHECK (
        agent_type IN (
            'topic', 'script', 'material', 'sound', 'video', 'publish', 'optimization'
        )
    );

ALTER TABLE agent_runs
    DROP CONSTRAINT agent_runs_type_check,
    ADD CONSTRAINT agent_runs_type_check CHECK (
        agent_type IN (
            'topic', 'script', 'material', 'sound', 'video', 'publish', 'optimization'
        )
    );

CREATE TABLE voice_catalog_syncs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_id UUID NOT NULL REFERENCES ai_models(id) ON DELETE RESTRICT,
    trigger_source VARCHAR(20) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'queued',
    page_limit INT NOT NULL DEFAULT 30,
    page_count INT NOT NULL DEFAULT 0,
    speaker_count INT NOT NULL DEFAULT 0,
    error_summary TEXT,
    requested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT voice_catalog_syncs_trigger_check CHECK (
        trigger_source IN ('admin', 'scheduled', 'workspace')
    ),
    CONSTRAINT voice_catalog_syncs_status_check CHECK (
        status IN ('queued', 'running', 'succeeded', 'failed')
    ),
    CONSTRAINT voice_catalog_syncs_page_limit_check CHECK (page_limit BETWEEN 1 AND 100),
    CONSTRAINT voice_catalog_syncs_counts_check CHECK (page_count >= 0 AND speaker_count >= 0),
    CONSTRAINT voice_catalog_syncs_terminal_check CHECK (
        (status IN ('queued', 'running') AND completed_at IS NULL) OR
        (status IN ('succeeded', 'failed') AND completed_at IS NOT NULL)
    )
);

CREATE UNIQUE INDEX voice_catalog_syncs_one_active_per_model
    ON voice_catalog_syncs(model_id)
    WHERE status IN ('queued', 'running');
CREATE INDEX idx_voice_catalog_syncs_model_created
    ON voice_catalog_syncs(model_id, created_at DESC);

CREATE TRIGGER trigger_voice_catalog_syncs_updated_at
    BEFORE UPDATE ON voice_catalog_syncs
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

COMMENT ON TABLE voice_catalog_syncs IS
    '音色目录完整同步批次；失败批次不得改变上一次成功目录。';

CREATE TABLE voice_catalog_entries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model_id UUID NOT NULL REFERENCES ai_models(id) ON DELETE RESTRICT,
    voice_type VARCHAR(200) NOT NULL,
    resource_id VARCHAR(120) NOT NULL,
    name VARCHAR(200) NOT NULL,
    avatar_url TEXT,
    gender VARCHAR(40),
    age VARCHAR(40),
    categories JSONB NOT NULL DEFAULT '[]'::jsonb,
    normal_labels TEXT[] NOT NULL DEFAULT '{}',
    special_labels TEXT[] NOT NULL DEFAULT '{}',
    trial_url TEXT,
    short_trial_url TEXT,
    languages JSONB NOT NULL DEFAULT '[]'::jsonb,
    emotions JSONB NOT NULL DEFAULT '[]'::jsonb,
    description TEXT NOT NULL DEFAULT '',
    is_available BOOLEAN NOT NULL DEFAULT TRUE,
    first_seen_sync_id UUID NOT NULL REFERENCES voice_catalog_syncs(id) ON DELETE RESTRICT,
    last_seen_sync_id UUID NOT NULL REFERENCES voice_catalog_syncs(id) ON DELETE RESTRICT,
    catalog_version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT voice_catalog_entries_identity_nonempty CHECK (
        length(trim(voice_type)) > 0
        AND length(trim(resource_id)) > 0
        AND length(trim(name)) > 0
    ),
    CONSTRAINT voice_catalog_entries_json_check CHECK (
        jsonb_typeof(categories) = 'array'
        AND jsonb_typeof(languages) = 'array'
        AND jsonb_typeof(emotions) = 'array'
    ),
    CONSTRAINT voice_catalog_entries_version_check CHECK (catalog_version > 0),
    UNIQUE (model_id, resource_id, voice_type)
);

CREATE INDEX idx_voice_catalog_entries_available
    ON voice_catalog_entries(model_id, resource_id, name, voice_type)
    WHERE is_available = TRUE;

CREATE TRIGGER trigger_voice_catalog_entries_updated_at
    BEFORE UPDATE ON voice_catalog_entries
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

COMMENT ON TABLE voice_catalog_entries IS
    '模型版本下的动态音色目录；下线音色保留记录供历史快照审计。';

CREATE TABLE audio_material_inspections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    material_id UUID NOT NULL REFERENCES materials(id) ON DELETE RESTRICT,
    status VARCHAR(20) NOT NULL DEFAULT 'queued',
    idempotency_key VARCHAR(200) NOT NULL,
    source_sha256 VARCHAR(64),
    file_size_bytes BIGINT,
    duration_ms BIGINT,
    container_format VARCHAR(80),
    audio_codec VARCHAR(80),
    sample_rate_hz INT,
    channel_count INT,
    error_code VARCHAR(120),
    error_summary TEXT,
    locked_at TIMESTAMPTZ,
    worker_id VARCHAR(160),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT audio_material_inspections_status_check CHECK (
        status IN ('queued', 'running', 'succeeded', 'failed')
    ),
    CONSTRAINT audio_material_inspections_idempotency_nonempty CHECK (
        length(trim(idempotency_key)) > 0
    ),
    CONSTRAINT audio_material_inspections_terminal_check CHECK (
        (
            status IN ('queued', 'running')
            AND completed_at IS NULL
        ) OR (
            status = 'succeeded'
            AND completed_at IS NOT NULL
            AND source_sha256 ~ '^[0-9a-f]{64}$'
            AND file_size_bytes > 0
            AND duration_ms > 0
            AND length(trim(container_format)) > 0
            AND length(trim(audio_codec)) > 0
            AND sample_rate_hz > 0
            AND channel_count > 0
        ) OR (
            status = 'failed'
            AND completed_at IS NOT NULL
            AND length(trim(error_code)) > 0
            AND length(trim(error_summary)) > 0
        )
    ),
    UNIQUE (project_id, material_id, idempotency_key)
);

CREATE UNIQUE INDEX audio_material_inspections_one_active_per_material
    ON audio_material_inspections(material_id)
    WHERE status IN ('queued', 'running');
CREATE INDEX idx_audio_material_inspections_queue
    ON audio_material_inspections(created_at, id)
    WHERE status = 'queued';
CREATE INDEX idx_audio_material_inspections_material_created
    ON audio_material_inspections(material_id, created_at DESC);

CREATE TRIGGER trigger_audio_material_inspections_updated_at
    BEFORE UPDATE ON audio_material_inspections
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

COMMENT ON TABLE audio_material_inspections IS
    'ASR 计费确认前由 ffprobe 生成的真实音频时长、格式和源文件摘要快照。';

CREATE TABLE sound_subtitle_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
    parent_task_id UUID REFERENCES sound_subtitle_tasks(id) ON DELETE RESTRICT,
    task_type VARCHAR(20) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'queued',
    model_id UUID NOT NULL REFERENCES ai_models(id) ON DELETE RESTRICT,
    audio_inspection_id UUID REFERENCES audio_material_inspections(id) ON DELETE RESTRICT,
    source_audio_material_id UUID REFERENCES materials(id) ON DELETE RESTRICT,
    output_audio_material_id UUID REFERENCES materials(id) ON DELETE RESTRICT,
    output_subtitle_material_id UUID REFERENCES materials(id) ON DELETE RESTRICT,
    text_content TEXT NOT NULL DEFAULT '',
    voice_type VARCHAR(200),
    language VARCHAR(40),
    emotion VARCHAR(80),
    parameters JSONB NOT NULL DEFAULT '{}'::jsonb,
    model_snapshot JSONB,
    voice_snapshot JSONB,
    confirmation_snapshot JSONB NOT NULL,
    resource_usage JSONB NOT NULL DEFAULT '{}'::jsonb,
    timeline JSONB,
    result JSONB,
    idempotency_key VARCHAR(200) NOT NULL,
    request_id UUID NOT NULL DEFAULT gen_random_uuid(),
    upstream_log_id VARCHAR(240),
    upstream_submitted_at TIMESTAMPTZ,
    attempt_count INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 2,
    error_code VARCHAR(120),
    error_summary TEXT,
    staging_object_key TEXT,
    staging_source_sha256 VARCHAR(64),
    staging_status VARCHAR(24) NOT NULL DEFAULT 'none',
    cleanup_attempt_count INT NOT NULL DEFAULT 0,
    cleanup_error_summary TEXT,
    locked_at TIMESTAMPTZ,
    worker_id VARCHAR(160),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT sound_subtitle_tasks_type_check CHECK (
        task_type IN ('tts_preview', 'tts', 'asr', 'subtitle')
    ),
    CONSTRAINT sound_subtitle_tasks_status_check CHECK (
        status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')
    ),
    CONSTRAINT sound_subtitle_tasks_json_check CHECK (
        jsonb_typeof(parameters) = 'object'
        AND jsonb_typeof(confirmation_snapshot) = 'object'
        AND jsonb_typeof(resource_usage) = 'object'
        AND (model_snapshot IS NULL OR jsonb_typeof(model_snapshot) = 'object')
        AND (voice_snapshot IS NULL OR jsonb_typeof(voice_snapshot) = 'object')
        AND (timeline IS NULL OR jsonb_typeof(timeline) = 'array')
        AND (result IS NULL OR jsonb_typeof(result) = 'object')
    ),
    CONSTRAINT sound_subtitle_tasks_attempt_check CHECK (
        attempt_count >= 0 AND max_attempts BETWEEN 1 AND 2 AND attempt_count <= max_attempts
    ),
    CONSTRAINT sound_subtitle_tasks_staging_check CHECK (
        staging_status IN ('none', 'uploaded', 'cleanup_pending', 'cleaned')
        AND cleanup_attempt_count >= 0
        AND (
            staging_status = 'none'
            OR (
                length(trim(staging_object_key)) > 0
                AND staging_source_sha256 ~ '^[0-9a-f]{64}$'
            )
        )
    ),
    CONSTRAINT sound_subtitle_tasks_input_check CHECK (
        (task_type IN ('tts_preview', 'tts', 'subtitle') AND length(trim(text_content)) > 0)
        OR (
            task_type = 'asr'
            AND source_audio_material_id IS NOT NULL
            AND audio_inspection_id IS NOT NULL
        )
    ),
    CONSTRAINT sound_subtitle_tasks_idempotency_nonempty CHECK (
        length(trim(idempotency_key)) > 0
    ),
    UNIQUE (project_id, task_type, idempotency_key)
);

CREATE INDEX idx_sound_subtitle_tasks_project_created
    ON sound_subtitle_tasks(project_id, created_at DESC);
CREATE INDEX idx_sound_subtitle_tasks_queue
    ON sound_subtitle_tasks(created_at, id)
    WHERE status = 'queued';
CREATE INDEX idx_sound_subtitle_tasks_model
    ON sound_subtitle_tasks(model_id, created_at DESC);

CREATE TRIGGER trigger_sound_subtitle_tasks_updated_at
    BEFORE UPDATE ON sound_subtitle_tasks
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

COMMENT ON TABLE sound_subtitle_tasks IS
    '显式确认后的 TTS 试听、配音、ASR 和字幕任务；成功产物使用 Material 管理。';
COMMENT ON COLUMN sound_subtitle_tasks.model_snapshot IS
    'Worker 执行前锁定的不含任何凭据的语音模型快照。';
COMMENT ON COLUMN sound_subtitle_tasks.voice_snapshot IS
    '执行时音色名称、语言、情绪和目录版本快照；目录下线不回写历史。';

UPDATE video_workspace_menus
SET
    description = '生成可复用的 TTS 配音与真实时间轴字幕。',
    updated_at = NOW()
WHERE menu_key = 'sound-subtitle-generation';
