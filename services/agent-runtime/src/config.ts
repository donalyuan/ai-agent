import { isAbsolute, resolve } from "node:path";

import { RuntimeError } from "./errors.js";

export interface RuntimeConfig {
  host: string;
  port: number;
  databaseUrl: string;
  sqlitePath: string;
  workspaceRoot: string;
  shutdownTimeoutMs: number;
}

function required(env: NodeJS.ProcessEnv, name: string): string {
  const value = env[name]?.trim();
  if (!value) {
    throw new RuntimeError("config_invalid", 500, `缺少必需环境变量 ${name}`);
  }
  return value;
}

function positiveInteger(value: string | undefined, fallback: number, name: string): number {
  const parsed = value === undefined || value.trim() === "" ? fallback : Number(value);
  if (!Number.isSafeInteger(parsed) || parsed <= 0) {
    throw new RuntimeError("config_invalid", 500, `${name} 必须是正整数`);
  }
  return parsed;
}

function absolutePath(value: string, name: string): string {
  if (!isAbsolute(value)) {
    throw new RuntimeError("config_invalid", 500, `${name} 必须是绝对路径`);
  }
  return resolve(value);
}

export function loadConfig(env: NodeJS.ProcessEnv = process.env): RuntimeConfig {
  const port = positiveInteger(env.AGENT_RUNTIME_PORT, 8082, "AGENT_RUNTIME_PORT");
  if (port > 65_535) {
    throw new RuntimeError("config_invalid", 500, "AGENT_RUNTIME_PORT 必须小于等于 65535");
  }

  return {
    host: env.AGENT_RUNTIME_HOST?.trim() || "0.0.0.0",
    port,
    databaseUrl: required(env, "DATABASE_URL"),
    sqlitePath: absolutePath(
      env.AGENT_RUNTIME_SQLITE_PATH?.trim() || "/data/agent-sessions.sqlite",
      "AGENT_RUNTIME_SQLITE_PATH",
    ),
    workspaceRoot: absolutePath(
      env.AGENT_RUNTIME_WORKSPACE_ROOT?.trim() || "/workspace",
      "AGENT_RUNTIME_WORKSPACE_ROOT",
    ),
    shutdownTimeoutMs: positiveInteger(
      env.AGENT_RUNTIME_SHUTDOWN_TIMEOUT_MS,
      10_000,
      "AGENT_RUNTIME_SHUTDOWN_TIMEOUT_MS",
    ),
  };
}
