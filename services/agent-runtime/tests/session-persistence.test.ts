import { mkdtemp } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import type { AssistantMessage, UserMessage } from "@earendil-works/pi-ai";
import { describe, expect, it } from "vitest";

import { cleanupSession, SessionStore } from "../src/sessions.js";

const MODEL_ID = "11111111-1111-4111-8111-111111111111";

function user(text: string): UserMessage {
  return { role: "user", content: [{ type: "text", text }], timestamp: Date.now() };
}

function assistant(text: string): AssistantMessage {
  return {
    role: "assistant",
    content: [{ type: "text", text }],
    api: "faux",
    provider: "faux",
    model: "faux-1",
    stopReason: "stop",
    timestamp: Date.now(),
    usage: {
      input: 10,
      output: 5,
      cacheRead: 0,
      cacheWrite: 0,
      totalTokens: 15,
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
    },
  };
}

describe("Pi SQLite session persistence", () => {
  it("restores entries, cursor, active leaf, fork and compaction after restart", async () => {
    const root = await mkdtemp(join(tmpdir(), "novex-session-"));
    const database = join(root, "sessions.sqlite");
    let store = new SessionStore(database, root);
    const session = await store.create({ modelId: MODEL_ID, toolProfile: "chat", source: "test" });
    const sessionId = (await session.getMetadata()).id;
    const userId = await session.appendMessage(user("hello"));
    const assistantId = await session.appendMessage(assistant("world"));
    const compactId = await session.appendCompaction("summary", assistantId, 15, { memory: false });
    expect(await session.getLeafId()).toBe(compactId);
    await cleanupSession(session);
    await store.close();

    store = new SessionStore(database, root);
    const restored = await store.open(sessionId);
    expect(await restored.getLeafId()).toBe(compactId);
    await cleanupSession(restored);

    const all = await store.entries(sessionId, 0, 20);
    expect(all.map(({ sequence }) => sequence)).toEqual([1, 2, 3]);
    expect((await store.entries(sessionId, 1, 20)).map(({ sequence }) => sequence)).toEqual([2, 3]);
    expect(all[2]?.entry).toMatchObject({ type: "compaction", summary: "summary", details: { memory: false } });
    expect(all[0]?.entry).toMatchObject({ id: userId, type: "message" });

    const fork = await store.fork(sessionId, userId, "at");
    const forkId = (await fork.getMetadata()).id;
    await cleanupSession(fork);
    expect((await store.view(forkId)).parent_session_id).toBe(sessionId);
    expect(await store.entries(forkId, 0, 20)).toHaveLength(1);

    await store.move(sessionId, assistantId);
    expect((await store.view(sessionId)).active_leaf_id).toBe(assistantId);
    const movedEntries = await store.entries(sessionId, 0, 20);
    expect(movedEntries).toHaveLength(4);
    expect(movedEntries[3]?.entry).toMatchObject({ type: "leaf", targetId: assistantId });
    await store.close();
  });
});
