import { createHash } from "node:crypto";

import { convertToLlm, type AgentMessage } from "@earendil-works/pi-agent-core";
import { contentText } from "@earendil-works/pi-ai";

import { canonicalJson, type TrustLevel } from "./definitions.js";
import {
  ContextCompileError,
  type ContextAtomicGroup,
  type ContextCandidate,
  type ContextPayload,
  type ContextPriority,
} from "./context.js";

export type PiContextPhase = "turn" | "tool_loop" | "compaction" | "branch_summary";
export interface PendingContextInput { kind: "steer" | "follow_up"; text: string }
export interface AgentMessageMappingOptions {
  sessionId: string;
  phase: PiContextPhase;
  phaseInput: string;
  compiledAt: string;
  pendingInputs: readonly PendingContextInput[];
  summaryProviderInput?: boolean;
}

export interface AgentMessageContextMapping {
  candidates: ContextCandidate[];
  atomicGroups: ContextAtomicGroup[];
  originals: ReadonlyMap<string, AgentMessage>;
  remainingPendingInputs: readonly PendingContextInput[];
}

interface CandidateClassification {
  sourceKind: string;
  trust: TrustLevel;
  priority: ContextPriority;
  required: boolean;
}

/** Converts only public Pi AgentMessage values and retains an exact in-memory reverse mapping. */
export function mapAgentMessages(
  messages: readonly AgentMessage[],
  options: AgentMessageMappingOptions,
): AgentMessageContextMapping {
  const pending = [...options.pendingInputs];
  const toolOwners = new Map<string, { candidateId: string; groupId: string }>();
  const toolResults = new Map<string, string>();
  const originals = new Map<string, AgentMessage>();
  const candidates: ContextCandidate[] = [];
  messages.forEach((message, index) => {
    const logicalMessage = convertToLlm([message])[0];
    // Pi deliberately excludes some display-only messages from provider context.
    if (logicalMessage === undefined) return;
    const classification = classifyMessage(message, options, pending);
    const payload = logicalPayload(message, logicalMessage);
    const contentHash = sha256(canonicalJson(payload));
    const timestamp = messageTimestamp(message, options.compiledAt);
    const candidateId = `pi:${classification.sourceKind}:${index}:${contentHash.slice(0, 20)}`;
    const toolCallIds = assistantToolCallIds(message);
    let atomicGroupId: string | undefined;
    if (toolCallIds.length > 0) {
      atomicGroupId = `pi-tool:${sha256(canonicalJson(toolCallIds)).slice(0, 24)}`;
      for (const callId of toolCallIds) {
        if (toolOwners.has(callId)) atomicFailure();
        toolOwners.set(callId, { candidateId, groupId: atomicGroupId });
      }
    } else if (message.role === "toolResult") {
      if (toolResults.has(message.toolCallId)) atomicFailure();
      toolResults.set(message.toolCallId, candidateId);
    }
    originals.set(candidateId, structuredClone(message));
    candidates.push({
      candidate_id: candidateId,
      source_kind: classification.sourceKind,
      source_id: `${options.sessionId}:${message.role}:${String(message.timestamp)}:${index}`,
      source_version: contentHash,
      trust: classification.trust,
      priority: classification.priority,
      required: classification.required,
      render_order: index,
      observed_at: timestamp,
      supersedes: [],
      content_hash: contentHash,
      ...(atomicGroupId ? { atomic_group_id: atomicGroupId } : {}),
      payload,
    });
  });

  const groups = new Map<string, string[]>();
  for (const [callId, owner] of toolOwners) {
    const resultId = toolResults.get(callId);
    if (!resultId) atomicFailure();
    const members = groups.get(owner.groupId) ?? [owner.candidateId];
    members.push(resultId);
    groups.set(owner.groupId, members);
    const result = candidates.find((candidate) => candidate.candidate_id === resultId)!;
    result.atomic_group_id = owner.groupId;
  }
  for (const callId of toolResults.keys()) if (!toolOwners.has(callId)) atomicFailure();

  return {
    candidates,
    atomicGroups: [...groups.entries()].map(([group_id, member_ids]) => ({
      group_id,
      member_ids: [...new Set(member_ids)],
    })),
    originals,
    remainingPendingInputs: pending,
  };
}

/** Rehydrates the selected native messages in compiler render order. */
export function selectAgentMessages(
  mapping: AgentMessageContextMapping,
  selectedOrder: readonly string[],
): AgentMessage[] {
  return selectedOrder.map((candidateId) => {
    const message = mapping.originals.get(candidateId);
    if (!message) throw new ContextCompileError("finalize", "context_finalize_mismatch");
    return structuredClone(message);
  });
}

function classifyMessage(
  message: AgentMessage,
  options: AgentMessageMappingOptions,
  pending: PendingContextInput[],
): CandidateClassification {
  if (options.summaryProviderInput) {
    return { sourceKind: "user_instruction", trust: "user_instruction", priority: "p0", required: true };
  }
  if (message.role === "toolResult" || assistantToolCallIds(message).length > 0) {
    return { sourceKind: "pi_tool_exchange", trust: "reference", priority: "p0", required: true };
  }
  if (message.role === "user") {
    const text = contentText(message.content);
    const pendingIndex = pending.findIndex((item) => item.text === text);
    if (pendingIndex >= 0) {
      const [input] = pending.splice(pendingIndex, 1);
      return input!.kind === "steer"
        ? { sourceKind: "pi_steer", trust: "steer", priority: "p0", required: true }
        : { sourceKind: "pi_follow_up", trust: "follow_up", priority: "p0", required: true };
    }
    if (text === options.phaseInput) {
      return { sourceKind: "user_instruction", trust: "user_instruction", priority: "p0", required: true };
    }
  }
  if (options.phase === "compaction") {
    return { sourceKind: "pi_compaction", trust: "reference", priority: "p3", required: false };
  }
  return { sourceKind: "pi_branch_entry", trust: "reference", priority: "p2", required: false };
}

function logicalPayload(
  message: AgentMessage,
  logicalMessage: ReturnType<typeof convertToLlm>[number],
): ContextPayload {
  const thinking = message.role === "assistant"
    ? message.content
      .filter((item) => item.type === "thinking")
      .map((item) => item.thinking)
      .join("\n")
    : "";
  return {
    type: "message",
    message: {
      role: logicalMessage.role,
      content: structuredClone(logicalMessage.content),
      ...(thinking ? { thinking } : {}),
      ...(message.role === "toolResult" ? { tool_call_id: message.toolCallId } : {}),
    },
  };
}

function assistantToolCallIds(message: AgentMessage): string[] {
  if (message.role !== "assistant") return [];
  return message.content
    .filter((item) => item.type === "toolCall")
    .map((item) => item.id);
}

function messageTimestamp(message: AgentMessage, fallback: string): string {
  const timestamp = new Date(message.timestamp).toISOString();
  return Number.isNaN(Date.parse(timestamp)) ? fallback : timestamp;
}

function atomicFailure(): never {
  throw new ContextCompileError("schema", "context_atomic_group_invalid");
}

function sha256(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}
