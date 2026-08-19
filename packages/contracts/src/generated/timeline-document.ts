/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `TimelineDocument`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `TimelineDocument`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = string;
/**
 * This interface was referenced by `TimelineDocument`'s JSON-Schema
 * via the `definition` "revision".
 */
export type Revision = number;
/**
 * This interface was referenced by `TimelineDocument`'s JSON-Schema
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

export interface TimelineDocument {
  id: Uuid;
  schema_version: SchemaVersion;
  revision: Revision;
  status: Status;
  projectId: string;
  episodeId: string;
  cutId: Uuid;
  fps: number;
  width: number;
  height: number;
  tracks: Track[];
  clips: Clip[];
  captions: Caption[];
}
/**
 * This interface was referenced by `TimelineDocument`'s JSON-Schema
 * via the `definition` "track".
 */
export interface Track {
  id: Uuid;
  kind: "video" | "audio" | "caption" | "text";
  name: string;
}
/**
 * This interface was referenced by `TimelineDocument`'s JSON-Schema
 * via the `definition` "clip".
 */
export interface Clip {
  id: Uuid;
  trackId: Uuid;
  assetVersionId: string;
  timelineStartFrame: number;
  sourceInFrame: number;
  durationFrames: number;
}
/**
 * This interface was referenced by `TimelineDocument`'s JSON-Schema
 * via the `definition` "caption".
 */
export interface Caption {
  id: Uuid;
  startFrame: number;
  endFrame: number;
  text: string;
}
