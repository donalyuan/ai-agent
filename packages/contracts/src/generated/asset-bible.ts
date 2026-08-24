/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = "1.0.0";
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "entryType".
 */
export type EntryType =
  | "character"
  | "look"
  | "location"
  | "scene_visual"
  | "prop"
  | "visual_style";
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "hex64".
 */
export type Hex64 = string;

export interface AssetBible {
  id: Uuid;
  schema_version: "1.0.0";
  assetBible: AssetBible1;
  /**
   * @minItems 1
   */
  entries: [Entry, ...Entry[]];
  /**
   * @minItems 1
   */
  versions: [Version, ...Version[]];
  /**
   * @minItems 1
   */
  relationships: [Relationship, ...Relationship[]];
  /**
   * @minItems 1
   */
  assignments: [Assignment, ...Assignment[]];
  /**
   * @minItems 1
   */
  snapshots: [Snapshot, ...Snapshot[]];
  /**
   * @minItems 1
   */
  impactAnalyses: [ImpactAnalysis, ...ImpactAnalysis[]];
  /**
   * @minItems 1
   */
  acceptDecisions: [AcceptDecision, ...AcceptDecision[]];
  /**
   * @minItems 1
   */
  revisionTasks: [RevisionTask, ...RevisionTask[]];
}
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "assetBible".
 */
export interface AssetBible1 {
  id: Uuid;
  projectId: Uuid;
  revision: number;
  schema_version: SchemaVersion;
  currentVersionMap: {
    [k: string]: Uuid;
  };
}
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "entry".
 */
export interface Entry {
  id: Uuid;
  assetBibleId: Uuid;
  projectId: Uuid;
  entryType: EntryType;
  revision: number;
  schema_version: SchemaVersion;
  currentVersionId?: Uuid | null;
  disabled: boolean;
}
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "version".
 */
export interface Version {
  id: Uuid;
  entryId: Uuid;
  projectId: Uuid;
  entryType: EntryType;
  version: number;
  revision: 1;
  schema_version: SchemaVersion;
  attributes: {
    [k: string]: unknown;
  };
  contentHash: Hex64;
  actorUuid: Uuid;
  referenceAssetVersionRefs: OwnerReference[];
  generationSpecRefs: OwnerReference[];
}
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "ownerReference".
 */
export interface OwnerReference {
  ownerId: Uuid;
  revision: number;
  contentHash: Hex64;
  purpose: string;
}
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "relationship".
 */
export interface Relationship {
  id: Uuid;
  projectId: Uuid;
  sourceEntryId: Uuid;
  targetEntryId: Uuid;
  kind: "character_look" | "location_scene_visual" | "related";
  schema_version: SchemaVersion;
}
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "assignment".
 */
export interface Assignment {
  id: Uuid;
  projectId: Uuid;
  scopeType: "project" | "episode" | "scene" | "shot";
  scopeId: Uuid;
  entryId: Uuid;
  entryVersionId: Uuid;
  entryVersionRevision: number;
  entryVersionHash: Hex64;
  scopeRevision: number;
  revision: number;
  schema_version: SchemaVersion;
}
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "snapshot".
 */
export interface Snapshot {
  id: Uuid;
  projectId: Uuid;
  targetType: "project" | "episode" | "scene" | "shot";
  targetId: Uuid;
  targetRevision: number;
  revision: number;
  schema_version: SchemaVersion;
  status: "accepted" | "incomplete";
  overrideChain: Assignment[];
  /**
   * @minItems 1
   */
  resolvedRefs: [OwnerReference, ...OwnerReference[]];
  sourceRevisions: number[];
  contentHash: Hex64;
}
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "impactAnalysis".
 */
export interface ImpactAnalysis {
  id: Uuid;
  projectId: Uuid;
  entryId: Uuid;
  baseVersionId: Uuid;
  candidatePayloadHash: Hex64;
  status: "complete" | "incomplete";
  diagnostic?: string | null;
  revision: number;
  schema_version: SchemaVersion;
  targets: ImpactTarget[];
  targetSetHash: Hex64;
}
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "impactTarget".
 */
export interface ImpactTarget {
  targetType: "episode" | "scene" | "shot";
  targetId: Uuid;
  targetRevision: number;
  reason: string;
  snapshotId: Uuid;
  snapshotHash: Hex64;
  suggestedAction: "review" | "regenerate" | "acknowledge";
}
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "acceptDecision".
 */
export interface AcceptDecision {
  id: Uuid;
  projectId: Uuid;
  entryId: Uuid;
  analysisId: Uuid;
  oldVersionId: Uuid;
  newVersionId: Uuid;
  targetSetHash: Hex64;
  actorUuid: Uuid;
  correlationId: string;
  fingerprint: Hex64;
  schema_version: SchemaVersion;
}
/**
 * This interface was referenced by `AssetBible`'s JSON-Schema
 * via the `definition` "revisionTask".
 */
export interface RevisionTask {
  id: Uuid;
  projectId: Uuid;
  targetType: "episode" | "scene" | "shot";
  targetId: Uuid;
  targetRevision: number;
  entryId: Uuid;
  oldVersionId: Uuid;
  newVersionId: Uuid;
  oldSnapshotId: Uuid;
  oldSnapshotHash: Hex64;
  reason: string;
  status: "pending" | "acknowledged" | "resolved" | "superseded";
  revision: number;
  correlationId: string;
  schema_version: SchemaVersion;
}
