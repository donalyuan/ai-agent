import { randomUUID } from "node:crypto";
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";

import type { Pool } from "pg";

import { SessionCoordinator, type TextModelResolver, type UpgradeForkInput } from "./coordinator.js";
import { normalizeError, publicError, RuntimeError } from "./errors.js";
import { redactUnknown, safeJson } from "./redaction.js";
import type { ContextRecordListFilter, ModelCallListFilter } from "./persistence.js";
import type { SessionStore, ToolProfile } from "./sessions.js";

const MAX_BODY_BYTES = 1_048_576;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export interface RuntimeDependencies {
  sessions: SessionStore;
  coordinator: SessionCoordinator;
  models: TextModelResolver;
  pool: Pick<Pool, "end">;
}

function writeJson(response: ServerResponse, status: number, body: unknown): void {
  if (response.headersSent) return;
  const payload = safeJson(body);
  response.writeHead(status, {
    "content-type": "application/json; charset=utf-8",
    "content-length": Buffer.byteLength(payload),
  });
  response.end(payload);
}

async function readJson(request: IncomingMessage): Promise<Record<string, unknown>> {
  let size = 0;
  const chunks: Buffer[] = [];
  for await (const chunk of request) {
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    size += buffer.length;
    if (size > MAX_BODY_BYTES) throw new RuntimeError("bad_request", 413, "请求体过大");
    chunks.push(buffer);
  }
  if (chunks.length === 0) return {};
  try {
    const value: unknown = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    if (value === null || typeof value !== "object" || Array.isArray(value)) throw new Error("not an object");
    return value as Record<string, unknown>;
  } catch {
    throw new RuntimeError("bad_request", 400, "请求体必须是 JSON object");
  }
}

function nonEmptyString(body: Record<string, unknown>, key: string): string {
  const value = body[key];
  if (typeof value !== "string" || value.trim() === "") {
    throw new RuntimeError("bad_request", 400, `${key} 必须是非空字符串`);
  }
  return value.trim();
}

function optionalString(body: Record<string, unknown>, key: string): string | undefined {
  const value = body[key];
  if (value === undefined || value === null) return undefined;
  if (typeof value !== "string") throw new RuntimeError("bad_request", 400, `${key} 必须是字符串`);
  return value.trim() || undefined;
}

function rejectUnknownFields(body: Record<string, unknown>, allowed: readonly string[]): void {
  const known = new Set(allowed);
  const unknown = Object.keys(body).find((key) => !known.has(key));
  if (unknown) throw new RuntimeError("bad_request", 400, `不支持字段 ${unknown}`);
}

function objectField(body: Record<string, unknown>, key: string): Record<string, unknown> | undefined {
  const value = body[key];
  if (value === undefined) return undefined;
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new RuntimeError("bad_request", 400, `${key} 必须是 JSON object`);
  }
  return value as Record<string, unknown>;
}

function sessionId(value: string): string {
  if (!UUID.test(value)) throw new RuntimeError("bad_request", 400, "session_id 格式无效");
  return value;
}

