-- Full Crew 只保存既有画面领域返回的正式候选/素材引用，不复制素材任务。
CREATE TABLE production_scene_visual_manifests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    revision_epoch INT NOT NULL,
    package_id UUID NOT NULL,
    package_digest CHAR(64) NOT NULL,
    script_id UUID NOT NULL REFERENCES scripts(id) ON DELETE RESTRICT,
    script_version VARCHAR(120) NOT NULL,
    manifest_version VARCHAR(160) NOT NULL,
    manifest_digest CHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_scene_visual_manifests_epoch_fk
        FOREIGN KEY (run_id, revision_epoch)
        REFERENCES production_revision_epochs(run_id, epoch) ON DELETE RESTRICT,
    CONSTRAINT production_scene_visual_manifests_package_fk
        FOREIGN KEY (package_id, run_id)
        REFERENCES artifact_package_snapshots(id, run_id) ON DELETE RESTRICT,
    CONSTRAINT production_scene_visual_manifests_digest_check CHECK (
        package_digest ~ '^[0-9a-f]{64}$'
        AND manifest_digest ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT production_scene_visual_manifests_version_check CHECK (
        length(btrim(script_version)) > 0
        AND length(btrim(manifest_version)) > 0
    ),
    CONSTRAINT production_scene_visual_manifests_identity_unique
        UNIQUE (run_id, revision_epoch, package_id)
);

CREATE TABLE production_scene_visual_manifest_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manifest_id UUID NOT NULL REFERENCES production_scene_visual_manifests(id) ON DELETE RESTRICT,
    ordinal INT NOT NULL,
    scene_id UUID NOT NULL REFERENCES scenes(id) ON DELETE RESTRICT,
    scene_version VARCHAR(120) NOT NULL,
    candidate_id UUID NOT NULL REFERENCES scene_asset_candidates(id) ON DELETE RESTRICT,
    material_id UUID NOT NULL REFERENCES materials(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_scene_visual_manifest_items_ordinal_check CHECK (ordinal >= 0),
    CONSTRAINT production_scene_visual_manifest_items_version_check CHECK (
        length(btrim(scene_version)) > 0
    ),
    CONSTRAINT production_scene_visual_manifest_items_order_unique UNIQUE (manifest_id, ordinal),
    CONSTRAINT production_scene_visual_manifest_items_scene_unique UNIQUE (manifest_id, scene_id)
);

CREATE INDEX idx_production_scene_visual_manifests_run
    ON production_scene_visual_manifests(run_id, revision_epoch, created_at);

CREATE TRIGGER production_scene_visual_manifests_append_only
    BEFORE UPDATE OR DELETE ON production_scene_visual_manifests
    FOR EACH ROW EXECUTE FUNCTION reject_production_append_only_mutation();
CREATE TRIGGER production_scene_visual_manifest_items_append_only
    BEFORE UPDATE OR DELETE ON production_scene_visual_manifest_items
    FOR EACH ROW EXECUTE FUNCTION reject_production_append_only_mutation();

COMMENT ON TABLE production_scene_visual_manifests IS
    'SceneVisualManifest 的不可变恢复锚点；只引用既有 candidate/material 正式 ID。';
COMMENT ON TABLE production_scene_visual_manifest_items IS
    '按正式 Scene 顺序保存主画面候选与素材引用，不复制图片任务或 provider payload。';
