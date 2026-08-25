import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowRight,
  Check,
  CircleAlert,
  GitCompareArrows,
  LoaderCircle,
  MessageSquareText,
  Play,
  RefreshCw,
  RotateCcw,
  Send,
  ShieldCheck,
  X,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router";
import {
  assetEditReviewApi,
  assetEditReviewQueryKeys,
  hashUserInput,
} from "../asset-edit-review/api";
import {
  timelineReplacementHandoffSchema,
  unsupportedMediaEditInput,
  type ReviewCandidate,
  type ReviewPlan,
} from "../asset-edit-review/contracts";
import {
  EMPTY_REVIEW_SLICE,
  reviewSliceKey,
  useAssetEditReviewStore,
} from "../asset-edit-review/store";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "../shared/ui";
import { OwnerApiError } from "../workbench/api";

type Props = { projectId: string };
type ConfirmAction =
  | { kind: "execute"; plan: ReviewPlan }
  | {
      kind: "accept" | "reject" | "retake";
      plan: ReviewPlan;
      candidate: ReviewCandidate;
    };

const correlation = (prefix: string) =>
  `${prefix}:${typeof crypto.randomUUID === "function" ? crypto.randomUUID() : Date.now()}`;

const shortHash = (value: string) => `sha256 ${value.slice(0, 12)}...`;

function FactVersion({
  label,
  version,
}: {
  label: string;
  version: ReviewPlan["base"];
}) {
  return (
    <div className="grid gap-1 rounded-md border border-border bg-muted/40 p-3 text-sm">
      <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
        {label}
      </span>
      <strong>
        {version.kind === "image" ? "完整图片版本" : "完整视频版本"}
      </strong>
      <span className="font-mono text-xs text-muted-foreground">
        {version.assetVersionId} · rev {version.revision}
      </span>
      <span className="font-mono text-xs text-muted-foreground">
        {shortHash(version.contentHash)}
      </span>
      <span>{version.mimeType}</span>
    </div>
  );
}

function ContinuityGate({
  continuity,
}: {
  continuity: ReviewPlan["continuity"];
}) {
  const ready = continuity.status === "accepted_current";
  return (
    <div
      className={`flex items-start gap-2 rounded-md border p-3 text-sm ${ready ? "border-success/30 bg-success/10 text-success" : "border-warning/30 bg-warning/10 text-warning-foreground"}`}
    >
      {ready ? <ShieldCheck size={17} /> : <CircleAlert size={17} />}
      <div>
        <strong>{ready ? "Continuity 已对齐" : "continuity_stale"}</strong>
        <span>
          {continuity.snapshot
            ? `${continuity.snapshot.id} · rev ${continuity.snapshot.revision} · ${shortHash(continuity.snapshot.contentHash)}`
            : "未绑定 accepted ResolvedContinuitySnapshot"}
        </span>
        {continuity.tasks.map((task) => (
          <span
            key={task.id}
            className="font-mono text-xs text-muted-foreground"
          >
            task {task.id} · {task.status} · rev {task.revision}
          </span>
        ))}
      </div>
    </div>
  );
}

function ErrorState({ error }: { error: unknown }) {
  const code = error instanceof OwnerApiError ? error.code : "network_failure";
  const message =
    error instanceof Error ? error.message : "owner projection unavailable";
  return (
    <div
      className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"
      role="alert"
    >
      <CircleAlert size={17} />
      <span>
        <strong>{code}</strong>
        {message}
      </span>
    </div>
  );
}

