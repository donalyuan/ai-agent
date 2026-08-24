/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = string;
/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "hash".
 */
export type Hash = string;

export interface TimelineCurrent {
  id: Uuid;
  schema_version: SchemaVersion;
  revision: number;
  projectId: Uuid;
  episodeId: Uuid;
  fps: 30;
  clips: Clip[];
  soundCues: SoundCue[];
  captions: Caption[];
  ducking: null | Ducking;
  masterLimiter: MasterLimiter;
}
/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "clip".
 */
export interface Clip {
  id: Uuid;
  assetVersionId: Uuid;
  assetVersionRevision: number;
  assetVersionHash: Hash;
  derivativeFingerprint: Hash;
  sourceInFrame: number;
  durationFrames: number;
  timelineStartFrame: number;
  transform: Transform;
  transition: Transition;
}
/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "transform".
 */
export interface Transform {
  position: {
    x: number;
    y: number;
  };
  scale: number;
  opacity: number;
}
/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "transition".
 */
export interface Transition {
  type: "cut" | "crossfade";
  durationFrames: number;
}
/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "soundCue".
 */
export interface SoundCue {
  id: Uuid;
  track: "dialogue" | "music" | "ambience" | "effects";
  assetVersionId: Uuid;
  assetVersionRevision: number;
  assetVersionHash: Hash;
  startFrame: number;
  durationFrames: number;
  trigger: "manual" | "scene_start" | "shot_start" | "shot_end";
  triggerRef: null | TriggerRef;
  priority: number;
  /**
   * @maxItems 8
   */
  continuityRefs:
    | []
    | [OwnerRef]
    | [OwnerRef, OwnerRef]
    | [OwnerRef, OwnerRef, OwnerRef]
    | [OwnerRef, OwnerRef, OwnerRef, OwnerRef]
    | [OwnerRef, OwnerRef, OwnerRef, OwnerRef, OwnerRef]
    | [OwnerRef, OwnerRef, OwnerRef, OwnerRef, OwnerRef, OwnerRef]
    | [OwnerRef, OwnerRef, OwnerRef, OwnerRef, OwnerRef, OwnerRef, OwnerRef]
    | [
        OwnerRef,
        OwnerRef,
        OwnerRef,
        OwnerRef,
        OwnerRef,
        OwnerRef,
        OwnerRef,
        OwnerRef,
      ];
  gainDb: number;
  mute: boolean;
  solo: boolean;
  fadeInFrames: number;
  fadeOutFrames: number;
  authorizationStatus: "authorized";
  licenseStatus: "approved";
}
/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "triggerRef".
 */
export interface TriggerRef {
  ownerType: "scene" | "shot";
  id: Uuid;
  revision: number;
  startFrame: number;
  offsetFrames: number;
}
/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "ownerRef".
 */
export interface OwnerRef {
  ownerType: "asset_bible" | "scene" | "shot" | "shot_spec";
  id: Uuid;
  revision: number;
  hash: Hash;
}
/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "caption".
 */
export interface Caption {
  id: Uuid;
  text: string;
  startFrame: number;
  endFrame: number;
}
/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "ducking".
 */
export interface Ducking {
  enabled: boolean;
  dialogueIntervals: never[][];
  attenuationDb: number;
  attackFrames: number;
  releaseFrames: number;
  /**
   * @minItems 1
   */
  targetTracks: [
    "music" | "ambience" | "effects",
    ...("music" | "ambience" | "effects")[],
  ];
}
/**
 * This interface was referenced by `TimelineCurrent`'s JSON-Schema
 * via the `definition` "masterLimiter".
 */
export interface MasterLimiter {
  integratedLufs: -14;
  truePeakDbtp: -1;
}
