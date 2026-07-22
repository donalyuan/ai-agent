-- 作品库：不可变版本、版本产物、时间轴、差异计划及发布草稿交接。

ALTER TABLE work_versions
    ADD COLUMN status VARCHAR(24) NOT NULL DEFAULT 'draft',
    ADD COLUMN source_version_id UUID,
    ADD COLUMN derivation_kind VARCHAR(24) NOT NULL DEFAULT 'initial',
    ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ADD COLUMN completed_at TIMESTAMPTZ;

-- 已有版本按真实运行事实回填，避免把已执行版本误标为可编辑草稿。
UPDATE work_versions version
SET status = CASE
    WHEN EXISTS (
        SELECT 1 FROM work_generation_runs run
        WHERE run.work_version_id=version.id AND run.status='succeeded'
    ) THEN 'completed'
    WHEN EXISTS (
        SELECT 1 FROM work_generation_runs run
        WHERE run.work_version_id=version.id AND run.status IN ('failed','cancelled','waiting_manual')
    ) THEN 'failed'
    WHEN EXISTS (
        SELECT 1 FROM work_generation_runs run
        WHERE run.work_version_id=version.id
    ) THEN 'running'
    WHEN EXISTS (
        SELECT 1 FROM work_plans plan
        WHERE plan.work_version_id=version.id AND plan.status='confirmed'
    ) THEN 'confirmed'
    ELSE 'draft'
END;

ALTER TABLE work_versions
    ADD CONSTRAINT work_versions_status_check
        CHECK (status IN ('draft','confirmed','running','completed','failed')),
    ADD CONSTRAINT work_versions_source_not_self_check
        CHECK (source_version_id IS NULL OR source_version_id <> id),
    ADD CONSTRAINT work_versions_derivation_kind_check
        CHECK (derivation_kind IN ('initial','edit','full_regeneration')),
    ADD CONSTRAINT work_versions_id_work_unique UNIQUE (id, work_id),
    ADD CONSTRAINT work_versions_source_same_work_fk
        FOREIGN KEY (source_version_id, work_id)
        REFERENCES work_versions(id, work_id) ON DELETE RESTRICT;

CREATE TRIGGER trigger_work_versions_updated_at
BEFORE UPDATE ON work_versions
FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE OR REPLACE FUNCTION reject_immutable_work_version_snapshot_update()
RETURNS TRIGGER AS $$
BEGIN
    IF OLD.status <> 'draft' AND (
        NEW.work_id IS DISTINCT FROM OLD.work_id OR
        NEW.version_no IS DISTINCT FROM OLD.version_no OR
        NEW.source_version_id IS DISTINCT FROM OLD.source_version_id OR
        NEW.source_manifest_version IS DISTINCT FROM OLD.source_manifest_version OR
        NEW.input_snapshot IS DISTINCT FROM OLD.input_snapshot OR
        NEW.model_snapshot IS DISTINCT FROM OLD.model_snapshot OR
        NEW.parameter_snapshot IS DISTINCT FROM OLD.parameter_snapshot OR
        NEW.timeline_snapshot IS DISTINCT FROM OLD.timeline_snapshot OR
        NEW.prompt_snapshot IS DISTINCT FROM OLD.prompt_snapshot
    ) THEN
        RAISE EXCEPTION 'work version % snapshot is immutable after draft', OLD.id
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_work_versions_immutable_snapshot
BEFORE UPDATE ON work_versions
FOR EACH ROW EXECUTE FUNCTION reject_immutable_work_version_snapshot_update();

ALTER TABLE works
    ADD COLUMN current_version_id UUID REFERENCES work_versions(id) ON DELETE SET NULL,
    ADD COLUMN archived_at TIMESTAMPTZ;

UPDATE works SET archived_at=updated_at WHERE status='archived';
DROP INDEX works_one_active_script;
CREATE UNIQUE INDEX works_one_active_script
    ON works(script_id) WHERE archived_at IS NULL;

