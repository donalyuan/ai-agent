UPDATE video_workspace_menus
SET
    is_enabled = true,
    status = 'active',
    metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{phase}', '2'::jsonb, true),
    updated_at = NOW()
WHERE menu_key IN ('content-strategy', 'topic-generator');
