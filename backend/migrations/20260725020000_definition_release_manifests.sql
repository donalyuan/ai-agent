-- 每个代码 Registry digest 保存一份不可变生命周期快照；模板正文始终只属于代码发布物。
CREATE TABLE definition_release_manifests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    schema_version VARCHAR(16) NOT NULL DEFAULT '1',
    registry_digest CHAR(64) NOT NULL UNIQUE,
    published_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT definition_release_manifests_schema_check CHECK (schema_version = '1'),
    CONSTRAINT definition_release_manifests_digest_check CHECK (registry_digest ~ '^[0-9a-f]{64}$')
);

CREATE TABLE definition_release_manifest_entries (
    manifest_id UUID NOT NULL REFERENCES definition_release_manifests(id) ON DELETE RESTRICT,
    definition_kind VARCHAR(16) NOT NULL,
    definition_key VARCHAR(160) NOT NULL,
    definition_version VARCHAR(32) NOT NULL,
    definition_digest CHAR(64) NOT NULL,
    lifecycle_status VARCHAR(16) NOT NULL,
    executor_owner VARCHAR(16) NOT NULL,
    PRIMARY KEY (manifest_id, definition_kind, definition_key, definition_version),
    CONSTRAINT definition_release_manifest_entries_kind_check
        CHECK (definition_kind IN ('agent', 'prompt')),
    CONSTRAINT definition_release_manifest_entries_status_check
        CHECK (lifecycle_status IN ('candidate', 'active', 'supported', 'revoked')),
    CONSTRAINT definition_release_manifest_entries_owner_check
        CHECK (executor_owner IN ('rust', 'pi')),
    CONSTRAINT definition_release_manifest_entries_digest_check
        CHECK (definition_digest ~ '^[0-9a-f]{64}$')
);

COMMENT ON TABLE definition_release_manifests IS
    '代码发布 manifest 的不可变 registry digest；数据库不能反向覆盖运行 Definition。';
COMMENT ON TABLE definition_release_manifest_entries IS
    '某个代码 registry digest 对应的完整 Definition 生命周期快照；禁止保存模板正文。';

CREATE FUNCTION reject_definition_release_manifest_mutation() RETURNS TRIGGER AS $$
BEGIN
    RAISE EXCEPTION 'definition release manifests are immutable';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER definition_release_manifests_no_update
    BEFORE UPDATE OR DELETE ON definition_release_manifests
    FOR EACH ROW EXECUTE FUNCTION reject_definition_release_manifest_mutation();

CREATE TRIGGER definition_release_manifest_entries_no_update
    BEFORE UPDATE OR DELETE ON definition_release_manifest_entries
    FOR EACH ROW EXECUTE FUNCTION reject_definition_release_manifest_mutation();
