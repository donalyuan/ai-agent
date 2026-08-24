/* This file is generated from JSON Schema. Do not edit manually. */

export type MediaDerivative = {
  [k: string]: unknown;
} & {
  id: Uuid;
  schema_version: SchemaVersion;
  derivativeSchemaVersion: SchemaVersion;
  projectId: Uuid;
  inspectionId: Uuid;
  assetVersionId: Uuid;
  assetVersionRevision: number;
  sourceHash: Hash;
  sourceFingerprint: Hash;
  kind: "proxy" | "thumbnail" | "keyframe_index" | "waveform";
  status: "pending" | "ready" | "failed" | "stale";
  parameters: {
    [k: string]: unknown;
  };
  tool: string;
  toolVersion: string;
  operationKey: string;
  objectRef: null | ObjectRef;
  checksum: null | Hash;
  sizeBytes: number | null;
  retention: Retention;
  license: License;
  hold: boolean;
  rawDiagnostic: string | null;
};
/**
 * This interface was referenced by `undefined`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `undefined`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = string;
/**
 * This interface was referenced by `undefined`'s JSON-Schema
 * via the `definition` "hash".
 */
export type Hash = string;

/**
 * This interface was referenced by `undefined`'s JSON-Schema
 * via the `definition` "objectRef".
 */
export interface ObjectRef {
  profileId: string;
  objectKey: string;
  operationKey: string;
}
/**
 * This interface was referenced by `undefined`'s JSON-Schema
 * via the `definition` "retention".
 */
export interface Retention {
  policy: string;
  version: string;
}
/**
 * This interface was referenced by `undefined`'s JSON-Schema
 * via the `definition` "license".
 */
export interface License {
  status: string;
}
