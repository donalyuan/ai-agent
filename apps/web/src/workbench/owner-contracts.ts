import { z } from "zod";
import {
  frozenSelectionSchema,
  sourceBindingSchema,
  workflowSourceSnapshotSchema,
} from "./contracts";

const schemaVersion = z.literal("1.0.0");
const ownerId = z.string().trim().min(1);
const revision = z.number().int().positive();
const hash = z.string().regex(/^[0-9a-f]{64}$/i);
const userUuid = z.string().uuid();

export const ownerReferenceSchema = z
  .object({
    ownerId,
    projectId: ownerId.optional(),
    revision,
    contentHash: hash.optional(),
    type: ownerId.optional(),
  })
  .passthrough();

export const runStartCommandSchema = z
  .object({
    workflowVersionId: ownerId,
    nodeKeys: z.array(ownerId).min(1),
    scopeRefs: z.array(ownerReferenceSchema),
    ownerRefs: z.array(ownerReferenceSchema),
    selectionSnapshot: frozenSelectionSchema.optional(),
    idempotencyKey: ownerId,
    routeDecisionId: ownerId,
    expectedBindingRevision: revision,
    schemaVersion,
  })
  .strict();

export const successorRunCommandSchema = z
  .object({
    expectedPredecessorRevision: revision,
    reuseNodeIds: z.array(ownerId),
    selectionSnapshot: frozenSelectionSchema.optional(),
    schemaVersion,
  })
  .strict();

export const textRegenerateCommandSchema = z
  .object({
    candidateId: ownerId,
    expectedBatchRevision: revision,
    expectedCandidateRevision: revision,
    payload: z.record(z.string(), z.unknown()),
    sourceCandidateIds: z.array(ownerId),
    sourceHashes: z.array(hash),
  })
  .strict()
  .superRefine((value, context) => {
    if (value.sourceCandidateIds.length !== value.sourceHashes.length) {
      context.addIssue({
        code: "custom",
        message: "candidate source closure is incomplete",
      });
    }
  });

export const operationRecoverySchema = z
  .object({
    runId: ownerId,
    nodeRunId: ownerId,
    logicalOperation: ownerId,
    submissionState: z.enum([
      "pending",
      "started",
      "submission_unknown",
      "reconciled",
    ]),
    correlationId: ownerId,
    revision,
    schemaVersion,
  })
  .strict();

export const runCancelCommandSchema = z
  .object({ expectedRevision: revision, schemaVersion })
  .strict();

export const runInputSnapshotSchema = z
  .object({
    id: ownerId,
    schemaVersion,
    revision,
    projectId: ownerId,
    runId: ownerId,
    workflowVersionId: ownerId,
    workflowContentHash: hash,
    scopeRefs: z.array(ownerReferenceSchema),
    ownerRefs: z.array(ownerReferenceSchema),
    selectionSnapshot: frozenSelectionSchema,
    sourceSnapshot: workflowSourceSnapshotSchema,
    nodeInputs: z.array(
      z
        .object({
          nodeRunId: ownerId,
          nodeKey: ownerId,
          logicalOperation: ownerId,
          scopeRefs: z.array(ownerReferenceSchema),
        })
        .strict(),
    ),
    runnable: z.boolean(),
    diagnostic: z.string().nullable(),
    createdAt: z.string(),
  })
  .strict();

export const historicalRerunCommandSchema = z
  .object({ expectedSnapshotRevision: revision, schemaVersion })
  .strict();

export const budgetGateSchema = z
  .object({
    id: ownerId,
    runId: ownerId,
    nodeRunId: ownerId,
    logicalOperation: ownerId,
    requestFingerprint: hash,
    operationKind: z.enum(["text", "image", "video", "audio"]),
    batchSize: z.number().int().positive(),
    costStatus: z.enum(["estimated", "actual", "unknown"]),
    estimatedCost: z.string().nullable(),
    actualCost: z.string().nullable().optional(),
    currency: z.string().trim().min(1).nullable(),
    source: z.string().trim().min(1),
    thresholdSnapshotId: ownerId.nullable(),
    thresholdRevision: revision.nullable(),
    confirmationId: ownerId.nullable(),
    status: z.enum(["waiting_review", "confirmed", "rejected"]),
    revision,
    schemaVersion,
  })
  .strict();

export const budgetConfirmationCommandSchema = z
  .object({
    logicalOperation: ownerId,
    requestFingerprint: hash,
    confirmationId: ownerId,
    userUuid,
    expectedGateRevision: revision,
    schemaVersion,
  })
  .strict();

export const sourceMaterialCommandSchema = z
  .object({
    materialType: z.enum(["novel", "synopsis", "existing_script"]),
    inputMode: z.enum(["inline_text", "uploaded_file"]),
  })
  .strict();

