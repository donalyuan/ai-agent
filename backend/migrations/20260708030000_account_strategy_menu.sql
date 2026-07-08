-- 将账号策略资料沉淀为内容策略下的独立二级菜单。
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
    '20000000-0000-4000-8000-000000000009',
    parent.id,
    'account-strategy',
    '账号策略',
    '维护账号定位、受众画像、内容支柱和选题偏好，供选题生成与质量评审使用。',
    '/strategy/account',
    'badge',
    'page',
    'strategy.account',
    'topic-generation-agent',
    10,
    true,
    true,
    'active',
    '{"phase":2}'::jsonb
FROM video_workspace_menus parent
WHERE parent.menu_key = 'content-strategy'
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
    is_enabled = true,
    is_visible = true,
    status = 'active',
    metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{phase}', '2'::jsonb, true),
    updated_at = NOW()
WHERE menu_key = 'topic-history';

UPDATE video_workspace_menus
SET
    label = '当前选题池',
    description = '查看当前选题池，承接历史生成批次切换、选题确认和脚本生成。',
    sort_order = 30,
    is_enabled = true,
    is_visible = true,
    status = 'active',
    metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{phase}', '2'::jsonb, true),
    updated_at = NOW()
WHERE menu_key = 'topic-generator';
