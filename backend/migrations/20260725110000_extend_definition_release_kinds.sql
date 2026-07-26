-- Policy/Profile 发布证据与 Agent/Prompt 使用同一不可变 content release 和 lifecycle manifest。
ALTER TABLE definition_releases
    DROP CONSTRAINT definition_releases_kind_check,
    DROP CONSTRAINT definition_releases_owner_check,
    ALTER COLUMN definition_kind TYPE VARCHAR(32),
    ADD CONSTRAINT definition_releases_kind_check CHECK (
        definition_kind IN ('agent', 'prompt', 'context_policy', 'tokenizer_profile')
    ),
    ADD CONSTRAINT definition_releases_owner_check CHECK (
        executor_owner IN ('rust', 'pi', 'shared')
    );

ALTER TABLE definition_release_manifest_entries
    DROP CONSTRAINT definition_release_manifest_entries_kind_check,
    DROP CONSTRAINT definition_release_manifest_entries_owner_check,
    ALTER COLUMN definition_kind TYPE VARCHAR(32),
    ADD CONSTRAINT definition_release_manifest_entries_kind_check CHECK (
        definition_kind IN ('agent', 'prompt', 'context_policy', 'tokenizer_profile')
    ),
    ADD CONSTRAINT definition_release_manifest_entries_owner_check CHECK (
        executor_owner IN ('rust', 'pi', 'shared')
    );

COMMENT ON COLUMN definition_releases.executor_owner IS
    'Agent/Prompt 的执行 owner；跨 Runtime Policy 与 TokenizerProfile 使用 shared。';
COMMENT ON COLUMN definition_release_manifest_entries.executor_owner IS
    '该 Registry 生命周期快照中的执行 owner；跨 Runtime 定义使用 shared。';
