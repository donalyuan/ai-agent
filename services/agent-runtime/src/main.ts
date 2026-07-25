import { Pool } from "pg";
import { access } from "node:fs/promises";

import { loadConfig } from "./config.js";
import { SessionCoordinator } from "./coordinator.js";
import { assertProductionExecutionIntegrity, loadDefinitionRegistry } from "./definitions.js";
import { installShutdownHandlers } from "./lifecycle.js";
import { ModelConfigRepository } from "./models.js";
import { safeJson } from "./redaction.js";
import { RuntimeHttpServer } from "./server.js";
import { SessionStore } from "./sessions.js";

async function main(): Promise<void> {
  const config = loadConfig();
  const definitions = await loadDefinitionRegistry(config.definitionsDir);
  assertProductionExecutionIntegrity(definitions);
  const pool = new Pool({ connectionString: config.databaseUrl, max: 4 });
  const models = new ModelConfigRepository(pool);
  const sessions = new SessionStore(config.sqlitePath, config.workspaceRoot);
  await sessions.reconcileSessionDeletions();
  if ((await sessions.legacyMigrationPlan()).length > 0) {
    const backupPath = `${config.sqlitePath}.pre-versioned-agent-execution.bak`;
    try {
      await access(backupPath);
    } catch {
      await sessions.backupForHistoryMigration(backupPath);
    }
  }
  const coordinator = new SessionCoordinator(sessions, models, undefined, definitions);
  const runtime = new RuntimeHttpServer({ sessions, coordinator, models, pool });

  installShutdownHandlers(runtime, config.shutdownTimeoutMs);
  await runtime.listen(config.host, config.port);
  console.info(safeJson({ service: "novex-agent-runtime", status: "listening", host: config.host, port: config.port }));
}

main().catch((error: unknown) => {
  console.error(safeJson({ service: "novex-agent-runtime", status: "startup_failed", error }));
  process.exitCode = 1;
});
