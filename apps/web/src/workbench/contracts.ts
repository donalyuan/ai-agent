import { z } from "zod";

const schemaVersion = z.literal("1.0.0");
const ownerId = z.string().trim().min(1);
const positiveRevision = z.number().int().positive();
const hash = z.string().regex(/^[0-9a-f]{64}$/i);

export const projectSchema = z
  .object({
    id: ownerId,
    schemaVersion,
    revision: positiveRevision,
    status: z.string().trim().min(1),
    name: z.string().trim().min(1),
  })
  .strict();

export const episodeSchema = z
  .object({
    id: ownerId,
    schemaVersion,
    revision: positiveRevision,
    status: z.string().trim().min(1),
    projectId: ownerId,
    number: z.number().int().positive(),
    title: z.string().trim().min(1),
  })
  .strict();

export const creativeBriefSchema = z
  .object({
    id: ownerId,
    creativeBriefId: ownerId,
    projectId: ownerId,
    subject: z.string().trim().min(1),
    genre: z.string().trim().min(1),
    audience: z.string().trim().min(1),
    characterPremise: z.string().trim().min(1),
    style: z.string().trim().min(1),
    episodeDurationSeconds: z.number().int().positive(),
    episodeCount: z.number().int().positive(),
    scenesPerEpisode: z.number().int().positive(),
    shotsPerScene: z.number().int().positive(),
    revision: positiveRevision,
    schemaVersion,
    payloadHash: hash,
  })
  .strict();

export const creativeBriefCommandSchema = creativeBriefSchema
  .pick({
    subject: true,
    genre: true,
    audience: true,
    characterPremise: true,
    style: true,
    episodeDurationSeconds: true,
    episodeCount: true,
    scenesPerEpisode: true,
    shotsPerScene: true,
    schemaVersion: true,
  })
  .extend({
    creationMode: z.enum(["original", "adaptation"]),
    expectedRevision: positiveRevision,
    expectedBriefRevision: positiveRevision.nullable(),
  })
  .strict();

export const sourceBindingSchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    sourceMaterialId: ownerId,
    sourceMaterialRevision: positiveRevision,
    sourceContentHash: hash,
    creativeBriefId: ownerId,
    creativeBriefRevision: positiveRevision,
    creativeBriefPayloadHash: hash,
    parseStatus: z.string().trim().min(1),
    validationStatus: z.string().trim().min(1),
    bindingStatus: z.string().trim().min(1),
    bindingVersion: z.string().trim().min(1),
    schemaVersion,
  })
  .strict();

export const creativeProjectionSchema = z
  .object({
    projectId: ownerId,
    projectRevision: positiveRevision,
    creationMode: z.enum(["original", "adaptation"]).nullable(),
    creativeBrief: creativeBriefSchema.nullable(),
    creativeBriefHistory: z.array(creativeBriefSchema),
    settings: z.unknown().nullable(),
    settingsHistory: z.array(z.unknown()),
    sourceBinding: sourceBindingSchema.nullable(),
    storySpecRef: z.unknown().nullable(),
  })
  .strict();

export const sceneOwnerRefSchema = z
  .object({
    ownerId,
    revision: z.number().int().nonnegative(),
    contentHash: hash,
    purpose: z.string().trim().min(1),
  })
  .strict();

export const mediaEligibilitySchema = z
  .object({
    candidateId: ownerId,
    candidateRevision: positiveRevision,
    assetVersionId: ownerId,
    assetVersionRevision: z.number().int().nonnegative(),
    assetVersionHash: hash,
    projectId: ownerId,
    episodeId: ownerId,
    targetId: ownerId,
    status: z.string().trim().min(1),
    timelineReady: z.boolean(),
  })
  .passthrough();

export const shotProjectionSchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    episodeId: ownerId,
    sceneId: ownerId,
    number: z.number().int().positive(),
    schemaVersion,
    revision: positiveRevision,
    status: z.string().trim().min(1),
    specRef: sceneOwnerRefSchema.nullable(),
    continuitySnapshot: sceneOwnerRefSchema.nullable(),
    continuityTasks: z.array(sceneOwnerRefSchema.nullable()),
    currentImage: mediaEligibilitySchema.nullable(),
    currentVideo: mediaEligibilitySchema.nullable(),
  })
  .strict();

