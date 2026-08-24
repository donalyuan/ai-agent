import { describe, expect, it } from "vitest";
import { timelineSchema } from "./contracts";

const base = {
  id: "cut-1",
  projectId: "project-1",
  episodeId: "episode-1",
  schemaVersion: "1.0.0",
  revision: 1,
  fps: 30,
  clips: [],
  soundCues: [],
  captions: [],
  ducking: null,
  timelineFingerprint: "fingerprint-1",
};

describe("timeline owner contract", () => {
  it("requires canonical 30fps and separates sound cue tracks", () => {
    expect(timelineSchema.parse(base).fps).toBe(30);
    expect(timelineSchema.safeParse({ ...base, fps: 24 }).success).toBe(false);
  });

  it("rejects automation/keyframe-shaped cue payloads", () => {
    const result = timelineSchema.safeParse({
      ...base,
      soundCues: [
        {
          id: "cue-1",
          track: "music",
          assetVersionId: "audio-1",
          assetVersionRevision: 0,
          assetVersionHash: "a".repeat(64),
          startFrame: 0,
          durationFrames: 30,
          trigger: "manual",
          priority: 1,
          continuityRefs: [],
          gainDb: 0,
          mute: false,
          solo: false,
          fadeInFrames: 0,
          fadeOutFrames: 0,
          keyframes: [{ frame: 1, gainDb: -2 }],
        },
      ],
    });
    expect(result.success).toBe(false);
  });

  it("accepts the canonical structured SoundCue owner projection", () => {
    const cue = {
      id: "cue-1",
      track: "effects",
      assetVersionId: "audio-1",
      assetVersionRevision: 2,
      assetVersionHash: "a".repeat(64),
      startFrame: 30,
      durationFrames: 15,
      trigger: "shot_end",
      triggerRef: {
        ownerType: "shot",
        id: "shot-1",
        revision: 3,
        startFrame: 10,
        offsetFrames: 20,
      },
      priority: 50,
      continuityRefs: [
        {
          ownerType: "shot_spec",
          id: "spec-1",
          revision: 4,
          hash: "b".repeat(64),
        },
      ],
      gainDb: -3,
      mute: false,
      solo: false,
      fadeInFrames: 2,
      fadeOutFrames: 2,
      authorizationStatus: "authorized",
      licenseStatus: "approved",
    } as const;

    expect(
      timelineSchema.parse({ ...base, soundCues: [cue] }).soundCues[0],
    ).toEqual(cue);
    expect(
      timelineSchema.safeParse({
        ...base,
        soundCues: [{ ...cue, trigger: "shot", triggerRef: "shot-1" }],
      }).success,
    ).toBe(false);
    expect(
      timelineSchema.safeParse({
        ...base,
        soundCues: [{ ...cue, licenseStatus: "unknown" }],
      }).success,
    ).toBe(false);
  });
});
