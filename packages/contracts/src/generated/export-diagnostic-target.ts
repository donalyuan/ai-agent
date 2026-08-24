/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `ExportDiagnosticTarget`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `ExportDiagnosticTarget`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = string;

export interface ExportDiagnosticTarget {
  id: Uuid;
  schema_version: SchemaVersion;
  targetType:
    | "timeline"
    | "clip"
    | "caption"
    | "sound_cue"
    | "asset_version"
    | "renderer"
    | "storage"
    | "artifact";
  projectId: Uuid;
  episodeId: Uuid;
  timelineVersionId: null | Uuid;
  ownerId: null | string;
  ownerRevision: null | number;
  fieldPath: null | string;
  routeToken: string;
  code: string;
}