CREATE INDEX idx_works_project_archived_updated
    ON works(project_id, archived_at, updated_at DESC);
CREATE INDEX idx_work_versions_work_status_created
    ON work_versions(work_id, status, created_at DESC);

CREATE TABLE work_artifacts (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_version_id UUID NOT NULL REFERENCES work_versions(id) ON DELETE RESTRICT,
    role VARCHAR(32) NOT NULL,
    material_id UUID REFERENCES materials(id) ON DELETE RESTRICT,
    generation_step_id UUID REFERENCES work_generation_steps(id) ON DELETE RESTRICT,
    file_name VARCHAR(255) NOT NULL,
    storage_path TEXT NOT NULL,
    mime_type VARCHAR(160) NOT NULL,
    size_bytes BIGINT NOT NULL,
    sha256 CHAR(64) NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT work_artifacts_role_check CHECK (
        role IN ('final_video','subtitle','mix','audio_track','production_package','reusable_intermediate')
    ),
    CONSTRAINT work_artifacts_file_check CHECK (
        length(trim(file_name)) > 0 AND length(trim(storage_path)) > 0 AND length(trim(mime_type)) > 0
    ),
    CONSTRAINT work_artifacts_integrity_check CHECK (
        size_bytes >= 0 AND sha256 ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT work_artifacts_metadata_no_credentials_check
        CHECK (metadata = sanitize_material_metadata(metadata)),
    UNIQUE (work_version_id, role, file_name)
);

CREATE INDEX idx_work_artifacts_version_role
    ON work_artifacts(work_version_id, role, created_at);

CREATE TABLE work_timelines (
    work_version_id UUID PRIMARY KEY REFERENCES work_versions(id) ON DELETE RESTRICT,
    video_tracks JSONB NOT NULL DEFAULT '[]'::jsonb,
    audio_tracks JSONB NOT NULL DEFAULT '[]'::jsonb,
    subtitle_tracks JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT work_timelines_tracks_check CHECK (
        jsonb_typeof(video_tracks)='array' AND
        jsonb_typeof(audio_tracks)='array' AND
        jsonb_typeof(subtitle_tracks)='array'
    )
);

CREATE TABLE work_version_diff_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id UUID NOT NULL REFERENCES works(id) ON DELETE RESTRICT,
    source_version_id UUID NOT NULL,
    draft_version_id UUID NOT NULL,
    plan_version INT NOT NULL,
    source_fingerprint CHAR(64) NOT NULL,
    draft_fingerprint CHAR(64) NOT NULL,
    changes JSONB NOT NULL,
    affected_nodes JSONB NOT NULL,
    reused_artifact_ids JSONB NOT NULL DEFAULT '[]'::jsonb,
    resource_usage JSONB NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'analyzed',
    confirmed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT work_version_diff_source_fk
        FOREIGN KEY (source_version_id, work_id) REFERENCES work_versions(id, work_id) ON DELETE RESTRICT,
    CONSTRAINT work_version_diff_draft_fk
        FOREIGN KEY (draft_version_id, work_id) REFERENCES work_versions(id, work_id) ON DELETE RESTRICT,
    CONSTRAINT work_version_diff_versions_check CHECK (source_version_id <> draft_version_id),
    CONSTRAINT work_version_diff_plan_version_check CHECK (plan_version > 0),
    CONSTRAINT work_version_diff_fingerprint_check CHECK (
        source_fingerprint ~ '^[0-9a-f]{64}$' AND draft_fingerprint ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT work_version_diff_json_check CHECK (
        jsonb_typeof(changes)='array' AND jsonb_typeof(affected_nodes)='array' AND
        jsonb_typeof(reused_artifact_ids)='array' AND jsonb_typeof(resource_usage)='object'
    ),
    CONSTRAINT work_version_diff_status_check CHECK (status IN ('analyzed','confirmed','invalidated')),
    UNIQUE (draft_version_id, plan_version)
);

CREATE INDEX idx_work_version_diff_plans_draft_created
    ON work_version_diff_plans(draft_version_id, created_at DESC);

CREATE TABLE work_diff_confirmations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    diff_plan_id UUID NOT NULL UNIQUE REFERENCES work_version_diff_plans(id) ON DELETE RESTRICT,
    idempotency_key VARCHAR(200) NOT NULL UNIQUE,
    generation_run_id UUID NOT NULL REFERENCES work_generation_runs(id) ON DELETE RESTRICT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT work_diff_confirmations_key_check CHECK (length(trim(idempotency_key)) > 0)
);

