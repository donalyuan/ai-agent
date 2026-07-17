-- Allow gateway TTS models to reuse an official catalog for the same upstream model resource.

ALTER TABLE ai_models
    ADD COLUMN voice_catalog_source_model_id UUID
        REFERENCES ai_models(id) ON DELETE RESTRICT;

ALTER TABLE ai_models
    DROP CONSTRAINT ai_models_catalog_credentials_protocol_check,
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
            api_protocol <> 'volcengine_tts_v3'
            AND voice_catalog_source_model_id IS NULL
            AND catalog_access_key IS NULL
            AND catalog_secret_key IS NULL
        )
    ),
    ADD CONSTRAINT ai_models_voice_catalog_not_self_check CHECK (
        voice_catalog_source_model_id IS NULL OR voice_catalog_source_model_id <> id
    );

CREATE INDEX idx_ai_models_voice_catalog_source
    ON ai_models(voice_catalog_source_model_id)
    WHERE voice_catalog_source_model_id IS NOT NULL;

COMMENT ON COLUMN ai_models.voice_catalog_source_model_id IS
    'NULL means this TTS model owns an officially synchronized catalog; otherwise points to the official root catalog model reused by this gateway model.';
COMMENT ON CONSTRAINT ai_models_voice_catalog_binding_check ON ai_models IS
    'Official TTS catalog owners require AK/SK; shared gateway models require one source and must not store catalog credentials.';
