-- Extend Material as the reusable asset boundary for work-production artifacts.
-- Work tables are introduced by separate changes, so source IDs remain immutable UUID snapshots here.

CREATE OR REPLACE FUNCTION sanitize_material_metadata(document JSONB)
RETURNS JSONB
LANGUAGE plpgsql
IMMUTABLE
PARALLEL SAFE
AS $$
DECLARE
    result JSONB;
    entry RECORD;
    normalized_key TEXT;
BEGIN
    IF document IS NULL THEN
        RETURN '{}'::jsonb;
    END IF;

    CASE jsonb_typeof(document)
        WHEN 'object' THEN
            result := '{}'::jsonb;
            FOR entry IN SELECT key, value FROM jsonb_each(document)
            LOOP
                normalized_key := lower(regexp_replace(entry.key, '[^a-zA-Z0-9]', '', 'g'));
                IF normalized_key = ANY (ARRAY[
                    'cookie', 'setcookie', 'credential', 'credentials', 'headers',
                    'authheaders', 'authenticationheaders'
                ]) OR normalized_key ~ '(apikey|authorization|token|secret|password|privatekey)$' THEN
                    CONTINUE;
                END IF;
                result := result || jsonb_build_object(
                    entry.key,
                    sanitize_material_metadata(entry.value)
                );
            END LOOP;
            RETURN result;
        WHEN 'array' THEN
            SELECT COALESCE(
                jsonb_agg(sanitize_material_metadata(value)),
                '[]'::jsonb
            )
            INTO result
            FROM jsonb_array_elements(document);
            RETURN result;
        ELSE
            RETURN document;
    END CASE;
END;
$$;

COMMENT ON FUNCTION sanitize_material_metadata(JSONB) IS
    '递归移除素材 metadata 中的鉴权、密钥、口令和 Cookie 字段；用于迁移清理与数据库写入约束。';

-- Clean historical rows before enforcing the invariant so upgrades do not fail on old metadata.
UPDATE materials
SET metadata = sanitize_material_metadata(metadata)
WHERE metadata <> sanitize_material_metadata(metadata);

ALTER TABLE materials
    ADD CONSTRAINT materials_metadata_no_credentials_check
        CHECK (metadata = sanitize_material_metadata(metadata)),
    ADD CONSTRAINT materials_audio_usage_check
        CHECK (
            NOT (metadata ? 'audio_usage')
            OR (
                material_type = 'audio'
                AND jsonb_typeof(metadata -> 'audio_usage') = 'string'
                AND metadata ->> 'audio_usage' IN (
                    'tts', 'bgm', 'ambient', 'action_sfx', 'mixed', 'other'
                )
            )
        ),
    ADD CONSTRAINT materials_work_generation_snapshot_check
        CHECK (
            metadata ->> 'source' IS DISTINCT FROM 'work_generation'
            OR COALESCE((
                material_type IN ('video', 'audio', 'subtitle')
                AND metadata ->> 'storage_provider' = 'local'
                AND metadata ->> 'work_id' ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
                AND metadata ->> 'work_version_id' ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
                AND metadata ->> 'generation_run_id' ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
                AND metadata ->> 'generation_step_id' ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
                AND jsonb_typeof(metadata -> 'artifact_role') = 'string'
                AND length(trim(metadata ->> 'artifact_role')) > 0
                AND jsonb_typeof(metadata -> 'model_snapshot') = 'object'
                AND jsonb_typeof(metadata -> 'voice_snapshot') = 'object'
                AND jsonb_typeof(metadata -> 'prompt_snapshot') = 'object'
                AND jsonb_typeof(metadata -> 'timeline_snapshot') = 'object'
                AND jsonb_typeof(metadata -> 'resource_usage') = 'object'
                AND (
                    material_type <> 'audio'
                    OR metadata ->> 'audio_usage' IN (
                        'tts', 'bgm', 'ambient', 'action_sfx', 'mixed', 'other'
                    )
                )
                AND (
                    material_type <> 'subtitle'
                    OR (
                        metadata ->> 'alignment_source' IN ('tts_timestamp', 'asr')
                        AND metadata ->> 'source_audio_material_id' ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$'
                    )
                )
            ), false)
        );

COMMENT ON CONSTRAINT materials_metadata_no_credentials_check ON materials IS
    '素材审计快照禁止保存 API Key、Authorization、Token、Secret、口令、Cookie 或 credentials。';
COMMENT ON CONSTRAINT materials_audio_usage_check ON materials IS
    '音频用途是 audio 素材的可选标准分类；历史音频允许为空。';
COMMENT ON CONSTRAINT materials_work_generation_snapshot_check ON materials IS
    '作品生成素材必须使用自管存储并保留作品、版本、运行、步骤和产物角色 UUID 快照。';

CREATE INDEX idx_materials_project_source_updated
    ON materials(project_id, (metadata ->> 'source'), updated_at DESC);

CREATE INDEX idx_materials_project_audio_usage_updated
    ON materials(project_id, (metadata ->> 'audio_usage'), updated_at DESC)
    WHERE material_type = 'audio';

CREATE INDEX idx_materials_project_work_version_updated
    ON materials(
        project_id,
        (metadata ->> 'work_id'),
        (metadata ->> 'work_version_id'),
        updated_at DESC
    )
    WHERE metadata ->> 'source' = 'work_generation';
