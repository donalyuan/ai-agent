-- Add the dedicated material-generation workspace entry without mutating applied menu migrations.

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
    '30000000-0000-4000-8000-000000000002',
    parent.id,
    'asset-generation',
    '素材生成',
    '按脚本分镜生成、复用和选择素材候选，AI 视频生成必须人工二次确认。',
    '/materials/generation',
    'image-plus',
    'page',
    'materials.asset-generation',
    'material-generation-agent',
    20,
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
