-- Phase 5 当前是人工发布工作台，不提供自动排程或账号管理入口。
UPDATE video_workspace_menus
SET menu_key='publication-workbench', label='发布工作台',
    description='准备抖音和小红书发布包，打开官方创作者页面并登记人工发布结果。',
    route_path='/publishing/workbench', icon='send', module_key='publishing.workbench',
    agent_key='manual-publication-operations', is_enabled=true, is_visible=true, status='active',
    metadata='{"phase":5,"mode":"manual_web_handoff"}'::jsonb, updated_at=NOW()
WHERE menu_key='publish-scheduler';

UPDATE video_workspace_menus SET is_enabled=true,is_visible=true,status='active',updated_at=NOW()
WHERE menu_key='publishing';