export const sourceMaterialVersionSchema = z
  .object({
    id: ownerId,
    sourceMaterialId: ownerId,
    projectId: ownerId,
    materialType: z.enum(["novel", "synopsis", "existing_script"]),
    inputMode: z.enum(["inline_text", "uploaded_file"]),
    revision,
    contentHash: hash,
    parseStatus: z.enum(["pending", "parsed", "failed"]),
    validationStatus: z.enum(["pending", "valid", "invalid"]),
    assetVersionId: ownerId.nullable(),
    schemaVersion,
  })
  .strict()
  .superRefine((value, context) => {
    if (value.inputMode === "inline_text" && value.assetVersionId !== null) {
      context.addIssue({
        code: "custom",
        message: "inline source cannot reference AssetVersion",
      });
    }
    if (value.inputMode === "uploaded_file" && value.assetVersionId === null) {
      context.addIssue({
        code: "custom",
        message: "uploaded source requires AssetVersion",
      });
    }
  });

export const textRunSourceBindingSchema = sourceBindingSchema
  .extend({ runId: ownerId, runRevision: revision })
  .strict();

export const shotCardOwnerGroupSchema = z
  .object({
    ownerId,
    revision,
    contentHash: hash.optional(),
    status: z.enum([
      "ready",
      "pending",
      "partial",
      "unavailable",
      "stale",
      "failed",
    ]),
  })
  .passthrough();

export const shotCardSchema = z
  .object({
    projectId: ownerId,
    episodeId: ownerId,
    sceneId: ownerId,
    shotId: ownerId,
    scene: shotCardOwnerGroupSchema,
    shotSpec: shotCardOwnerGroupSchema,
    continuity: shotCardOwnerGroupSchema,
    image: shotCardOwnerGroupSchema.nullable(),
    video: shotCardOwnerGroupSchema.nullable(),
    generation: shotCardOwnerGroupSchema.nullable(),
    review: shotCardOwnerGroupSchema.nullable(),
    derivative: shotCardOwnerGroupSchema.nullable(),
    durationMs: z.number().int().positive().nullable(),
    promptSummary: z.string().max(256).nullable(),
    modelId: ownerId.nullable(),
    modelRevision: revision.nullable(),
    cost: z
      .object({
        value: z.string().nullable(),
        currency: z.string().nullable(),
        status: z.enum(["estimated", "actual", "unknown"]),
        source: z.string(),
      })
      .strict()
      .nullable(),
  })
  .strict()
  .superRefine((value, context) => {
    for (const group of [value.scene, value.shotSpec, value.continuity]) {
      if (group.ownerId === "" || group.revision < 1) {
        context.addIssue({
          code: "custom",
          message: "ShotCard owner group is incomplete",
        });
      }
    }
  });

export const assetBibleEntryTypeSchema = z.enum([
  "character",
  "look",
  "location",
  "scene_visual",
  "prop",
  "visual_style",
]);

export const assetBibleVersionSchema = z
  .object({
    id: ownerId,
    entryId: ownerId,
    projectId: ownerId,
    entryType: assetBibleEntryTypeSchema,
    revision,
    contentHash: hash,
    status: z.enum(["candidate", "accepted", "disabled"]),
    schemaVersion,
  })
  .strict();

export const assetBibleAssignmentSchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    level: z.enum(["project", "episode", "scene", "shot"]),
    targetId: ownerId,
    entryId: ownerId,
    versionId: ownerId,
    versionRevision: revision,
    contentHash: hash,
    revision,
    schemaVersion,
  })
  .strict();

export const resolvedContinuitySchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    targetId: ownerId,
    revision,
    contentHash: hash,
    status: z.enum(["accepted", "incomplete", "stale"]),
    chain: z.array(assetBibleAssignmentSchema),
    schemaVersion,
  })
  .strict();

const impactTargetSchema = z
  .object({
    targetType: z.enum(["episode", "scene", "shot"]),
    targetId: ownerId,
    targetRevision: revision,
    reason: z.string().trim().min(1),
    snapshotId: ownerId,
    snapshotHash: hash,
    suggestedAction: ownerId,
  })
  .strict();

export const assetBibleImpactSchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    entryId: ownerId,
    entryRevision: revision,
    assetBibleRevision: revision,
    targets: z.array(impactTargetSchema),
    targetSetHash: hash,
    complete: z.boolean(),
    diagnostic: z.string().nullable(),
    revision,
    schemaVersion,
  })
  .strict();

export const assetBibleAcceptCommandSchema = z
  .object({
    analysisId: ownerId,
    expectedAnalysisRevision: revision,
    expectedEntryRevision: revision,
    expectedAssetBibleRevision: revision,
    targetSetHash: hash,
    targets: z.array(impactTargetSchema),
    actorUuid: userUuid,
    schemaVersion,
  })
  .strict();

export const continuityTaskSchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    targetId: ownerId,
    targetType: z.enum(["episode", "scene", "shot"]),
    status: z.enum(["pending", "acknowledged", "resolved", "cancelled"]),
    sourceSnapshotId: ownerId,
    sourceSnapshotHash: hash,
    revision,
    schemaVersion,
  })
  .strict();

export type RunInputSnapshot = z.infer<typeof runInputSnapshotSchema>;
export type ShotCardProjection = z.infer<typeof shotCardSchema>;
