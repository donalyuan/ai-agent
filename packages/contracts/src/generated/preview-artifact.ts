/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `PreviewArtifact`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `PreviewArtifact`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = string;
/**
 * This interface was referenced by `PreviewArtifact`'s JSON-Schema
 * via the `definition` "hash".
 */
export type Hash = string;

export interface PreviewArtifact {
  id: Uuid;
  schema_version: SchemaVersion;
  projectId: Uuid;
  episodeId: Uuid;
  cutId: Uuid;
  cutRevision: number;
  timelineFingerprint: Hash;
  renderPlanHash: Hash;
  status: "pending" | "ready" | "failed" | "stale";
  proxyDerivativeIds: Uuid[];
  rawDiagnostic: string | null;
}
