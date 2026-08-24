import { describe, expect, it } from "vitest";
import {
  acceptCommandSchema,
  reviewSessionSchema,
  textReviewClosureSchema,
  timelineReplacementHandoffSchema,
  turnPlanCommandSchema,
  unsupportedMediaEditInput,
} from "./contracts";

const hash = "a".repeat(64);
const version = {
  assetVersionId: "version-1",
  revision: 0,
  contentHash: hash,
  kind: "image" as const,
  projectId: "project-1",
  mimeType: "image/png",
};

const continuity = {
  status: "accepted_current" as const,
  snapshot: {
    id: "snapshot-1",
    revision: 1,
    contentHash: hash,
    targetId: "shot-1",
  },
  chain: [{ targetId: "shot-1", level: "shot", revision: 1 }],
  tasks: [],
};

const session = {
  id: "session-1",
  schemaVersion: "1.0.0",
  revision: 1,
  status: "active",
  projectId: "project-1",
  episodeId: "episode-1",
  targetId: "shot-1",
  selection: {
    projectId: "project-1",
    episodeId: "episode-1",
    targetId: "shot-1",
    primary: version,
    references: [],
  },
  continuity,
  conversation: {
    id: "session-1",
    schemaVersion: "1.0.0",
    projectId: "project-1",
    episodeId: "episode-1",
    revision: 3,
    messages: [
      {
        id: "message-1",
        sessionId: "session-1",
        sequence: 1,
        role: "user",
        contentHash: hash,
        status: "complete",
        correlationId: "corr-user",
      },
      {
        id: "message-2",
        sessionId: "session-1",
        sequence: 2,
        role: "agent",
        contentHash: "b".repeat(64),
        status: "complete",
        correlationId: "corr-agent",
      },
    ],
    turns: [
      {
        id: "turn-1",
        sessionId: "session-1",
        sequence: 1,
        userMessageId: "message-1",
        agentMessageId: "message-2",
        status: "complete",
        revision: 2,
      },
    ],
  },
  plans: [],
};

describe("asset edit review contracts", () => {
  it("accepts one owner-scoped session and rejects duplicate message sequence", () => {
    expect(reviewSessionSchema.parse(session).id).toBe("session-1");
    expect(
      reviewSessionSchema.safeParse({
        ...session,
        conversation: {
          ...session.conversation,
          messages: [
            session.conversation.messages[0],
            { ...session.conversation.messages[1], sequence: 1 },
          ],
        },
      }).success,
    ).toBe(false);
  });

  it("rejects foreign and duplicate exact refs before accept", () => {
    const command = {
      action: "accept",
      expectedRevision: 1,
      expectedBaseVersionId: "version-1",
      scope: ["shot-1"],
      candidateFacts: {
        candidateId: "candidate-1",
        projectId: "project-1",
        episodeId: "episode-1",
        targetId: "shot-1",
        assetVersionId: "version-2",
        assetVersionRevision: 0,
        assetVersionHash: hash,
        expectedTargetRevision: 2,
      },
      references: [
        { referenceId: "shot-1", expectedRevision: 2 },
        { referenceId: "shot-1", expectedRevision: 2 },
      ],
    };
    expect(acceptCommandSchema.safeParse(command).success).toBe(false);
  });

  it("freezes completed turn, selection, refs and operation identity", () => {
    const parsed = turnPlanCommandSchema.parse({
      schemaVersion: "1.0.0",
      sessionId: "session-1",
      conversationId: "session-1",
      turnId: "turn-1",
      turnStatus: "complete",
      episodeId: "episode-1",
      targetId: "shot-1",
      kind: "image",
      base: version,
      references: [],
      instruction: "增强人物轮廓",
      runId: "run-1",
      nodeRunId: "node-1",
      logicalOperation: "review:turn-1:plan-1",
      correlationId: "corr-plan",
    });
    expect(parsed.turnStatus).toBe("complete");
    expect(
      turnPlanCommandSchema.safeParse({ ...parsed, turnStatus: "pending" })
        .success,
    ).toBe(false);
    expect(
      turnPlanCommandSchema.safeParse({ ...parsed, kind: "audio" }).success,
    ).toBe(false);
  });

  it("rejects unsupported partial media inputs instead of dropping fields", () => {
    expect(unsupportedMediaEditInput(new URLSearchParams("mask=x"))).toBe(
      "unsupported_feature",
    );
    expect(
      unsupportedMediaEditInput(new URLSearchParams("startTime=1&endTime=2")),
    ).toBe("unsupported_feature");
    expect(
      unsupportedMediaEditInput(new URLSearchParams("shotId=shot-1")),
    ).toBe(null);
  });

  it("requires accepted-current and ready derivative for timeline handoff", () => {
    const handoff = {
      schemaVersion: "1.0.0",
      projectId: "project-1",
      episodeId: "episode-1",
      shotId: "shot-1",
      candidateId: "candidate-1",
      takeId: "take-1",
      assetVersionId: "version-2",
      assetVersionRevision: 0,
      assetVersionHash: hash,
      derivativeFingerprint: "b".repeat(64),
      acceptedCurrent: true,
      derivativeStatus: "ready",
    };
    expect(timelineReplacementHandoffSchema.parse(handoff).shotId).toBe(
      "shot-1",
    );
    expect(
      timelineReplacementHandoffSchema.safeParse({
        ...handoff,
        derivativeStatus: "pending",
      }).success,
    ).toBe(false);
  });

  it("keeps text successor closure immutable and blocks partial batches", () => {
    expect(
      textReviewClosureSchema.parse({
        schemaVersion: "1.0.0",
        batchId: "batch-1",
        revision: 2,
        status: "stale",
        successorBatchId: "batch-2",
        staleCandidateIds: ["candidate-1"],
        diagnostic: "upstream_successor",
        immutable: true,
      }).status,
    ).toBe("stale");
    expect(
      textReviewClosureSchema.safeParse({
        schemaVersion: "1.0.0",
        batchId: "batch-1",
        revision: 2,
        status: "partial",
        staleCandidateIds: [],
        diagnostic: null,
        immutable: false,
      }).success,
    ).toBe(false);
  });
});
