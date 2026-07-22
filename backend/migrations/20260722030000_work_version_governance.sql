-- 作品版本治理：选定当前可编辑草稿，并清理从未形成运行或下游事实的冗余草稿。

-- 旧数据可能尚未回填 current_version_id；优先保留最新可用计划绑定的未运行草稿。
UPDATE works work
SET current_version_id = candidate.work_version_id,
    status = 'planned',
    updated_at = NOW()
FROM (
    SELECT DISTINCT ON (plan.work_id)
        plan.work_id,
        plan.work_version_id
    FROM work_plans plan
    JOIN work_versions version ON version.id = plan.work_version_id
    JOIN works owner ON owner.id = plan.work_id
    WHERE owner.current_version_id IS NULL
      AND version.status = 'draft'
      AND plan.status IN ('draft', 'ready')
      AND NOT EXISTS (
          SELECT 1 FROM work_generation_runs run
          WHERE run.work_version_id = version.id
      )
    ORDER BY
        plan.work_id,
        CASE plan.status WHEN 'ready' THEN 0 ELSE 1 END,
        plan.plan_version DESC,
        plan.updated_at DESC
) candidate
WHERE work.id = candidate.work_id
  AND work.current_version_id IS NULL;

-- 临时表固定完整安全谓词，后续先删计划再删版本，避免依赖级联删除。
CREATE TEMP TABLE redundant_work_version_drafts ON COMMIT DROP AS
SELECT version.id
FROM work_versions version
JOIN works work ON work.id = version.work_id
WHERE version.status = 'draft'
  AND work.current_version_id IS NOT NULL
  AND work.current_version_id <> version.id
  AND NOT EXISTS (
      SELECT 1 FROM work_plans plan
      WHERE plan.work_version_id = version.id
        AND plan.status <> 'invalidated'
  )
  AND NOT EXISTS (
      SELECT 1 FROM work_generation_runs run
      WHERE run.work_version_id = version.id
  )
  AND NOT EXISTS (
      SELECT 1
      FROM work_generation_steps step
      JOIN work_generation_runs run ON run.id = step.run_id
      WHERE run.work_version_id = version.id
  )
  AND NOT EXISTS (
      SELECT 1
      FROM work_generation_attempts attempt
      JOIN work_generation_steps step ON step.id = attempt.step_id
      JOIN work_generation_runs run ON run.id = step.run_id
      WHERE run.work_version_id = version.id
  )
  AND NOT EXISTS (
      SELECT 1 FROM work_artifacts artifact
      WHERE artifact.work_version_id = version.id
  )
  AND NOT EXISTS (
      SELECT 1 FROM work_timelines timeline
      WHERE timeline.work_version_id = version.id
  )
  AND NOT EXISTS (
      SELECT 1 FROM work_version_diff_plans diff
      WHERE diff.source_version_id = version.id
         OR diff.draft_version_id = version.id
  )
  AND NOT EXISTS (
      SELECT 1
      FROM work_diff_confirmations confirmation
      JOIN work_version_diff_plans diff ON diff.id = confirmation.diff_plan_id
      WHERE diff.source_version_id = version.id
         OR diff.draft_version_id = version.id
  )
  AND NOT EXISTS (
      SELECT 1 FROM publication_handoffs handoff
      WHERE handoff.work_version_id = version.id
  )
  AND NOT EXISTS (
      SELECT 1 FROM work_versions derived
      WHERE derived.source_version_id = version.id
  );

DELETE FROM work_plans plan
USING redundant_work_version_drafts candidate
WHERE plan.work_version_id = candidate.id
  AND plan.status = 'invalidated';

DELETE FROM work_versions version
USING redundant_work_version_drafts candidate
WHERE version.id = candidate.id;

CREATE INDEX idx_work_versions_draft_derivation
    ON work_versions(work_id, source_version_id, derivation_kind, version_no DESC)
    WHERE status = 'draft';

COMMENT ON INDEX idx_work_versions_draft_derivation IS
    '按作品、来源版本和派生类型定位可复用草稿；并发创建仍由作品行锁串行化。';
