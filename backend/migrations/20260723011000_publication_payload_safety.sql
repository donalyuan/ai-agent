-- 发布域除敏感键外，还禁止带查询参数的 URL 与服务器绝对路径进入 JSON 快照。
CREATE OR REPLACE FUNCTION publication_json_is_safe(document JSONB)
RETURNS BOOLEAN
LANGUAGE SQL
IMMUTABLE
PARALLEL SAFE
AS $$
    SELECT document = sanitize_material_metadata(document)
       AND document::text !~* 'https?://[^"[:space:]]+\?'
       AND document::text !~* '"/(app|server|home|tmp|var|root)/'
$$;

ALTER TABLE publication_targets
    DROP CONSTRAINT publication_targets_result_safe,
    ADD CONSTRAINT publication_targets_result_safe
        CHECK (publication_json_is_safe(result_snapshot));

ALTER TABLE publication_packages
    DROP CONSTRAINT publication_packages_manifest_safe,
    ADD CONSTRAINT publication_packages_manifest_safe
        CHECK (publication_json_is_safe(manifest));

ALTER TABLE publication_events
    DROP CONSTRAINT publication_events_payload_safe,
    ADD COLUMN idempotency_key VARCHAR(200),
    ADD CONSTRAINT publication_events_payload_safe
        CHECK (publication_json_is_safe(payload)),
    ADD CONSTRAINT publication_events_idempotency_key_check
        CHECK (idempotency_key IS NULL OR length(trim(idempotency_key)) > 0),
    ADD CONSTRAINT publication_events_target_idempotency_unique
        UNIQUE (publication_target_id, idempotency_key);

COMMENT ON FUNCTION publication_json_is_safe(JSONB) IS
    '拒绝发布 JSON 中的凭据键、带查询参数 URL 和服务器绝对路径。';
