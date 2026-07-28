-- 计划声明的修订命令是普通 ProductionRun 唯一允许追加用户指令的入口。
-- 指令与 revision epoch 一同追加保存，Context Compiler 只能读取当前 epoch
-- 且 owner_role 与待执行角色完全匹配的记录。
CREATE TABLE production_revision_instructions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    run_id UUID NOT NULL REFERENCES production_runs(id) ON DELETE RESTRICT,
    revision_epoch INT NOT NULL,
    owner_role VARCHAR(80) NOT NULL,
    actor_type VARCHAR(32) NOT NULL,
    actor_id VARCHAR(120) NOT NULL,
    source VARCHAR(80) NOT NULL,
    trust VARCHAR(40) NOT NULL,
    instruction TEXT NOT NULL,
    instruction_digest CHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT production_revision_instructions_epoch_fk
        FOREIGN KEY (run_id, revision_epoch)
        REFERENCES production_revision_epochs(run_id, epoch) ON DELETE RESTRICT,
    CONSTRAINT production_revision_instructions_epoch_check CHECK (revision_epoch > 0),
    CONSTRAINT production_revision_instructions_owner_check CHECK (
        length(btrim(owner_role)) > 0
    ),
    CONSTRAINT production_revision_instructions_actor_check CHECK (
        actor_type = 'local_operator' AND length(btrim(actor_id)) > 0
    ),
    CONSTRAINT production_revision_instructions_source_check CHECK (
        source = 'script_revision_command'
    ),
    CONSTRAINT production_revision_instructions_trust_check CHECK (
        trust = 'user_instruction'
    ),
    CONSTRAINT production_revision_instructions_instruction_check CHECK (
        length(btrim(instruction)) > 0
        AND instruction_digest ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT production_revision_instructions_identity_unique
        UNIQUE (run_id, revision_epoch, owner_role)
);

CREATE INDEX idx_production_revision_instructions_owner
    ON production_revision_instructions(run_id, revision_epoch, owner_role);

CREATE TRIGGER production_revision_instructions_append_only
    BEFORE UPDATE OR DELETE ON production_revision_instructions
    FOR EACH ROW EXECUTE FUNCTION reject_production_append_only_mutation();

COMMENT ON TABLE production_revision_instructions IS
    '受控修订命令追加的用户指令；仅当前 revision epoch 的目标 owner 可编译进 Context。';
COMMENT ON COLUMN production_revision_instructions.instruction_digest IS
    '对 instruction 正文计算的 canonical SHA-256，用于 role input 与 Context 审计闭合。';
