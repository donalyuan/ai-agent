-- 允许虚拟制作团队角色执行在 agent_runs 中记录，
-- 以支持 context_compile_attempts 的外键约束（错误路径审计）。
--
-- 追加 production 类型；同时保留实际已存在的 sound 类型。

ALTER TABLE agent_runs
    DROP CONSTRAINT IF EXISTS agent_runs_type_check;

ALTER TABLE agent_runs
    ADD CONSTRAINT agent_runs_type_check CHECK (
        agent_type IN (
            'topic', 'script', 'material', 'sound', 'video',
            'publish', 'optimization', 'production'
        )
    );

COMMENT ON CONSTRAINT agent_runs_type_check ON agent_runs IS
    '允许的 agent 类型；production 用于虚拟制作团队角色执行';
