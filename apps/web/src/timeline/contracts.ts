import { z } from "zod";

const hash = z.string().regex(/^[0-9a-f]{64}$/);

const ownerRefSchema = z
  .object({
    ownerType: z.enum(["asset_bible", "scene", "shot", "shot_spec"]),
    id: z.string().min(1),
    revision: z.number().int().positive(),
    hash,
  })
  .strict();

const triggerRefSchema = z
  .object({
    ownerType: z.enum(["scene", "shot"]),
    id: z.string().min(1),
    revision: z.number().int().positive(),
    startFrame: z.number().int().nonnegative(),
    offsetFrames: z.number().int().nonnegative(),
  })
  .strict();

export const clipSchema = z
  .object({
    id: z.string().min(1),
    assetVersionId: z.string().min(1),
    assetVersionRevision: z.number().int().nonnegative(),
    assetVersionHash: hash,
    timelineStart: z.number().int().nonnegative(),
    durationFrames: z.number().int().positive(),
    inFrame: z.number().int().nonnegative(),
    outFrame: z.number().int().positive(),
    transform: z
      .object({
        position: z.object({ x: z.number(), y: z.number() }),
        scale: z.number().positive(),
        opacity: z.number().min(0).max(1),
      })
      .optional(),
    transition: z.string().optional(),
  })
  .passthrough();

export const soundCueSchema = z
  .object({
    id: z.string().min(1),
    track: z.enum(["dialogue", "music", "ambience", "effects"]),
    assetVersionId: z.string().min(1),
    assetVersionRevision: z.number().int().nonnegative(),
    assetVersionHash: hash,
    startFrame: z.number().int().nonnegative(),
    durationFrames: z.number().int().positive(),
    trigger: z.enum(["manual", "scene_start", "shot_start", "shot_end"]),
    triggerRef: triggerRefSchema.nullable(),
    priority: z.number().int().min(0).max(100),
    continuityRefs: z.array(ownerRefSchema).max(8),
    gainDb: z.number(),
    mute: z.boolean(),
    solo: z.boolean(),
    fadeInFrames: z.number().int().nonnegative(),
    fadeOutFrames: z.number().int().nonnegative(),
    authorizationStatus: z.literal("authorized"),
    licenseStatus: z.literal("approved"),
  })
  .strict();

export const captionSchema = z
  .object({
    id: z.string().min(1),
    text: z.string(),
    startFrame: z.number().int().nonnegative(),
    endFrame: z.number().int().positive(),
  })
  .passthrough();

export const timelineSchema = z.object({
  id: z.string().min(1),
  projectId: z.string().min(1),
  episodeId: z.string().min(1),
  schemaVersion: z.string(),
  revision: z.number().int().positive(),
  fps: z.literal(30),
  clips: z.array(clipSchema),
  soundCues: z.array(soundCueSchema),
  captions: z.array(captionSchema),
  ducking: z
    .object({
      enabled: z.boolean(),
      dialogueIntervals: z.array(
        z.tuple([z.number().int().nonnegative(), z.number().int().positive()]),
      ),
      attenuationDb: z.number().nonnegative(),
      attackFrames: z.number().int().nonnegative(),
      releaseFrames: z.number().int().nonnegative(),
      targetTracks: z.array(z.enum(["music", "ambience", "effects"])),
    })
    .nullable(),
  timelineFingerprint: z.string().min(1),
});

export const timelineVersionSchema = z.object({
  id: z.string().min(1),
  projectId: z.string().min(1),
  episodeId: z.string().min(1),
  schemaVersion: z.string(),
  revision: z.number().int().positive(),
  sourceCutRevision: z.number().int().positive(),
  name: z.string().min(1),
  snapshot: z.record(z.string(), z.unknown()),
});

export type Timeline = z.infer<typeof timelineSchema>;
export type TimelineVersion = z.infer<typeof timelineVersionSchema>;