export const sceneProjectionSchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    episodeId: ownerId,
    number: z.number().int().positive(),
    title: z.string(),
    schemaVersion,
    revision: positiveRevision,
    sceneOrderRevision: positiveRevision.optional(),
    status: z.string().trim().min(1),
    specRef: sceneOwnerRefSchema.nullable(),
    shots: z.array(shotProjectionSchema),
  })
  .strict();

export const workflowVersionSchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    templateKey: z.literal("drama-mvp-a-default"),
    scopeType: z.enum(["project", "episode", "scene", "shot"]),
    scopeIds: z.array(ownerId).nonempty(),
    definition: z.object({ nodes: z.array(z.unknown()) }).passthrough(),
    revision: positiveRevision,
    contentHash: hash,
    status: z.literal("published"),
    versionNumber: positiveRevision,
    schemaVersion,
    bindingId: ownerId,
    bindingRevision: positiveRevision,
  })
  .strict();

export const frozenSelectionSchema = z
  .object({
    selectionSnapshotId: ownerId,
    provider: z.literal("mock"),
    providerId: ownerId,
    profile: z.literal("local-test-offline"),
    profileId: ownerId,
    modelId: ownerId,
    adapterKey: z.literal("mock"),
    adapterIdentity: z.literal("local_workspace"),
    profileRevision: positiveRevision,
    capabilitySnapshotId: ownerId,
    capabilityRevision: positiveRevision,
    capabilityOperation: z.enum([
      "text.generate",
      "image.generate",
      "video.submit",
    ]),
    capabilitySnapshots: z
      .partialRecord(
        z.enum(["text.generate", "image.generate", "video.submit"]),
        z.object({ id: ownerId, revision: positiveRevision }).strict(),
      )
      .refine((value) => Object.keys(value).length > 0),
    skills: z.array(ownerId).min(1),
    skillRevisionIds: z.array(ownerId).min(1),
    skillDigests: z.array(ownerId).min(1),
    decision: z.enum(["fixed", "selected", "manual"]),
    decisionRevision: positiveRevision,
    routeStatus: z.literal("selected"),
    source: z.enum(["explicit-local-profile", "skill-route-decision"]),
    routeDecisionId: ownerId.optional(),
    routeSelectionId: ownerId.optional(),
  })
  .strict()
  .superRefine((value, context) => {
    if (
      value.skills.length !== value.skillRevisionIds.length ||
      value.skills.length !== value.skillDigests.length
    ) {
      context.addIssue({
        code: "custom",
        message: "skill selection arrays must align",
      });
    }
  });

export const workflowSourceSnapshotSchema = z
  .object({
    workflowVersionId: ownerId,
    versionNumber: positiveRevision,
    contentHash: hash,
    definition: z.object({ nodes: z.array(z.unknown()) }).passthrough(),
    scopeType: z.enum(["project", "episode", "scene", "shot"]),
    scopeIds: z.array(ownerId).min(1),
    bindingId: ownerId,
    bindingRevision: positiveRevision,
    templateKey: z.literal("drama-mvp-a-default"),
    schemaVersion,
  })
  .strict();

function validateSafeSummary(
  value: unknown,
  context: z.RefinementCtx,
  path: string[] = [],
): void {
  if (typeof value === "string" && value.length > 256) {
    context.addIssue({
      code: "custom",
      message: `run summary string exceeds 256 characters at ${path.join(".")}`,
    });
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((item, index) =>
      validateSafeSummary(item, context, [...path, String(index)]),
    );
    return;
  }
  if (value && typeof value === "object") {
    for (const [key, child] of Object.entries(value)) {
      if (/secret|credential|objectkey|prompt|payload|bytes|url/i.test(key)) {
        context.addIssue({
          code: "custom",
          message: `unsafe run summary field: ${key}`,
        });
      }
      validateSafeSummary(child, context, [...path, key]);
    }
  }
}

const safeSummarySchema = z.unknown().superRefine((value, context) => {
  validateSafeSummary(value, context);
});

