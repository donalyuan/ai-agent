/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `EpisodeExportBatch`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `EpisodeExportBatch`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = string;
/**
 * This interface was referenced by `EpisodeExportBatch`'s JSON-Schema
 * via the `definition` "hash".
 */
export type Hash = string;

export interface EpisodeExportBatch {
  id: Uuid;
  schema_version: SchemaVersion;
  revision: number;
  projectId: Uuid;
  exportProfile: "light";
  settings: Settings;
  status: "queued" | "succeeded" | "partially_failed" | "failed";
  /**
   * @minItems 1
   */
  jobs: [Job, ...Job[]];
  /**
   * @minItems 1
   */
  members: [Member, ...Member[]];
}
/**
 * This interface was referenced by `EpisodeExportBatch`'s JSON-Schema
 * via the `definition` "settings".
 */
export interface Settings {
  aspectRatio: "9:16" | "16:9" | "1:1";
  width: 1080 | 1920;
  height: 1080 | 1920;
  fps: 30;
  container: "mp4";
  videoCodec: "h264";
  pixelFormat: "yuv420p";
  audioCodec: "aac";
  sampleRate: 48000;
  subtitleEncoding: "UTF-8";
}
/**
 * This interface was referenced by `EpisodeExportBatch`'s JSON-Schema
 * via the `definition` "job".
 */
export interface Job {
  id: Uuid;
  projectId: Uuid;
  episodeId: Uuid;
  timelineVersionId: Uuid;
  batchId: Uuid;
  revision: number;
  status:
    | "queued"
    | "preflighting"
    | "rendering"
    | "packaging"
    | "succeeded"
    | "failed"
    | "cancel_requested"
    | "cancelled";
  packagingPhase: "uploading" | "verifying" | "registering" | null;
  logicalOperation: string;
  renderPlanHash: Hash | null;
  rendererDiagnostic: string | null;
  diagnostics: {
    [k: string]: unknown;
  }[];
  /**
   * @minItems 3
   * @maxItems 3
   */
  artifacts: [Artifact, Artifact, Artifact];
}
/**
 * This interface was referenced by `EpisodeExportBatch`'s JSON-Schema
 * via the `definition` "artifact".
 */
export interface Artifact {
  id: Uuid;
  artifactType: "mp4" | "srt" | "light_manifest";
  status: "pending" | "verified" | "failed" | "held";
  sizeBytes: number | null;
  checksum: Hash | null;
  mimeType: string | null;
  hold: boolean;
  licenseStatus: string;
  expiresAt: string;
}
/**
 * This interface was referenced by `EpisodeExportBatch`'s JSON-Schema
 * via the `definition` "member".
 */
export interface Member {
  episodeId: Uuid;
  timelineVersionId: Uuid;
  timelineVersionRevision: 1;
  outputBaseName: string;
  exportJobId: Uuid;
  status:
    | "queued"
    | "preflighting"
    | "rendering"
    | "packaging"
    | "succeeded"
    | "failed"
    | "cancel_requested"
    | "cancelled";
}
