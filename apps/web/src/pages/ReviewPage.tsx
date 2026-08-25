import { useMutation, useQuery } from "@tanstack/react-query";
import { CircleAlert, CircleCheck, FileText, Layers3 } from "lucide-react";
import { useState } from "react";
import { useParams } from "react-router";
import { queryClient } from "../app/query-client";
import { AssetEditReviewPage } from "./AssetEditReviewPage";
import {
  Badge,
  Button,
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "../shared/ui";
import { ErrorNotice, PageIntro, QueryNotice } from "../ui";
import { OwnerApiError, queryKeys, workbenchApi } from "../workbench/api";

function LegacyTextReview() {
  const { projectId = "" } = useParams();
  const [decision, setDecision] = useState<string | null>(null);
  const [pendingDecision, setPendingDecision] = useState<
    "accept" | "reject" | "retake" | null
  >(null);
  const [mediaGate, setMediaGate] = useState<{
    status: "ready" | "blocked";
    missingOwners: string[];
  } | null>(null);
  const batches = useQuery({
    queryKey: queryKeys.textReview(projectId),
    queryFn: () => workbenchApi.listTextReviews(projectId),
    enabled: Boolean(projectId),
  });
  const batch = batches.data?.[0];
  const decide = useMutation({
    mutationFn: (action: "accept" | "reject" | "retake") => {
      if (!batch) return Promise.reject(new Error("没有可审核 batch"));
      return workbenchApi.decideTextReview(
        projectId,
        batch.id,
        batch.revision,
        action,
      );
    },
    onSuccess: (result) => {
      const handoff = (result as { handoff?: { id?: string } } | null)?.handoff;
      if (handoff?.id) {
        void workbenchApi
          .getMediaGate(projectId, handoff.id)
          .then((gate) => setMediaGate(gate));
      }
      void queryClient.invalidateQueries({
        queryKey: queryKeys.textReview(projectId),
      });
    },
    onError: (error) => {
      if (!(error instanceof OwnerApiError) || error.status !== 409) return;
      setDecision(null);
      setMediaGate(null);
      void queryClient.invalidateQueries({
        queryKey: queryKeys.textReview(projectId),
      });
    },
  });
  const confirmDecision = () => {
    if (!pendingDecision) return;
    setDecision(pendingDecision);
    decide.mutate(pendingDecision);
    setPendingDecision(null);
  };
  return (
    <section className="mx-auto flex w-full max-w-screen-2xl flex-col gap-6 p-4 sm:p-6 lg:p-8">
      <PageIntro
        eyebrow="CANDIDATE REVIEW / S04-S07"
        title="一次确认，解锁下游媒体"
        detail="只显示 owner 返回的 TextReviewBatch 与 candidate。accept / reject / retake 是唯一审核动作。"
      />
      <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_18rem]">
        <Card className="shadow-sm">
          <CardContent className="p-5">
            {batches.isPending && (
              <QueryNotice isPending error={null} empty="" />
            )}
            {batches.error && <ErrorNotice error={batches.error} />}
            {!batches.isPending && !batches.error && !batch && (
              <div className="grid place-items-center gap-3 rounded-md border border-dashed border-border p-8 text-center text-sm text-muted-foreground">
                <Layers3 size={24} />
                <strong>没有待审核 TextReviewBatch</strong>
                <span>
                  请从项目工作台显式生成文本 Run；页面不会自行 ensure 或提交
                  Provider。
                </span>
              </div>
            )}
            {batch && (
              <>
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div>
                    <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                      TEXT REVIEW BATCH
                    </span>
                    <h3>{batch.id}</h3>
                  </div>
                  <Badge
                    variant={
                      batch.status === "pending_review"
                        ? "warning"
                        : "secondary"
                    }
                  >
                    {batch.status}
                  </Badge>
                </div>
                <div className="mt-4 grid gap-2">
                  {batch.candidates.map((candidate) => (
                    <div
                      className="flex items-center justify-between gap-3 rounded-md border border-border p-3"
                      key={candidate.id}
                    >
                      <FileText size={16} />
                      <span>
                        <strong>{candidate.kind}</strong>
                        <small className="font-mono text-xs text-muted-foreground">
                          {candidate.id} / rev {candidate.revision} /{" "}
                          {candidate.payloadHash.slice(0, 12)}...
                        </small>
                      </span>
                      <Badge variant="secondary">{candidate.status}</Badge>
                    </div>
                  ))}
                </div>
                <div className="mt-4 flex flex-wrap gap-2">
                  <Button
                    variant={decision === "reject" ? "destructive" : "outline"}
                    disabled={
                      decide.isPending || batch.status !== "pending_review"
                    }
                    onClick={() => {
                      setPendingDecision("reject");
                    }}
                  >
                    Reject
                  </Button>
                  <Button
                    variant={decision === "retake" ? "secondary" : "outline"}
                    disabled={
                      decide.isPending || batch.status !== "pending_review"
                    }
                    onClick={() => {
                      setPendingDecision("retake");
                    }}
                  >
                    Retake
                  </Button>
                  <Button
                    variant={decision === "accept" ? "default" : "outline"}
                    disabled={
                      decide.isPending || batch.status !== "pending_review"
                    }
                    onClick={() => {
                      setPendingDecision("accept");
                    }}
                  >
                    Accept <CircleCheck size={16} />
                  </Button>
                </div>
                {decision && (
                  <div className="mt-4 flex items-center gap-2 rounded-md border border-success/30 bg-success/10 p-3 text-sm text-success">
                    <CircleCheck size={15} /> 已选择 {decision}；提交仍需 owner
                    revision、candidate hash 与全部 ack。
                  </div>
                )}
                {decide.error && <ErrorNotice error={decide.error} />}
                {pendingDecision && (
                  <div
                    className="mt-4 grid gap-2 rounded-md border border-warning/30 bg-warning/10 p-4 text-sm text-warning-foreground"
                    role="dialog"
                    aria-label="确认审核动作"
                  >
                    <strong>
                      确认 {pendingDecision} 当前 TextReviewBatch？
                    </strong>
                    <span>
                      只发送一次 owner command；旧 batch 与候选保持 immutable。
                    </span>
                    <div className="flex flex-wrap gap-2">
                      <Button
                        variant="outline"
                        onClick={() => setPendingDecision(null)}
                      >
                        取消
                      </Button>
                      <Button onClick={confirmDecision}>确认提交</Button>
                    </div>
                  </div>
                )}
              </>
            )}
          </CardContent>
        </Card>
        <Card className="h-fit shadow-sm">
          <CardHeader className="pb-0">
            <span className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              OWNER GATE
            </span>
            <CardTitle className="mt-1">媒体入口状态</CardTitle>
          </CardHeader>
          <CardContent className="grid gap-3 pt-5">
            <div className="grid gap-2 text-sm">
              <div className="flex items-center justify-between gap-3">
                <span>Project/Episode ack</span>
                <strong
                  className={
                    mediaGate?.status === "ready"
                      ? "text-success"
                      : "text-warning-foreground"
                  }
                >
                  {mediaGate?.status === "ready"
                    ? "ready"
                    : mediaGate?.missingOwners?.length
                      ? `缺少 ${mediaGate.missingOwners.join(", ")}`
                      : "待审核"}
                </strong>
              </div>
              <div className="flex items-center justify-between gap-3">
                <span>AssetBible snapshot</span>
                <strong className="text-warning-foreground">待确认</strong>
              </div>
              <div className="flex items-center justify-between gap-3">
                <span>Provider submit</span>
                <strong>blocked</strong>
              </div>
            </div>
            <div className="flex items-start gap-2 rounded-md border border-warning/30 bg-warning/10 p-3 text-sm text-warning-foreground">
              <CircleAlert size={15} /> 未完成 batch ack 前不显示可执行媒体入口
            </div>
          </CardContent>
        </Card>
      </div>
    </section>
  );
}

export function ReviewPage() {
  const { projectId = "" } = useParams();
  return (
    <>
      <AssetEditReviewPage projectId={projectId} />
      <LegacyTextReview />
    </>
  );
}
