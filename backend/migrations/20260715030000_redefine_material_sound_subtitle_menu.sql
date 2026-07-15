-- Replace the obsolete material-search placeholder with the planned sound/subtitle workspace entry.
-- The dedicated sound-subtitle change will enable this menu when its workflow is implemented.

UPDATE video_workspace_menus
SET
    menu_key = 'sound-subtitle-generation',
    label = '声音与字幕生成',
    description = '生成 TTS 配音与对齐字幕；对应能力完成前仅作为计划入口展示。',
    route_path = '/materials/sound-subtitle-generation',
    icon = 'audio-lines',
    menu_type = 'page',
    module_key = 'materials.sound-subtitle-generation',
    agent_key = 'sound-generation-agent',
    sort_order = 30,
    is_enabled = false,
    is_visible = true,
    status = 'planned',
    metadata = jsonb_set(COALESCE(metadata, '{}'::jsonb), '{phase}', '4'::jsonb, true),
    updated_at = NOW()
WHERE menu_key = 'material-search';