function modelCallListQuery(url: URL, fixedSessionId?: string): {
  filter: ModelCallListFilter;
  limit: number;
  offset: number;
} {
  const offset = Number(url.searchParams.get("offset") ?? "0");
  const limit = Number(url.searchParams.get("limit") ?? "100");
  if (!Number.isSafeInteger(offset) || offset < 0 || !Number.isSafeInteger(limit) || limit < 1 || limit > 500) {
    throw new RuntimeError("bad_request", 400, "ModelCall 分页参数无效");
  }
  const ownerType = url.searchParams.get("owner_type");
  const ownerId = url.searchParams.get("owner_id");
  let owner: string | undefined = fixedSessionId;
  if (fixedSessionId !== undefined) {
    if ((ownerType !== null && ownerType !== "session") || (ownerId !== null && ownerId !== fixedSessionId)) {
      throw new RuntimeError("bad_request", 400, "Session ModelCall owner 筛选冲突");
    }
  } else if (ownerType === null && ownerId === null) {
    owner = undefined;
  } else if (ownerType === "session" && ownerId !== null && UUID.test(ownerId)) {
    owner = ownerId;
  } else {
    throw new RuntimeError("bad_request", 400, "owner_type 与 owner_id 必须成对且类型有效");
  }
  const status = url.searchParams.get("status");
  if (status !== null && !["prepared", "succeeded", "failed", "aborted"].includes(status)) {
    throw new RuntimeError("bad_request", 400, "status 无效");
  }
  const modelId = url.searchParams.get("model_id") ?? undefined;
  if (modelId !== undefined && !UUID.test(modelId)) throw new RuntimeError("bad_request", 400, "model_id 格式无效");
  const timestamp = (key: string): string | undefined => {
    const value = url.searchParams.get(key);
    if (value === null) return undefined;
    const parsed = new Date(value);
    if (!Number.isFinite(parsed.getTime())) throw new RuntimeError("bad_request", 400, `${key} 格式无效`);
    return parsed.toISOString();
  };
  const preparedFrom = timestamp("prepared_from");
  const preparedTo = timestamp("prepared_to");
  if (preparedFrom !== undefined && preparedTo !== undefined && preparedFrom > preparedTo) {
    throw new RuntimeError("bad_request", 400, "prepared_from 不得晚于 prepared_to");
  }
  const text = (key: string): string | undefined => url.searchParams.get(key)?.trim() || undefined;
  const filter: ModelCallListFilter = {};
  if (owner !== undefined) filter.sessionId = owner;
  const nodeKey = text("node_key");
  const agentKey = text("agent_key");
  const agentVersion = text("agent_version");
  const promptKey = text("prompt_key");
  const promptVersion = text("prompt_version");
  if (nodeKey !== undefined) filter.nodeKey = nodeKey;
  if (agentKey !== undefined) filter.agentKey = agentKey;
  if (agentVersion !== undefined) filter.agentVersion = agentVersion;
  if (promptKey !== undefined) filter.promptKey = promptKey;
  if (promptVersion !== undefined) filter.promptVersion = promptVersion;
  if (modelId !== undefined) filter.modelId = modelId;
  if (status === "prepared" || status === "succeeded" || status === "failed" || status === "aborted") {
    filter.status = status;
  }
  if (preparedFrom !== undefined) filter.preparedFrom = preparedFrom;
  if (preparedTo !== undefined) filter.preparedTo = preparedTo;
  return {
    filter,
    limit,
    offset,
  };
}

function contextListQuery(url: URL, fixedSessionId?: string): {
  filter: ContextRecordListFilter;
  limit: number;
  offset: number;
} {
  const offset = Number(url.searchParams.get("offset") ?? "0");
  const limit = Number(url.searchParams.get("limit") ?? "100");
  if (!Number.isSafeInteger(offset) || offset < 0 || !Number.isSafeInteger(limit) || limit < 1 || limit > 500) {
    throw new RuntimeError("bad_request", 400, "Context 分页参数无效");
  }
  const ownerType = url.searchParams.get("owner_type");
  const ownerId = url.searchParams.get("owner_id");
  let sessionId = fixedSessionId;
  if (fixedSessionId !== undefined) {
    if ((ownerType !== null && ownerType !== "session") || (ownerId !== null && ownerId !== fixedSessionId)) {
      throw new RuntimeError("bad_request", 400, "Session Context owner 筛选冲突");
    }
  } else if (ownerType === null && ownerId === null) {
    sessionId = undefined;
  } else if (ownerType === "session" && ownerId !== null && UUID.test(ownerId)) {
    sessionId = ownerId;
  } else {
    throw new RuntimeError("bad_request", 400, "owner_type 与 owner_id 必须成对且类型有效");
  }
  const rawType = url.searchParams.get("record_type");
  if (rawType !== null && rawType !== "snapshot" && rawType !== "compile_attempt") {
    throw new RuntimeError("bad_request", 400, "record_type 无效");
  }
  return {
    filter: {
      ...(sessionId ? { sessionId } : {}),
      ...(rawType ? { recordType: rawType } : {}),
      ...(url.searchParams.get("node_key")?.trim() ? { nodeKey: url.searchParams.get("node_key")!.trim() } : {}),
    },
    limit,
    offset,
  };
}

