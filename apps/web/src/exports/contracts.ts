import { z } from "zod";

export const exportArtifactSchema = z
  .object({
    id: z.string().min(1),
    artifactType: z.enum(["mp4", "srt", "light_manifest"]),
    status: z.enum(["pending", "verified", "failed", "held"]),
    sizeBytes: z.number().int().nonnegative().nullable(),
    checksum: z
      .string()
      .regex(/^[0-9a-f]{64}$/)
      .nullable(),
    mimeType: z.string().nullable(),
    hold: z.boolean(),
    licenseStatus: z.string().min(1),
    expiresAt: z.string().min(1),
  })
  .strict();

export const exportJobSchema = z
  .object({
    id: z.string().min(1),
    projectId: z.string().min(1),
    episodeId: z.string().min(1),
    timelineVersionId: z.string().min(1),
    batchId: z.string().min(1),
    revision: z.number().int().positive(),
    status: z.enum([
      "queued",
      "preflighting",
      "rendering",
      "packaging",
      "succeeded",
      "failed",
      "cancel_requested",
      "cancelled",
    ]),
    packagingPhase: z
      .enum(["uploading", "verifying", "registering"])
      .nullable(),
    logicalOperation: z.string().min(1),
    renderPlanHash: z
      .string()
      .regex(/^[0-9a-f]{64}$/)
      .nullable(),
    rendererDiagnostic: z.string().nullable(),
    diagnostics: z.array(z.record(z.string(), z.unknown())),
    artifacts: z.array(exportArtifactSchema).length(3),
  })
  .strict();

const exportMemberSchema = z
  .object({
    episodeId: z.string().min(1),
    timelineVersionId: z.string().min(1),
    timelineVersionRevision: z.literal(1),
    outputBaseName: z.string().regex(/^[A-Za-z0-9._-]{1,120}$/),
    exportJobId: z.string().min(1),
    status: exportJobSchema.shape.status,
  })
  .strict();

export const exportBatchSchema = z
  .object({
    id: z.string().min(1),
    schemaVersion: z.literal("1.0.0"),
    revision: z.number().int().positive(),
    projectId: z.string().min(1),
    exportProfile: z.literal("light"),
    settings: z.record(z.string(), z.unknown()),
    status: z.enum(["queued", "succeeded", "partially_failed", "failed"]),
    jobs: z.array(exportJobSchema).min(1),
    members: z.array(exportMemberSchema).min(1),
  })
  .strict();

export const artifactGrantSchema = z
  .object({
    schemaVersion: z.literal("1.0.0"),
    artifactId: z.string().min(1),
    expiresAt: z.number().int().positive(),
    action: z.literal("read"),
    accessPath: z.string().regex(/^\/v1\/asset-media-grants\/[A-Za-z0-9]+$/),
  })
  .strict();

export type ExportBatch = z.infer<typeof exportBatchSchema>;
export type ExportJob = z.infer<typeof exportJobSchema>;
export type ExportArtifact = z.infer<typeof exportArtifactSchema>;

export function latestJobsByEpisode(jobs: ExportJob[]) {
  const latest = new Map<string, ExportJob>();
  for (const job of jobs) latest.set(job.episodeId, job);
  return latest;
}

export function downloadableArtifact(job: ExportJob, artifact: ExportArtifact) {
  const expiresAt = Date.parse(artifact.expiresAt);
  return (
    job.status === "succeeded" &&
    artifact.status === "verified" &&
    !artifact.hold &&
    artifact.licenseStatus === "approved" &&
    Number.isFinite(expiresAt) &&
    expiresAt > Date.now()
  );
}
