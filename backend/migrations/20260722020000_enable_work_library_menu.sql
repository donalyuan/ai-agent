-- 作品库是作品生产的独立二级模块，路由和启用状态由数据库统一下发。
INSERT INTO video_workspace_menus (
    id, parent_id, menu_key, label, description, route_path, icon, menu_type, module_key,
    agent_key, sort_order, is_enabled, is_visible, status, metadata
)
SELECT '30000000-0000-4000-8000-000000000005', parent.id, 'work-library', '作品库',
       '按不可变版本管理成片、时间轴、调用审计、下载和发布草稿交接。',
       '/production/library', 'library', 'page', 'production.work-library', 'work',
       30, true, true, 'active', '{"phase":4}'::jsonb
FROM video_workspace_menus parent WHERE parent.menu_key = 'production'
ON CONFLICT (menu_key) DO UPDATE SET parent_id = EXCLUDED.parent_id, label = EXCLUDED.label,
    description = EXCLUDED.description, route_path = EXCLUDED.route_path, icon = EXCLUDED.icon,
    module_key = EXCLUDED.module_key, agent_key = EXCLUDED.agent_key, sort_order = EXCLUDED.sort_order,
    is_enabled = EXCLUDED.is_enabled, is_visible = EXCLUDED.is_visible, status = EXCLUDED.status,
    metadata = EXCLUDED.metadata, updated_at = NOW();