async function writeSse(response: ServerResponse, event: string, data: unknown): Promise<void> {
  if (response.destroyed || response.writableEnded) throw new Error("SSE connection is closed");
  const payload = `event: ${event}\ndata: ${safeJson(data)}\n\n`;
  if (response.write(payload)) return;
  await new Promise<void>((resolve, reject) => {
    response.once("drain", resolve);
    response.once("error", reject);
  });
}

export class RuntimeHttpServer {
  private readonly server: Server;
  private closed = false;

  constructor(private readonly dependencies: RuntimeDependencies) {
    this.server = createServer((request, response) => void this.handle(request, response));
  }

  async listen(host: string, port: number): Promise<void> {
    await new Promise<void>((resolve, reject) => {
      this.server.once("error", reject);
      this.server.listen(port, host, () => {
        this.server.off("error", reject);
        resolve();
      });
    });
  }

  get port(): number {
    const address = this.server.address();
    if (!address || typeof address === "string") throw new Error("Runtime server is not listening");
    return (address as AddressInfo).port;
  }

  async close(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    await this.dependencies.coordinator.close();
    await new Promise<void>((resolve, reject) => {
      this.server.close((error) => (error ? reject(error) : resolve()));
      this.server.closeAllConnections();
    });
    await this.dependencies.pool.end();
  }

  private async handle(request: IncomingMessage, response: ServerResponse): Promise<void> {
    try {
      await this.route(request, response);
    } catch (error) {
      const normalized = normalizeError(error);
      if (response.headersSent) {
        if (!response.writableEnded) {
          await writeSse(response, "run_failed", publicError(normalized));
          response.end();
        }
        return;
      }
      writeJson(response, normalized.status, publicError(normalized));
    }
  }

