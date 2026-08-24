import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  creativeBriefCommandSchema,
  frozenSelectionSchema,
  mediaEligibilitySchema,
  runDetailSchema,
  sceneProjectionSchema,
} from "./contracts";
import {
  OwnerApiError,
  localOfflineSelection,
  queryKeys,
  workbenchApi,
} from "./api";
import {
  assetBibleAcceptCommandSchema,
  assetBibleImpactSchema,
  budgetConfirmationCommandSchema,
  budgetGateSchema,
  historicalRerunCommandSchema,
  operationRecoverySchema,
  runCancelCommandSchema,
  runInputSnapshotSchema,
  runStartCommandSchema,
  shotCardSchema,
  sourceMaterialVersionSchema,
  successorRunCommandSchema,
  textRegenerateCommandSchema,
} from "./owner-contracts";
import {
  episodeSliceKey,
  usePresentationStore,
  validateEpisodeSlice,
} from "./presentation-store";
import { createTraceparent } from "./trace-context";

const hash = "a".repeat(64);
const otherHash = "b".repeat(64);
const userUuid = "11111111-1111-4111-8111-111111111111";

const selection = localOfflineSelection("project-a");
const workflowSource = {
  workflowVersionId: "workflow-1",
  versionNumber: 1,
  contentHash: hash,
  definition: { nodes: [{ key: "text.generate" }] },
  scopeType: "project" as const,
  scopeIds: ["project-a"],
  bindingId: "binding-1",
  bindingRevision: 1,
  templateKey: "drama-mvp-a-default" as const,
  schemaVersion: "1.0.0" as const,
};

beforeEach(() => {
  localStorage.clear();
  usePresentationStore.setState({ slices: {}, diagnostics: {} });
  vi.unstubAllGlobals();
});

