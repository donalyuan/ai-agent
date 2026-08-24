/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `AssetEditPlan`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `AssetEditPlan`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = "1.0.0";
/**
 * This interface was referenced by `AssetEditPlan`'s JSON-Schema
 * via the `definition` "revision".
 */
export type Revision = number;
/**
 * This interface was referenced by `AssetEditPlan`'s JSON-Schema
 * via the `definition` "hash".
 */
export type Hash = string;

export interface AssetEditPlan {
  id: Uuid;
  schema_version: SchemaVersion;
  revision: Revision;
  status: "pending_review" | "stale" | "executing";
  projectId: Uuid;
  episodeId: Uuid;
  targetId?: Uuid;
  base: VersionRef;
  references: VersionRef[];
  instruction: string;
  turnId: Uuid;
  continuity?: ContinuityRef;
}
/**
 * This interface was referenced by `AssetEditPlan`'s JSON-Schema
 * via the `definition` "versionRef".
 */
export interface VersionRef {
  id: Uuid;
  projectId: Uuid;
  revision: number;
  contentHash: Hash;
  kind: "image" | "video";
  mimeType: string;
}
/**
 * This interface was referenced by `AssetEditPlan`'s JSON-Schema
 * via the `definition` "continuityRef".
 */
export interface ContinuityRef {
  id: Uuid;
  revision: number;
  contentHash: Hash;
  targetId: Uuid;
}
