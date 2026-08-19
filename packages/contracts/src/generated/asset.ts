/* This file is generated from JSON Schema. Do not edit manually. */

/**
 * This interface was referenced by `Asset`'s JSON-Schema
 * via the `definition` "uuid".
 */
export type Uuid = string;
/**
 * This interface was referenced by `Asset`'s JSON-Schema
 * via the `definition` "schemaVersion".
 */
export type SchemaVersion = string;
/**
 * This interface was referenced by `Asset`'s JSON-Schema
 * via the `definition` "revision".
 */
export type Revision = number;
/**
 * This interface was referenced by `Asset`'s JSON-Schema
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

export interface Asset {
  id: Uuid;
  schema_version: SchemaVersion;
  revision: Revision;
  status: Status;
  projectId: string;
  kind: "image" | "video" | "audio" | "text" | "document";
  name: string;
}
