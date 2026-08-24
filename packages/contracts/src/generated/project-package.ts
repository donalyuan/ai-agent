/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = string;
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "hash".
 */
export type Hash = string;

export interface ProjectPackage {
  id: Uuid;
  schema_version: SchemaVersion;
  manifestVersion: SchemaVersion;
  exportProfile: "light";
  projectId: Uuid;
  episode: OwnerRef;
  timelineVersion: OwnerRef;
  /**
   * @minItems 1
   */
  assetVersionRefs: [AssetRef, ...AssetRef[]];
  soundCues: AudioRef[];
  authorization: Provenance;
  license: Provenance;
  loudness: Loudness;
  /**
   * @minItems 1
   */
  models: [Model, ...Model[]];
  /**
   * @minItems 1
   */
  skillRevisions: [Skill, ...Skill[]];
  parameters: {
    [k: string]: unknown;
  };
  usage: Usage;
  cost: Cost;
  /**
   * @minItems 2
   */
  references: [Reference, Reference, ...Reference[]];
}
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "ownerRef".
 */
export interface OwnerRef {
  id: Uuid;
  revision: number;
  hash: Hash;
}
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "assetRef".
 */
export interface AssetRef {
  id: Uuid;
  revision: number;
  hash: Hash;
  authorization: Provenance;
  license: Provenance;
}
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "provenance".
 */
export interface Provenance {
  status: string;
  source: string;
  recordId: string;
}
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "audioRef".
 */
export interface AudioRef {
  cueId: Uuid;
  track: "dialogue" | "music" | "ambience" | "effects";
  assetVersionId: Uuid;
  startFrame: number;
  durationFrames: number;
}
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "loudness".
 */
export interface Loudness {
  integratedLufs: number;
  truePeakDbtp: number;
  measuredBy: string;
  measurementVersion: string;
}
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "model".
 */
export interface Model {
  providerId: string;
  profileId: string;
  modelId: string;
  capabilitySnapshotId: string;
}
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "skill".
 */
export interface Skill {
  id: string;
  revision: number;
  digest: Hash;
}
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "usage".
 */
export interface Usage {
  value: number;
  unit: string;
  status: "measured" | "estimated" | "unknown";
  source: string;
}
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "cost".
 */
export interface Cost {
  value: number | "unknown";
  currency: string;
  status: "measured" | "estimated" | "unknown";
  source: string;
}
/**
 * This interface was referenced by `ProjectPackage`'s JSON-Schema
 * via the `definition` "reference".
 */
export interface Reference {
  artifactType: "mp4" | "srt";
  artifactId: Uuid;
}
