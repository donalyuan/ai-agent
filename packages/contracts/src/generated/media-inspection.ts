/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `MediaInspection`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `MediaInspection`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = string;
/**
 * This interface was referenced by `MediaInspection`'s JSON-Schema
 * via the `definition` "hash".
 */
export type Hash = string;
/**
 * This interface was referenced by `MediaInspection`'s JSON-Schema
 * via the `definition` "status".
 */
export type Status = "pending" | "ready" | "failed" | "stale";

export interface MediaInspection {
  id: Uuid;
  schema_version: SchemaVersion;
  revision: number;
  projectId: Uuid;
  assetVersionId: Uuid;
  assetVersionRevision: number;
  sourceHash: Hash;
  sourceFingerprint: Hash;
  status: Status;
  metadata: Metadata;
  tool: string;
  toolVersion: string;
  operationKey: string;
  retention: Retention;
  license: License;
  hold: boolean;
  rawDiagnostic: string | null;
}
/**
 * This interface was referenced by `MediaInspection`'s JSON-Schema
 * via the `definition` "metadata".
 */
export interface Metadata {
  mimeType: string;
  sizeBytes: number;
  checksum: Hash;
  durationFrames: number;
  timebase: string;
  fpsNumerator: number;
  fpsDenominator: number;
  frameCount: number;
  width: number;
  height: number;
  videoCodec: string;
  pixelFormat: string;
  audioTracks: number;
  sampleRate: number;
  channels: number;
}
/**
 * This interface was referenced by `MediaInspection`'s JSON-Schema
 * via the `definition` "retention".
 */
export interface Retention {
  policy: string;
  version: string;
}
/**
 * This interface was referenced by `MediaInspection`'s JSON-Schema
 * via the `definition` "license".
 */
export interface License {
  status: string;
}
