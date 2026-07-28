-- 后续 migration 新增的失效事实与质量映射同样属于生产审计历史。
-- 在数据库边界禁止原地修改或删除，避免非 Repository 写路径破坏追踪链。

CREATE TRIGGER production_script_invalidations_append_only
    BEFORE UPDATE OR DELETE ON production_script_invalidations
    FOR EACH ROW EXECUTE FUNCTION reject_production_append_only_mutation();

CREATE TRIGGER production_package_invalidations_append_only
    BEFORE UPDATE OR DELETE ON production_package_invalidations
    FOR EACH ROW EXECUTE FUNCTION reject_production_append_only_mutation();

CREATE TRIGGER take_review_ledger_versions_append_only
    BEFORE UPDATE OR DELETE ON take_review_ledger_versions
    FOR EACH ROW EXECUTE FUNCTION reject_production_append_only_mutation();

CREATE TRIGGER continuity_ledgers_append_only
    BEFORE UPDATE OR DELETE ON continuity_ledgers
    FOR EACH ROW WHEN (OLD.audit_status = 'complete')
    EXECUTE FUNCTION reject_production_append_only_mutation();

CREATE TRIGGER take_reviews_append_only
    BEFORE UPDATE OR DELETE ON take_reviews
    FOR EACH ROW WHEN (OLD.audit_status = 'complete')
    EXECUTE FUNCTION reject_production_append_only_mutation();

COMMENT ON TRIGGER production_script_invalidations_append_only
    ON production_script_invalidations IS
    '脚本替换失效事实只允许追加，禁止 UPDATE 或 DELETE。';
COMMENT ON TRIGGER production_package_invalidations_append_only
    ON production_package_invalidations IS
    'ProductionPackage 失效事实只允许追加，禁止 UPDATE 或 DELETE。';
COMMENT ON TRIGGER take_review_ledger_versions_append_only
    ON take_review_ledger_versions IS
    'TakeReview 与 ContinuityLedger 精确版本映射只允许追加，禁止 UPDATE 或 DELETE。';
COMMENT ON TRIGGER continuity_ledgers_append_only
    ON continuity_ledgers IS
    '完整审计的 ContinuityLedger 只允许追加，legacy_partial_audit 历史行保持只读。';
COMMENT ON TRIGGER take_reviews_append_only
    ON take_reviews IS
    '完整审计的 TakeReview 只允许追加，legacy_partial_audit 历史行保持只读。';
