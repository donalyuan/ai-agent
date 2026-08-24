import { z } from "zod";

const hashSchema = z.string().regex(/^[0-9a-f]{64}$/);
const schemaVersion = z.literal("1.0.0");

export const assetVersionRefSchema = z.object({
  assetVersionId: z.string().min(1),
  revision: z.number().int().nonnegative(),
  contentHash: hashSchema,
  kind: z.enum(["image", "video"]),
  projectId: z.string().min(1),
  mimeType: z.string().regex(/^[^/]+\/[^/]+$/),
});

export type AssetVersionRef = z.infer<typeof assetVersionRefSchema>;

export const assetVersionOwnerSchema = z.object({
  id: z.string().min(1),
  schemaVersion,
  revision: z.number().int().nonnegative(),
  projectId: z.string().min(1),
  contentHash: hashSchema,
  mimeType: z.string().regex(/^[^/]+\/[^/]+$/),
});

export const continuityTaskSchema = z.object({
  id: z.string().min(1),
  targetId: z.string().min(1),
  status: z.enum(["pending", "acknowledged", "resolved", "superseded"]),
  revision: z.number().int().positive(),
});

export const continuityProjectionSchema = z.object({
  status: z.enum(["accepted_current", "continuity_stale", "not_bound"]),
  snapshot: z
    .object({
      id: z.string().min(1),
      revision: z.number().int().positive(),
      contentHash: hashSchema,
      targetId: z.string().min(1),
    })
    .nullable(),
  chain: z.array(
    z.object({
      targetId: z.string().min(1),
      level: z.enum(["project", "episode", "scene", "shot"]),
      revision: z.number().int().positive(),
    }),
  ),
  tasks: z.array(continuityTaskSchema),
});

export const conversationMessageSchema = z.object({
  id: z.string().min(1),
  sessionId: z.string().min(1),
  sequence: z.number().int().positive(),
  role: z.enum(["user", "agent"]),
  contentHash: hashSchema,
  status: z.enum(["complete", "pending", "failed"]),
  correlationId: z.string().min(1),
});

export const conversationTurnSchema = z.object({
  id: z.string().min(1),
  sessionId: z.string().min(1),
  sequence: z.number().int().positive(),
  userMessageId: z.string().min(1),
  agentMessageId: z.string().min(1).nullable(),
  status: z.enum(["pending", "complete", "failed", "cancelled"]),
  revision: z.number().int().positive(),
});

export const conversationSchema = z
  .object({
    id: z.string(),
    schemaVersion,
    projectId: z.string().optional(),
    episodeId: z.string().optional(),
    revision: z.number().int().nonnegative(),
    messages: z.array(conversationMessageSchema),
    turns: z.array(conversationTurnSchema),
  })
  .superRefine((value, context) => {
    const messageSequences = value.messages.map((item) => item.sequence);
    const turnSequences = value.turns.map((item) => item.sequence);
    if (new Set(messageSequences).size !== messageSequences.length) {
      context.addIssue({
        code: "custom",
        message: "duplicate message sequence",
      });
    }
    if (new Set(turnSequences).size !== turnSequences.length) {
      context.addIssue({ code: "custom", message: "duplicate turn sequence" });
    }
    if (
      value.id &&
      [...value.messages, ...value.turns].some(
        (item) => item.sessionId !== value.id,
      )
    ) {
      context.addIssue({
        code: "custom",
        message: "foreign conversation item",
      });
    }
  });

export const candidateSchema = z.object({
  id: z.string().min(1),
  schemaVersion,
  revision: z.number().int().positive(),
  status: z.enum([
    "generated",
    "pending_review",
    "accepted",
    "rejected",
    "stale",
    "superseded",
  ]),
  projectId: z.string().min(1),
  episodeId: z.string().min(1),
  targetId: z.string().min(1),
  assetVersion: assetVersionRefSchema,
  provenance: z.record(z.string(), z.unknown()),
});

export const planSchema = z.object({
  id: z.string().min(1),
  schemaVersion,
  revision: z.number().int().positive(),
  projectId: z.string().min(1),
  episodeId: z.string().min(1),
  targetId: z.string().min(1),
  turnId: z.string().min(1),
  status: z.enum(["pending_review", "stale", "executing"]),
  instruction: z.string().min(1),
  base: assetVersionRefSchema,
  references: z.array(assetVersionRefSchema),
  cost: z.object({
    status: z.enum(["known", "estimated", "unknown"]),
    source: z.string().min(1),
    currency: z.string().nullable(),
    estimated: z.number().nonnegative().nullable(),
  }),
  impact: z.object({
    id: z.string().nullable(),
    status: z.enum(["clear", "stale", "continuity_stale"]),
    reasons: z.array(z.string()),
    staleTargets: z.array(z.string()),
  }),
  continuity: continuityProjectionSchema,
  candidates: z.array(candidateSchema),
});

