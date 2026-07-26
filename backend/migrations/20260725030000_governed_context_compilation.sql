-- Add explicit governed-context behavior without guessing values for historical models.
ALTER TABLE ai_models
    ADD COLUMN context_window BIGINT,
    ADD COLUMN tokenizer_profile_key VARCHAR(128),
    ADD COLUMN tokenizer_profile_version VARCHAR(32),
    ADD CONSTRAINT ai_models_context_window_range_check CHECK (
        context_window IS NULL OR context_window BETWEEN 1 AND 2147483647
    ),
    ADD CONSTRAINT ai_models_tokenizer_profile_key_format_check CHECK (
        tokenizer_profile_key IS NULL
        OR tokenizer_profile_key ~ '^[a-z0-9][a-z0-9._-]{0,127}$'
    ),
    ADD CONSTRAINT ai_models_tokenizer_profile_version_format_check CHECK (
        tokenizer_profile_version IS NULL
        OR tokenizer_profile_version ~ '^[0-9]+\.[0-9]+\.[0-9]+$'
    ),
    ADD CONSTRAINT ai_models_context_profile_complete_check CHECK (
        (context_window IS NULL AND tokenizer_profile_key IS NULL AND tokenizer_profile_version IS NULL)
        OR
        (context_window IS NOT NULL AND tokenizer_profile_key IS NOT NULL AND tokenizer_profile_version IS NOT NULL)
    ),
    ADD CONSTRAINT ai_models_non_text_context_empty_check CHECK (
        model_type = 'text'
        OR (context_window IS NULL AND tokenizer_profile_key IS NULL AND tokenizer_profile_version IS NULL)
    );

CREATE INDEX idx_ai_models_tokenizer_profile
    ON ai_models(tokenizer_profile_key, tokenizer_profile_version)
    WHERE model_type = 'text' AND deleted_at IS NULL;

COMMENT ON COLUMN ai_models.context_window IS
    '操作者确认的文本模型输入窗口；历史未知配置保持 NULL，Runtime 不得从 settings 或模型名推断。';
COMMENT ON COLUMN ai_models.tokenizer_profile_key IS
    '显式引用 Definition Registry TokenizerProfile key；非文本模型必须为空。';
COMMENT ON COLUMN ai_models.tokenizer_profile_version IS
    '显式引用 Definition Registry TokenizerProfile version；与 key 和 context_window 同时为空或同时存在。';
