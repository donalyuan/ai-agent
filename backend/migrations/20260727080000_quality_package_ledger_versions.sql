-- QualityPackage 必须能证明每个 TakeReview 使用了哪些不可变 ContinuityLedger 版本。
-- 关系使用独立表和真实外键保存，禁止只在 JSON content 中写自由字符串引用。
CREATE TABLE take_review_ledger_versions (
    take_review_id UUID NOT NULL REFERENCES take_reviews(id) ON DELETE RESTRICT,
    ordinal INT NOT NULL,
    continuity_ledger_id UUID NOT NULL REFERENCES continuity_ledgers(id) ON DELETE RESTRICT,
    shot_contract_id UUID NOT NULL REFERENCES shot_contracts(id) ON DELETE RESTRICT,
    ledger_version INT NOT NULL,
    content_digest CHAR(64) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (take_review_id, ordinal),
    CONSTRAINT take_review_ledger_versions_values_check CHECK (
        ordinal >= 0 AND ledger_version > 0 AND content_digest ~ '^[0-9a-f]{64}$'
    ),
    CONSTRAINT take_review_ledger_versions_ledger_unique UNIQUE (
        take_review_id, continuity_ledger_id
    ),
    CONSTRAINT take_review_ledger_versions_shot_unique UNIQUE (
        take_review_id, shot_contract_id
    )
);

COMMENT ON TABLE take_review_ledger_versions IS
    'TakeReview 对当前适用 Shot 的 ContinuityLedger 精确版本引用；旧 WorkVersion 的映射只保留审计，不得进入新 QualityPackage。';
COMMENT ON COLUMN take_review_ledger_versions.ordinal IS
    '按 RequiredTake 中 Scene/Shot 的确定性顺序保存，QualityPackage 构建时必须精确复验。';
