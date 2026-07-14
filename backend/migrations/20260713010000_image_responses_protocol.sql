-- Allow the Responses image tool for image deployments while keeping every other
-- model-type/protocol combination unchanged.
ALTER TABLE ai_models
    DROP CONSTRAINT ai_models_type_protocol_check;

ALTER TABLE ai_models
    ADD CONSTRAINT ai_models_type_protocol_check CHECK (
        (model_type = 'text' AND api_protocol IN (
            'openai_responses', 'openai_chat_completions'
        )) OR
        (model_type = 'image' AND api_protocol IN (
            'openai_images', 'openai_responses', 'jimeng_visual'
        )) OR
        (model_type = 'video' AND api_protocol IN ('runway_api', 'kling_api'))
    );

COMMENT ON CONSTRAINT ai_models_type_protocol_check ON ai_models IS
    '限制模型类型与可执行协议组合；图片模型额外支持 Responses 图片工具协议。';