  private async route(request: IncomingMessage, response: ServerResponse): Promise<void> {
    const method = request.method ?? "GET";
    const url = new URL(request.url ?? "/", "http://runtime.local");
    const segments = url.pathname.split("/").filter(Boolean).map(decodeURIComponent);

    if (method === "GET" && url.pathname === "/health") {
      writeJson(response, 200, { service: "novex-agent-runtime", status: "ok" });
      return;
    }
    if (method === "GET" && url.pathname === "/ready") {
      await this.dependencies.models.ping();
      await this.dependencies.sessions.ping();
      writeJson(response, 200, { service: "novex-agent-runtime", status: "ready", postgresql: "ok", sqlite: "ok" });
      return;
    }
    if (segments[0] === "model-calls") {
      if (segments.length === 1 && method === "GET") {
        const query = modelCallListQuery(url);
        const page = await this.dependencies.coordinator.listModelCalls(query.filter, query.limit, query.offset);
        writeJson(response, 200, {
          schema_version: "1",
          source_runtime: "pi",
          ...page,
          limit: query.limit,
          offset: query.offset,
        });
        return;
      }
      const callId = segments[1];
      if (!callId || !UUID.test(callId)) throw new RuntimeError("bad_request", 400, "model_call_id 格式无效");
      if (segments.length === 2 && method === "GET") {
        writeJson(response, 200, this.dependencies.coordinator.modelCall(callId));
        return;
      }
      if (segments[2] === "export" && segments.length === 3 && method === "GET") {
        writeJson(response, 200, this.dependencies.coordinator.exportModelCall(callId));
        return;
      }
      if (segments[2] === "replay" && segments.length === 3 && method === "POST") {
        const body = await readJson(request);
        rejectUnknownFields(body, ["mode"]);
        if (body.mode !== undefined && body.mode !== "dry_run") {
          throw new RuntimeError("bad_request", 400, "replay 仅支持 dry_run；真实对比必须创建 EvalRun");
        }
        writeJson(response, 200, this.dependencies.coordinator.dryRunReplay(callId));
        return;
      }
      throw new RuntimeError("not_found", 404, "路由不存在");
    }
    if (segments[0] === "contexts") {
      if (segments.length === 1 && method === "GET") {
        const query = contextListQuery(url);
        const page = await this.dependencies.coordinator.listContexts(query.filter, query.limit, query.offset);
        writeJson(response, 200, {
          schema_version: "2", source_runtime: "pi", ...page, limit: query.limit, offset: query.offset,
        });
        return;
      }
      const contextId = segments[1];
      if (!contextId || !UUID.test(contextId)) throw new RuntimeError("bad_request", 400, "context_id 格式无效");
      if (segments.length === 2 && method === "GET") {
        writeJson(response, 200, this.dependencies.coordinator.contextRecord(contextId));
        return;
      }
      if (segments[2] === "export" && segments.length === 3 && method === "GET") {
        writeJson(response, 200, this.dependencies.coordinator.exportContextRecord(contextId));
        return;
      }
      throw new RuntimeError("not_found", 404, "路由不存在");
    }
    if (segments[0] === "migration" && segments[1] === "plan" && segments.length === 2 && method === "GET") {
      writeJson(response, 200, await this.dependencies.coordinator.legacyMigrationPlan());
      return;
    }
    if (segments[0] !== "sessions") throw new RuntimeError("not_found", 404, "路由不存在");

    if (segments.length === 1 && method === "POST") {
      const body = await readJson(request);
      rejectUnknownFields(body, ["agent_key", "model_id", "tool_profile", "source"]);
      const agentKey = nonEmptyString(body, "agent_key");
      const modelId = nonEmptyString(body, "model_id");
      if (!UUID.test(modelId)) throw new RuntimeError("bad_request", 400, "model_id 格式无效");
      const profile = body.tool_profile;
      if (profile !== "chat" && profile !== "workspace") {
        throw new RuntimeError("bad_request", 400, "tool_profile 必须是 chat 或 workspace");
      }
      const source = optionalString(body, "source") ?? "local_api";
      const created = await this.dependencies.coordinator.createSession({
        agentKey,
        modelId,
        toolProfile: profile satisfies ToolProfile,
        source,
      });
      writeJson(response, 201, created);
      return;
    }
    if (segments.length === 1 && method === "GET") {
      writeJson(response, 200, { sessions: await this.dependencies.coordinator.listSessions() });
      return;
    }

    const id = sessionId(segments[1] ?? "");
    const command = segments[2];
    if (segments.length === 2 && method === "GET") {
      writeJson(response, 200, await this.dependencies.coordinator.sessionView(id));
      return;
    }
    if (segments.length === 2 && method === "DELETE") {
      await this.dependencies.coordinator.deleteSession(id);
      response.writeHead(204).end();
      return;
    }
    if (command === "entries" && method === "GET") {
      const after = Number(url.searchParams.get("after_sequence") ?? "0");
      const limit = Number(url.searchParams.get("limit") ?? "200");
      if (!Number.isSafeInteger(after) || after < 0 || !Number.isSafeInteger(limit) || limit < 1 || limit > 1_000) {
        throw new RuntimeError("bad_request", 400, "entries 游标或 limit 无效");
      }
      writeJson(response, 200, { entries: await this.dependencies.coordinator.sessionEntries(id, after, limit) });
      return;
    }
    if (command === "model-calls" && method === "GET") {
      const query = modelCallListQuery(url, id);
      const page = await this.dependencies.coordinator.listModelCalls(query.filter, query.limit, query.offset);
      writeJson(response, 200, {
        schema_version: "1",
        source_runtime: "pi",
        ...page,
        limit: query.limit,
        offset: query.offset,
      });
      return;
    }
    if (command === "contexts" && method === "GET") {
      const query = contextListQuery(url, id);
      const page = await this.dependencies.coordinator.listContexts(query.filter, query.limit, query.offset);
      writeJson(response, 200, {
        schema_version: "2", source_runtime: "pi", ...page, limit: query.limit, offset: query.offset,
      });
      return;
    }
    if (command === "prompt" && method === "POST") {
      await this.prompt(request, response, id);
      return;
    }
    if ((command === "steer" || command === "follow-up") && method === "POST") {
      const text = nonEmptyString(await readJson(request), "text");
      if (command === "steer") await this.dependencies.coordinator.steer(id, text);
      else await this.dependencies.coordinator.followUp(id, text);
      writeJson(response, 202, { status: "queued" });
      return;
    }
    if (command === "abort" && method === "POST") {
      await this.dependencies.coordinator.abort(id);
      writeJson(response, 202, { status: "aborted" });
      return;
    }
    if (command === "compact" && method === "POST") {
      const instructions = optionalString(await readJson(request), "instructions");
      writeJson(response, 200, await this.dependencies.coordinator.compact(id, instructions));
      return;
    }
    if (command === "tree" && method === "POST") {
      const body = await readJson(request);
      const entryId = nonEmptyString(body, "entry_id");
      const summarize = body.summarize;
      if (summarize !== undefined && typeof summarize !== "boolean") {
        throw new RuntimeError("bad_request", 400, "summarize 必须是 boolean");
      }
      const instructions = optionalString(body, "instructions");
      const label = optionalString(body, "label");
      writeJson(
        response,
        200,
        await this.dependencies.coordinator.navigateTree(id, entryId, {
          summarize: summarize ?? false,
          ...(instructions ? { instructions } : {}),
          ...(label ? { label } : {}),
        }),
      );
      return;
    }
    if (command === "fork" && method === "POST") {
      const body = await readJson(request);
      rejectUnknownFields(body, ["entry_id", "position", "upgrade"]);
      const entryId = optionalString(body, "entry_id");
      const position = body.position ?? "at";
      if (position !== "at" && position !== "before") {
        throw new RuntimeError("bad_request", 400, "position 必须是 at 或 before");
      }
      const rawUpgrade = objectField(body, "upgrade");
      let upgrade: UpgradeForkInput | undefined;
      if (rawUpgrade) {
        rejectUnknownFields(rawUpgrade, ["agent_key", "agent_version", "model_id", "tool_profile", "legacy_prompt_disposition"]);
        const modelId = nonEmptyString(rawUpgrade, "model_id");
        if (!UUID.test(modelId)) throw new RuntimeError("bad_request", 400, "upgrade.model_id 格式无效");
        const profile = rawUpgrade.tool_profile;
        if (profile !== "chat" && profile !== "workspace") {
          throw new RuntimeError("bad_request", 400, "upgrade.tool_profile 必须是 chat 或 workspace");
        }
        const disposition = rawUpgrade.legacy_prompt_disposition;
        if (disposition !== undefined && disposition !== "discard" && disposition !== "user_instruction") {
          throw new RuntimeError("bad_request", 400, "upgrade.legacy_prompt_disposition 必须是 discard 或 user_instruction");
        }
        upgrade = {
          agentKey: nonEmptyString(rawUpgrade, "agent_key"),
          agentVersion: nonEmptyString(rawUpgrade, "agent_version"),
          modelId,
          toolProfile: profile,
          ...(disposition ? { legacyPromptDisposition: disposition } : {}),
        };
      }
      writeJson(response, 201, await this.dependencies.coordinator.forkSession(id, entryId, position, upgrade));
      return;
    }
    throw new RuntimeError("not_found", 404, "路由不存在");
  }

