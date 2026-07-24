import { Pool } from "pg";

import { loadConfig } from "./config.js";
import { SessionCoordinator } from "./coordinator.js";
import { installShutdownHandlers } from "./lifecycle.js";
import { ModelConfigRepository } from "./models.js";
import { safeJson } from "./redaction.js";
import { RuntimeHttpServer } from "./server.js";
import { SessionStore } from "./sessions.js";

async function main(): Promise<void> {
  const config = loadConfig();
  const pool = new Pool({ connectionString: config.databaseUrl, max: 4 });
  const models = new ModelConfigRepository(pool);
  const sessions = new SessionStore(config.sqlitePath, config.workspaceRoot);
  const coordinator = new SessionCoordinator(sessions, models);
  const runtime = new RuntimeHttpServer({ sessions, coordinator, models, pool });

  installShutdownHandlers(runtime, config.shutdownTimeoutMs);
  await runtime.listen(config.host, config.port);
  console.info(safeJson({ service: "novex-agent-runtime", status: "listening", host: config.host, port: config.port }));
}

main().catch((error: unknown) => {
  console.error(safeJson({ service: "novex-agent-runtime", status: "startup_failed", error }));
  process.exitCode = 1;
});
