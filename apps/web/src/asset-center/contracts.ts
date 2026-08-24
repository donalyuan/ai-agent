import { z } from "zod";

export const catalogItemSchema = z.object({
  id: z.string(),
  projectId: z.string(),
  revision: z.number(),
  name: z.string(),
  kind: z.enum(["image", "video", "audio", "text", "document"]),
  status: z.string(),
  sourceType: z.string(),
  catalogRole: z.string().nullable().optional(),
  tags: z.array(z.string()),
  authorizationStatus: z.string(),
  copyrightOwner: z.string().nullable().optional(),
  licenseLabel: z.string().nullable().optional(),
  licenseReference: z.string().nullable().optional(),
  updatedAt: z.string(),
  versionCount: z.number(),
  processingStatus: z.enum(["unknown", "pending", "ready", "failed", "stale"]),
  latestVersion: z
    .object({
      id: z.string(),
      revision: z.number(),
      contentHash: z.string(),
      checksum: z.string(),
      mimeType: z.string(),
      sizeBytes: z.number(),
      durationMs: z.number().nullable().optional(),
    })
    .nullable(),
});

export const catalogSchema = z.object({
  schemaVersion: z.string(),
  items: z.array(catalogItemSchema),
  nextCursor: z.string().nullable(),
});

export type CatalogItem = z.infer<typeof catalogItemSchema>;
export type CatalogPage = z.infer<typeof catalogSchema>;
export type FilterState = {
  kind: string;
  role: string;
  source: string;
  authorization: string;
  processing: string;
  tag: string;
};

export const emptyAssetFilters: FilterState = {
  kind: "",
  role: "",
  source: "",
  authorization: "",
  processing: "",
  tag: "",
};
