-- Add OpenAI-compatible audio speech gateways as audio-only TTS models.

ALTER TABLE ai_models
    DROP CONSTRAINT ai_models_protocol_check,
    DROP CONSTRAINT ai_models_type_protocol_check,
    DROP CONSTRAINT ai_models_voice_catalog_binding_check;

ALTER TABLE ai_models
    ADD CONSTRAINT ai_models_protocol_check CHECK (
        api_protocol IN (
            'openai_responses', 'openai_chat_completions', 'openai_images',
            'volcengine_ark_images', 'runway_api', 'kling_api',
            'volcengine_tts_v3', 'openai_audio_speech', 'volcengine_asr_v3'
        )
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
            'volcengine_tts_v3', 'openai_audio_speech', 'volcengine_asr_v3'
        ))
    ),
    ADD CONSTRAINT ai_models_voice_catalog_binding_check CHECK (
        (
            model_type = 'speech'
            AND api_protocol = 'volcengine_tts_v3'
            AND (
                (
                    voice_catalog_source_model_id IS NULL
                    AND catalog_access_key IS NOT NULL
                    AND catalog_secret_key IS NOT NULL
                ) OR (
                    voice_catalog_source_model_id IS NOT NULL
                    AND catalog_access_key IS NULL
                    AND catalog_secret_key IS NULL
                )
            )
        ) OR (
            model_type = 'speech'
            AND api_protocol = 'openai_audio_speech'
            AND voice_catalog_source_model_id IS NOT NULL
            AND catalog_access_key IS NULL
            AND catalog_secret_key IS NULL
        ) OR (
            api_protocol NOT IN ('volcengine_tts_v3', 'openai_audio_speech')
            AND voice_catalog_source_model_id IS NULL
            AND catalog_access_key IS NULL
            AND catalog_secret_key IS NULL
        )
    );

COMMENT ON CONSTRAINT ai_models_voice_catalog_binding_check ON ai_models IS
    'Native TTS may own or share a catalog; OpenAI Audio Speech must share an official native TTS catalog.';