CREATE TABLE publication_handoffs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    work_id UUID NOT NULL REFERENCES works(id) ON DELETE RESTRICT,
    work_version_id UUID NOT NULL,
    final_video_artifact_id UUID NOT NULL REFERENCES work_artifacts(id) ON DELETE RESTRICT,
    subtitle_artifact_id UUID REFERENCES work_artifacts(id) ON DELETE RESTRICT,
    status VARCHAR(24) NOT NULL DEFAULT 'draft',
    idempotency_key VARCHAR(200) NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT publication_handoffs_version_fk
        FOREIGN KEY (work_version_id, work_id) REFERENCES work_versions(id, work_id) ON DELETE RESTRICT,
    CONSTRAINT publication_handoffs_status_check CHECK (status='draft'),
    CONSTRAINT publication_handoffs_key_check CHECK (length(trim(idempotency_key)) > 0),
    CONSTRAINT publication_handoffs_payload_no_credentials_check
        CHECK (payload = sanitize_material_metadata(payload)),
    UNIQUE (work_version_id, idempotency_key)
);

CREATE INDEX idx_publication_handoffs_version_created
    ON publication_handoffs(work_version_id, created_at DESC);

COMMENT ON COLUMN work_versions.status IS
    '版本状态：draft 可编辑；confirmed/running/completed/failed 的快照均不可原地修改。';
COMMENT ON COLUMN work_versions.source_version_id IS
    '继续修改或整体重生成时引用的同作品来源版本；旧版本保持不变。';
COMMENT ON COLUMN work_artifacts.metadata IS
    '产物审计元数据；禁止保存凭据、Token、Cookie 和密钥。';
COMMENT ON TABLE work_timelines IS
    '按版本保存视频、TTS/原声/已有音频和字幕轨道的不可变引用。';
COMMENT ON COLUMN work_version_diff_plans.resource_usage IS
    '只保存任务数、视频秒数、TTS 字符数、ASR 秒数等非金额资源用量。';
COMMENT ON TABLE publication_handoffs IS
    '发布运营草稿交接；只保存明确作品版本与产物引用，不触发平台发布。';

-- 运行事实统一驱动版本终态；确认插入 queued run 时即锁定版本快照。
CREATE OR REPLACE FUNCTION sync_work_version_from_generation_run()
RETURNS TRIGGER AS $$
DECLARE
    next_version_status VARCHAR(24);
    next_work_status VARCHAR(24);
BEGIN
    next_version_status := CASE
        WHEN NEW.status = 'succeeded' THEN 'completed'
        WHEN NEW.status IN ('failed','cancelled','waiting_manual') THEN 'failed'
        ELSE 'running'
    END;
    next_work_status := CASE
        WHEN NEW.status = 'succeeded' THEN 'succeeded'
        WHEN NEW.status IN ('failed','cancelled','waiting_manual') THEN 'failed'
        ELSE 'running'
    END;
    UPDATE work_versions
    SET status=next_version_status,
        completed_at=CASE WHEN next_version_status IN ('completed','failed') THEN COALESCE(completed_at,NOW()) ELSE NULL END
    WHERE id=NEW.work_version_id;
    UPDATE works
    SET status=next_work_status,current_version_id=NEW.work_version_id,updated_at=NOW()
    WHERE id=NEW.work_id;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_sync_work_version_from_generation_run
AFTER INSERT OR UPDATE OF status ON work_generation_runs
FOR EACH ROW EXECUTE FUNCTION sync_work_version_from_generation_run();
