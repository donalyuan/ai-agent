-- Restore the original image protocol boundary without rewriting model records.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM ai_models
        WHERE model_type = 'image'
          AND api_protocol = 'openai_responses'
    ) THEN
        RAISE EXCEPTION
            'cannot remove image responses protocol while matching ai_models records exist';
    END IF;
END
$$;

ALTER TABLE ai_models
    DROP CONSTRAINT ai_models_type_protocol_check;

ALTER TABLE ai_models
    ADD CONSTRAINT ai_models_type_protocol_check CHECK (
        (model_type = 'text' AND api_protocol IN (
            'openai_responses', 'openai_chat_completions'
        )) OR
        (model_type = 'image' AND api_protocol IN (
            'openai_images', 'jimeng_visual'
        )) OR
        (model_type = 'video' AND api_protocol IN ('runway_api', 'kling_api'))
    );

COMMENT ON CONSTRAINT ai_models_type_protocol_check ON ai_models IS
    '限制模型类型与可执行协议组合；Responses 仅用于文本模型。';