export const reviewSessionSchema = z
  .object({
    id: z.string().min(1),
    schemaVersion,
    revision: z.number().int().positive(),
    status: z.enum(["active", "closed"]),
    projectId: z.string().min(1),
    episodeId: z.string().min(1),
    targetId: z.string().min(1),
    selection: z.object({
      projectId: z.string().min(1),
      episodeId: z.string().min(1),
      targetId: z.string().min(1),
      primary: assetVersionRefSchema,
      references: z.array(assetVersionRefSchema),
    }),
    continuity: continuityProjectionSchema,
    conversation: conversationSchema,
    plans: z.array(planSchema),
  })
  .superRefine((value, context) => {
    if (
      value.projectId !== value.selection.projectId ||
      value.episodeId !== value.selection.episodeId ||
      value.targetId !== value.selection.targetId ||
      value.selection.primary.projectId !== value.projectId ||
      value.selection.references.some(
        (item) => item.projectId !== value.projectId,
      )
    ) {
      context.addIssue({
        code: "custom",
        message: "foreign session selection",
      });
    }
    const ids = value.selection.references.map((item) => item.assetVersionId);
    if (
      new Set(ids).size !== ids.length ||
      ids.includes(value.selection.primary.assetVersionId)
    ) {
      context.addIssue({
        code: "custom",
        message: "duplicate session reference",
      });
    }
    if (
      value.conversation.id &&
      (value.conversation.projectId !== value.projectId ||
        value.conversation.episodeId !== value.episodeId)
    ) {
      context.addIssue({
        code: "custom",
        message: "foreign conversation scope",
      });
    }
  });

export type ReviewSession = z.infer<typeof reviewSessionSchema>;
export type ReviewPlan = z.infer<typeof planSchema>;
export type ReviewCandidate = z.infer<typeof candidateSchema>;

export const sessionIndexSchema = z.object({
  schemaVersion,
  items: z.array(
    z.object({
      id: z.string().min(1),
      revision: z.number().int().positive(),
      projectId: z.string().min(1),
      episodeId: z.string().min(1),
      targetId: z.string().min(1),
      status: z.enum(["active", "closed"]),
    }),
  ),
});

export const turnPlanCommandSchema = z.object({
  schemaVersion,
  sessionId: z.string().min(1),
  conversationId: z.string().min(1),
  turnId: z.string().min(1),
  turnStatus: z.literal("complete"),
  episodeId: z.string().min(1),
  targetId: z.string().min(1),
  kind: z.enum(["image", "video"]),
  base: assetVersionRefSchema,
  references: z.array(assetVersionRefSchema),
  instruction: z.string().trim().min(1),
  runId: z.string().min(1),
  nodeRunId: z.string().min(1),
  logicalOperation: z.string().min(1),
  correlationId: z.string().min(1),
});

export const acceptCommandSchema = z
  .object({
    action: z.literal("accept"),
    expectedRevision: z.number().int().positive(),
    expectedBaseVersionId: z.string().min(1),
    scope: z.array(z.string().min(1)).min(1),
    candidateFacts: z.object({
      candidateId: z.string().min(1),
      projectId: z.string().min(1),
      episodeId: z.string().min(1),
      targetId: z.string().min(1),
      assetVersionId: z.string().min(1),
      assetVersionRevision: z.number().int().nonnegative(),
      assetVersionHash: hashSchema,
      expectedTargetRevision: z.number().int().positive(),
    }),
    references: z
      .array(
        z.object({
          referenceId: z.string().min(1),
          expectedRevision: z.number().int().positive(),
        }),
      )
      .min(1),
  })
  .superRefine((value, context) => {
    const ids = value.references.map((item) => item.referenceId);
    if (new Set(ids).size !== ids.length) {
      context.addIssue({
        code: "custom",
        message: "duplicate accept reference",
      });
    }
  });

export const timelineReplacementHandoffSchema = z
  .object({
    schemaVersion,
    projectId: z.string().min(1),
    episodeId: z.string().min(1),
    shotId: z.string().min(1),
    candidateId: z.string().min(1),
    takeId: z.string().min(1),
    assetVersionId: z.string().min(1),
    assetVersionRevision: z.number().int().nonnegative(),
    assetVersionHash: hashSchema,
    derivativeFingerprint: hashSchema,
    acceptedCurrent: z.literal(true),
    derivativeStatus: z.literal("ready"),
  })
  .strict();

export const textReviewClosureSchema = z
  .object({
    schemaVersion,
    batchId: z.string().min(1),
    revision: z.number().int().positive(),
    status: z.enum([
      "pending_review",
      "accepted",
      "rejected",
      "retake",
      "stale",
    ]),
    successorBatchId: z.string().min(1).optional(),
    staleCandidateIds: z.array(z.string().min(1)),
    diagnostic: z.string().nullable(),
    immutable: z.literal(true),
  })
  .strict();

const unsupportedKeys = new Set([
  "mask",
  "selection",
  "region",
  "layer",
  "start",
  "end",
  "startTime",
  "endTime",
  "timeRange",
  "segment",
  "keyframes",
]);

export function unsupportedMediaEditInput(params: URLSearchParams) {
  return [...params.keys()].some((key) => unsupportedKeys.has(key))
    ? "unsupported_feature"
    : null;
}
