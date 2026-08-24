/* This file is generated from JSON Schema. Do not edit manually. */

export type AssetCenter =
  | CatalogPage
  | Reservation
  | MediaProjection
  | UsageProjection
  | TimelineSelection;
export type SchemaVersion = string;
export type CatalogItem = Asset & {
  versionCount: number;
  processingStatus: "unknown" | "pending" | "ready" | "failed" | "stale";
  latestVersion: SafeVersion | null;
  [k: string]: unknown;
};
export type Uuid = string;
export type Hash = string;

export interface CatalogPage {
  contractKind: "catalog_page";
  schema_version: SchemaVersion;
  /**
   * @maxItems 100
   */
  items: CatalogItem[];
  nextCursor: string | null;
}
export interface Asset {
  /**
   * This interface was referenced by `Asset`'s JSON-Schema
   * via the `definition` "uuid".
   */
  id: string;
  /**
   * This interface was referenced by `Asset`'s JSON-Schema
   * via the `definition` "schemaVersion".
   */
  schema_version: string;
  /**
   * This interface was referenced by `Asset`'s JSON-Schema
   * via the `definition` "revision".
   */
  revision: number;
  /**
   * This interface was referenced by `Asset`'s JSON-Schema
   * via the `definition` "status".
   */
  status:
    | "draft"
    | "generated"
    | "pending_review"
    | "approved"
    | "rejected"
    | "superseded"
    | "archived";
  projectId: string;
  kind: "image" | "video" | "audio" | "text" | "document";
  name: string;
  sourceType:
    | "user_upload"
    | "provider_generated"
    | "source_material"
    | "imported";
  catalogRole?:
    | "character"
    | "location"
    | "prop"
    | "storyboard"
    | "video_take"
    | "dialogue"
    | "music"
    | "ambience"
    | "effects"
    | "other"
    | null;
  /**
   * @maxItems 32
   */
  tags: string[];
  authorizationStatus:
    | "unknown"
    | "declared"
    | "verified"
    | "restricted"
    | "expired";
  copyrightOwner?: string | null;
  licenseLabel?: string | null;
  licenseReference?: string | null;
  updatedAt: string;
  [k: string]: unknown;
}
export interface SafeVersion {
  id: Uuid;
  revision: number;
  contentHash: Hash;
  checksum: Hash;
  mimeType: string;
  sizeBytes: number;
  durationMs?: number | null;
}
export interface Reservation {
  contractKind: "reservation";
  schema_version: SchemaVersion;
  id: Uuid;
  projectId: Uuid;
  assetId: Uuid;
  revision: number;
  operationKey: string;
  fingerprint: Hash;
  status: "reserved" | "registered" | "cancelled" | "failed";
  registeredVersionId?: Uuid | null;
  expectedAssetRevision: number;
  declaredKind: "image" | "video" | "audio" | "text" | "document";
  declaredMimeType: string;
  declaredSizeBytes: number;
  declaredChecksum: Hash;
  storageProfileId: string;
  storageProfileRevision: number;
  storageProfileSnapshotHash: Hash;
  diagnostic?: string | null;
}
export interface MediaProjection {
  contractKind: "media_projection";
  schema_version: SchemaVersion;
  projectId: Uuid;
  assetVersionId: Uuid;
  assetVersionRevision: number;
  sourceHash: Hash;
  status: "pending" | "ready" | "failed" | "stale" | "unavailable";
  diagnostic?: string | null;
  derivatives: {
    id: Uuid;
    kind: "proxy" | "thumbnail" | "keyframe_index" | "waveform";
    status: "pending" | "ready" | "failed" | "stale";
    grantAvailable: boolean;
  }[];
}
export interface UsageProjection {
  contractKind: "usage_projection";
  schema_version: SchemaVersion;
  projectId: Uuid;
  assetVersionId: Uuid;
  status: "complete" | "partial" | "unavailable";
  diagnostic?: string | null;
  unavailableOwners: string[];
  references: {
    ownerType: string;
    ownerId: string;
    ownerRevision: number;
    scope: {
      projectId: Uuid;
      [k: string]: string;
    };
    state: string;
    sourceHash: Hash;
    deepLink: string;
  }[];
}
export interface TimelineSelection {
  contractKind: "timeline_selection";
  schema_version: SchemaVersion;
  projectId: Uuid;
  episodeId: Uuid;
  assetVersionId: Uuid;
  assetVersionRevision: number;
  assetVersionHash: Hash;
  authorizationStatus: "verified";
  licenseLabel?: string | null;
}
