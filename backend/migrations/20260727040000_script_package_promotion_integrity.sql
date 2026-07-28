-- A Full Crew ScriptPackage is an immutable promotion source. The application
-- transaction serializes a run, while this index protects the same invariant
-- against any future writer that bypasses that service boundary.
CREATE UNIQUE INDEX scripts_full_crew_package_unique
    ON scripts(production_run_id, script_package_id)
    WHERE production_run_id IS NOT NULL AND script_package_id IS NOT NULL;

COMMENT ON INDEX scripts_full_crew_package_unique IS
    '同一 ProductionRun 的同一 ScriptPackage 只能晋升为一个正式 Script；脚本修订必须使用新的 package。';
