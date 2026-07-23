-- 人工发布运营事实；不保存账号、凭据或自动发布任务。
CREATE TABLE publication_plans (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    handoff_id UUID NOT NULL UNIQUE REFERENCES publication_handoffs(id) ON DELETE RESTRICT,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE publication_targets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    publication_plan_id UUID NOT NULL REFERENCES publication_plans(id) ON DELETE RESTRICT,
    platform VARCHAR(24) NOT NULL,
    status VARCHAR(24) NOT NULL DEFAULT 'draft',
    title TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    tags JSONB NOT NULL DEFAULT '[]'::jsonb,
    cover_artifact_id UUID REFERENCES work_artifacts(id) ON DELETE RESTRICT,
    planned_at TIMESTAMPTZ,
    draft_revision INTEGER NOT NULL DEFAULT 1,
    handed_off_at TIMESTAMPTZ,
    published_at TIMESTAMPTZ,
    published_url TEXT,
    result_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT publication_targets_platform_check CHECK (platform IN ('douyin', 'xiaohongshu')),
    CONSTRAINT publication_targets_status_check CHECK (status IN ('draft','ready','handed_off','needs_attention','published','cancelled')),
    CONSTRAINT publication_targets_revision_check CHECK (draft_revision > 0),
    CONSTRAINT publication_targets_tags_check CHECK (jsonb_typeof(tags) = 'array'),
    CONSTRAINT publication_targets_result_safe CHECK (result_snapshot = sanitize_material_metadata(result_snapshot)),
    CONSTRAINT publication_targets_published_check CHECK ((status = 'published') = (published_at IS NOT NULL AND published_url IS NOT NULL)),
    UNIQUE (publication_plan_id, platform)
);

CREATE TABLE publication_packages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    publication_target_id UUID NOT NULL REFERENCES publication_targets(id) ON DELETE RESTRICT,
    draft_revision INTEGER NOT NULL,
    platform_rule_version VARCHAR(80) NOT NULL,
    manifest JSONB NOT NULL,
    manifest_sha256 CHAR(64) NOT NULL,
    package_storage_path TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT publication_packages_revision_check CHECK (draft_revision > 0),
    CONSTRAINT publication_packages_manifest_safe CHECK (manifest = sanitize_material_metadata(manifest)),
    CONSTRAINT publication_packages_sha_check CHECK (manifest_sha256 ~ '^[0-9a-f]{64}$'),
    UNIQUE (publication_target_id, draft_revision)
);

CREATE TABLE publication_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    publication_target_id UUID NOT NULL REFERENCES publication_targets(id) ON DELETE RESTRICT,
    event_type VARCHAR(40) NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT publication_events_type_check CHECK (event_type IN ('created','draft_updated','package_generated','downloaded','copied','handed_off','needs_attention','published','result_corrected','cancelled')),
    CONSTRAINT publication_events_payload_safe CHECK (payload = sanitize_material_metadata(payload))
);

CREATE INDEX idx_publication_targets_pending ON publication_targets(status, planned_at) WHERE status NOT IN ('published','cancelled');
CREATE INDEX idx_publication_events_target_created ON publication_events(publication_target_id, created_at DESC);
CREATE TRIGGER trigger_publication_plans_updated_at BEFORE UPDATE ON publication_plans FOR EACH ROW EXECUTE FUNCTION set_updated_at();
CREATE TRIGGER trigger_publication_targets_updated_at BEFORE UPDATE ON publication_targets FOR EACH ROW EXECUTE FUNCTION set_updated_at();

COMMENT ON TABLE publication_plans IS '一个完成作品版本交接对应一个人工发布计划；状态由目标派生。';
COMMENT ON TABLE publication_targets IS '抖音/小红书独立人工发布目标；planned_at 仅用于提醒，绝不创建 Worker。';
COMMENT ON TABLE publication_packages IS '不可变发布包 manifest；禁止内部路径、签名 URL 和凭据。';
COMMENT ON TABLE publication_events IS '人工发布追加式审计事件；不得覆盖既有事件。';
COMMENT ON TABLE publish_tasks IS 'Legacy 发布任务表，仅保留历史记录；人工发布运营新代码不得写入。';
