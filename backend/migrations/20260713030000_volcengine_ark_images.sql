-- Replace the retired Jimeng Visual contract with the Volcengine Ark Images API.
-- Existing rows must be handled explicitly because credentials and settings are incompatible.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM ai_models WHERE api_protocol = 'jimeng_visual'
    ) THEN
        RAISE EXCEPTION
            'cannot remove jimeng_visual while matching ai_models records exist';
    END IF;

    IF EXISTS (
        SELECT 1 FROM asset_generation_tasks WHERE provider = 'jimeng'
    ) THEN
        RAISE EXCEPTION
            'cannot remove jimeng provider while matching asset_generation_tasks records exist';
    END IF;
END
$$;

ALTER TABLE ai_models
    DROP CONSTRAINT ai_models_protocol_check,
    DROP CONSTRAINT ai_models_type_protocol_check;

ALTER TABLE ai_models
    ADD CONSTRAINT ai_models_protocol_check CHECK (
        api_protocol IN (
            'openai_responses', 'openai_chat_completions', 'openai_images',
            'volcengine_ark_images', 'runway_api', 'kling_api'
        )
    ),
    ADD CONSTRAINT ai_models_type_protocol_check CHECK (
        (model_type = 'text' AND api_protocol IN (
            'openai_responses', 'openai_chat_completions'
        )) OR
        (model_type = 'image' AND api_protocol IN (
            'openai_images', 'volcengine_ark_images'
        )) OR
        (model_type = 'video' AND api_protocol IN ('runway_api', 'kling_api'))
    );

ALTER TABLE asset_generation_tasks
    ADD CONSTRAINT asset_generation_tasks_provider_check CHECK (
        provider IN ('gpt-image-2', 'volcengine-ark')
    );

COMMENT ON CONSTRAINT ai_models_protocol_check ON ai_models IS
    '限制可持久化的显式 API 协议；旧 jimeng_visual 协议已删除。';
COMMENT ON CONSTRAINT ai_models_type_protocol_check ON ai_models IS
    '限制模型类型与可执行协议组合；图片支持 OpenAI Images 和火山方舟图片协议。';
COMMENT ON CONSTRAINT asset_generation_tasks_provider_check ON asset_generation_tasks IS
    '限制素材生成任务的供应商审计值。';
COMMENT ON COLUMN asset_generation_tasks.provider IS
    '历史供应商审计字段；图片任务使用 gpt-image-2 或 volcengine-ark。';