  private async prompt(request: IncomingMessage, response: ServerResponse, id: string): Promise<void> {
    const text = nonEmptyString(await readJson(request), "text");
    const runId = randomUUID();
    let accepted = false;
    let terminal = false;

    const disconnect = (): void => {
      if (accepted && !terminal) void this.dependencies.coordinator.abort(id).catch(() => undefined);
    };
    response.once("close", disconnect);
    try {
      const assistant = await this.dependencies.coordinator.prompt(
        id,
        text,
        async (model) => {
          response.writeHead(200, {
            "content-type": "text/event-stream; charset=utf-8",
            "cache-control": "no-cache, no-transform",
            connection: "keep-alive",
          });
          accepted = true;
          await writeSse(response, "run_started", { run_id: runId, session_id: id, model });
        },
        async (event) => {
          const type =
            event !== null && typeof event === "object" && typeof (event as { type?: unknown }).type === "string"
              ? (event as { type: string }).type
              : "pi_event";
          await writeSse(response, type, event);
        },
      );
      terminal = true;
      await writeSse(response, "run_completed", {
        run_id: runId,
        status: assistant.stopReason === "aborted" ? "aborted" : "completed",
        assistant: redactUnknown(assistant),
      });
      response.end();
    } catch (error) {
      if (!accepted) throw error;
      terminal = true;
      if (!response.destroyed && !response.writableEnded) {
        await writeSse(response, "run_failed", { run_id: runId, ...publicError(error) });
        response.end();
      }
    } finally {
      response.off("close", disconnect);
    }
  }
}
