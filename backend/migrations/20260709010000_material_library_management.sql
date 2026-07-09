-- Enable the first material library management slice without mutating applied migrations.

ALTER TABLE materials
    ADD COLUMN status VARCHAR(20) NOT NULL DEFAULT 'active';

ALTER TABLE materials
    DROP CONSTRAINT materials_type_check;

ALTER TABLE materials
    ADD CONSTRAINT materials_type_check
        CHECK (material_type IN ('video', 'image', 'audio', 'subtitle'));

ALTER TABLE materials
    ADD CONSTRAINT materials_status_check
        CHECK (status IN ('active', 'archived'));

COMMENT ON COLUMN materials.status IS '素材库状态：active 可用，archived 已归档但保留历史引用。';

CREATE INDEX idx_materials_project_status_updated
    ON materials(project_id, status, updated_at DESC);

UPDATE video_workspace_menus
SET
    is_enabled = true,
    status = 'active',
    metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{phase}', '3'::jsonb, true),
    updated_at = NOW()
WHERE menu_key = 'material-management';

INSERT INTO video_workspace_menus (
    id,
    parent_id,
    menu_key,
    label,
    description,
    route_path,
    icon,
    menu_type,
    module_key,
    agent_key,
    sort_order,
    is_enabled,
    is_visible,
    status,
    metadata
)
SELECT
    '30000000-0000-4000-8000-000000000001',
    parent.id,
    'material-library',
    '素材库',
    '登记和管理当前账号下的视频、图片、音频和字幕素材。',
    '/materials/library',
    'folder-open',
    'page',
    'materials.library',
    NULL,
    10,
    true,
    true,
    'active',
    '{"phase":3}'::jsonb
FROM video_workspace_menus parent
WHERE parent.menu_key = 'material-management'
ON CONFLICT (menu_key) DO UPDATE
SET
    parent_id = EXCLUDED.parent_id,
    label = EXCLUDED.label,
    description = EXCLUDED.description,
    route_path = EXCLUDED.route_path,
    icon = EXCLUDED.icon,
    menu_type = EXCLUDED.menu_type,
    module_key = EXCLUDED.module_key,
    agent_key = EXCLUDED.agent_key,
    sort_order = EXCLUDED.sort_order,
    is_enabled = EXCLUDED.is_enabled,
    is_visible = EXCLUDED.is_visible,
    status = EXCLUDED.status,
    metadata = EXCLUDED.metadata,
    updated_at = NOW();

UPDATE video_workspace_menus
SET
    sort_order = 20,
    updated_at = NOW()
WHERE menu_key = 'material-search';
