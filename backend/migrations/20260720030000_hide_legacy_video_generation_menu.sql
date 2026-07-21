-- `work-generation` 已替代早期 Phase 4 的 `video-generation` 占位入口。
-- 保留旧记录用于历史审计，但禁止菜单 API 继续返回重复入口。
UPDATE video_workspace_menus
SET is_enabled = false,
    is_visible = false,
    status = 'disabled',
    updated_at = NOW()
WHERE menu_key = 'video-generation';
