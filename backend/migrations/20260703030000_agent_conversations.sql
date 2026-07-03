-- Persist generic conversational Agent sessions and messages.
-- This keeps dialogue state separate from one-off agent_runs while still linking
-- each user turn back to run/step records for replay and later evaluation.

CREATE TABLE agent_conversations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID REFERENCES projects(id) ON DELETE SET NULL,
    agent_type VARCHAR(30) NOT NULL,
    subject_type VARCHAR(60),
    subject_id UUID,
    title VARCHAR(160) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_conversations_agent_type_check CHECK (
        agent_type IN ('topic', 'script', 'material', 'video', 'publish', 'optimization')
    ),
    CONSTRAINT agent_conversations_status_check CHECK (status IN ('active', 'archived')),
    CONSTRAINT agent_conversations_subject_pair_check CHECK (
        (subject_type IS NULL AND subject_id IS NULL)
        OR (subject_type IS NOT NULL AND subject_id IS NOT NULL)
    )
);

COMMENT ON TABLE agent_conversations IS '通用 Agent 对话会话，绑定项目和可选业务资源。';
COMMENT ON COLUMN agent_conversations.subject_type IS '会话绑定资源类型，例如 script。';
COMMENT ON COLUMN agent_conversations.subject_id IS '会话绑定资源 ID，例如 scripts.id。';
COMMENT ON COLUMN agent_conversations.metadata IS '会话级上下文，保存 skill、工具、记忆策略等扩展配置。';

CREATE INDEX idx_agent_conversations_project ON agent_conversations(project_id, created_at DESC)
    WHERE project_id IS NOT NULL;
CREATE INDEX idx_agent_conversations_subject ON agent_conversations(subject_type, subject_id)
    WHERE subject_id IS NOT NULL;
CREATE INDEX idx_agent_conversations_agent_type ON agent_conversations(agent_type);

CREATE TRIGGER trigger_agent_conversations_updated_at
    BEFORE UPDATE ON agent_conversations
    FOR EACH ROW
    EXECUTE FUNCTION set_updated_at();

CREATE TABLE agent_messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    conversation_id UUID NOT NULL REFERENCES agent_conversations(id) ON DELETE CASCADE,
    role VARCHAR(20) NOT NULL,
    content TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT agent_messages_role_check CHECK (role IN ('system', 'user', 'assistant', 'tool')),
    CONSTRAINT agent_messages_content_check CHECK (length(btrim(content)) > 0)
);

COMMENT ON TABLE agent_messages IS 'Agent 对话消息，按会话保存用户、助手、系统和工具消息。';
COMMENT ON COLUMN agent_messages.metadata IS '消息级结构化数据，例如关联 run、tool call 或修改产物。';

CREATE INDEX idx_agent_messages_conversation_created ON agent_messages(conversation_id, created_at ASC, id ASC);
