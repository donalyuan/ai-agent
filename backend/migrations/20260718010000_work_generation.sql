-- 作品生成：作品身份、不可变版本、可迭代计划及一次确认运行。

ALTER TABLE ai_models
    DROP CONSTRAINT ai_models_protocol_check,
    DROP CONSTRAINT ai_models_type_protocol_check;
ALTER TABLE ai_models
    ADD CONSTRAINT ai_models_protocol_check CHECK (
        api_protocol IN (
            'openai_responses', 'openai_chat_completions', 'openai_images',
            'volcengine_ark_images', 'volcengine_ark_video', 'runway_api', 'kling_api',
            'volcengine_tts_v3', 'openai_audio_speech', 'volcengine_asr_v3'
        )
    ),
    ADD CONSTRAINT ai_models_type_protocol_check CHECK (
        (model_type = 'text' AND api_protocol IN ('openai_responses', 'openai_chat_completions')) OR
        (model_type = 'image' AND api_protocol IN ('openai_images', 'volcengine_ark_images')) OR
        (model_type = 'video' AND api_protocol IN ('volcengine_ark_video', 'runway_api', 'kling_api')) OR
        (model_type = 'speech' AND api_protocol IN ('volcengine_tts_v3', 'openai_audio_speech', 'volcengine_asr_v3'))
    );

CREATE TABLE works (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE RESTRICT,
    title VARCHAR(200) NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'draft',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT works_status_check CHECK (status IN ('draft', 'planned', 'running', 'succeeded', 'failed', 'archived'))
);
CREATE INDEX idx_works_project_updated ON works(project_id, updated_at DESC);
CREATE UNIQUE INDEX works_one_active_script ON works(script_id) WHERE status <> 'archived';
CREATE TRIGGER trigger_works_updated_at BEFORE UPDATE ON works FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE work_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    version_no INT NOT NULL,
    source_manifest_version VARCHAR(64) NOT NULL,
    input_snapshot JSONB NOT NULL,
    model_snapshot JSONB NOT NULL,
    parameter_snapshot JSONB NOT NULL,
    timeline_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    prompt_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT work_versions_version_check CHECK (version_no > 0),
    UNIQUE (work_id, version_no)
);
CREATE INDEX idx_work_versions_work_created ON work_versions(work_id, created_at DESC);

CREATE TABLE work_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    work_version_id UUID NOT NULL REFERENCES work_versions(id) ON DELETE CASCADE,
    plan_version INT NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'draft',
    input_fingerprint CHAR(64) NOT NULL,
    llm_model_id UUID REFERENCES ai_models(id) ON DELETE RESTRICT,
    video_model_id UUID REFERENCES ai_models(id) ON DELETE RESTRICT,
    tts_model_id UUID REFERENCES ai_models(id) ON DELETE RESTRICT,
    capability_snapshot JSONB NOT NULL,
    output_snapshot JSONB NOT NULL,
    prompt_snapshot JSONB NOT NULL,
    timeline_snapshot JSONB NOT NULL,
    resource_usage JSONB NOT NULL DEFAULT '{}'::jsonb,
    warnings JSONB NOT NULL DEFAULT '[]'::jsonb,
    invalidated_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT work_plans_status_check CHECK (status IN ('draft', 'ready', 'invalidated', 'confirmed')),
    CONSTRAINT work_plans_version_check CHECK (plan_version > 0),
    UNIQUE (work_version_id, plan_version)
);
CREATE INDEX idx_work_plans_work_updated ON work_plans(work_id, updated_at DESC);
CREATE TRIGGER trigger_work_plans_updated_at BEFORE UPDATE ON work_plans FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE work_generation_runs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id UUID NOT NULL REFERENCES works(id) ON DELETE CASCADE,
    work_version_id UUID NOT NULL REFERENCES work_versions(id) ON DELETE RESTRICT,
    work_plan_id UUID NOT NULL REFERENCES work_plans(id) ON DELETE RESTRICT,
    idempotency_key VARCHAR(200) NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'queued',
    model_snapshot JSONB NOT NULL,
    capability_snapshot JSONB NOT NULL,
    voice_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    prompt_snapshot JSONB NOT NULL,
    timeline_snapshot JSONB NOT NULL,
    parameter_snapshot JSONB NOT NULL,
    resource_usage JSONB NOT NULL DEFAULT '{}'::jsonb,
    error_code VARCHAR(120),
    error_summary TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT work_generation_runs_status_check CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
    CONSTRAINT work_generation_runs_key_check CHECK (length(trim(idempotency_key)) > 0),
    UNIQUE (work_id, idempotency_key)
);
CREATE INDEX idx_work_generation_runs_queue ON work_generation_runs(status, created_at) WHERE status = 'queued';
CREATE TRIGGER trigger_work_generation_runs_updated_at BEFORE UPDATE ON work_generation_runs FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE work_generation_steps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES work_generation_runs(id) ON DELETE CASCADE,
    step_no INT NOT NULL,
    step_type VARCHAR(32) NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'queued',
    input_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    output_snapshot JSONB,
    external_task_id VARCHAR(200),
    error_summary TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT work_generation_steps_status_check CHECK (status IN ('queued', 'running', 'succeeded', 'failed', 'cancelled')),
    UNIQUE (run_id, step_no)
);
CREATE INDEX idx_work_generation_steps_queue ON work_generation_steps(status, created_at) WHERE status = 'queued';
CREATE TRIGGER trigger_work_generation_steps_updated_at BEFORE UPDATE ON work_generation_steps FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- 作品生产导航由数据库菜单配置驱动。
UPDATE video_workspace_menus SET is_enabled = true, status = 'active', updated_at = NOW() WHERE menu_key = 'production';
INSERT INTO video_workspace_menus (
    id, parent_id, menu_key, label, description, route_path, icon, menu_type, module_key, agent_key,
    sort_order, is_enabled, is_visible, status, metadata
)
SELECT '30000000-0000-4000-8000-000000000003', parent.id, 'work-generation', '作品生成',
       '汇总完整主画面、旁白和声音素材，一次确认生成完整作品。', '/production/generation', 'clapperboard',
       'page', 'production.work-generation', 'work', 10, true, true, 'active', '{"phase":4}'::jsonb
FROM video_workspace_menus parent WHERE parent.menu_key = 'production'
ON CONFLICT (menu_key) DO UPDATE SET parent_id = EXCLUDED.parent_id, label = EXCLUDED.label,
    description = EXCLUDED.description, route_path = EXCLUDED.route_path, icon = EXCLUDED.icon,
    module_key = EXCLUDED.module_key, agent_key = EXCLUDED.agent_key, is_enabled = EXCLUDED.is_enabled,
    is_visible = EXCLUDED.is_visible, status = EXCLUDED.status, metadata = EXCLUDED.metadata, updated_at = NOW();

ALTER TABLE agent_conversations DROP CONSTRAINT agent_conversations_agent_type_check;
ALTER TABLE agent_conversations ADD CONSTRAINT agent_conversations_agent_type_check CHECK (
    agent_type IN ('topic', 'script', 'material', 'sound', 'work', 'video', 'publish', 'optimization')
);
ALTER TABLE agent_runs DROP CONSTRAINT agent_runs_type_check;
ALTER TABLE agent_runs ADD CONSTRAINT agent_runs_type_check CHECK (
    agent_type IN ('topic', 'script', 'material', 'sound', 'work', 'video', 'publish', 'optimization')
);