export function AssetEditReviewPage({ projectId }: Props) {
  const [params, setParams] = useSearchParams();
  const episodeId = params.get("episodeId") ?? "";
  const requestedSessionId = params.get("sessionId") ?? "";
  const requestedVersionId = params.get("assetVersionId") ?? "";
  const unsupported = unsupportedMediaEditInput(params);
  const queryClient = useQueryClient();
  const stored = useAssetEditReviewStore(
    (state) =>
      state.slices[reviewSliceKey(projectId, episodeId)] ?? EMPTY_REVIEW_SLICE,
  );
  const restoreOwnerSession = useAssetEditReviewStore(
    (state) => state.restoreOwnerSession,
  );
  const clearSlice = useAssetEditReviewStore((state) => state.clearSlice);
  const [message, setMessage] = useState("");
  const [referenceInput, setReferenceInput] = useState("");
  const [selectedReferences, setSelectedReferences] = useState<
    ReviewPlan["references"]
  >([]);
  const [planInstruction, setPlanInstruction] = useState("");
  const [selectedTurnId, setSelectedTurnId] = useState("");
  const [selectedPlanId, setSelectedPlanId] = useState("");
  const [confirm, setConfirm] = useState<ConfirmAction | null>(null);
  const [staleRevision, setStaleRevision] = useState(false);

  const sessions = useQuery({
    queryKey: assetEditReviewQueryKeys.sessions(projectId, episodeId),
    queryFn: () => assetEditReviewApi.listSessions(projectId, episodeId),
    enabled: Boolean(projectId && episodeId && !unsupported),
  });
  const selectedVersion = useQuery({
    queryKey: assetEditReviewQueryKeys.version(projectId, requestedVersionId),
    queryFn: () =>
      assetEditReviewApi.getAssetVersion(projectId, requestedVersionId),
    enabled: Boolean(requestedVersionId && !requestedSessionId && !unsupported),
  });
  const requestedSessionInvalid = Boolean(
    sessions.data &&
    requestedSessionId &&
    !sessions.data.items.some((item) => item.id === requestedSessionId),
  );
  const activeSessionId = requestedSessionId
    ? sessions.data?.items.some((item) => item.id === requestedSessionId)
      ? requestedSessionId
      : ""
    : (stored.activeSessionId &&
      sessions.data?.items.some((item) => item.id === stored.activeSessionId)
        ? stored.activeSessionId
        : sessions.data?.items[0]?.id) || "";
  const session = useQuery({
    queryKey: assetEditReviewQueryKeys.session(projectId, activeSessionId),
    queryFn: () => assetEditReviewApi.getSession(projectId, activeSessionId),
    enabled: Boolean(activeSessionId && !unsupported),
  });

  useEffect(() => {
    if (session.data) restoreOwnerSession(session.data);
  }, [restoreOwnerSession, session.data]);
  useEffect(() => {
    if (
      sessions.data &&
      requestedSessionId &&
      !sessions.data.items.some((item) => item.id === requestedSessionId)
    ) {
      clearSlice(projectId, episodeId, "active_session_scope_invalid");
    }
  }, [clearSlice, episodeId, projectId, requestedSessionId, sessions.data]);

  const invalidateSession = async () => {
    await queryClient.invalidateQueries({
      queryKey: assetEditReviewQueryKeys.session(projectId, activeSessionId),
    });
  };
  const recoverRevisionConflict = async (error: unknown) => {
    if (error instanceof OwnerApiError && error.status === 409) {
      setStaleRevision(true);
      setConfirm(null);
      await queryClient.refetchQueries({
        queryKey: assetEditReviewQueryKeys.session(projectId, activeSessionId),
      });
      await queryClient.refetchQueries({
        queryKey: assetEditReviewQueryKeys.sessions(projectId, episodeId),
      });
      setStaleRevision(false);
    }
  };
  const sendMessage = useMutation({
    mutationFn: async () => {
      if (!session.data || !message.trim()) throw new Error("消息不能为空");
      return assetEditReviewApi.appendMessage(
        projectId,
        session.data.id,
        await hashUserInput(message.trim()),
        correlation("review-message"),
        session.data.conversation.revision,
      );
    },
    onSuccess: async () => {
      setMessage("");
      await invalidateSession();
    },
  });
  const addReference = useMutation({
    mutationFn: async () => {
      const id = referenceInput.trim();
      if (!id) throw new Error("AssetVersion ID 不能为空");
      const value = await assetEditReviewApi.getAssetVersion(projectId, id);
      if (
        value.assetVersionId === selectedVersion.data?.assetVersionId ||
        selectedReferences.some(
          (item) => item.assetVersionId === value.assetVersionId,
        )
      ) {
        throw new OwnerApiError(
          422,
          "duplicate_reference",
          "引用不能重复或等于 primary",
        );
      }
      return value;
    },
    onSuccess: (value) => {
      setSelectedReferences((current) => [...current, value]);
      setReferenceInput("");
    },
  });
  const createSession = useMutation({
    mutationFn: async () => {
      const version = selectedVersion.data;
      const targetId = params.get("shotId") ?? "";
      const snapshotId = params.get("continuitySnapshotId") ?? "";
      const snapshotRevision = Number(params.get("continuitySnapshotRevision"));
      const snapshotHash = params.get("continuitySnapshotHash") ?? "";
      if (
        !version ||
        version.revision !== Number(params.get("assetVersionRevision")) ||
        version.contentHash !== params.get("assetVersionHash") ||
        !targetId ||
        !snapshotId ||
        !Number.isInteger(snapshotRevision) ||
        snapshotRevision < 1 ||
        !/^[0-9a-f]{64}$/.test(snapshotHash)
      ) {
        throw new OwnerApiError(
          409,
          "base_version_conflict",
          "AssetVersion 或 continuity identity 与 owner 不匹配",
        );
      }
      return assetEditReviewApi.createSession(
        projectId,
        episodeId,
        targetId,
        version,
        selectedReferences,
        {
          id: snapshotId,
          revision: snapshotRevision,
          contentHash: snapshotHash,
        },
      );
    },
    onSuccess: async (result) => {
      const next = new URLSearchParams(params);
      next.set("sessionId", result.id);
      setParams(next, { replace: true });
      await queryClient.invalidateQueries({
        queryKey: assetEditReviewQueryKeys.sessions(projectId, episodeId),
      });
    },
  });
  const generatePlan = useMutation({
    mutationFn: async () => {
      const current = session.data;
      const turn = current?.conversation.turns.find(
        (item) => item.id === selectedTurnId,
      );
      if (!current || !turn) throw new Error("请选择已完成 Agent turn");
      return assetEditReviewApi.generatePlan({
        schemaVersion: "1.0.0",
        sessionId: current.id,
        conversationId: current.conversation.id,
        turnId: turn.id,
        turnStatus: turn.status as "complete",
        episodeId: current.episodeId,
        targetId: current.targetId,
        kind: current.selection.primary.kind,
        base: current.selection.primary,
        references: current.selection.references,
        instruction: planInstruction,
        runId: params.get("runId") ?? "",
        nodeRunId: params.get("nodeRunId") ?? "",
        logicalOperation: correlation(`plan:${turn.id}`),
        correlationId: correlation("review-plan"),
      });
    },
    onSuccess: async () => {
      setStaleRevision(false);
      await invalidateSession();
    },
    onError: recoverRevisionConflict,
  });
  const execute = useMutation({
    mutationFn: async (plan: ReviewPlan) => {
      const runId = params.get("runId") ?? "";
      const nodeRunId = params.get("nodeRunId") ?? "";
      const logicalOperation = `asset-edit:${plan.id}:execute`;
      return assetEditReviewApi.executePlan(projectId, plan.id, {
        planRevision: plan.revision,
        runId,
        nodeRunId,
        logicalOperation,
        correlationId: correlation("review-execute"),
        requestFingerprint: await hashUserInput(
          `${plan.id}:${plan.revision}:${runId}:${nodeRunId}:${logicalOperation}`,
        ),
      });
    },
    onSuccess: async () => {
      setStaleRevision(false);
      await invalidateSession();
    },
    onError: recoverRevisionConflict,
  });
  const review = useMutation({
    mutationFn: async ({
      kind,
      plan,
      candidate,
    }: Exclude<ConfirmAction, { kind: "execute" }>) => {
      const expectedTargetRevision = Number(
        candidate.provenance.expectedTargetRevision,
      );
      if (kind === "accept") {
        return assetEditReviewApi.reviewCandidate(projectId, candidate.id, {
          action: "accept",
          expectedRevision: candidate.revision,
          expectedBaseVersionId: plan.base.assetVersionId,
          scope: [plan.targetId],
          candidateFacts: {
            candidateId: candidate.id,
            projectId: candidate.projectId,
            episodeId: candidate.episodeId,
            targetId: candidate.targetId,
            assetVersionId: candidate.assetVersion.assetVersionId,
            assetVersionRevision: candidate.assetVersion.revision,
            assetVersionHash: candidate.assetVersion.contentHash,
            expectedTargetRevision,
          },
          references: [
            {
              referenceId: plan.targetId,
              expectedRevision: expectedTargetRevision,
            },
          ],
        });
      }
      return assetEditReviewApi.reviewCandidate(projectId, candidate.id, {
        action: kind,
        expectedRevision: candidate.revision,
        expectedBaseVersionId: plan.base.assetVersionId,
        scope: [plan.targetId],
        logicalOperation:
          kind === "retake" ? correlation(`retake:${candidate.id}`) : undefined,
      });
    },
    onSuccess: async () => {
      setStaleRevision(false);
      await invalidateSession();
    },
    onError: recoverRevisionConflict,
  });

  const mutationError =
    addReference.error ||
    createSession.error ||
    sendMessage.error ||
    generatePlan.error ||
    execute.error ||
    review.error;
  const currentPlan =
    session.data?.plans.find((plan) => plan.id === selectedPlanId) ??
    session.data?.plans[0];
  const completedTurns = useMemo(
    () =>
      session.data?.conversation.turns.filter(
        (item) => item.status === "complete",
      ) ?? [],
    [session.data],
  );

  if (unsupported) {
    return (
      <section className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 sm:p-6 lg:p-8 gap-5">
        <div
          className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive unsupported"
          role="alert"
        >
          <CircleAlert size={20} />
          <span>
            <strong>unsupported_feature</strong>
            MVP-A 只审核完整 image/video
            AssetVersion；mask、选区、局部区域、图层和时间范围不会被静默删除或提交。
          </span>
        </div>
      </section>
    );
  }

  return (
    <section className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 sm:p-6 lg:p-8 gap-5">
      <header className="flex flex-col justify-between gap-4 sm:flex-row sm:items-start">
        <div>
          <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            ASSET EDIT / OWNER REVIEW
          </span>
          <h2>候选审片台</h2>
          <p>对话只形成待审核计划；执行、接受与重拍始终由独立确认触发。</p>
        </div>
        <div className="rounded-md border border-border bg-muted px-3 py-2 text-sm">
          <span className="project-led" />
          <strong>Mock + Local offline</strong>
          <span className="font-mono text-xs text-muted-foreground">
            adapter: local_workspace
          </span>
        </div>
      </header>

      {!episodeId && (
        <div className="rounded-lg border border-dashed border-border bg-card p-8 text-center text-sm text-muted-foreground">
          <MessageSquareText size={24} />
          <strong>请选择 Episode</strong>
          <span>
            从工作台 Shot 的“审核”入口进入，Review 不会借用其他 Episode
            session。
          </span>
        </div>
      )}
      {(sessions.isPending || session.isPending) && episodeId && (
        <div className="flex items-center gap-2 rounded-md border border-border bg-muted p-3 text-sm">
          <LoaderCircle className="animate-spin" size={18} /> 正在读取 AssetEdit
          owner...
        </div>
      )}
      {(sessions.error || session.error) && (
        <ErrorState error={sessions.error || session.error} />
      )}
      {requestedSessionInvalid && (
        <div
          className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"
          role="alert"
        >
          <CircleAlert size={17} />
          <span>
            <strong>active_session_scope_invalid</strong>
            指定 session 不属于当前 Episode；已清除恢复状态且不会借用其他
            Episode session。
          </span>
        </div>
      )}
      {!sessions.isPending &&
        sessions.data?.items.length === 0 &&
        !requestedSessionId &&
        (requestedVersionId ? (
          <div className="rounded-lg border border-border bg-card p-5 shadow-sm">
            <div>
              <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                PRIMARY SELECTION
              </span>
              <h3>从完整 AssetVersion 建立审核会话</h3>
              <p>
                owner 会再次核对 project、revision、hash、MIME 与 accepted
                continuity snapshot。
              </p>
            </div>
            {selectedVersion.isPending && (
              <LoaderCircle className="animate-spin" size={18} />
            )}
            {selectedVersion.error && (
              <ErrorState error={selectedVersion.error} />
            )}
            {selectedVersion.data && (
              <>
                <FactVersion
                  label="PRIMARY"
                  version={{ ...selectedVersion.data } as ReviewPlan["base"]}
                />
                <div className="mt-4 grid gap-2 rounded-md border border-border p-3">
                  <label htmlFor="review-reference-id">
                    显式 reference AssetVersion ID
                  </label>
                  <div>
                    <input
                      id="review-reference-id"
                      value={referenceInput}
                      onChange={(event) =>
                        setReferenceInput(event.target.value)
                      }
                    />
                    <button
                      type="button"
                      disabled={
                        !referenceInput.trim() || addReference.isPending
                      }
                      onClick={() => addReference.mutate()}
                    >
                      添加引用
                    </button>
                  </div>
                  {selectedReferences.map((reference) => (
                    <div
                      className="flex items-center justify-between gap-2 rounded-md bg-muted px-2 py-1"
                      key={reference.assetVersionId}
                    >
                      <span className="font-mono text-xs text-muted-foreground">
                        {reference.assetVersionId} · rev {reference.revision} ·{" "}
                        {shortHash(reference.contentHash)}
                      </span>
                      <button
                        type="button"
                        title="移除引用"
                        aria-label={`移除引用 ${reference.assetVersionId}`}
                        onClick={() =>
                          setSelectedReferences((current) =>
                            current.filter(
                              (item) =>
                                item.assetVersionId !==
                                reference.assetVersionId,
                            ),
                          )
                        }
                      >
                        <X size={13} />
                      </button>
                    </div>
                  ))}
                </div>
                <button
                  className="mt-4 inline-flex h-10 items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                  disabled={createSession.isPending}
                  onClick={() => createSession.mutate()}
                >
                  <MessageSquareText size={15} /> 创建审核会话
                </button>
              </>
            )}
          </div>
        ) : (
          <div className="rounded-lg border border-dashed border-border bg-card p-8 text-center text-sm text-muted-foreground">
            <GitCompareArrows size={24} />
            <strong>本集没有 AssetEditSession</strong>
            <span>
              先从完整 image/video AssetVersion 创建明确
              session；打开页面不会生成计划或调用 Provider。
            </span>
          </div>
        ))}

      {session.data && (
        <>
          <div className="flex flex-wrap gap-3 rounded-md bg-muted p-3 text-sm">
            <span>
              Episode <b>{session.data.episodeId}</b>
            </span>
            <span>
              Target <b>{session.data.targetId}</b>
            </span>
            <span className="font-mono text-xs text-muted-foreground">
              Session {session.data.id} · rev {session.data.revision}
            </span>
          </div>
          <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_minmax(22rem,0.8fr)]">
            <section
              className="rounded-lg border border-border bg-card p-5 shadow-sm"
              aria-label="Agent 对话"
            >
              <div className="flex items-start gap-2">
                <MessageSquareText size={17} />
                <div>
                  <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                    CONVERSATION
                  </span>
                  <h3>会话轮次</h3>
                </div>
              </div>
              <div className="mt-4 grid max-h-96 gap-3 overflow-y-auto pr-1">
                {session.data.conversation.messages.map((item) => (
                  <div
                    className={`grid gap-1 rounded-md border p-3 text-sm ${item.role === "user" ? "border-primary/30 bg-primary/5" : "border-border bg-muted/40"}`}
                    key={item.id}
                  >
                    <span>{item.role === "user" ? "你" : "Agent"}</span>
                    <div>
                      <strong>{item.status}</strong>
                      <small className="font-mono text-xs text-muted-foreground">
                        #{item.sequence} · {shortHash(item.contentHash)}
                      </small>
                      <small className="font-mono text-xs text-muted-foreground">
                        {item.correlationId}
                      </small>
                    </div>
                  </div>
                ))}
              </div>
              <label className="mt-4 grid gap-1 text-sm">
                <span>追加用户消息</span>
                <textarea
                  value={message}
                  onChange={(event) => setMessage(event.target.value)}
                  placeholder="描述希望调整的完整画面或视频版本"
                />
              </label>
              <button
                className="mt-3 inline-flex h-10 items-center justify-center gap-2 rounded-md border border-border bg-background px-4 text-sm font-semibold text-foreground hover:bg-accent disabled:pointer-events-none disabled:opacity-50"
                disabled={!message.trim() || sendMessage.isPending}
                onClick={() => sendMessage.mutate()}
              >
                <Send size={15} /> 发送消息
              </button>
              <div className="mt-4 grid gap-3 rounded-md border border-border p-3">
                <label>
                  <span>已完成 Agent turn</span>
                  <select
                    value={selectedTurnId}
                    onChange={(event) => setSelectedTurnId(event.target.value)}
                  >
                    <option value="">选择轮次</option>
                    {completedTurns.map((turn) => (
                      <option value={turn.id} key={turn.id}>
                        #{turn.sequence} · {turn.id}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  <span>计划指令</span>
                  <textarea
                    value={planInstruction}
                    onChange={(event) => setPlanInstruction(event.target.value)}
                  />
                </label>
                <button
                  className="inline-flex h-10 items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                  disabled={
                    !selectedTurnId ||
                    !planInstruction.trim() ||
                    session.data.continuity.status !== "accepted_current" ||
                    !params.get("runId") ||
                    !params.get("nodeRunId") ||
                    generatePlan.isPending
                  }
                  onClick={() => generatePlan.mutate()}
                >
                  <RefreshCw size={15} /> 从本轮生成编辑计划
                </button>
                {(!params.get("runId") || !params.get("nodeRunId")) && (
                  <small className="text-sm text-warning-foreground">
                    缺少 runId / nodeRunId，只读恢复可用，计划生成保持禁用。
                  </small>
                )}
              </div>
            </section>

            <section
              className="rounded-lg border border-border bg-card p-5 shadow-sm"
              aria-label="计划与候选比较"
            >
              {!currentPlan && (
                <div className="rounded-lg border border-dashed border-border bg-card p-8 text-center text-sm text-muted-foreground">
                  <GitCompareArrows size={24} />
                  <strong>尚无待审核计划</strong>
                  <span>完成 Agent reply 后，明确选择 turn 生成计划。</span>
                </div>
              )}
              {currentPlan && (
                <>
                  {session.data.plans.length > 1 && (
                    <label className="grid gap-1 text-sm">
                      <span>审核计划</span>
                      <select
                        aria-label="审核计划"
                        value={currentPlan.id}
                        onChange={(event) =>
                          setSelectedPlanId(event.target.value)
                        }
                      >
                        {session.data.plans.map((plan) => (
                          <option value={plan.id} key={plan.id}>
                            {plan.instruction} · {plan.status}
                          </option>
                        ))}
                      </select>
                    </label>
                  )}
                  <div className="flex flex-wrap items-start justify-between gap-3">
                    <div>
                      <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                        ASSET EDIT PLAN
                      </span>
                      <h3>{currentPlan.instruction}</h3>
                      <span className="font-mono text-xs text-muted-foreground">
                        {currentPlan.id} · rev {currentPlan.revision}
                      </span>
                    </div>
                    <span className="rounded-full bg-muted px-2 py-1 text-xs font-semibold text-muted-foreground">
                      {currentPlan.status}
                    </span>
                  </div>
                  <ContinuityGate continuity={currentPlan.continuity} />
                  <div className="mt-4 grid gap-3 sm:grid-cols-3">
                    <div>
                      <span>费用</span>
                      <strong>{currentPlan.cost.status}</strong>
                      <small>{currentPlan.cost.source}</small>
                    </div>
                    <div>
                      <span>影响</span>
                      <strong>{currentPlan.impact.status}</strong>
                      <small>
                        {currentPlan.impact.reasons.join(" / ") ||
                          "无 stale target"}
                      </small>
                    </div>
                    <div>
                      <span>引用范围</span>
                      <strong>{currentPlan.targetId}</strong>
                      <small>exact reference only</small>
                    </div>
                  </div>
                  <div className="mt-4 grid gap-3 md:grid-cols-2">
                    <FactVersion label="BASE" version={currentPlan.base} />
                    {currentPlan.candidates[0] ? (
                      <FactVersion
                        label="RESULT"
                        version={currentPlan.candidates[0].assetVersion}
                      />
                    ) : (
                      <div className="grid gap-1 rounded-md border border-dashed border-border p-3 text-sm text-muted-foreground">
                        <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                          RESULT
                        </span>
                        <strong>执行后等待候选</strong>
                      </div>
                    )}
                  </div>
                  <button
                    className="mt-4 inline-flex h-10 items-center justify-center gap-2 rounded-md bg-primary px-4 text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
                    disabled={
                      currentPlan.status !== "pending_review" ||
                      currentPlan.impact.status !== "clear" ||
                      currentPlan.continuity.status !== "accepted_current" ||
                      !params.get("runId") ||
                      !params.get("nodeRunId")
                    }
                    onClick={() =>
                      setConfirm({ kind: "execute", plan: currentPlan })
                    }
                  >
                    <Play size={15} /> 执行计划
                  </button>

                  {currentPlan.candidates.map((candidate) => {
                    const targetRevision = Number(
                      candidate.provenance.expectedTargetRevision,
                    );
                    const canAccept =
                      !staleRevision &&
                      candidate.status === "pending_review" &&
                      Number.isInteger(targetRevision) &&
                      targetRevision > 0 &&
                      currentPlan.status === "pending_review" &&
                      currentPlan.impact.status === "clear" &&
                      currentPlan.continuity.status === "accepted_current";
                    const handoff =
                      candidate.assetVersion.kind === "video"
                        ? timelineReplacementHandoffSchema.safeParse({
                            schemaVersion: "1.0.0",
                            projectId: candidate.projectId,
                            episodeId: candidate.episodeId,
                            shotId: candidate.targetId,
                            candidateId: candidate.id,
                            takeId: candidate.provenance.takeId,
                            assetVersionId:
                              candidate.assetVersion.assetVersionId,
                            assetVersionRevision:
                              candidate.assetVersion.revision,
                            assetVersionHash:
                              candidate.assetVersion.contentHash,
                            derivativeFingerprint:
                              candidate.provenance.derivativeFingerprint,
                            acceptedCurrent:
                              candidate.provenance.acceptedCurrent,
                            derivativeStatus:
                              candidate.provenance.derivativeStatus,
                          })
                        : null;
                    return (
                      <div
                        className="mt-3 flex flex-wrap gap-2"
                        key={candidate.id}
                      >
                        <div>
                          <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                            {candidate.assetVersion.kind.toUpperCase()}{" "}
                            CANDIDATE
                          </span>
                          <strong>
                            {candidate.id} · {candidate.status}
                          </strong>
                          <small>
                            Provider{" "}
                            {String(
                              candidate.provenance.providerStatus ?? "unknown",
                            )}{" "}
                            → candidate → human review → scenes CAS
                          </small>
                          <small>
                            derivative:{" "}
                            {String(
                              candidate.provenance.derivativeStatus ??
                                "unknown",
                            )}
                            （只影响 Timeline/preview/export）
                          </small>
                        </div>
                        <div className="flex flex-wrap gap-2">
                          <button
                            disabled={
                              staleRevision ||
                              currentPlan.status !== "pending_review" ||
                              candidate.status !== "pending_review"
                            }
                            onClick={() =>
                              setConfirm({
                                kind: "reject",
                                plan: currentPlan,
                                candidate,
                              })
                            }
                          >
                            <X size={15} /> 拒绝
                          </button>
                          <button
                            disabled={
                              staleRevision ||
                              currentPlan.status !== "pending_review" ||
                              candidate.status !== "pending_review"
                            }
                            onClick={() =>
                              setConfirm({
                                kind: "retake",
                                plan: currentPlan,
                                candidate,
                              })
                            }
                          >
                            <RotateCcw size={15} /> 重拍
                          </button>
                          <button
                            className="primary"
                            disabled={!canAccept}
                            onClick={() =>
                              setConfirm({
                                kind: "accept",
                                plan: currentPlan,
                                candidate,
                              })
                            }
                          >
                            <Check size={15} /> 接受候选
                          </button>
                          {handoff?.success ? (
                            <Link
                              className="inline-flex items-center gap-1 text-sm font-medium text-primary hover:underline"
                              to={`/projects/${handoff.data.projectId}/episodes/${handoff.data.episodeId}/timeline?handoff=${encodeURIComponent(JSON.stringify(handoff.data))}`}
                            >
                              进入 Timeline 替换确认 <ArrowRight size={14} />
                            </Link>
                          ) : (
                            <span className="text-sm text-warning-foreground">
                              Timeline handoff 等待 accepted-current + matching
                              derivative ready
                            </span>
                          )}
                        </div>
                      </div>
                    );
                  })}
                </>
              )}
            </section>
          </div>
          {mutationError && <ErrorState error={mutationError} />}
        </>
      )}

      <Dialog
        open={Boolean(confirm)}
        onOpenChange={(open) => !open && setConfirm(null)}
      >
        <DialogContent
          className="max-w-lg"
          aria-describedby="review-confirm-description"
        >
          <DialogTitle>
            {confirm?.kind === "execute"
              ? "确认执行编辑计划"
              : confirm?.kind === "accept"
                ? "确认接受候选"
                : confirm?.kind === "reject"
                  ? "确认拒绝候选"
                  : "确认生成重拍 successor"}
          </DialogTitle>
          <DialogDescription id="review-confirm-description">
            {confirm?.kind === "execute"
              ? "将创建可收费 operation；不会自动接受结果。"
              : confirm?.kind === "accept"
                ? "将以一个 owner command 对精确 Shot revision 执行 all-or-nothing CAS；不会复制 AssetVersion 或媒体字节。"
                : confirm?.kind === "reject"
                  ? "基础 AssetVersion 保持不变。"
                  : "将使用新的 logicalOperation，旧候选保持可审计。"}
          </DialogDescription>
          <div className="flex flex-wrap justify-end gap-2">
            <DialogClose asChild>
              <button className="inline-flex h-10 items-center justify-center rounded-md border border-border bg-background px-4 text-sm font-semibold text-foreground hover:bg-accent">
                取消
              </button>
            </DialogClose>
            <button
              className="inline-flex h-10 items-center justify-center rounded-md bg-primary px-4 text-sm font-semibold text-primary-foreground hover:bg-primary/90 disabled:pointer-events-none disabled:opacity-50"
              onClick={() => {
                if (!confirm) return;
                if (confirm.kind === "execute") execute.mutate(confirm.plan);
                else review.mutate(confirm);
                setConfirm(null);
              }}
            >
              {confirm?.kind === "execute"
                ? "确认执行"
                : confirm?.kind === "accept"
                  ? "确认接受"
                  : confirm?.kind === "reject"
                    ? "确认拒绝"
                    : "确认重拍"}
            </button>
          </div>
        </DialogContent>
      </Dialog>
    </section>
  );
}