export const runEventSchema = z
  .object({
    id: ownerId,
    runId: ownerId,
    sequence: z.number().int().positive(),
    eventType: z.string().trim().min(1),
    correlationId: ownerId,
    payload: safeSummarySchema,
    nodeRunId: ownerId.nullable().optional(),
    schemaVersion,
    revision: positiveRevision,
    createdAt: z.string().optional(),
  })
  .strict();

export const runDetailSchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    schemaVersion,
    revision: positiveRevision,
    status: z.enum([
      "queued",
      "running",
      "waiting_review",
      "succeeded",
      "failed",
      "cancel_requested",
      "cancelled",
    ]),
    workflowVersionId: ownerId,
    sourceSnapshot: workflowSourceSnapshotSchema,
    selectionSnapshot: frozenSelectionSchema,
    createdAt: z.string(),
    updatedAt: z.string(),
    elapsedSeconds: z.number().int().nonnegative(),
    allowedActions: z.array(z.enum(["cancel", "createSuccessor"])),
    failure: z
      .object({
        code: z.string().optional(),
        message: z.string().optional(),
        retryable: z.boolean().optional(),
      })
      .passthrough()
      .nullable(),
    nodes: z.array(
      z
        .object({
          id: ownerId,
          revision: positiveRevision,
          nodeKey: z.string().trim().min(1),
          status: z.string().trim().min(1),
          logicalOperation: ownerId,
          scopeRefs: z.array(z.record(z.string(), z.unknown())),
          inputSummary: safeSummarySchema,
          outputSummary: safeSummarySchema,
          failure: safeSummarySchema,
          submissionState: z.string().trim().min(1),
        })
        .strict(),
    ),
    recentEvents: z.array(runEventSchema),
  })
  .strict()
  .transform((value) => ({
    ...value,
    workflowVersionNumber: value.sourceSnapshot.versionNumber,
    workflowContentHash: value.sourceSnapshot.contentHash,
    latestEventSequence: value.recentEvents.reduce(
      (latest, event) => Math.max(latest, event.sequence),
      0,
    ),
    allowedActions: {
      cancel: value.allowedActions.includes("cancel"),
      createSuccessor: value.allowedActions.includes("createSuccessor"),
    },
  }));

export const textReviewBatchSchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    runId: ownerId,
    revision: positiveRevision,
    status: z.enum(["building", "pending_review", "accepted", "rejected"]),
    schemaVersion,
    candidates: z.array(
      z
        .object({
          id: ownerId,
          kind: z.string().trim().min(1),
          payloadHash: hash,
          status: z.string().trim().min(1),
          revision: positiveRevision,
        })
        .passthrough(),
    ),
  })
  .passthrough();

export const skillRouteDecisionSchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    nodeKey: ownerId,
    launchId: ownerId,
    revision: positiveRevision,
    candidates: z.array(
      z
        .object({
          name: ownerId,
          version: ownerId,
          score: z.number().int(),
          digest: z.string().nullable().optional(),
          scoreSource: z.string().optional(),
        })
        .passthrough(),
    ),
    selected: z.object({ name: ownerId, version: ownerId }).nullable(),
    needsManualSelection: z.boolean(),
    fallbackReason: z.string().nullable(),
    auditStages: z.array(z.string()),
  })
  .passthrough();

export const sourceMaterialSchema = z
  .object({
    id: ownerId,
    projectId: ownerId,
    materialType: z.enum(["novel", "synopsis", "existing_script"]),
    inputMode: z.enum(["inline_text", "uploaded_file"]),
    revision: positiveRevision,
    current: z
      .object({
        id: ownerId,
        revision: positiveRevision,
        contentHash: hash,
        parseStatus: z.string(),
        validationStatus: z.string(),
        assetVersionId: ownerId.nullable(),
      })
      .passthrough()
      .nullable(),
  })
  .passthrough();

export type Project = z.infer<typeof projectSchema>;
export type Episode = z.infer<typeof episodeSchema>;
export type CreativeProjection = z.infer<typeof creativeProjectionSchema>;
export type SceneProjection = z.infer<typeof sceneProjectionSchema>;
export type RunDetail = z.infer<typeof runDetailSchema>;
