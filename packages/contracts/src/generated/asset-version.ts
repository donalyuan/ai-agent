/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `AssetVersion`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `AssetVersion`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = string;
/**
 * This interface was referenced by `AssetVersion`'s JSON-Schema
 * via the `definition` "revision".
 */
export type Revision = number;
/**
 * This interface was referenced by `AssetVersion`'s JSON-Schema
 * via the `definition` "status".
 */
export type Status =
  | "draft"
  | "generated"
  | "pending_review"
  | "approved"
  | "rejected"
  | "superseded"
  | "archived";
/**
 * This interface was referenced by `AssetVersion`'s JSON-Schema
 * via the `definition` "hash".
 */
export type Hash = string;

export interface AssetVersion {
  id: Uuid;
  schema_version: SchemaVersion;
  revision: Revision;
  status: Status;
  projectId: string;
  assetId: string;
  versionNumber: number;
  contentHash: Hash;
  storageObject: StorageObject;
}
/**
 * This interface was referenced by `AssetVersion`'s JSON-Schema
 * via the `definition` "storageObject".
 */
export interface StorageObject {
  storageProvider: string;
  bucket: string;
  region?: string;
  objectKey: string;
  eTag?: string;
  checksum: Hash;
  mimeType: string;
  sizeBytes: number;
  media?: {
    durationMs?: number;
    width?: number;
    height?: number;
  };
}