describe("owner contracts", () => {
  it("为每个 Web request 生成 canonical W3C traceparent", () => {
    expect(createTraceparent()).toMatch(/^00-[0-9a-f]{32}-[0-9a-f]{16}-01$/);
  });
  it("只接受 canonical CreativeBrief command 字段", () => {
    const payload = {
      creationMode: "original",
      subject: "潮汐",
      genre: "悬疑",
      audience: "成人",
      characterPremise: "灯塔守望者",
      style: "克制写实",
      episodeDurationSeconds: 60,
      episodeCount: 2,
      scenesPerEpisode: 2,
      shotsPerScene: 3,
      schemaVersion: "1.0.0",
      expectedRevision: 1,
      expectedBriefRevision: null,
    };
    expect(creativeBriefCommandSchema.parse(payload)).toEqual(payload);
    expect(() =>
      creativeBriefCommandSchema.parse({ ...payload, cueType: "legacy" }),
    ).toThrow();
    expect(() =>
      creativeBriefCommandSchema.parse({ ...payload, schema_version: "1.0.0" }),
    ).toThrow();
  });

  it("拒绝跨项目 storyboard 和非 canonical owner DTO", () => {
    const scene = {
      id: "scene-1",
      projectId: "project-a",
      episodeId: "episode-a",
      number: 1,
      title: "码头",
      schemaVersion: "1.0.0",
      revision: 1,
      status: "ready",
      specRef: null,
      shots: [],
    };
    expect(sceneProjectionSchema.parse(scene).projectId).toBe("project-a");
    expect(() =>
      sceneProjectionSchema.parse({ ...scene, project_id: "project-b" }),
    ).toThrow();
  });

  it("接受首个 AssetVersion revision 0 的 canonical media eligibility", () => {
    const eligibility = {
      candidateId: "candidate-1",
      candidateRevision: 1,
      assetVersionId: "version-1",
      assetVersionRevision: 0,
      assetVersionHash: hash,
      projectId: "project-a",
      episodeId: "episode-a",
      targetId: "shot-a",
      status: "accepted_current",
      timelineReady: true,
    };
    expect(mediaEligibilitySchema.parse(eligibility)).toEqual(eligibility);
  });

  it("Run detail safe summary 拒绝 secret、objectKey 与 raw payload", () => {
    const run = {
      id: "run-1",
      projectId: "project-a",
      schemaVersion: "1.0.0",
      revision: 1,
      status: "running",
      workflowVersionId: "workflow-1",
      sourceSnapshot: workflowSource,
      selectionSnapshot: selection,
      createdAt: "2026-08-23T00:00:00+00:00",
      updatedAt: "2026-08-23T00:00:01+00:00",
      elapsedSeconds: 1,
      allowedActions: ["cancel"],
      failure: null,
      nodes: [
        {
          id: "node-1",
          revision: 1,
          nodeKey: "text.generate",
          status: "running",
          logicalOperation: "text.generate:operation-1",
          scopeRefs: [],
          inputSummary: { count: 2 },
          outputSummary: {},
          failure: {},
          submissionState: "started",
        },
      ],
      recentEvents: [
        {
          id: "event-1",
          runId: "run-1",
          sequence: 1,
          eventType: "run.started",
          correlationId: "run-1",
          payload: { status: "running" },
          nodeRunId: null,
          schemaVersion: "1.0.0",
          revision: 1,
        },
      ],
    };
    expect(runDetailSchema.parse(run)).toEqual(
      expect.objectContaining({ status: "running", latestEventSequence: 1 }),
    );
    expect(() =>
      runDetailSchema.parse({
        ...run,
        nodes: [
          { ...run.nodes[0], inputSummary: { objectKey: "private/key" } },
        ],
      }),
    ).toThrow(/unsafe run summary field/i);
  });

  it("冻结 Local profile、Run create/successor 与历史 rerun 的精确身份", () => {
    expect(frozenSelectionSchema.parse(selection).adapterIdentity).toBe(
      "local_workspace",
    );
    const start = {
      workflowVersionId: "workflow-1",
      nodeKeys: ["text.generate"],
      scopeRefs: [],
      ownerRefs: [],
      idempotencyKey: "operation-1",
      routeDecisionId: "route-decision-1",
      expectedBindingRevision: 1,
      schemaVersion: "1.0.0",
    };
    expect(runStartCommandSchema.parse(start)).toEqual(start);
    expect(() =>
      runStartCommandSchema.parse({
        ...start,
        routeDecisionId: undefined,
      }),
    ).toThrow();
    expect(
      successorRunCommandSchema.parse({
        expectedPredecessorRevision: 2,
        reuseNodeIds: ["node-success"],
        schemaVersion: "1.0.0",
      }).reuseNodeIds,
    ).toEqual(["node-success"]);
    expect(
      historicalRerunCommandSchema.parse({
        expectedSnapshotRevision: 1,
        schemaVersion: "1.0.0",
      }).expectedSnapshotRevision,
    ).toBe(1);
    expect(
      operationRecoverySchema.parse({
        runId: "run-1",
        nodeRunId: "node-1",
        logicalOperation: "text.generate:operation-1",
        submissionState: "submission_unknown",
        correlationId: "correlation-1",
        revision: 2,
        schemaVersion: "1.0.0",
      }).submissionState,
    ).toBe("submission_unknown");
    expect(
      textRegenerateCommandSchema.parse({
        candidateId: "candidate-1",
        expectedBatchRevision: 2,
        expectedCandidateRevision: 1,
        payload: { subject: "潮汐" },
        sourceCandidateIds: ["source-candidate-1"],
        sourceHashes: [hash],
      }).sourceHashes,
    ).toEqual([hash]);
  });

  it.each(["queued", "running", "waiting_review"])(
    "%s Run 允许构造一次精确 cancel CAS",
    () => {
      expect(
        runCancelCommandSchema.parse({
          expectedRevision: 2,
          schemaVersion: "1.0.0",
        }),
      ).toEqual({ expectedRevision: 2, schemaVersion: "1.0.0" });
      expect(() =>
        runCancelCommandSchema.parse({
          expectedRevision: 0,
          schemaVersion: "1.0.0",
        }),
      ).toThrow();
    },
  );

  it("历史输入快照不接受 current alias、implicit upgrade 或缺失 owner hash", () => {
    const snapshot = {
      id: "snapshot-1",
      schemaVersion: "1.0.0",
      revision: 1,
      projectId: "project-a",
      runId: "run-1",
      workflowVersionId: "workflow-1",
      workflowContentHash: hash,
      scopeRefs: [],
      ownerRefs: [],
      selectionSnapshot: selection,
      sourceSnapshot: workflowSource,
      nodeInputs: [
        {
          nodeRunId: "node-1",
          nodeKey: "text.generate",
          logicalOperation: "text.generate:operation-1",
          scopeRefs: [],
        },
      ],
      runnable: true,
      diagnostic: null,
      createdAt: "2026-08-23T00:00:00+00:00",
    };
    expect(runInputSnapshotSchema.parse(snapshot).runId).toBe("run-1");
    expect(() =>
      runInputSnapshotSchema.parse({ ...snapshot, current: true }),
    ).toThrow();
    expect(() =>
      runInputSnapshotSchema.parse({
        ...snapshot,
        workflowContentHash: "current",
      }),
    ).toThrow();
  });

  it("SourceMaterial input mode 严格约束 AssetVersion handoff", () => {
    const inline = {
      id: "source-version-1",
      sourceMaterialId: "source-1",
      projectId: "project-a",
      materialType: "novel",
      inputMode: "inline_text",
      revision: 1,
      contentHash: hash,
      parseStatus: "parsed",
      validationStatus: "valid",
      assetVersionId: null,
      schemaVersion: "1.0.0",
    };
    expect(sourceMaterialVersionSchema.parse(inline).assetVersionId).toBeNull();
    expect(() =>
      sourceMaterialVersionSchema.parse({
        ...inline,
        assetVersionId: "asset-version-1",
      }),
    ).toThrow(/inline source/i);
    expect(() =>
      sourceMaterialVersionSchema.parse({
        ...inline,
        inputMode: "uploaded_file",
      }),
    ).toThrow(/requires AssetVersion/i);
  });

  it("费用确认绑定 operation/fingerprint/revision 和稳定 UUID", () => {
    const gate = {
      id: "gate-1",
      runId: "run-1",
      nodeRunId: "node-1",
      logicalOperation: "media.generate:image:operation-1",
      requestFingerprint: hash,
      operationKind: "image",
      batchSize: 4,
      costStatus: "unknown",
      estimatedCost: null,
      actualCost: null,
      currency: null,
      source: "provider_unconfigured",
      thresholdSnapshotId: null,
      thresholdRevision: null,
      confirmationId: null,
      status: "waiting_review",
      revision: 1,
      schemaVersion: "1.0.0",
    };
    expect(budgetGateSchema.parse(gate).costStatus).toBe("unknown");
    const confirmation = {
      logicalOperation: "media.generate:image:operation-1",
      requestFingerprint: hash,
      confirmationId: "confirmation-1",
      userUuid,
      expectedGateRevision: 2,
      schemaVersion: "1.0.0",
    };
    expect(budgetConfirmationCommandSchema.parse(confirmation)).toEqual(
      confirmation,
    );
    expect(() =>
      budgetConfirmationCommandSchema.parse({
        ...confirmation,
        requestFingerprint: otherHash.slice(1),
      }),
    ).toThrow();
  });

  it("ShotCard 保留分组 owner facts，拒绝未知合成 current 字段", () => {
    const group = {
      ownerId: "owner-1",
      revision: 1,
      contentHash: hash,
      status: "ready",
    };
    const card = {
      projectId: "project-a",
      episodeId: "episode-a",
      sceneId: "scene-a",
      shotId: "shot-a",
      scene: group,
      shotSpec: { ...group, ownerId: "shot-spec-1" },
      continuity: { ...group, ownerId: "snapshot-1" },
      image: null,
      video: null,
      generation: null,
      review: null,
      derivative: null,
      durationMs: 3000,
      promptSummary: "雨夜码头，中景",
      modelId: null,
      modelRevision: null,
      cost: { value: null, currency: null, status: "unknown", source: "owner" },
    };
    expect(shotCardSchema.parse(card).continuity.ownerId).toBe("snapshot-1");
    expect(() => shotCardSchema.parse({ ...card, current: true })).toThrow();
  });

  it("AssetBible impact accept 强制完整 set hash 与 exact CAS", () => {
    const target = {
      targetType: "shot",
      targetId: "shot-a",
      targetRevision: 2,
      reason: "character look changed",
      snapshotId: "snapshot-1",
      snapshotHash: hash,
      suggestedAction: "review",
    };
    const impact = {
      id: "analysis-1",
      projectId: "project-a",
      entryId: "entry-1",
      entryRevision: 2,
      assetBibleRevision: 3,
      targets: [target],
      targetSetHash: otherHash,
      complete: true,
      diagnostic: null,
      revision: 1,
      schemaVersion: "1.0.0",
    };
    expect(assetBibleImpactSchema.parse(impact).targets).toHaveLength(1);
    expect(
      assetBibleAcceptCommandSchema.parse({
        analysisId: impact.id,
        expectedAnalysisRevision: impact.revision,
        expectedEntryRevision: impact.entryRevision,
        expectedAssetBibleRevision: impact.assetBibleRevision,
        targetSetHash: impact.targetSetHash,
        targets: impact.targets,
        actorUuid: userUuid,
        schemaVersion: "1.0.0",
      }).targetSetHash,
    ).toBe(otherHash);
    expect(() =>
      assetBibleImpactSchema.parse({ ...impact, targets: undefined }),
    ).toThrow();
  });
});

