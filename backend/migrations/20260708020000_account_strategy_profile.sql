ALTER TABLE projects
    ADD COLUMN strategy_profile JSONB NOT NULL DEFAULT '{}'::jsonb;

COMMENT ON COLUMN projects.strategy_profile IS '内容账号结构化策略资料，供内容策略页、选题 Agent、质量闸门和主题组评审使用。';
