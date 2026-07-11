-- Register model deployments as the single runtime source for text, image and video adapters.
CREATE TABLE ai_models (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    display_name VARCHAR(120) NOT NULL,
    model_type VARCHAR(16) NOT NULL,
    provider_name VARCHAR(80) NOT NULL,
    api_protocol VARCHAR(40) NOT NULL,
    protocol_version VARCHAR(40) NOT NULL DEFAULT '',
    auth_scheme VARCHAR(30) NOT NULL,
    request_base_url TEXT NOT NULL,
    upstream_model VARCHAR(160) NOT NULL,
    api_key TEXT NOT NULL,
    api_secret TEXT,
    timeout_seconds INT NOT NULL DEFAULT 30,
    reasoning_effort VARCHAR(20),
    max_output_tokens INT,
    settings JSONB NOT NULL DEFAULT '{}'::jsonb,
    sort_order INT NOT NULL DEFAULT 0,
    remark TEXT NOT NULL DEFAULT '',
    status VARCHAR(20) NOT NULL DEFAULT 'enabled',
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    last_call_status VARCHAR(20) NOT NULL DEFAULT 'never',
    last_call_at TIMESTAMPTZ,
    last_error_summary TEXT,
    source VARCHAR(30) NOT NULL DEFAULT 'admin',
    source_key VARCHAR(160),
    version BIGINT NOT NULL DEFAULT 1,
    deleted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT ai_models_type_check CHECK (model_type IN ('text', 'image', 'video')),
    CONSTRAINT ai_models_protocol_check CHECK (api_protocol IN (
        'openai_responses', 'openai_chat_completions', 'openai_images',
        'jimeng_visual', 'runway_api', 'kling_api'
    )),
    CONSTRAINT ai_models_auth_scheme_check CHECK (auth_scheme IN ('bearer', 'access_key_secret')),
    CONSTRAINT ai_models_status_check CHECK (status IN ('enabled', 'disabled', 'deleted')),
    CONSTRAINT ai_models_last_call_status_check CHECK (last_call_status IN ('never', 'success', 'failed')),
    CONSTRAINT ai_models_source_check CHECK (source IN ('admin', 'environment_import')),
    CONSTRAINT ai_models_timeout_check CHECK (timeout_seconds > 0 AND timeout_seconds <= 3600),
    CONSTRAINT ai_models_max_output_tokens_check CHECK (
        max_output_tokens IS NULL OR max_output_tokens > 0
    ),
    CONSTRAINT ai_models_version_check CHECK (version > 0),
    CONSTRAINT ai_models_type_protocol_check CHECK (
        (model_type = 'text' AND api_protocol IN ('openai_responses', 'openai_chat_completions')) OR
        (model_type = 'image' AND api_protocol IN ('openai_images', 'jimeng_visual')) OR
        (model_type = 'video' AND api_protocol IN ('runway_api', 'kling_api'))
    ),
    CONSTRAINT ai_models_default_state_check CHECK (
        NOT is_default OR (status = 'enabled' AND deleted_at IS NULL)
    ),
    CONSTRAINT ai_models_deleted_state_check CHECK (
        (status = 'deleted') = (deleted_at IS NOT NULL)
    )
);

COMMENT ON TABLE ai_models IS '文本、图片和视频模型部署注册表；运行时模型配置的唯一来源。';
COMMENT ON COLUMN ai_models.api_key IS '按已确认风险原文保存；管理响应、日志和运行快照不得返回该字段。';
COMMENT ON COLUMN ai_models.api_secret IS '可选第二凭据，按原文保存且不得进入响应、日志和运行快照。';
COMMENT ON COLUMN ai_models.request_base_url IS 'API 根地址；稳定请求路径由 api_protocol 对应 adapter 追加。';
COMMENT ON COLUMN ai_models.settings IS '经模型类型专属结构反序列化和校验后才能传入 provider。';
COMMENT ON COLUMN ai_models.version IS '管理编辑和生命周期操作使用的乐观锁版本。';

CREATE UNIQUE INDEX ai_models_one_default_per_type
    ON ai_models(model_type)
    WHERE is_default = TRUE AND deleted_at IS NULL;
CREATE UNIQUE INDEX ai_models_source_key_unique
    ON ai_models(source_key)
    WHERE source_key IS NOT NULL;
CREATE INDEX idx_ai_models_type_status_sort
    ON ai_models(model_type, status, is_default DESC, sort_order ASC, created_at ASC)
    WHERE deleted_at IS NULL;

CREATE TRIGGER trigger_ai_models_updated_at
    BEFORE UPDATE ON ai_models
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

ALTER TABLE agent_runs
    ADD COLUMN model_id UUID REFERENCES ai_models(id) ON DELETE RESTRICT,
    ADD COLUMN model_snapshot JSONB;

ALTER TABLE asset_generation_tasks
    DROP CONSTRAINT asset_generation_tasks_provider_check,
    ADD COLUMN model_id UUID REFERENCES ai_models(id) ON DELETE RESTRICT,
    ADD COLUMN model_snapshot JSONB;

COMMENT ON COLUMN agent_runs.model_id IS '本次 Agent 运行实际解析的模型部署；历史数据允许为空。';
COMMENT ON COLUMN agent_runs.model_snapshot IS '不含凭据的模型执行快照，后续编辑不得回写历史。';
COMMENT ON COLUMN asset_generation_tasks.model_id IS '图片任务选择的模型部署；历史数据允许为空。';
COMMENT ON COLUMN asset_generation_tasks.model_snapshot IS 'Worker 真正执行前写入的不含凭据模型快照。';
COMMENT ON COLUMN asset_generation_tasks.provider IS '历史供应商审计字段；新任务由 model_id 解析实际供应商。';

CREATE INDEX idx_agent_runs_model ON agent_runs(model_id) WHERE model_id IS NOT NULL;
CREATE INDEX idx_asset_generation_tasks_model
    ON asset_generation_tasks(model_id)
    WHERE model_id IS NOT NULL;
