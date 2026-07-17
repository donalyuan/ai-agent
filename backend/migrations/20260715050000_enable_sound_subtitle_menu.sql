-- The sound/subtitle workflow is now implemented in the video workspace.

UPDATE video_workspace_menus
SET
    description = '生成可复用的 TTS 配音与真实时间轴字幕。',
    is_enabled = true,
    status = 'active',
    updated_at = NOW()
WHERE menu_key = 'sound-subtitle-generation';
