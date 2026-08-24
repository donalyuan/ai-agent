/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `ExportArtifact`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `ExportArtifact`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = string;
/**
 * This interface was referenced by `ExportArtifact`'s JSON-Schema
 * via the `definition` "hash".
 */
export type Hash = string;

export interface ExportArtifact {
  id: Uuid;
  schema_version: SchemaVersion;
  projectId: Uuid;
  episodeId: Uuid;
  timelineVersionId: Uuid;
  exportJobId: Uuid;
  artifactType: "mp4" | "srt" | "light_manifest";
  status: "pending" | "verified" | "failed" | "held";
  objectRef: {
    profileId: string;
    objectKey: string;
    operationKey: string;
  };
  sizeBytes: number;
  checksum: Hash;
  mimeType: "video/mp4" | "application/x-subrip" | "application/json";
  retention: {
    policy: string;
    version: string;
    expiresAt: string;
  };
  license: {
    status: string;
    source: string;
  };
  hold: boolean;
}