describe("workbench API and presentation state", () => {
  it("Query keys 不含 Draft identity", () => {
    const keys = [
      queryKeys.project("p"),
      queryKeys.storyboard("p", "e"),
      queryKeys.workflow("p"),
    ].flat();
    expect(keys.join(":")).not.toMatch(/draft/i);
  });

  it("展示 store 不接受 message、Run、candidate 或 AssetVersion owner 正文", () => {
    const unsafePatch = {
      selectedShotId: "shot-a",
      messages: [{ id: "message-1" }],
      run: { id: "run-1" },
    } as unknown as Parameters<
      ReturnType<typeof usePresentationStore.getState>["patchSlice"]
    >[2];
    usePresentationStore.getState().patchSlice("p", "episode-a", unsafePatch);
    const stored = usePresentationStore
      .getState()
      .getSlice("p", "episode-a") as unknown as Record<string, unknown>;
    expect(stored.messages).toBeUndefined();
    expect(stored.run).toBeUndefined();
  });

  it("owner DTO 错误在 command 前返回明确 contract diagnostic", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true,
      json: async () => [{ id: "p", schema_version: "1.0.0" }],
    });
    vi.stubGlobal("fetch", fetchMock);
    await expect(workbenchApi.listProjects()).rejects.toEqual(
      expect.objectContaining<Partial<OwnerApiError>>({
        code: "owner_contract_invalid",
      }),
    );
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("按 projectId + episodeId 隔离并清除 foreign selection", () => {
    usePresentationStore.getState().patchSlice("p", "episode-a", {
      collapsedSceneIds: ["scene-a"],
      selectedShotId: "shot-a",
      activeSessionId: "session-a",
    });
    usePresentationStore.getState().patchSlice("p", "episode-b", {
      collapsedSceneIds: ["scene-b"],
      selectedShotId: "shot-b",
    });
    const result = validateEpisodeSlice(
      usePresentationStore.getState().getSlice("p", "episode-a"),
      {
        projectId: "p",
        episodeId: "episode-a",
        shotIds: new Set(),
        assetIds: new Set(),
        sessionIds: new Set(),
      },
    );
    expect(result.slice.selectedShotId).toBeNull();
    expect(result.slice.activeSessionId).toBeNull();
    expect(result.diagnostics).toContain("selected_shot_scope_invalid");
    expect(
      usePresentationStore.getState().slices[episodeSliceKey("p", "episode-b")]
        .collapsedSceneIds,
    ).toEqual(["scene-b"]);
  });
});
