import { describe, expect, it } from "vitest";
import type { AgentMessage } from "@earendil-works/pi-agent-core";

import { ContextCompileError } from "../src/context.js";
import { mapAgentMessages, selectAgentMessages } from "../src/pi-context.js";

const compiledAt = "2026-07-25T00:00:00.000Z";

function user(content: AgentMessage extends infer _ ? string : never, timestamp = 1): AgentMessage {
  return { role: "user", content, timestamp };
}

function assistant(content: any[], timestamp = 2): AgentMessage {
  return {
    role: "assistant", content, api: "openai-responses", provider: "fixture", model: "fixture",
    usage: { input: 1, output: 1, cacheRead: 0, cacheWrite: 0, totalTokens: 2,
      cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } },
    stopReason: "stop", timestamp,
  } as AgentMessage;
}

function toolResult(id: string, timestamp = 3): AgentMessage {
  return { role: "toolResult", toolCallId: id, toolName: "read", content: [{ type: "text", text: "ok" }],
    details: { path: "a.txt" }, isError: false, timestamp } as AgentMessage;
}

describe("Pi AgentMessage governed context mapping", () => {
  it("preserves message content, thinking, image assets and stable identity", () => {
    const messages: AgentMessage[] = [
      user("current request"),
      assistant([
        { type: "thinking", thinking: "private reasoning" },
        { type: "text", text: "answer" },
        { type: "image", data: "image-bytes", mimeType: "image/png" },
      ]),
    ];
    const first = mapAgentMessages(messages, {
      sessionId: "session-1", phase: "turn", phaseInput: "current request", compiledAt, pendingInputs: [],
    });
    const second = mapAgentMessages(structuredClone(messages), {
      sessionId: "session-1", phase: "turn", phaseInput: "current request", compiledAt, pendingInputs: [],
    });

    expect(first.candidates.map((item) => item.candidate_id)).toEqual(second.candidates.map((item) => item.candidate_id));
    expect(first.candidates[0]).toMatchObject({ source_kind: "user_instruction", trust: "user_instruction", priority: "p0", required: true });
    expect(first.candidates[1]?.payload).toMatchObject({
      type: "message", message: { role: "assistant", thinking: "private reasoning" },
    });
    expect(selectAgentMessages(first, first.candidates.map((item) => item.candidate_id))).toEqual(messages);
  });

  it("keeps steer and follow-up as P0 sources without changing definitions or gates", () => {
    const mapped = mapAgentMessages([user("change"), user("continue", 2)], {
      sessionId: "session-1", phase: "turn", phaseInput: "start", compiledAt,
      pendingInputs: [{ kind: "steer", text: "change" }, { kind: "follow_up", text: "continue" }],
    });
    expect(mapped.candidates).toEqual(expect.arrayContaining([
      expect.objectContaining({ source_kind: "pi_steer", trust: "steer", priority: "p0", required: true }),
      expect.objectContaining({ source_kind: "pi_follow_up", trust: "follow_up", priority: "p0", required: true }),
    ]));
  });

  it("forms one atomic group for assistant tool calls and matching results", () => {
    const request = assistant([{ type: "toolCall", id: "call-1", name: "read", arguments: { path: "a.txt" } }]);
    const mapped = mapAgentMessages([user("read"), request, toolResult("call-1")], {
      sessionId: "session-1", phase: "tool_loop", phaseInput: "read", compiledAt, pendingInputs: [],
    });
    expect(mapped.atomicGroups).toHaveLength(1);
    expect(mapped.atomicGroups[0]?.member_ids).toHaveLength(2);
    expect(mapped.candidates.filter((item) => item.atomic_group_id)).toHaveLength(2);
  });

  it.each([
    ["orphan result", [toolResult("missing")]],
    ["missing result", [assistant([{ type: "toolCall", id: "call-1", name: "read", arguments: {} }])]],
    ["duplicate call", [
      assistant([{ type: "toolCall", id: "call-1", name: "read", arguments: {} }]),
      assistant([{ type: "toolCall", id: "call-1", name: "read", arguments: {} }], 4),
      toolResult("call-1"),
    ]],
  ])("fails closed for %s", (_name, messages) => {
    expect(() => mapAgentMessages(messages as AgentMessage[], {
      sessionId: "session-1", phase: "tool_loop", phaseInput: "", compiledAt, pendingInputs: [],
    })).toThrow(expect.objectContaining<Partial<ContextCompileError>>({ code: "context_atomic_group_invalid" }));
  });
});
