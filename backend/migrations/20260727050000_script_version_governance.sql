-- Script semantic revisions create new immutable Script/Scene aggregates. Old
-- workflow facts remain queryable; separate invalidation rows describe which
-- downstream facts a replacement Script can no longer consume.

ALTER TABLE production_revision_epochs
    DROP CONSTRAINT production_revision_epochs_reason_check;
ALTER TABLE production_revision_epochs
    ADD CONSTRAINT production_revision_epochs_reason_check CHECK (
        reason_type IN (
            'initial', 'brief_reject', 'script_reject', 'production_reject',
            'script_semantic_revision', 'production_expression_revision',
            'quality_rework'
        )
        AND length(btrim(reason)) > 0
    );

ALTER TABLE work_versions
    ADD COLUMN invalidated_at TIMESTAMPTZ;
ALTER TABLE work_versions DROP CONSTRAINT work_versions_status_check;
ALTER TABLE work_versions
    ADD CONSTRAINT work_versions_status_check CHECK (
        status IN ('draft','confirmed','running','completed','failed','invalidated')
    ),
    ADD CONSTRAINT work_versions_invalidation_check CHECK (
        (status = 'invalidated' AND invalidated_at IS NOT NULL)
        OR (status <> 'invalidated' AND invalidated_at IS NULL)
    );
COMMENT ON COLUMN work_versions.invalidated_at IS
    '仅未确认 draft 可因正式 Script 替换而失效；confirmed/running/history 永不改写。';

CREATE TABLE production_script_invalidations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    revision_epoch INT NOT NULL,
    source_script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE RESTRICT,
    replacement_script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE RESTRICT,
    reason VARCHAR(80) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_script_invalidations_epoch_fk
        FOREIGN KEY (run_id, revision_epoch)
        REFERENCES production_revision_epochs(run_id, epoch) ON DELETE RESTRICT,
    CONSTRAINT production_script_invalidations_distinct_check
        CHECK (source_script_id <> replacement_script_id),
    CONSTRAINT production_script_invalidations_reason_check
        CHECK (reason = 'script_semantic_revision'),
    CONSTRAINT production_script_invalidations_identity_unique
        UNIQUE (run_id, source_script_id, replacement_script_id)
);
COMMENT ON TABLE production_script_invalidations IS
    '新 Script 晋升后对旧 Script 的追加失效事实；旧 Script/Scene 与媒体历史保持不可变。';

CREATE TABLE production_package_invalidations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    package_id UUID NOT NULL,
    run_id UUID NOT NULL,
    revision_epoch INT NOT NULL,
    source_script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE RESTRICT,
    replacement_script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE RESTRICT,
    reason VARCHAR(80) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_package_invalidations_package_fk
        FOREIGN KEY (package_id, run_id)
        REFERENCES artifact_package_snapshots(id, run_id) ON DELETE RESTRICT,
    CONSTRAINT production_package_invalidations_epoch_fk
        FOREIGN KEY (run_id, revision_epoch)
        REFERENCES production_revision_epochs(run_id, epoch) ON DELETE RESTRICT,
    CONSTRAINT production_package_invalidations_reason_check
        CHECK (reason = 'script_semantic_revision'),
    CONSTRAINT production_package_invalidations_package_unique UNIQUE (package_id)
);
COMMENT ON TABLE production_package_invalidations IS
    '不修改 immutable package snapshot，通过独立事实标记旧 ProductionPackage 不得被新 Script 消费。';

CREATE INDEX idx_production_script_invalidations_run
    ON production_script_invalidations(run_id, revision_epoch, created_at);
CREATE INDEX idx_production_package_invalidations_run
    ON production_package_invalidations(run_id, revision_epoch, created_at);
